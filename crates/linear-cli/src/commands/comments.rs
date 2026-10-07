//! Comment lists, shared by the issue, project, document and initiative
//! `comment list` commands: fetching every page, and printing threads or JSON.
use crate::graphql::operations::user::UserRef;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;

use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;
use serde::de::DeserializeOwned;
use unicode_width::UnicodeWidthStr;

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
use crate::platform::markdown_terminal::{self, RenderOptions};
use crate::platform::style::{bold, heading, underline};
use crate::platform::terminal_text::{multiline, single_line, wrap_parts};

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
/// empty message ("issue"); long threads are paged on a terminal when `paging`
/// is on.
pub fn print(
    ctx: &Ctx,
    comments: &[CommentNode],
    as_json: bool,
    noun: &str,
    paging: bool,
) -> Result<()> {
    if as_json {
        ctx.print(render_json(comments))
    } else if comments.is_empty() {
        ctx.print(format!("No comments found for this {noun}\n"))
    } else if ctx.stdout_tty() {
        let rendered = render_terminal(comments, Utc::now(), &ctx.render_options());
        ctx.page(&rendered, paging)
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
    resolved_at: &'a Option<crate::graphql::scalars::DateTime>,
    resolving_comment_id: &'a Option<String>,
    resolving_user: &'a Option<UserRef>,
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
            resolved_at: &node.resolved_at,
            resolving_comment_id: &node.resolving_comment_id,
            resolving_user: &node.resolving_user,
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
    let resolved = if node.resolved_at.is_some() {
        format!(" {RESOLVED}")
    } else {
        String::new()
    };
    format!(
        "{} {verb} {} [{}]{resolved}",
        bold(&format!("@{}", single_line(author(node))), color),
        ago(node.created_at.0, now, &Local),
        node.id.inner()
    )
}

/// Marks a resolved thread after its top-level comment's ID.
pub const RESOLVED: &str = "[resolved]";

/// Whether the thread `node` is in is resolved: a top-level comment carries
/// its thread's state, and a reply has it from its parent.
pub fn in_resolved_thread(node: &CommentNode) -> bool {
    match &node.parent {
        Some(parent) => parent.resolved_at.is_some(),
        None => node.resolved_at.is_some(),
    }
}

/// Comments grouped for display: threads oldest first, each with its
/// replies oldest first, then replies whose parent is not in the list.
struct Threads<'a> {
    roots: Vec<(&'a CommentNode, Vec<&'a CommentNode>)>,
    orphans: Vec<(&'a CommentNode, &'a str)>,
}

impl<'a> Threads<'a> {
    fn new(nodes: &'a [CommentNode]) -> Self {
        let mut roots: Vec<&CommentNode> =
            nodes.iter().filter(|node| node.parent.is_none()).collect();
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
        // Oldest first, as `issue view` and Linear show a conversation.
        roots.sort_by_key(|node| node.created_at);
        orphans.sort_by_key(|(node, _)| node.created_at);
        let roots = roots
            .into_iter()
            .map(|root| {
                let mut siblings = replies.remove(root.id.inner()).unwrap_or_default();
                siblings.sort_by_key(|node| node.created_at);
                (root, siblings)
            })
            .collect();
        Self { roots, orphans }
    }
}

fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
    let threads = Threads::new(nodes);
    let mut output = String::new();
    for (root, replies) in threads.roots {
        output.push_str(&header(root, "commented", now, color));
        output.push('\n');
        if let Some(quote) = &root.quoted_text {
            output.push_str("> ");
            output.push_str(&multiline(quote));
            output.push('\n');
        }
        output.push_str(&multiline(&root.body));
        output.push('\n');
        if !replies.is_empty() {
            output.push('\n');
            for reply in replies {
                output.push_str(&indent(&header(reply, "replied", now, color)));
                output.push('\n');
                if let Some(quote) = &reply.quoted_text {
                    output.push_str(&indent(&format!("> {}", multiline(quote))));
                    output.push('\n');
                }
                output.push_str(&indent(&multiline(&reply.body)));
                output.push('\n');
            }
        }
        output.push('\n');
    }
    for (reply, parent) in threads.orphans {
        output.push_str(&indent(&header(
            reply,
            &format!("replied to [{parent}]"),
            now,
            color,
        )));
        output.push('\n');
        if let Some(quote) = &reply.quoted_text {
            output.push_str(&indent(&format!("> {}", multiline(quote))));
            output.push('\n');
        }
        output.push_str(&indent(&multiline(&reply.body)));
        output.push_str("\n\n");
    }
    output
}

