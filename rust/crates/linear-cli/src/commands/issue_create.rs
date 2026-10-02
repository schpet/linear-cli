use super::issue_write::{self as shared, AssignSelf, Backend, CreateSettings, Parent, Ui};
use crate::{
    error::{AppError, AppErrorKind},
    graphql::{edit::Edit, scalars::TimelessDate},
};
#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub title: Option<String>,
    pub start: bool,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
    pub parent: Option<String>,
    pub priority: Option<f64>,
    pub estimate: Option<f64>,
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
pub type Input = crate::graphql::operations::issue_create::IssueCreateInput;
impl Fields {
    pub fn local(&self) -> Result<Option<String>, AppError> {
        shared::description(
            self.description.as_deref(),
            self.description_file.as_deref(),
        )
    }
    pub fn full_interactive(&self, description: Option<&str>, stdout_tty: bool) -> bool {
        stdout_tty
            && !self.no_interactive
            && shared::truthy(self.title.as_deref()).is_none()
            && shared::truthy(self.assignee.as_deref()).is_none()
            && shared::truthy(self.due_date.as_deref()).is_none()
            && self.priority.is_none()
            && self.estimate.is_none()
            && shared::truthy(description).is_none()
            && self.labels.is_empty()
            && shared::truthy(self.team.as_deref()).is_none()
            && shared::truthy(self.state.as_deref()).is_none()
            && shared::truthy(self.milestone.as_deref()).is_none()
            && shared::truthy(self.cycle.as_deref()).is_none()
            && !self.start
            && self.template.is_none()
    }
    pub fn require_flag_title(&self) -> Result<(), AppError> {
        if shared::truthy(self.title.as_deref()).is_none() && self.template.is_none() {
            return Err(shared::validation("Title is required when not using interactive mode")
                .with_suggestion("Use --title, pass --template to take the title from a template, or run without any flags (or only --parent/--project) for interactive mode."));
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
    ) -> impl std::future::Future<Output = Result<String, AppError>> + Send;
}
pub async fn parent<B: Backend>(
    backend: &B,
    reference: Option<&str>,
) -> Result<(Option<String>, Option<Parent>), AppError> {
    match shared::truthy(reference) {
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
) -> Result<String, AppError> {
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
    Err(AppError::not_found("Project", value))
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
) -> Result<FlagInput, AppError> {
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
                .ok_or_else(|| shared::validation("Could not determine team key"))?;
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
                        id.ok_or_else(|| AppError::not_found("Team", &reference))?,
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
    if let Some(value) = shared::truthy(assignee.as_deref()) {
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
                .ok_or_else(|| AppError::not_found("Issue label", value))?,
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
                .with_suggestion("Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly."))?;
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
            due_date: Edit::set_or_unchanged(fields.due_date.clone().map(TimelessDate)),
            parent_id: Edit::set_or_unchanged(parent_id),
            priority: Edit::set_or_unchanged(shared::integer(fields.priority, "priority")?),
            estimate: Edit::set_or_unchanged(shared::integer(fields.estimate, "estimate")?),
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
pub fn flag_output(issue: &shared::Created) -> String {
    format!("{}\n", issue.url)
}
pub fn interactive_output(issue: &shared::Created, title: &str) -> String {
    format!(
        "✓ Created issue {}: {title}\n{}\n",
        issue.identifier, issue.url
    )
}

/// Offers the near matches for a reference that did not resolve. `None` when
/// there are none or the user declines them all.
pub fn select_option<U: Ui>(
    ui: &mut U,
    kind: &str,
    original: &str,
    options: &[shared::Named],
) -> Result<Option<String>, AppError> {
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
    let selected = ui.choose(&message, &menu, 0, false)?;
    let index = selected.parse::<usize>().map_err(|error| {
        AppError::new(AppErrorKind::Invariant, "menu returned an unknown choice").with_source(error)
    })?;
    Ok(candidates.get(index).map(|option| option.id.clone()))
}

impl From<&crate::cli::issue::IssueCreate> for Fields {
    fn from(action: &crate::cli::issue::IssueCreate) -> Self {
        Self {
            title: action.title.clone(),
            start: action.start,
            assignee: action.assignee.clone(),
            due_date: action.due_date.clone(),
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
