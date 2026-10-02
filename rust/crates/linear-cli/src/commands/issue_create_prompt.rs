use super::{
    issue_create::{self, Fields, Input},
    issue_write::{
        self as shared, AssignSelf, Backend, CreateSettings, Label, Named, Parent, State, Ui,
    },
};
use crate::graphql::operations::number::WholeNumber;
use crate::platform::network_owner;
use crate::{error::AppError, graphql::edit::Edit};
fn option(id: &str, name: &str) -> Named {
    Named {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}
fn yes_no<U: Ui>(ui: &mut U, message: &str) -> Result<bool, AppError> {
    Ok(ui.choose(
        message,
        &[option("no", "No"), option("yes", "Yes")],
        0,
        false,
    )? == "yes")
}
fn project_menu<U: Ui>(ui: &mut U, projects: &[Named]) -> Result<Option<String>, AppError> {
    if projects.is_empty() {
        return Ok(None);
    }
    let mut rows = vec![option("__none__", "No project")];
    rows.extend_from_slice(projects);
    let answer = ui.choose("Which project should this issue belong to?", &rows, 0, true)?;
    Ok((answer != "__none__").then_some(answer))
}
async fn additional<B: Backend, U: Ui>(
    backend: &B,
    ui: &mut U,
    team: &shared::Team,
    states: &[State],
    labels: &[Label],
    include_project: bool,
    auto: bool,
) -> Result<More, AppError> {
    let default = shared::default_state(states)?;
    let name = default
        .as_ref()
        .and_then(|id| states.iter().find(|s| &s.id == id))
        .map(|s| s.name.as_str());
    let mut fields = vec![
        option(
            "workflow_state",
            &name
                .map(|name| format!("Workflow state ({name})"))
                .unwrap_or_else(|| "Workflow state".to_owned()),
        ),
        option(
            "assignee",
            if auto {
                "Assignee (self)"
            } else {
                "Assignee (unassigned)"
            },
        ),
        option("priority", "Priority"),
        option("labels", "Labels"),
        option("estimate", "Estimate"),
    ];
    if include_project {
        fields.push(option("project", "Project"))
    }
    let selected = ui.checkbox("Select additional fields to configure", &fields, false)?;
    let mut more = More::default();
    // Source resets all additional-field state, including existing default state.
    if auto {
        ui.suspend()?;
        more.assignee = Some(backend.viewer().await?)
    }
    // Checkbox returns membership in declaration order, not toggle order.
    for field in selected {
        match field.as_str() {
            "workflow_state" if !states.is_empty() => {
                let options: Vec<_> = states
                    .iter()
                    .map(|s| option(&s.id, &format!("{} ({})", s.name, s.kind)))
                    .collect();
                let index = default
                    .as_ref()
                    .and_then(|id| options.iter().position(|o| &o.id == id))
                    .unwrap_or(0);
                more.state = Some(ui.choose(
                    "Which workflow state should this issue be in?",
                    &options,
                    index,
                    false,
                )?);
            }
            "workflow_state" => (),
            "assignee" => {
                let answer = yes_no(ui, "Assign this issue to yourself?")?;
                ui.suspend()?;
                more.assignee = if answer {
                    Some(backend.viewer().await?)
                } else {
                    None
                };
            }
            "priority" => {
                let values = [
                    (0_u32, "No priority"),
                    (1, "Urgent"),
                    (2, "High"),
                    (3, "Medium"),
                    (4, "Low"),
                ];
                let options = values
                    .into_iter()
                    .map(|(value, label)| {
                        let glyph = super::issue_read::priority(WholeNumber(value));
                        option(&value.to_string(), &format!("{glyph} {label}"))
                    })
                    .collect::<Vec<_>>();
                let value =
                    ui.choose("What priority should this issue have?", &options, 0, false)?;
                let priority = value.parse::<i32>().map_err(|error| {
                    shared::validation("selected priority is not an integer").with_source(error)
                })?;
                more.priority = (priority != 0).then_some(priority);
            }
            "labels" if !labels.is_empty() => {
                let options: Vec<_> = labels.iter().map(|l| option(&l.id, &l.name)).collect();
                more.labels = ui.checkbox(
                    "Select labels (use space to select, enter to confirm)",
                    &options,
                    true,
                )?;
            }
            "labels" => (),
            "estimate" => {
                more.estimate = shared::menu_estimate(&ui.text(
                    "Estimate (leave blank for none)",
                    0,
                    Some(""),
                )?)?
            }
            "project" => {
                ui.suspend()?;
                let projects = backend.projects(team.key.clone()).await?;
                more.project = project_menu(ui, &projects)?;
            }
            _ => {
                return Err(shared::validation(
                    "selected additional field is not a declared menu member",
                ));
            }
        }
    }
    Ok(more)
}
#[derive(Default)]
struct More {
    assignee: Option<String>,
    priority: Option<i32>,
    estimate: Option<i32>,
    labels: Vec<String>,
    state: Option<String>,
    project: Option<String>,
}
pub struct Interactive {
    pub input: Input,
    pub title: String,
    pub start: bool,
}
pub fn prompt<B: Backend, U: Ui>(
    backend: &B,
    ui: &mut U,
    settings: &CreateSettings,
    fields: &Fields,
) -> Result<Interactive, AppError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            shared::validation("Could not create interactive issue runtime").with_source(error)
        })?;
    std::thread::scope(|scope| {
        runtime.block_on(prompt_in_scope(scope, backend, ui, settings, fields))
    })
}

