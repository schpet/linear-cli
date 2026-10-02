//! Agent-session reads preserve source selections, connection shape and order.
use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::relative_time::format_relative_time;
use crate::commands::style;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::agent_session::{
    AgentActivityContent, AgentActivityType, AgentSession, AgentSessionStatus, AgentSessionType,
    GetAgentSessionDetails, GetAgentSessionDetailsVariables, GetIssueAgentSessions,
    GetIssueAgentSessionsVariables, SessionComments,
};
use crate::graphql::transport::GraphQlTransport;

pub const VIEW_CONTEXT: &str = "Failed to fetch agent session details";
pub const LIST_CONTEXT: &str = "Failed to list agent sessions";

pub fn view_request(id: &str) -> GraphQlRequest<GetAgentSessionDetailsVariables> {
    GraphQlRequest::with_variables(GetAgentSessionDetails::build(
        GetAgentSessionDetailsVariables { id: id.to_owned() },
    ))
}

pub fn list_request(issue_id: &str) -> GraphQlRequest<GetIssueAgentSessionsVariables> {
    GraphQlRequest::with_variables(GetIssueAgentSessions::build(
        GetIssueAgentSessionsVariables {
            issue_id: issue_id.to_owned(),
        },
    ))
}

pub async fn view(transport: &GraphQlTransport, id: &str) -> Result<AgentSession, AppError> {
    let data: GetAgentSessionDetails = transport
        .execute(&view_request(id))
        .await
        .map_err(AppError::from)?;
    ensure_supported(&data.agent_session)?;
    Ok(data.agent_session)
}

pub async fn list(
    transport: &GraphQlTransport,
    id: &str,
    status: Option<crate::cli::AgentSessionStatus>,
) -> Result<SessionComments, AppError> {
    let data: GetIssueAgentSessions = transport
        .execute(&list_request(id))
        .await
        .map_err(AppError::from)?;
    Ok(filter(data.issue.comments, status))
}

pub fn ensure_supported(session: &AgentSession) -> Result<(), AppError> {
    for activity in &session.activities.nodes {
        activity.content.ensure_supported()?;
    }
    Ok(())
}

pub fn filter(
    mut comments: SessionComments,
    status: Option<crate::cli::AgentSessionStatus>,
) -> SessionComments {
    if let Some(status) = status {
        let status = match status {
            crate::cli::AgentSessionStatus::Pending => AgentSessionStatus::Pending,
            crate::cli::AgentSessionStatus::Active => AgentSessionStatus::Active,
            crate::cli::AgentSessionStatus::Complete => AgentSessionStatus::Complete,
            crate::cli::AgentSessionStatus::AwaitingInput => AgentSessionStatus::AwaitingInput,
            crate::cli::AgentSessionStatus::Error => AgentSessionStatus::Error,
            crate::cli::AgentSessionStatus::Stale => AgentSessionStatus::Stale,
        };
        comments.nodes.retain(|comment| {
            comment
                .agent_session
                .as_ref()
                .is_some_and(|session| session.status == status)
        });
    }
    comments
}

pub fn json(value: &impl Serialize) -> Result<Vec<u8>, AppError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not serialize agent sessions",
        )
        .with_source(error)
    })?;
    bytes.push(b'\n');
    Ok(bytes)
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

fn activity_detail(
    content: &AgentActivityContent,
) -> Result<(AgentActivityType, String), AppError> {
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
            return Err(AppError::new(
                AppErrorKind::Invariant,
                "unsupported activity was accepted",
            ));
        }
    })
}

pub fn markdown<Tz: TimeZone>(
    session: &AgentSession,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Result<String, AppError> {
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

pub fn text(comments: &SessionComments, columns: usize, color: bool) -> Vec<u8> {
    let sessions: Vec<_> = comments
        .nodes
        .iter()
        .filter_map(|comment| comment.agent_session.as_ref())
        .collect();
    if sessions.is_empty() {
        return b"No agent sessions found for this issue.\n".to_vec();
    }
    let agent_width = sessions
        .iter()
        .map(|session| display_width(&session.app_user.name))
        .max()
        .unwrap_or(0)
        .max(5);
    let available_width = columns
        .saturating_sub(1 + 13 + 10 + agent_width + 3)
        .max(10);
    let header = [
        pad("STATUS", 13),
        pad("AGENT", agent_width),
        pad("CREATED", 10),
        "SUMMARY".to_owned(),
    ]
    .join(" ");
    let mut output = format!(
        "{}\n",
        style::bold(&style::apply(4, 24, &header, color), color)
    );
    for session in sessions {
        let status = pad(status_name(session.status), 13);
        let status = match session.status {
            AgentSessionStatus::Active => style::apply(32, 39, &status, color),
            AgentSessionStatus::Pending | AgentSessionStatus::AwaitingInput => {
                style::apply(33, 39, &status, color)
            }
            AgentSessionStatus::Complete | AgentSessionStatus::Stale => {
                style::apply(90, 39, &status, color)
            }
            AgentSessionStatus::Error => status,
        };
        let summary = match session.summary.as_deref().filter(|value| !value.is_empty()) {
            Some(summary) => truncate_text(&summary.replace('\n', " "), available_width),
            None => style::apply(90, 39, "--", color),
        };
        output.push_str(&format!(
            "{status} {} {} {summary}\n",
            pad(&session.app_user.name, agent_width),
            pad(&created_date(&session.created_at.0), 10)
        ));
    }
    output.into_bytes()
}

/// The UTC calendar date of a timestamp, or the raw text when it does not parse.
fn created_date(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp).map_or_else(
        |_| timestamp.to_owned(),
        |date| date.to_utc().format("%Y-%m-%d").to_string(),
    )
}
