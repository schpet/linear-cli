//! `issue agent-session view`: one session and its activities.
use chrono::{DateTime, TimeZone, Utc};

use crate::cli::issue::IssueAgentSessionView;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::agent_session::{
    AgentActivityContent, AgentActivityType, AgentSession, AgentSessionType,
    GetAgentSessionDetails, GetAgentSessionDetailsVariables,
};
use crate::graphql::pagination::{self, Page};
use crate::refs::reject_linear_url;

use super::status_name;

pub fn run(ctx: &Ctx, args: &IssueAgentSessionView) -> Result<()> {
    view(ctx, args).context("Failed to fetch agent session details")
}

fn view(ctx: &Ctx, args: &IssueAgentSessionView) -> Result<()> {
    reject_linear_url(&args.session_id, "an agent session ID")?;
    let client = ctx.client()?;
    let session = ctx.spin(!args.json, fetch_session(client, &args.session_id))?;
    if args.json {
        return ctx.print(json::render(&session));
    }
    ctx.show_markdown(&markdown(&session, Utc::now(), &chrono::Local)?, false)
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

fn ensure_supported(session: &AgentSession) -> Result<(), Error> {
    for activity in &session.activities.nodes {
        activity.content.ensure_supported()?;
    }
    Ok(())
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

fn markdown<Tz: TimeZone>(
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
        format!("**Created:** {}", ago(session.created_at.0, now, zone)),
    ]);
    for (label, value) in [
        ("Started", &session.started_at),
        ("Ended", &session.ended_at),
        ("Dismissed", &session.dismissed_at),
    ] {
        if let Some(value) = value {
            lines.push(format!("**{label}:** {}", ago(value.0, now, zone)));
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
                ago(activity.created_at.0, now, zone)
            ));
        }
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests;