/// A comment's ID in brackets, linked to the comment when `links` is on,
/// with its display width.
pub fn id_part(id: &str, url: &str, links: bool) -> (String, usize) {
    let text = format!("[{id}]");
    let width = text.width();
    if links {
        (markdown_terminal::hyperlink(&text, url), width)
    } else {
        (text, width)
    }
}

/// A comment's header on the terminal, `@alice commented 3 days ago [id]`:
/// the author, the action, then `suffix`, wrapped to the terminal between
/// parts with every line starting with `indent`.
pub fn terminal_header(
    author: &str,
    action: &str,
    suffix: Vec<(String, usize)>,
    indent: &str,
    options: &RenderOptions,
) -> String {
    let author = format!("@{}", single_line(author));
    let mut parts = vec![
        (heading(&author, options.styled), author.width()),
        (underline(action, options.styled), action.width()),
    ];
    parts.extend(suffix);
    wrap_parts(&parts, indent, usize::from(options.columns.get()))
}

/// Markdown rendered for the terminal two columns in, as a reply under its
/// thread.
pub fn render_reply(markdown: &str, options: &RenderOptions) -> String {
    let narrower = RenderOptions {
        columns: NonZeroU16::new(options.columns.get().saturating_sub(2))
            .unwrap_or(NonZeroU16::MIN),
        ..options.clone()
    };
    markdown_terminal::render(markdown, &narrower)
        .lines()
        .map(|line| {
            if line.is_empty() {
                "\n".to_owned()
            } else {
                format!("  {line}\n")
            }
        })
        .collect()
}

/// A comment's body as Markdown, after the text it quotes as a block quote.
fn body_markdown(node: &CommentNode) -> String {
    match &node.quoted_text {
        Some(quote) => {
            let quoted: Vec<String> = quote.lines().map(|line| format!("> {line}")).collect();
            format!("{}\n\n{}", quoted.join("\n"), node.body)
        }
        None => node.body.clone(),
    }
}

/// Threads for the terminal, laid out like the comments of `issue view`:
/// wrapped headers, then each body rendered as Markdown.
fn render_terminal(nodes: &[CommentNode], now: DateTime<Utc>, options: &RenderOptions) -> String {
    let threads = Threads::new(nodes);
    let action =
        |verb: &str, node: &CommentNode| format!("{verb} {}", ago(node.created_at.0, now, &Local));
    let id = |node: &CommentNode| {
        let mut parts = vec![id_part(node.id.inner(), &node.url, options.styled)];
        if node.resolved_at.is_some() {
            parts.push((RESOLVED.to_owned(), RESOLVED.width()));
        }
        parts
    };
    let mut out = String::new();
    for (root, replies) in threads.roots {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&terminal_header(
            author(root),
            &action("commented", root),
            id(root),
            "",
            options,
        ));
        out.push('\n');
        out.push_str(&markdown_terminal::render(&body_markdown(root), options));
        for reply in replies {
            out.push('\n');
            out.push_str(&terminal_header(
                author(reply),
                &action("replied", reply),
                id(reply),
                "  ",
                options,
            ));
            out.push('\n');
            out.push_str(&render_reply(&body_markdown(reply), options));
        }
    }
    for (reply, parent) in threads.orphans {
        if !out.is_empty() {
            out.push('\n');
        }
        let action = format!("{} to [{parent}]", action("replied", reply));
        out.push_str(&terminal_header(
            author(reply),
            &action,
            id(reply),
            "  ",
            options,
        ));
        out.push('\n');
        out.push_str(&render_reply(&body_markdown(reply), options));
    }
    out
}
