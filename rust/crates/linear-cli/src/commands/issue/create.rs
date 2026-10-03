use chrono::NaiveDate;

use super::write::{self as shared, Backend, CreateSettings, Named, Parent, Ui};
use super::write_network::NetworkBackend;
use crate::{
    cli::{issue::IssueCreate, values::Priority},
    commands::{outcome, team_key::configured_team_key},
    config::AssignSelf,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{edit::Edit, scalars::TimelessDate},
    platform::{
        editor,
        prompt::{Choice, Prompter, Text},
        spinner::Spinner,
    },
};

pub fn run(ctx: &Ctx, args: &IssueCreate) -> Result<()> {
    create(ctx, args).context("Failed to create issue")
}

fn create(ctx: &Ctx, args: &IssueCreate) -> Result<()> {
    let fields = Fields::from(args);
    let description = fields.local()?;
    let interactive = fields.full_interactive(description.as_deref(), ctx.interactive());
    if !interactive {
        fields.require_flag_title()?;
    }
    let mut ui = Prompts::new(ctx, !interactive);
    let start = create_with(ctx, &mut ui, &fields, description, interactive)?;
    // The spinner stops before `--start` runs version control commands.
    drop(ui);
    if let Some(identifier) = start {
        super::start::work_on(ctx, &identifier, None, None)?;
    }
    Ok(())
}

/// Creates the issue; returns the issue to start work on with `--start`.
fn create_with(
    ctx: &Ctx,
    ui: &mut Prompts<'_>,
    fields: &Fields,
    description: Option<String>,
    interactive: bool,
) -> Result<Option<String>> {
    let backend = backend(ctx)?;
    let settings = settings(ctx);
    let (input, start) = if interactive {
        let prompted = ctx.block_on(super::create_prompt::prompt(
            &backend, ui, &settings, fields,
        ))?;
        ui.output("Creating issue...\n\n")?;
        (prompted.input, prompted.start)
    } else {
        let fallback = !fields.no_interactive && ctx.interactive();
        let assembled = ctx.block_on(flag_input(
            &backend,
            ui,
            &settings,
            fields,
            description,
            fallback,
        ))?;
        ui.pause();
        ui.output(&flag_header(&assembled.team_display))?;
        (assembled.input, fields.start)
    };
    let issue = ctx.spin(!interactive, backend.create(input))?;
    ctx.print(output(&issue))?;
    Ok(start.then(|| issue.identifier.clone()))
}

/// The API backend issue creation and updates resolve names through.
pub(super) fn backend(ctx: &Ctx) -> Result<NetworkBackend> {
    Ok(NetworkBackend {
        client: ctx.client()?.clone(),
        options: ctx.options().clone(),
        workspace: ctx.scope()?.workspace.map(str::to_owned),
    })
}

fn settings(ctx: &Ctx) -> CreateSettings {
    let options = ctx.options();
    CreateSettings {
        default_team: configured_team_key(options),
        assign_self: options.issue_create_assign_self(),
        ask_project: options.issue_create_ask_project(),
    }
}

/// Terminal prompts for issue creation. Without prompts (all fields from
/// flags) a spinner runs, pausing while a near-match question is asked.
struct Prompts<'a> {
    ctx: &'a Ctx,
    spin: bool,
    spinner: Option<Spinner>,
}

impl<'a> Prompts<'a> {
    fn new(ctx: &'a Ctx, spin: bool) -> Self {
        let mut prompts = Self {
            ctx,
            spin,
            spinner: None,
        };
        prompts.resume();
        prompts
    }

    fn pause(&mut self) {
        self.spinner = None;
    }

    fn resume(&mut self) {
        if self.spin && self.spinner.is_none() {
            self.spinner = Some(self.ctx.spinner(true, ""));
        }
    }

    fn ask<T>(&mut self, question: impl FnOnce(&Prompter<'_>) -> Result<T>) -> Result<T> {
        self.pause();
        let answer = question(&self.ctx.prompter()?);
        self.resume();
        answer
    }
}

impl Ui for Prompts<'_> {
    fn text(&mut self, text: Text<'_>) -> Result<String> {
        self.ask(|prompter| prompter.text(text))
    }

