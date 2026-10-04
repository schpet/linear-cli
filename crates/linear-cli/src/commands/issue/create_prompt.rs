use super::{
    create::{self as issue_create, Fields, Input},
    write::{self as shared, Backend, CreateSettings, Label, Named, Parent, State, Ui},
};
use crate::cli::values::{Priority, estimate};
use crate::config::AssignSelf;
use crate::graphql::scalars::WholeNumber;
use crate::platform::prompt::{Choice, Text};
use crate::{error::Error, graphql::edit::Edit};
#[derive(Clone, Copy)]
enum Field {
    WorkflowState,
    Assignee,
    Priority,
    Labels,
    Estimate,
    Project,
}

enum Next {
    Submit,
    MoreFields,
}

fn yes_no<U: Ui>(ui: &mut U, message: &str, default: bool) -> Result<bool, Error> {
    ui.choose(
        message,
        vec![Choice::new("No", false), Choice::new("Yes", true)],
        usize::from(default),
    )
}
fn project_menu<U: Ui>(
    ui: &mut U,
    team: &crate::refs::team::ResolvedTeam,
    projects: &[Named],
) -> Result<Option<String>, Error> {
    if projects.is_empty() {
        ui.output(&format!(
            "Team {} has no projects, so the issue gets none.\n",
            team.key
        ))?;
        return Ok(None);
    }
    let rows = std::iter::once(Choice::new("No project", None))
        .chain(
            projects
                .iter()
                .map(|project| Choice::new(&project.name, Some(project.id.clone()))),
        )
        .collect();
    ui.choose("Which project should this issue belong to?", rows, 0)
}
async fn additional<B: Backend, U: Ui>(
    backend: &B,
    ui: &mut U,
    team: &crate::refs::team::ResolvedTeam,
    states: &[State],
    labels: &[Label],
    include_project: bool,
    auto: bool,
) -> Result<More, Error> {
    let default = shared::default_state(states);
    let name = default
        .as_ref()
        .and_then(|id| states.iter().find(|s| &s.id == id))
        .map(|s| s.name.as_str());
    let mut fields = vec![
        Choice::new(
            name.map(|name| format!("Workflow state ({name})"))
                .unwrap_or_else(|| "Workflow state".to_owned()),
            Field::WorkflowState,
        ),
        Choice::new(
            if auto {
                "Assignee (self)"
            } else {
                "Assignee (unassigned)"
            },
            Field::Assignee,
        ),
        Choice::new("Priority", Field::Priority),
        Choice::new("Labels", Field::Labels),
        Choice::new("Estimate", Field::Estimate),
    ];
    if include_project {
        fields.push(Choice::new("Project", Field::Project))
    }
    let selected = ui.checkbox("Select more fields to set:", fields)?;
    let mut more = More::default();
    // Choosing more fields starts them over, including the default state.
    if auto {
        more.assignee = Some(backend.viewer().await?)
    }
    for field in selected {
        match field {
            Field::WorkflowState if !states.is_empty() => {
                let options: Vec<_> = states
                    .iter()
                    .map(|s| Choice::new(format!("{} ({})", s.name, s.kind), s.id.clone()))
                    .collect();
                let index = default
                    .as_ref()
                    .and_then(|id| states.iter().position(|state| &state.id == id))
                    .unwrap_or(0);
                more.state = Some(ui.choose(
                    "Which workflow state should this issue be in?",
                    options,
                    index,
                )?);
            }
            Field::WorkflowState => ui.output(&format!(
                "Team {} has no workflow states to choose from.\n",
                team.key
            ))?,
            Field::Assignee => {
                let answer = yes_no(ui, "Assign this issue to yourself?", auto)?;
                more.assignee = if answer {
                    Some(backend.viewer().await?)
                } else {
                    None
                };
            }
            Field::Priority => {
                let values = [
                    (Priority::None, "No priority"),
                    (Priority::Urgent, "Urgent"),
                    (Priority::High, "High"),
                    (Priority::Medium, "Medium"),
                    (Priority::Low, "Low"),
                ];
                let options = values
                    .into_iter()
                    .map(|(value, label)| {
                        let glyph =
                            super::list_view::priority(WholeNumber(value.number().unsigned_abs()));
                        Choice::new(format!("{glyph} {label}"), value)
                    })
                    .collect::<Vec<_>>();
                let value = ui.choose("What priority should this issue have?", options, 0)?;
                more.priority = (value != Priority::None).then(|| value.number());
            }
            Field::Labels if !labels.is_empty() => {
                let options: Vec<_> = labels
                    .iter()
                    .map(|l| Choice::new(&l.name, l.id.clone()))
                    .collect();
                more.labels = ui.checkbox("Select labels:", options)?;
            }
            Field::Labels => ui.output(&format!(
                "Team {} has no labels to choose from.\n",
                team.key
            ))?,
            Field::Estimate => {
                more.estimate =
                    ui.parsed(Text::new("Estimate (leave blank for none)"), &estimate)?;
            }
            Field::Project => {
                let projects = backend.projects(team.key.clone()).await?;
                more.project = project_menu(ui, team, &projects)?;
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
    pub start: bool,
    /// The title and team key, for the final confirmation.
    pub title: String,
    pub team_key: String,
}
/// Asks for a new issue's fields, looking up the team's states, labels and
/// projects along the way.
pub async fn prompt<B: Backend, U: Ui>(
    backend: &B,
    ui: &mut U,
    settings: &CreateSettings,
    fields: &Fields,
) -> Result<Interactive, Error> {
    let (parent_id, parent_data) = issue_create::parent(backend, fields.parent.as_deref()).await?;
    let initial_project = match &fields.project {
        Some(value) => Some(issue_create::project(backend, ui, value, true).await?),
        None => None,
    };
    let auto = async {
        match settings.assign_self {
            AssignSelf::Always => Ok(true),
            AssignSelf::Never => Ok(false),
            AssignSelf::Auto => backend.auto_assign().await,
        }
    };
    let team = async {
        match settings.default_team.clone().filter(|key| !key.is_empty()) {
            Some(key) => backend.find_team(key).await,
            None => Ok(None),
        }
    };
    let (auto, team) = tokio::try_join!(auto, team)?;
    if let Some(parent) = &parent_data {
        ui.output(&format!(
            "Creating sub-issue for: {}: {}\n\n",
            parent.identifier, parent.title
        ))?
    }
    let title = ui.text(Text::new("What's the title of your issue?").required())?;
    let team = match team {
        Some(team) => team,
        None => {
            let teams = backend.teams().await?;
            if teams.is_empty() {
                return Err(crate::refs::team::none_accessible());
            }
            let options = teams
                .into_iter()
                .map(|team| Choice::new(format!("{} ({})", team.name, team.key), team))
                .collect();
            ui.choose("Which team should this issue belong to?", options, 0)?
        }
    };
    let ask_project = settings.ask_project
        && parent_data.is_none()
        && initial_project.as_deref().is_none_or(str::is_empty);
    let projects = async {
        if ask_project {
            backend.projects(team.key.clone()).await.map(Some)
        } else {
            Ok(None)
        }
    };
    let (states, labels, projects) = tokio::try_join!(
        backend.states(team.key.clone()),
        backend.labels(team.key.clone()),
        projects,
    )?;
    let editor = ui.discover_editor()?;
    let editor_label = editor.as_deref();
    let message = editor_label
        .map(|label| format!("Description [(e) to launch {label}]"))
        .unwrap_or_else(|| "Description".to_owned());
    let raw = ui.text(Text::new(&message))?;
    let description = if raw == "e" {
        if let Some(editor) = editor_label {
            ui.output(&format!("Opening {editor}...\n"))?;
            let text = ui.optional_editor()?;
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                ui.output(&format!(
                    "Description entered ({} characters)\n",
                    text.chars().count()
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
        (!raw.is_empty()).then_some(raw)
    };
    let mut project = initial_project.clone();
    if let Some(projects) = projects {
        project = project_menu(ui, &team, &projects)?;
    }
    let next = ui.choose(
        "What's next?",
        vec![
            Choice::new("Submit issue", Next::Submit),
            Choice::new("Add more fields", Next::MoreFields),
        ],
        0,
    )?;
    let mut more = More {
        state: shared::default_state(&states),
        ..Default::default()
    };
    if auto {
        more.assignee = Some(backend.viewer().await?)
    }
    match next {
        Next::Submit => (),
        Next::MoreFields => {
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
        }
    }
    let start = yes_no(
        ui,
        "Start working on this issue now? (creates branch and updates status)",
        false,
    )?;
    let project = project.or_else(|| parent_data.and_then(|Parent { project_id, .. }| project_id));
    Ok(Interactive {
        start,
        team_key: team.key,
        input: Input {
            title: Edit::Set(title.clone()),
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
        title,
    })
}

#[cfg(test)]
mod tests;
