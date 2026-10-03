//! `issue agent-session`: agent sessions on an issue's comments.
mod list;
mod view;

use crate::cli::issue::IssueAgentSessionCommand;
use crate::ctx::Ctx;
use crate::error::Result;
use crate::graphql::operations::agent_session::AgentSessionStatus;

pub fn run(ctx: &Ctx, command: &IssueAgentSessionCommand) -> Result<()> {
    match command {
        IssueAgentSessionCommand::List(args) => list::run(ctx, args),
        IssueAgentSessionCommand::View(args) => view::run(ctx, args),
    }
}

fn status_name(status: AgentSessionStatus) -> &'static str {
    match status {
        AgentSessionStatus::Pending => "pending",
        AgentSessionStatus::Active => "active",
        AgentSessionStatus::Complete => "complete",
        AgentSessionStatus::AwaitingInput => "awaitingInput",
        AgentSessionStatus::Error => "error",
        AgentSessionStatus::Stale => "stale",
    }
}
