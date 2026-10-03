//! `team states`: a team's workflow states in display order.
use serde::Serialize;

use super::TeamArg;
use crate::cli::team::TeamStates;
use crate::commands::json;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::team::WorkflowState;
use crate::graphql::scalars::Float;
use crate::refs::workflow_states;

pub fn run(ctx: &Ctx, args: &TeamStates) -> Result<()> {
    states(ctx, args).context("Failed to list workflow states")
}

fn states(ctx: &Ctx, args: &TeamStates) -> Result<()> {
    let team = TeamArg::prepare(ctx, args.team.as_deref())?;
    let client = ctx.client()?;
    let mut states = ctx.spin(!args.json, async {
        let key = team.key(client).await?;
        Ok::<_, Error>(workflow_states::fetch(client, key).await?)
    })?;
    args.limit.apply(&mut states);
    if args.json {
        ctx.print(render_json(&states))
    } else if states.is_empty() {
        ctx.print("No workflow states found for this team.\n")
    } else {
        ctx.print(render_text(&states).render_for(ctx))
    }
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
    let states: Vec<_> = states
        .iter()
        .map(|state| JsonState {
            id: state.id.inner(),
            name: &state.name,
            state_type: &state.state_type,
            position: &state.position,
        })
        .collect();
    json::render(&states)
}

fn render_text(states: &[WorkflowState]) -> Table {
    let mut table = Table::new([Column::fixed("NAME"), Column::fixed("TYPE")]);
    for state in states {
        table.row([
            Cell::from(state.name.as_str()),
            Cell::from(state.state_type.as_str()),
        ]);
    }
    table
}
