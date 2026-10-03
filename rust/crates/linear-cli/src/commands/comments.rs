//! Comment lists, shared by the issue, project, document and initiative
//! `comment list` commands: fetching every page, and printing threads or JSON.
use crate::graphql::operations::user::UserRef;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cli::Limit;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::comment::{
    CommentBotActor, CommentConnection, CommentExternalUser, CommentNode, CommentParent,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::style::bold;

/// A comments query for one kind of entity.
pub trait CommentSource: QueryBuilder<Self::Variables> + DeserializeOwned {
    /// The entity, capitalized, as in "Issue not found".
    const ENTITY: &'static str;
    type Variables: Serialize;

    /// Variables for one page of up to `first` comments after cursor `after`.
    fn variables(id: &str, after: Option<String>, first: i32) -> Self::Variables;

    /// The page of comments, or `None` when the entity does not exist.
    fn comments(self) -> Option<CommentConnection>;
}

/// The comments on entity `id` (which the user called `original`), up to `limit`.
pub async fn fetch<S: CommentSource>(
    client: &LinearClient,
    original: &str,
    id: &str,
    limit: Limit,
) -> Result<Vec<CommentNode>> {
    let not_found = || Error::not_found(S::ENTITY, original);
    pagination::collect(limit.max(), |after, first| {
        let variables = S::variables(id, after, first);
        async move {
            let response: S = client
                .query(variables)
                .await
                .map_err(|failure| failure.or_not_found(S::ENTITY, original))?;
            let connection = response.comments().ok_or_else(not_found)?;
            Ok(Page {
                nodes: connection.nodes,
                page_info: connection.page_info,
            })
        }
    })
    .await
}

/// Prints comments as threads, or as JSON. `noun` names the entity in the
/// empty message ("issue").
pub fn print(ctx: &Ctx, comments: &[CommentNode], as_json: bool, noun: &str) -> Result<()> {
    if as_json {
        ctx.print(render_json(comments))
    } else if comments.is_empty() {
        ctx.print(format!("No comments found for this {noun}\n"))
    } else {
        ctx.print(render_text(comments, Utc::now(), ctx.color()))
    }
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
    user: &'a Option<UserRef>,
    external_user: &'a Option<CommentExternalUser>,
    bot_actor: &'a Option<CommentBotActor>,
    parent: &'a Option<CommentParent>,
}

fn render_json(nodes: &[CommentNode]) -> Vec<u8> {
    let comments: Vec<_> = nodes
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
    json::render(&comments)
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
        ago(node.created_at.0, now, &Local),
        node.id.inner()
    )
}

fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
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
    roots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
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
            siblings.sort_by_key(|node| node.created_at);
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
    orphans.sort_by_key(|(node, _)| node.created_at);
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
