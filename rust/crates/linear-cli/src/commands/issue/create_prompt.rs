use super::{
    create::{self as issue_create, Fields, Input},
    write::{self as shared, AssignSelf, Backend, CreateSettings, Label, Named, Parent, State, Ui},
};
use crate::cli::values::estimate;
use crate::graphql::scalars::WholeNumber;
use crate::platform::prompt::Text;
use crate::{error::Error, graphql::edit::Edit};
fn option(id: &str, name: &str) -> Named {
    Named {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}
fn yes_no<U: Ui>(ui: &mut U, message: &str) -> Result<bool, Error> {
    Ok(ui.choose(message, &[option("no", "No"), option("yes", "Yes")], 0)? == "yes")
}
fn project_menu<U: Ui>(ui: &mut U, projects: &[Named]) -> Result<Option<String>, Error> {
    if projects.is_empty() {
        return Ok(None);
    }
    let mut rows = vec![option("__none__", "No project")];
    rows.extend_from_slice(projects);
    let answer = ui.choose("Which project should this issue belong to?", &rows, 0)?;
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
) -> Result<More, Error> {
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
    let selected = ui.checkbox("Select additional fields to configure", &fields)?;
    let mut more = More::default();
    // Choosing more fields starts them over, including the default state.
    if auto {
        more.assignee = Some(backend.viewer().await?)
    }
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
                )?);
            }
            "workflow_state" => (),
            "assignee" => {
                let answer = yes_no(ui, "Assign this issue to yourself?")?;
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
                        let glyph = super::read::priority(WholeNumber(value));
                        option(&value.to_string(), &format!("{glyph} {label}"))
                    })
                    .collect::<Vec<_>>();
                let value = ui.choose("What priority should this issue have?", &options, 0)?;
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
                )?;
            }
            "labels" => (),
            "estimate" => {
                let check = |raw: &str| estimate(raw).map(drop);
                let answer =
                    ui.text(Text::new("Estimate (leave blank for none)").with_check(&check))?;
                more.estimate = (!answer.is_empty())
                    .then(|| estimate(&answer))
                    .transpose()
                    .map_err(shared::validation)?;
            }
            "project" => {
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
            let options: Vec<_> = teams
                .iter()
                .map(|t| option(&t.id, &format!("{} ({})", t.name, t.key)))
                .collect();
            let selected = ui.choose("Which team should this issue belong to?", &options, 0)?;
            teams
                .into_iter()
                .find(|t| t.id == selected)
                .expect("the picked team is one of the options")
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
        project = project_menu(ui, &projects)?;
    }
    let next = ui.choose(
        "What's next?",
        &[
            option("submit", "Submit issue"),
            option("more_fields", "Add more fields"),
        ],
        0,
    )?;
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

#[cfg(test)]
mod tests;
