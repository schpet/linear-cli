//! `team states`: a team's workflow states in display order.
use cynic::QueryBuilder;
use serde::Serialize;

use super::TeamArg;
use crate::cli::team::TeamStates;
use crate::commands::display::{display_width, pad};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::number::Float;
use crate::graphql::operations::workflow_states::{
    GetWorkflowStates, GetWorkflowStatesVariables, WorkflowState,
};
use crate::platform::style;
use crate::workflow_states;

pub fn run(ctx: &Ctx, args: &TeamStates) -> Result<()> {
    states(ctx, args).context("Failed to list workflow states")
}

fn states(ctx: &Ctx, args: &TeamStates) -> Result<()> {
    let team = TeamArg::prepare(ctx, args.team.as_deref())?;
    let client = ctx.client()?;
    let mut states = ctx.spin(!args.json, async {
        let key = team.key(client).await?;
        let response: GetWorkflowStates = client.execute(&request(key)).await?;
        Ok::<_, Error>(response.team.states.nodes)
    })?;
    workflow_states::sort(&mut states);
    if args.json {
        ctx.print(render_json(&states))
    } else {
        ctx.print(render_text(&states, ctx.color()))
    }
}

/// The workflow states of the team with key `team_key`.
pub fn request(team_key: String) -> GraphQlRequest<GetWorkflowStatesVariables> {
    GraphQlRequest::with_variables(GetWorkflowStates::build(GetWorkflowStatesVariables {
        team_key,
    }))
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

fn render_json(states: &[WorkflowState]) -> Vec<u8> {
    let nodes = states
        .iter()
        .map(|state| JsonState {
            id: state.id.inner(),
            name: &state.name,
            state_type: &state.state_type,
            position: &state.position,
        })
        .collect();
    let mut bytes = serde_json::to_vec_pretty(&JsonConnection { nodes })
        .expect("workflow state JSON always serializes");
    bytes.push(b'\n');
    bytes
}

fn render_text(states: &[WorkflowState], color: bool) -> String {
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
    let header = format!("{} {}", pad("NAME", name_width), pad("TYPE", type_width));
    let mut output = format!(
        "{}\n",
        style::bold(&style::underline(&header, color), color)
    );
    for state in states {
        output.push_str(&format!(
            "{} {}\n",
            pad(&state.name, name_width),
            pad(&state.state_type, type_width)
        ));
    }
    output
}
