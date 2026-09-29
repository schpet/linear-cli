//! `project comment list`: typed root connection, complete pagination, and threads.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::future::Future;

use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::render::style::bold;
use crate::commands::relative_time::format_relative_time;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::project_comments::{
    CommentBotActor, CommentExternalUser, CommentNode, CommentParent, CommentUser,
    GetProjectComments, GetProjectCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page, PaginationError};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};

pub const CONTEXT: &str = "Failed to list comments";

pub fn request(id: &str, after: Option<String>) -> GraphQlRequest<GetProjectCommentsVariables> {
    GraphQlRequest::with_variables(GetProjectComments::build(GetProjectCommentsVariables {
        id: id.to_owned(),
        filter_id: cynic::Id::new(id),
        after,
    }))
}

/// Complete every request before returning a byte of output to the caller.
pub async fn run(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
    json: bool,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(
        original,
        id,
        |query| async move {
            transport
                .execute(&query)
                .await
                .map_err(|failure| translate_failure(failure, original))
        },
        json,
        color,
        Utc::now(),
    )
    .await
}

pub async fn run_with<F, Fut>(
    original: &str,
    id: &str,
    mut fetch: F,
    json: bool,
    color: bool,
    now: DateTime<Utc>,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetProjectCommentsVariables>) -> Fut,
    Fut: Future<Output = Result<GetProjectComments, AppError>>,
{
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let query = request(id, after);
        let pending = fetch(query);
        async move {
            let data = pending.await?;
            if data.project.is_none() {
                return Err(AppError::not_found("Project", original));
            }
            Ok::<Page<CommentNode>, AppError>(Page {
                nodes: data.comments.nodes,
                page_info: data.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(pagination_error)?;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    if json {
        render_json(&result.nodes, &page_info)
    } else {
        Ok(render_text(&result.nodes, now, color).into_bytes())
    }
}

fn translate_failure(failure: TransportFailure, original: &str) -> AppError {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return AppError::not_found("Project", original);
    }
    AppError::from(failure)
}

fn pagination_error(error: PaginationError<AppError>) -> AppError {
    let error = match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            AppError::new(
                AppErrorKind::Validation,
                "Linear reported more comments but did not return a usable cursor",
            )
            .with_suggestion("Rerun the command; if it persists, report it.")
        }
    };
    error.with_context(CONTEXT)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonComment<'a>>,
    page_info: &'a PageInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonComment<'a> {
    id: &'a cynic::Id,
    body: &'a str,
    quoted_text: &'a Option<String>,
    created_at: &'a crate::graphql::scalars::DateTime,
    updated_at: &'a crate::graphql::scalars::DateTime,
    edited_at: &'a Option<crate::graphql::scalars::DateTime>,
    url: &'a str,
    user: &'a Option<CommentUser>,
    external_user: &'a Option<CommentExternalUser>,
    bot_actor: &'a Option<CommentBotActor>,
    parent: &'a Option<CommentParent>,
}

pub fn render_json(nodes: &[CommentNode], page_info: &PageInfo) -> Result<Vec<u8>, AppError> {
    let nodes = nodes
        .iter()
        .map(|node| JsonComment {
            id: &node.id,
            body: &node.body,
            quoted_text: &node.quoted_text,
            created_at: &node.created_at,
            updated_at: &node.updated_at,
            edited_at: &node.edited_at,
            url: &node.url,
            user: &node.user,
            external_user: &node.external_user,
            bot_actor: &node.bot_actor,
            parent: &node.parent,
        })
        .collect();
    let mut output =
        serde_json::to_vec_pretty(&JsonConnection { nodes, page_info }).map_err(|error| {
            AppError::new(
                AppErrorKind::Invariant,
                "could not serialize project comments",
            )
            .with_source(error)
            .with_context(CONTEXT)
        })?;
    output.push(b'\n');
    Ok(output)
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn author(node: &CommentNode) -> &str {
    node.user
        .as_ref()
        .and_then(|user| nonempty(&user.display_name).or_else(|| nonempty(&user.name)))
        .or_else(|| {
            node.external_user
                .as_ref()
                .and_then(|user| nonempty(&user.display_name).or_else(|| nonempty(&user.name)))
        })
        .or_else(|| {
            node.bot_actor.as_ref().map(|bot| {
                bot.name
                    .as_deref()
                    .and_then(nonempty)
                    .unwrap_or(&bot.bot_type)
            })
        })
        .unwrap_or("Unknown")
}

fn timestamp_millis(node: &CommentNode) -> Option<i64> {
    DateTime::parse_from_rfc3339(&node.created_at.0)
        .ok()
        .map(|date| date.timestamp_millis())
}

fn compare_time(left: &CommentNode, right: &CommentNode, descending: bool) -> Ordering {
    let order = match (timestamp_millis(left), timestamp_millis(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    if descending && timestamp_millis(left).is_some() && timestamp_millis(right).is_some() {
        order.reverse()
    } else {
        order
    }
}

fn indent(text: &str) -> String {
    text.split('\n')
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn header(node: &CommentNode, verb: &str, now: DateTime<Utc>, color: bool) -> String {
    format!(
        "{} {verb} {} [{}]",
        bold(&format!("@{}", author(node)), color),
        format_relative_time(&node.created_at.0, now, &Local),
        node.id.inner()
    )
}

pub fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
    if nodes.is_empty() {
        return "No comments found for this project\n".to_owned();
    }
    let mut roots: Vec<&CommentNode> = nodes.iter().filter(|node| node.parent.is_none()).collect();
    let root_ids: HashSet<&str> = roots.iter().map(|node| node.id.inner()).collect();
    let mut replies: HashMap<&str, Vec<&CommentNode>> = HashMap::new();
    let mut orphans: Vec<(&CommentNode, &str)> = Vec::new();
    for node in nodes {
        if let Some(parent) = &node.parent {
            if root_ids.contains(parent.id.inner()) {
                replies.entry(parent.id.inner()).or_default().push(node);
            } else {
                orphans.push((node, parent.id.inner()));
            }
        }
    }
    roots.sort_by(|left, right| compare_time(left, right, true));
    let mut output = String::new();
    for root in roots {
        output.push_str(&header(root, "commented", now, color));
        output.push('\n');
        if let Some(quote) = &root.quoted_text {
            output.push_str("> ");
            output.push_str(quote);
            output.push('\n');
        }
        output.push_str(&root.body);
        output.push('\n');
        if let Some(siblings) = replies.get_mut(root.id.inner())
            && !siblings.is_empty()
        {
            output.push('\n');
            siblings.sort_by(|left, right| compare_time(left, right, false));
            for reply in siblings {
                output.push_str(&indent(&header(reply, "replied", now, color)));
                output.push('\n');
                if let Some(quote) = &reply.quoted_text {
                    output.push_str(&indent(&format!("> {quote}")));
                    output.push('\n');
                }
                output.push_str(&indent(&reply.body));
                output.push('\n');
            }
        }
        output.push('\n');
    }
    orphans.sort_by(|(left, _), (right, _)| compare_time(left, right, false));
    for (reply, parent) in orphans {
        output.push_str(&indent(&header(
            reply,
            &format!("replied to [{parent}]"),
            now,
            color,
        )));
        output.push('\n');
        if let Some(quote) = &reply.quoted_text {
            output.push_str(&indent(&format!("> {quote}")));
            output.push('\n');
        }
        output.push_str(&indent(&reply.body));
        output.push_str("\n\n");
    }
    output
}
