//! `issue agent-session list`: the sessions started from an issue's comments.
use chrono::{DateTime, Utc};

use crate::cli::issue::IssueAgentSessionList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::agent_session::{
    AgentSessionStatus, GetIssueAgentSessions, GetIssueAgentSessionsVariables, ListSession,
    SessionComment,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::style;

use super::status_name;

pub fn run(ctx: &Ctx, args: &IssueAgentSessionList) -> Result<()> {
    list(ctx, args).context("Failed to list agent sessions")
}

fn list(ctx: &Ctx, args: &IssueAgentSessionList) -> Result<()> {
    let identifier = super::super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let comments = ctx.spin(!args.json, fetch_comments(client, &identifier))?;
    let mut sessions = sessions(comments, args.status);
    args.limit.apply(&mut sessions);
    if args.json {
        return ctx.print(json::render(&sessions));
    }
    if sessions.is_empty() {
        return ctx.print("No agent sessions found for this issue.\n");
    }
    ctx.print(table(&sessions, Utc::now()).render_for(ctx))
}

/// Every comment on the issue, for the agent sessions they started.
async fn fetch_comments(client: &LinearClient, id: &str) -> Result<Vec<SessionComment>> {
    pagination::collect(None, |after, first| async move {
        let data: GetIssueAgentSessions = client
            .query(GetIssueAgentSessionsVariables {
                issue_id: id.to_owned(),
                after,
                first,
            })
            .await?;
        Ok(Page {
            nodes: data.issue.comments.nodes,
            page_info: data.issue.comments.page_info,
        })
    })
    .await
}

/// The sessions started from the comments, in comment order, with `status`
/// only when given.
fn sessions(
    comments: Vec<SessionComment>,
    status: Option<crate::cli::AgentSessionStatus>,
) -> Vec<ListSession> {
    let status = status.map(|status| match status {
        crate::cli::AgentSessionStatus::Pending => AgentSessionStatus::Pending,
        crate::cli::AgentSessionStatus::Active => AgentSessionStatus::Active,
        crate::cli::AgentSessionStatus::Complete => AgentSessionStatus::Complete,
        crate::cli::AgentSessionStatus::AwaitingInput => AgentSessionStatus::AwaitingInput,
        crate::cli::AgentSessionStatus::Error => AgentSessionStatus::Error,
        crate::cli::AgentSessionStatus::Stale => AgentSessionStatus::Stale,
    });
    comments
        .into_iter()
        .filter_map(|comment| comment.agent_session)
        .filter(|session| status.is_none_or(|status| session.status == status))
        .collect()
}

fn table(sessions: &[ListSession], now: DateTime<Utc>) -> Table {
    let mut table = Table::new([
        Column::fixed("ID"),
        Column::fixed("STATUS"),
        Column::fixed("AGENT").droppable(2),
        Column::fixed("CREATED").droppable(1),
        Column::flexible("SUMMARY"),
    ]);
    for session in sessions {
        let status = status_name(session.status);
        let status = match session.status {
            AgentSessionStatus::Active => Cell::styled(status, style::green),
            AgentSessionStatus::Pending | AgentSessionStatus::AwaitingInput => {
                Cell::styled(status, style::yellow)
            }
            AgentSessionStatus::Complete | AgentSessionStatus::Stale => {
                Cell::styled(status, style::gray)
            }
            AgentSessionStatus::Error => Cell::styled(status, style::red),
        };
        let summary = match session.summary.as_deref().filter(|value| !value.is_empty()) {
            Some(summary) => Cell::from(summary.split_whitespace().collect::<Vec<_>>().join(" ")),
            None => Cell::styled("-", style::gray),
        };
        table.row([
            Cell::from(session.id.inner()),
            status,
            Cell::from(session.app_user.name.as_str()),
            Cell::styled(ago(session.created_at.0, now, &chrono::Local), style::gray),
            summary,
        ]);
    }
    table
}
