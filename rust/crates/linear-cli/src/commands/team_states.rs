//! `team states`: a team's workflow states in display order.

use std::future::Future;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, pad};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::number::Float;
use crate::graphql::operations::workflow_states::{
    GetWorkflowStates, GetWorkflowStatesVariables, WorkflowState,
};
use crate::graphql::transport::GraphQlTransport;
use crate::workflow_states;

pub const CONTEXT: &str = "Failed to fetch workflow states";

pub fn request(team_key: String) -> GraphQlRequest<GetWorkflowStatesVariables> {
    GraphQlRequest::with_variables(GetWorkflowStates::build(GetWorkflowStatesVariables {
        team_key,
    }))
}

pub async fn run_with<F, Fut>(
    team_key: String,
    json: bool,
    color: bool,
    fetch: F,
) -> Result<Vec<u8>, AppError>
where
    F: FnOnce(GraphQlRequest<GetWorkflowStatesVariables>) -> Fut,
    Fut: Future<Output = Result<GetWorkflowStates, AppError>>,
{
    let response = fetch(request(team_key)).await?;
    let mut states = response.team.states.nodes;
    workflow_states::sort(&mut states);
    if json {
        render_json(&states)
    } else {
        Ok(render_text(&states, color).into_bytes())
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    team_key: String,
    json: bool,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(team_key, json, color, |request| async move {
        transport.execute(&request).await.map_err(AppError::from)
    })
    .await
}

#[derive(Serialize)]
struct JsonConnection<'a> {
    nodes: Vec<JsonState<'a>>,
}

#[derive(Serialize)]
struct JsonState<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    state_type: &'a str,
    position: &'a Float,
}

fn render_json(states: &[WorkflowState]) -> Result<Vec<u8>, AppError> {
    let nodes = states
        .iter()
        .map(|state| {
            Ok(JsonState {
                id: state.id.inner(),
                name: &state.name,
                state_type: &state.state_type,
                position: &state.position,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut bytes = serde_json::to_vec_pretty(&JsonConnection { nodes }).map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not serialize workflow states",
        )
        .with_source(error)
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn render_text(states: &[WorkflowState], color: bool) -> String {
    if states.is_empty() {
        return "No workflow states found for this team.\n".to_owned();
    }
    let name_width = states
        .iter()
        .map(|state| display_width(&state.name))
        .max()
        .unwrap_or(0)
        .max(display_width("NAME"));
    let type_width = states
        .iter()
        .map(|state| display_width(&state.state_type))
        .max()
        .unwrap_or(0)
        .max(display_width("TYPE"));
    let name_header = pad("NAME", name_width);
    let type_header = pad("TYPE", type_width);
    let mut output = if color {
        format!("\x1b[4m{name_header}\x1b[24m \x1b[4m{type_header}\x1b[24m\x1b[0m\n")
    } else {
        format!("{name_header} {type_header}\n")
    };
    for state in states {
        output.push_str(&pad(&state.name, name_width));
        output.push(' ');
        output.push_str(&pad(&state.state_type, type_width));
        output.push('\n');
    }
    output
}