    fn parsed<T>(
        &mut self,
        text: Text<'_>,
        parse: &dyn Fn(&str) -> std::result::Result<T, String>,
    ) -> Result<Option<T>> {
        self.ask(|prompter| prompter.parsed(text, parse))
    }

    fn choose(&mut self, message: &str, options: &[Named], default: usize) -> Result<String> {
        let choices = options
            .iter()
            .map(|option| Choice::new(&option.name, option.id.clone()))
            .collect();
        self.ask(|prompter| prompter.select_from(message, choices, default))
    }

    fn checkbox(&mut self, message: &str, options: &[Named]) -> Result<Vec<String>> {
        let choices = options
            .iter()
            .map(|option| Choice::new(&option.name, option.id.clone()))
            .collect();
        self.ask(|prompter| prompter.multi_select(message, choices))
    }

    fn output(&mut self, text: &str) -> Result<()> {
        self.ctx.print(text)?;
        self.ctx.flush()
    }

    fn error(&mut self, text: &str) -> Result<()> {
        self.ctx.eprint(text)
    }

    fn discover_editor(&mut self) -> Result<Option<String>> {
        Ok(editor::configured_name(&self.ctx.config().child_env))
    }

    fn optional_editor(&mut self) -> Result<Option<String>> {
        self.pause();
        let edited = self.ctx.edit_text("");
        self.resume();
        Ok(crate::commands::text_input::edited_body(&edited?))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub title: Option<String>,
    pub start: bool,
    pub assignee: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub parent: Option<String>,
    pub priority: Option<Priority>,
    pub estimate: Option<i32>,
    pub description: Option<String>,
    pub description_file: Option<String>,
    pub labels: Vec<String>,
    pub team: Option<String>,
    pub project: Option<String>,
    pub state: Option<String>,
    pub milestone: Option<String>,
    pub cycle: Option<String>,
    pub template: Option<String>,
    pub use_default_template: bool,
    pub no_interactive: bool,
}
pub type Input = crate::graphql::operations::issue::IssueCreateInput;
impl Fields {
    pub fn local(&self) -> Result<Option<String>, Error> {
        shared::description(
            self.description.as_deref(),
            self.description_file.as_deref(),
        )
    }
    /// No field flags were given on a terminal, so every field is asked for.
    pub fn full_interactive(&self, description: Option<&str>, terminal: bool) -> bool {
        terminal
            && !self.no_interactive
            && self.title.as_deref().is_none_or(str::is_empty)
            && self.assignee.as_deref().is_none_or(str::is_empty)
            && self.due_date.is_none()
            && self.priority.is_none()
            && self.estimate.is_none()
            && description.is_none_or(str::is_empty)
            && self.labels.is_empty()
            && self.team.as_deref().is_none_or(str::is_empty)
            && self.state.as_deref().is_none_or(str::is_empty)
            && self.milestone.as_deref().is_none_or(str::is_empty)
            && self.cycle.as_deref().is_none_or(str::is_empty)
            && !self.start
            && self.template.is_none()
    }
    pub fn require_flag_title(&self) -> Result<(), Error> {
        if self.title.as_deref().is_none_or(str::is_empty) && self.template.is_none() {
            return Err(shared::validation("Title is required when not using interactive mode")
                .with_hint("Use --title, pass --template to take the title from a template, or run without any flags (or only --parent/--project) for interactive mode."));
        }
        Ok(())
    }
}
// Template resolution intentionally separate from project templates: type ISSUE,
// team availability and all selected fields checked; never parse templateData.
pub trait Templates {
    fn issue_template(
        &self,
        reference: String,
        team_id: String,
    ) -> impl std::future::Future<Output = Result<String, Error>> + Send;
}
pub async fn parent<B: Backend>(
    backend: &B,
    reference: Option<&str>,
) -> Result<(Option<String>, Option<Parent>), Error> {
    match reference.filter(|reference| !reference.is_empty()) {
        None => Ok((None, None)),
        Some(reference) => {
            let id = backend.parent_id(reference.to_owned()).await?;
            let data = backend.parent_metadata(id.clone()).await?;
            Ok((Some(id), data))
        }
    }
}
pub async fn project<B: Backend, U: Ui>(
    backend: &B,
    ui: &mut U,
    value: &str,
    interactive: bool,
) -> Result<String, Error> {
    if let Some(id) = backend.project(value.to_owned()).await? {
        return Ok(id);
    }
    if interactive {
        let options = backend.project_options(value.to_owned()).await?;
        if !options.is_empty()
            && let Some(id) = select_option(ui, "Project", value, &options)?
        {
            return Ok(id);
        }
    }
    Err(Error::not_found("Project", value))
}
pub struct FlagInput {
    pub input: Input,
    pub team_display: String,
}
pub async fn flag_input<B: Backend + Templates, U: Ui>(
    backend: &B,
    ui: &mut U,
    settings: &CreateSettings,
    fields: &Fields,
    description: Option<String>,
    interactive_fallback: bool,
) -> Result<FlagInput, Error> {
    let (team_id, team_reference) = match &fields.team {
        Some(value) => {
            let team = backend.team(value.clone()).await?;
            (team.id, team.key)
        }
        None => {
            let reference = settings
                .default_team
                .clone()
                .filter(|v| !v.is_empty())
                .ok_or_else(crate::commands::team_key::no_team)?;
            match backend.find_team(reference.clone()).await? {
                Some(team) => (team.id, reference),
                None => {
                    let id = if interactive_fallback {
                        let options = backend.team_options(reference.clone()).await?;
                        if options.is_empty() {
                            None
                        } else {
                            select_option(ui, "Team", &reference, &options)?
                        }
                    } else {
                        None
                    };
                    (
                        id.ok_or_else(|| Error::not_found("Team", &reference))?,
                        reference,
                    )
                }
            }
        }
    };
    let template = match &fields.template {
        Some(value) => Some(
            backend
                .issue_template(value.clone(), team_id.clone())
                .await?,
        ),
        None => None,
    };
    let assignee = if fields.start && fields.assignee.is_none() {
        Some("self".to_owned())
    } else {
        fields.assignee.clone()
    };
    if fields.start && assignee.as_deref() != Some("self") {
        return Err(shared::validation(
            "Cannot use --start and a non-self --assignee",
        ));
    }
    // State BEFORE always-self Viewer, even when explicit assignee later overrides.
    let state = match &fields.state {
        Some(value) => Some(backend.state(team_reference.clone(), value.clone()).await?),
        None => None,
    };
    let mut assignee_id = if settings.assign_self == AssignSelf::Always {
        Some(backend.viewer().await?)
    } else {
        None
    };
    if let Some(value) = assignee.as_deref().filter(|value| !value.is_empty()) {
        assignee_id = Some(backend.user(value.to_owned()).await?)
    }
    let mut label_ids = Vec::new();
    for value in &fields.labels {
        let mut id = backend.label(team_reference.clone(), value.clone()).await?;
        if id.as_deref().is_none_or(str::is_empty) && interactive_fallback {
            let options = backend
                .label_options(team_reference.clone(), value.clone())
                .await?;
            if !options.is_empty() {
                id = select_option(ui, "Issue label", value, &options)?
            }
        }
        label_ids.push(
            id.filter(|id| !id.is_empty())
                .ok_or_else(|| Error::not_found("Issue label", value))?,
        );
    }
    let project = match &fields.project {
        Some(value) => Some(project(backend, ui, value, interactive_fallback).await?),
        None => None,
    };
    let milestone = match &fields.milestone {
        Some(value) if crate::refs::is_linear_uuid(value) => Some(value.clone()),
        Some(value) => {
            let project=project.clone().ok_or_else(||shared::validation("--milestone requires --project to be set")
                .with_hint("Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly."))?;
            Some(backend.milestone(project, value.clone()).await?)
        }
        None => None,
    };
    let cycle = match &fields.cycle {
        Some(value) => Some(backend.cycle(team_id.clone(), value.clone()).await?),
        None => None,
    };
    let (parent_id, parent_data) = parent(backend, fields.parent.as_deref()).await?;
    let project = match project.filter(|v| !v.is_empty()) {
        Some(project) => Edit::Set(project),
        None => match parent_data {
            Some(parent) => Edit::set_or_clear(parent.project_id),
            None => Edit::Unchanged,
        },
    };
    Ok(FlagInput {
        team_display: team_reference,
        input: Input {
            title: Edit::set_or_unchanged(fields.title.clone()),
            assignee_id: Edit::set_or_unchanged(assignee_id),
            due_date: Edit::set_or_unchanged(fields.due_date.map(TimelessDate::from)),
            parent_id: Edit::set_or_unchanged(parent_id),
            priority: Edit::set_or_unchanged(fields.priority.map(Priority::number)),
            estimate: Edit::set_or_unchanged(fields.estimate),
            label_ids: Some(label_ids),
            team_id,
            project_id: project,
            project_milestone_id: Edit::set_or_unchanged(milestone),
            cycle_id: Edit::set_or_unchanged(cycle),
            state_id: Edit::set_or_unchanged(state),
            template_id: Edit::set_or_unchanged(template),
            use_default_template: if fields.template.is_none() {
                Edit::Set(fields.use_default_template)
            } else {
                Edit::Unchanged
            },
            description: Edit::set_or_unchanged(description),
        },
    })
}
pub fn flag_header(team: &str) -> String {
    format!("Creating issue in {team}\n\n")
}
fn output(issue: &shared::Created) -> String {
    outcome::done(
        "Created",
        "issue",
        &format!("{}: {}", issue.identifier, issue.title),
        Some(&issue.url),
    )
}

/// Offers the near matches for a reference that did not resolve. `None` when
/// there are none or the user declines them all.
pub fn select_option<U: Ui>(
    ui: &mut U,
    kind: &str,
    original: &str,
    options: &[shared::Named],
) -> Result<Option<String>, Error> {
    let mut seen = std::collections::HashSet::new();
    let candidates: Vec<&shared::Named> = options
        .iter()
        .filter(|option| seen.insert(option.id.as_str()))
        .collect();
    let (message, labels): (String, Vec<&str>) = match candidates.as_slice() {
        [] => return Ok(None),
        [only] => (
            format!(
                "{kind} named {original} does not exist, but {} exists. Is this what you meant?",
                only.name
            ),
            vec!["yes", "no"],
        ),
        many => (
            format!(
                "{kind} with {original} does not exist, but the following exist. Is any of these what you meant?"
            ),
            many.iter()
                .map(|option| option.name.as_str())
                .chain(["none of the above"])
                .collect(),
        ),
    };
    // Menu ids are positions, so no candidate id can be mistaken for the decline entry.
    let menu: Vec<shared::Named> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| shared::Named {
            id: index.to_string(),
            name: (*label).to_owned(),
        })
        .collect();
    let selected = ui.choose(&message, &menu, 0)?;
    let index = selected
        .parse::<usize>()
        .map_err(|error| Error::new("menu returned an unknown choice").with_source(error))?;
    Ok(candidates.get(index).map(|option| option.id.clone()))
}

impl From<&crate::cli::issue::IssueCreate> for Fields {
    fn from(action: &crate::cli::issue::IssueCreate) -> Self {
        Self {
            title: action.title.clone(),
            start: action.start,
            assignee: action.assignee.clone(),
            due_date: action.due_date,
            parent: action.parent.clone(),
            priority: action.priority,
            estimate: action.estimate,
            description: action.description.clone(),
            description_file: action.description_file.clone(),
            labels: action.label.clone(),
            team: action.team.clone(),
            project: action.project.clone(),
            state: action.state.clone(),
            milestone: action.milestone.clone(),
            cycle: action.cycle.clone(),
            template: action.template.clone(),
            use_default_template: !action.no_use_default_template,
            no_interactive: action.no_interactive,
        }
    }
}
