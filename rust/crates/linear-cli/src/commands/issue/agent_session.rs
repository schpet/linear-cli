//! `issue agent-session list/view`: agent sessions on an issue's comments.
use chrono::{DateTime, TimeZone, Utc};

use crate::cli::issue::{IssueAgentSessionList, IssueAgentSessionView};
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

use crate::graphql::operations::agent_session::{
    AgentActivityContent, AgentActivityType, AgentSession, AgentSessionStatus, AgentSessionType,
    GetAgentSessionDetails, GetAgentSessionDetailsVariables, GetIssueAgentSessions,
    GetIssueAgentSessionsVariables, ListSession, SessionComment,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::style;
use crate::refs::reject_linear_url;

pub fn view(ctx: &Ctx, args: &IssueAgentSessionView) -> Result<()> {
    print_session(ctx, args).context("Failed to fetch agent session details")
}

pub fn list(ctx: &Ctx, args: &IssueAgentSessionList) -> Result<()> {
    print_sessions(ctx, args).context("Failed to list agent sessions")
}

fn print_session(ctx: &Ctx, args: &IssueAgentSessionView) -> Result<()> {
    reject_linear_url(&args.session_id, "an agent session ID")?;
    let client = ctx.client()?;
    let session = ctx.spin(!args.json, fetch_session(client, &args.session_id))?;
    if args.json {
        return ctx.print(json::render(&session));
    }
    ctx.show_markdown(&markdown(&session, Utc::now(), &chrono::Local)?, false)
}

fn print_sessions(ctx: &Ctx, args: &IssueAgentSessionList) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
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

/// The session with every one of its activities.
async fn fetch_session(client: &LinearClient, id: &str) -> Result<AgentSession> {
    let session = pagination::collect_within(
        None,
        |after, first| async move {
            let data: GetAgentSessionDetails = client
                .query(GetAgentSessionDetailsVariables {
                    id: id.to_owned(),
                    first,
                    after,
                })
                .await?;
            Ok(data.agent_session)
        },
        |session| Page {
            nodes: std::mem::take(&mut session.activities.nodes),
            page_info: session.activities.page_info.clone(),
        },
        |session, page| {
            session.activities.nodes = page.nodes;
            session.activities.page_info = page.page_info;
        },
    )
    .await?;
    ensure_supported(&session)?;
    Ok(session)
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

pub fn ensure_supported(session: &AgentSession) -> Result<(), Error> {
    for activity in &session.activities.nodes {
        activity.content.ensure_supported()?;
    }
    Ok(())
}

/// The sessions started from the comments, in comment order, with `status`
/// only when given.
pub fn sessions(
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

pub fn status_name(status: AgentSessionStatus) -> &'static str {
    match status {
        AgentSessionStatus::Pending => "pending",
        AgentSessionStatus::Active => "active",
        AgentSessionStatus::Complete => "complete",
        AgentSessionStatus::AwaitingInput => "awaitingInput",
        AgentSessionStatus::Error => "error",
        AgentSessionStatus::Stale => "stale",
    }
}

fn activity_type_name(kind: AgentActivityType) -> &'static str {
    match kind {
        AgentActivityType::Action => "action",
        AgentActivityType::Elicitation => "elicitation",
        AgentActivityType::Error => "error",
        AgentActivityType::Prompt => "prompt",
        AgentActivityType::Response => "response",
        AgentActivityType::Thought => "thought",
    }
}

fn activity_detail(content: &AgentActivityContent) -> Result<(AgentActivityType, String), Error> {
    let body = |kind, body: &str| {
        (
            kind,
            if body.is_empty() {
                String::new()
            } else {
                format!(" - {}", body.replace('\n', " "))
            },
        )
    };
    Ok(match content {
        AgentActivityContent::AgentActivityThoughtContent(value) => {
            body(value.activity_type, &value.body)
        }
        AgentActivityContent::AgentActivityResponseContent(value) => {
            body(value.activity_type, &value.body)
        }
        AgentActivityContent::AgentActivityPromptContent(value) => {
            body(value.activity_type, &value.body)
        }
        AgentActivityContent::AgentActivityErrorContent(value) => {
            body(value.activity_type, &value.body)
        }
        AgentActivityContent::AgentActivityElicitationContent(value) => {
            body(value.activity_type, &value.body)
        }
        AgentActivityContent::AgentActivityActionContent(value) => (
            value.activity_type,
            if value.action.is_empty() {
                String::new()
            } else {
                format!(" - {}: {}", value.action, value.parameter)
            },
        ),
        AgentActivityContent::Unsupported(_) => {
            content.ensure_supported()?;
            return Err(Error::new("unsupported activity was accepted"));
        }
    })
}

pub fn markdown<Tz: TimeZone>(
    session: &AgentSession,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Result<String, Error> {
    ensure_supported(session)?;
    let mut lines = vec![
        "# Agent Session".to_owned(),
        String::new(),
        format!("**ID:** {}", session.id.inner()),
        format!("**Status:** {}", status_name(session.status)),
        format!(
            "**Type:** {}",
            match session.session_type {
                Some(AgentSessionType::CommentThread) => "commentThread",
                None => "null",
            }
        ),
        format!("**Agent:** {}", session.app_user.name),
    ];
    if let Some(creator) = &session.creator {
        lines.push(format!("**Creator:** {}", creator.name));
    }
    if let Some(issue) = &session.issue {
        lines.push(format!("**Issue:** {} - {}", issue.identifier, issue.title));
    }
    lines.extend([
        String::new(),
        format!(
            "**Created:** {}",
            format_relative_time(&session.created_at.0, now, zone)
        ),
    ]);
    for (label, value) in [
        ("Started", &session.started_at),
        ("Ended", &session.ended_at),
        ("Dismissed", &session.dismissed_at),
    ] {
        if let Some(value) = value.as_ref().filter(|value| !value.0.is_empty()) {
            lines.push(format!(
                "**{label}:** {}",
                format_relative_time(&value.0, now, zone)
            ));
            if label == "Dismissed"
                && let Some(user) = &session.dismissed_by
            {
                lines.push(format!("**Dismissed by:** {}", user.name));
            }
        }
    }
    if let Some(link) = session
        .external_link
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.extend([String::new(), format!("**External Link:** {link}")]);
    }
    if let Some(summary) = session.summary.as_deref().filter(|value| !value.is_empty()) {
        lines.extend([
            String::new(),
            "## Summary".to_owned(),
            String::new(),
            summary.to_owned(),
        ]);
    }
    if !session.activities.nodes.is_empty() {
        lines.extend([String::new(), "## Activities".to_owned(), String::new()]);
        for activity in &session.activities.nodes {
            let (kind, detail) = activity_detail(&activity.content)?;
            lines.push(format!(
                "- **{}** ({}){detail}",
                activity_type_name(kind),
                format_relative_time(&activity.created_at.0, now, zone)
            ));
        }
    }
    Ok(lines.join("\n"))
}

pub fn table(sessions: &[ListSession], now: DateTime<Utc>) -> Table {
    let mut table = Table::new([
        Column::fixed("ID"),
        Column::fixed("STATUS"),
        Column::fixed("AGENT"),
        Column::fixed("CREATED"),
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
            Cell::styled(
                format_relative_time(&session.created_at.0, now, &chrono::Local),
                style::gray,
            ),
            summary,
        ]);
    }
    table
}

#[cfg(test)]
mod tests;