async fn prompt_in_scope<'scope, 'env, B: Backend, U: Ui>(
    scope: &'scope std::thread::Scope<'scope, 'env>,
    backend: &B,
    ui: &mut U,
    settings: &CreateSettings,
    fields: &Fields,
) -> Result<Interactive, AppError> {
    let (parent_id, parent_data) = issue_create::parent(backend, fields.parent.as_deref()).await?;
    let initial_project = match &fields.project {
        Some(value) => Some(issue_create::project(backend, ui, value, true).await?),
        None => None,
    };
    let auto_backend = backend.clone();
    let mode = settings.assign_self;
    let team_backend = backend.clone();
    let default_team = settings.default_team.clone();
    let (first_phase, auto, team) = network_owner::pair(
        scope,
        async move {
            match mode {
                AssignSelf::Always => Ok(true),
                AssignSelf::Never => Ok(false),
                AssignSelf::Auto => auto_backend.auto_assign().await,
            }
        },
        async move {
            match default_team.filter(|key| !key.is_empty()) {
                Some(key) => team_backend.find_team(key).await,
                None => Ok(None),
            }
        },
    )?;
    if let Some(parent) = &parent_data {
        ui.output(&format!(
            "Creating sub-issue for: {}: {}\n\n",
            parent.identifier, parent.title
        ))?
    }
    let title = ui.text("What's the title of your issue?", 1, None)?;
    ui.suspend()?;
    // Exact SOURCE await order. Background tasks already live during title.
    let team = team.take()?;
    let auto = auto.take()?;
    first_phase.close()?;
    let team = match team {
        Some(team) => team,
        None => {
            let teams = backend.teams().await?;
            let options: Vec<_> = teams
                .iter()
                .map(|t| option(&t.id, &format!("{} ({})", t.name, t.key)))
                .collect();
            let selected =
                ui.choose("Which team should this issue belong to?", &options, 0, true)?;
            teams
                .into_iter()
                .find(|t| t.id == selected)
                .ok_or_else(|| AppError::not_found("Team", &selected))?
        }
    };
    let state_backend = backend.clone();
    let state_key = team.key.clone();
    let label_backend = backend.clone();
    let label_key = team.key.clone();
    let project_backend = backend.clone();
    let project_key = team.key.clone();
    let ask_project = settings.ask_project
        && parent_data.is_none()
        && initial_project.as_deref().is_none_or(str::is_empty);
    let (second_phase, states, labels, projects) = network_owner::triple(
        scope,
        async move { state_backend.states(state_key).await },
        async move { label_backend.labels(label_key).await },
        async move {
            if ask_project {
                project_backend.projects(project_key).await.map(Some)
            } else {
                Ok(None)
            }
        },
    )?;
    ui.suspend()?;
    let editor = ui.discover_editor()?;
    let editor_label = editor
        .as_deref()
        .and_then(|editor| editor.rsplit('/').next())
        .filter(|label| !label.is_empty());
    let message = editor_label
        .map(|label| format!("Description [(e) to launch {label}]"))
        .unwrap_or_else(|| "Description".to_owned());
    let raw = ui.text(&message, 0, Some(""))?;
    let description = if raw == "e" {
        ui.suspend()?;
        if let Some(editor) = editor_label {
            ui.output(&format!("Opening {editor}...\n"))?;
            // Existing optional editor rediscovers the literal editor and owns temp.
            let text = ui.optional_editor()?;
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                ui.output(&format!(
                    "Description entered ({} characters)\n",
                    text.encode_utf16().count()
                ))?;
                Some(text)
            } else {
                ui.output("No description entered\n")?;
                None
            }
        } else {
            ui.error("No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>\n")?;
            None
        }
    } else {
        let text = raw.trim();
        (!text.is_empty()).then(|| text.to_owned())
    };
    ui.suspend()?;
    let mut project = initial_project.clone();
    // Projects/project menu BEFORE states BEFORE labels, regardless of arrival.
    if let Some(projects) = projects.take()? {
        project = project_menu(ui, &projects)?;
        ui.suspend()?
    }
    let states = states.take()?;
    let labels = labels.take()?;
    second_phase.close()?;
    let next = ui.choose(
        "What's next?",
        &[
            option("submit", "Submit issue"),
            option("more_fields", "Add more fields"),
        ],
        0,
        false,
    )?;
    ui.suspend()?;
    let mut more = More {
        state: shared::default_state(&states)?,
        ..Default::default()
    };
    if auto {
        more.assignee = Some(backend.viewer().await?)
    }
    if next == "more_fields" {
        more = additional(
            backend,
            ui,
            &team,
            &states,
            &labels,
            !settings.ask_project
                && parent_data.is_none()
                && initial_project.as_deref().is_none_or(str::is_empty),
            auto,
        )
        .await?;
        project = more.project.clone().or(project);
    } else if next != "submit" {
        return Err(shared::validation(
            "next action is not a declared menu member",
        ));
    }
    let start = yes_no(
        ui,
        "Start working on this issue now? (creates branch and updates status)",
    )?;
    ui.suspend()?;
    // Both scoped phase owners abort+join before mutation/normal return; Drop
    // enforces the same cleanup on any preceding prompt/await error.
    let project = project.or_else(|| parent_data.and_then(|Parent { project_id, .. }| project_id));
    Ok(Interactive {
        title: title.clone(),
        start,
        input: Input {
            title: Edit::Set(title),
            assignee_id: Edit::set_or_unchanged(more.assignee),
            due_date: Edit::Unchanged,
            parent_id: Edit::set_or_unchanged(parent_id),
            priority: Edit::set_or_unchanged(more.priority),
            estimate: Edit::set_or_unchanged(more.estimate),
            label_ids: Some(more.labels),
            team_id: team.id,
            project_id: Edit::set_or_clear(project),
            project_milestone_id: Edit::Unchanged,
            cycle_id: Edit::Unchanged,
            state_id: Edit::set_or_unchanged(more.state),
            template_id: Edit::Unchanged,
            use_default_template: Edit::Set(fields.use_default_template),
            description: Edit::set_or_unchanged(description),
        },
    })
}
