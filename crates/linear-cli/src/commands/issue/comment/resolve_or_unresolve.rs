//! What `issue comment resolve` and `issue comment unresolve` share: one
//! thread, or several with a preview and a summary. A thread is named by its
//! top-level comment, and nothing is asked before changing it.
use std::collections::HashSet;

use crate::cli::BulkArgs;
use crate::client::LinearClient;
use crate::commands::bulk::{self, BulkInput, BulkResult, Found, Skipped, Verb};
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::comment::{
    CommentForResolution, GetCommentForResolution, GetCommentVariables, ResolveComment,
    ResolveCommentVariables, UnresolveComment,
};
use crate::platform::terminal_text::single_line;
use crate::refs::{is_linear_uuid, reject_comment_url, reject_linear_url};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Resolve,
    Unresolve,
}

impl Mode {
    const fn verb(self) -> Verb {
        match self {
            Self::Resolve => Verb {
                present: "resolve",
                past: "resolved",
            },
            Self::Unresolve => Verb {
                present: "reopen",
                past: "reopened",
            },
        }
    }
    const fn command(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Unresolve => "unresolve",
        }
    }
    const fn done(self) -> &'static str {
        match self {
            Self::Resolve => "Resolved",
            Self::Unresolve => "Reopened",
        }
    }
    const fn state(self) -> &'static str {
        match self {
            Self::Resolve => "resolved",
            Self::Unresolve => "open",
        }
    }
}

pub struct Request<'a> {
    pub comment_ids: &'a [String],
    /// `--with`: the reply that resolved the thread.
    pub with: Option<&'a str>,
    pub bulk: &'a BulkArgs,
}

pub fn run(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    let (ids, with) = collect(mode, request)?;
    match ids.as_slice() {
        [id] => single(ctx, mode, id, with.as_deref())
            .context(format!("Failed to {} comment thread", mode.verb().present)),
        _ => several(ctx, mode, ids),
    }
}

/// Every listed thread once, and `--with`, checked and lowercased before
/// anything is sent.
fn collect(mode: Mode, request: &Request<'_>) -> Result<(Vec<String>, Option<String>)> {
    let mut argv = request.comment_ids.to_vec();
    argv.extend(request.bulk.bulk.iter().flatten().cloned());
    let input = BulkInput {
        argv: Some(&argv),
        file: request.bulk.bulk_file.as_deref(),
        stdin: request.bulk.bulk_stdin,
    };
    let listed = bulk::collect_ids(&input, &mut std::io::stdin().lock())?;
    if listed.is_empty() {
        return Err(Error::invalid("No comment IDs given").with_hint(format!(
            "Pass each thread's top-level comment ID, as in `linear issue comment {} <COMMENT>...`, or use --bulk-file or --bulk-stdin.",
            mode.command()
        )));
    }
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for id in &listed {
        let id = comment_id(id)?;
        if seen.insert(id.clone()) {
            ids.push(id);
        }
    }
    let with = request.with.map(comment_id).transpose()?;
    if with.is_some() && ids.len() > 1 {
        return Err(Error::invalid("--with takes exactly one thread")
            .with_hint("A reply belongs to one thread; resolve the others separately."));
    }
    Ok((ids, with))
}

/// A comment's UUID as Linear spells it, in lowercase. Comment links carry
/// only part of the ID, so they and anything else that is not a UUID are
/// refused.
fn comment_id(input: &str) -> Result<String> {
    reject_comment_url(input)?;
    reject_linear_url(input, "a comment UUID")?;
    if !is_linear_uuid(input) {
        return Err(
            Error::invalid(format!("Not a comment UUID: {input}")).with_hint(
                "Pass the comment's full UUID, from `linear issue comment list <issue> --json`.",
            ),
        );
    }
    Ok(input.to_ascii_lowercase())
}

/// A top-level comment on an issue, and its thread's state.
struct Thread {
    id: String,
    url: String,
    issue: String,
    resolved: bool,
    resolving_comment_id: Option<String>,
}

impl Thread {
    /// How the thread is shown, like `<id> on ENG-1`.
    fn name(&self) -> String {
        format!("{} on {}", self.id, single_line(&self.issue))
    }

    /// Whether the thread is already as `mode` leaves it, with `with` as its
    /// resolving reply when given.
    fn unchanged(&self, mode: Mode, with: Option<&str>) -> bool {
        match mode {
            Mode::Resolve => {
                self.resolved
                    && with.is_none_or(|with| self.resolving_comment_id.as_deref() == Some(with))
            }
            Mode::Unresolve => !self.resolved,
        }
    }
}

async fn fetch(client: &LinearClient, id: &str) -> Result<CommentForResolution> {
    let data: GetCommentForResolution = client
        .query(GetCommentVariables { id: id.to_owned() })
        .await
        .map_err(|failure| failure.or_not_found("Comment", id))?;
    data.comment.ok_or_else(|| Error::not_found("Comment", id))
}

/// The thread comment `id` starts; a reply is refused with its thread's ID in
/// the message, since a bulk summary shows only the message.
fn thread(mode: Mode, id: &str, comment: CommentForResolution) -> Result<Thread> {
    if let Some(root) = comment.parent_id {
        let root = root.to_ascii_lowercase();
        let fix = match mode {
            Mode::Resolve => format!(
                "resolve its thread with `linear issue comment resolve {root}`, adding `--with {id}` to record this reply as the one that resolved it"
            ),
            Mode::Unresolve => {
                format!("reopen its thread with `linear issue comment unresolve {root}`")
            }
        };
        return Err(Error::new(format!("Comment {id} is a reply; {fix}")));
    }
    let Some(issue) = comment.issue else {
        return Err(Error::new(format!("Comment {id} is not on an issue"))
            .with_hint("`linear issue comment` changes threads on issues only."));
    };
    Ok(Thread {
        id: id.to_owned(),
        url: comment.url,
        issue: issue.identifier,
        resolved: comment.resolved_at.is_some(),
        resolving_comment_id: comment
            .resolving_comment_id
            .map(|id| id.to_ascii_lowercase()),
    })
}

/// Fails unless `reply` (named `id`) is a reply in `thread`.
fn check_reply(id: &str, reply: &CommentForResolution, thread: &Thread) -> Result<()> {
    let hint = "--with takes a reply in the thread being resolved.";
    match reply.parent_id.as_deref().map(str::to_ascii_lowercase) {
        Some(parent) if parent == thread.id => Ok(()),
        Some(parent) => Err(Error::new(format!(
            "Comment {id} is a reply in thread {parent}, not {}",
            thread.id
        ))
        .with_hint(hint)),
        None => Err(Error::new(format!("Comment {id} is not a reply")).with_hint(hint)),
    }
}

/// Resolves or reopens thread `id` once, and checks that Linear did.
async fn change(client: &LinearClient, mode: Mode, id: &str, with: Option<&str>) -> Result<()> {
    let sent = match mode {
        Mode::Resolve => client
            .mutate(ResolveCommentVariables {
                id: id.to_owned(),
                resolving_comment_id: with.map(str::to_owned),
            })
            .await
            .map(|data: ResolveComment| data.comment_resolve),
        Mode::Unresolve => client
            .mutate(GetCommentVariables { id: id.to_owned() })
            .await
            .map(|data: UnresolveComment| data.comment_unresolve),
    };
    let payload = sent.map_err(|failure| {
        let uncertain = failure.outcome_unknown();
        let mut error = failure.or_not_found("Comment", id);
        if uncertain {
            error.push_message(&format!("; the thread may already be {}", mode.verb().past));
        }
        error
    })?;
    let comment = payload.comment;
    let same = |value: Option<&str>, expected: &str| {
        value.is_some_and(|value| value.eq_ignore_ascii_case(expected))
    };
    let changed = payload.success
        && same(Some(comment.id.inner()), id)
        && match mode {
            Mode::Resolve => {
                comment.resolved_at.is_some()
                    && with.is_none_or(|with| same(comment.resolving_comment_id.as_deref(), with))
            }
            Mode::Unresolve => comment.resolved_at.is_none(),
        };
    if changed {
        Ok(())
    } else {
        Err(Error::new(format!(
            "Linear did not {} the comment thread",
            mode.verb().present
        )))
    }
}

fn single(ctx: &Ctx, mode: Mode, id: &str, with: Option<&str>) -> Result<()> {
    let client = ctx.client()?;
    let (comment, reply) = ctx.spin(true, async {
        tokio::try_join!(fetch(client, id), async {
            match with {
                Some(with) => fetch(client, with).await.map(Some),
                None => Ok(None),
            }
        })
    })?;
    let thread = thread(mode, id, comment)?;
    if let (Some(with), Some(reply)) = (with, &reply) {
        check_reply(with, reply, &thread)?;
    }
    if thread.unchanged(mode, with) {
        return ctx.print(format!(
            "Comment thread {} is already {}.\n",
            thread.name(),
            mode.state()
        ));
    }
    ctx.spin(true, change(client, mode, id, with))?;
    ctx.print(outcome::done(
        mode.done(),
        "comment thread",
        &thread.name(),
        Some(&thread.url),
    ))
}

fn several(ctx: &Ctx, mode: Mode, ids: Vec<String>) -> Result<()> {
    let client = ctx.client()?;
    let verb = mode.verb();
    let (found, missing) = bulk::look_up(ctx, ids, |id| look_up_item(client, mode, id));
    ctx.eprint(bulk::preview(&found, &missing, "comment thread", verb))?;
    if found.is_empty() {
        return Err(bulk::none_found(&missing, "comment threads"));
    }
    let mut results = bulk::run(ctx, found, |found| apply_item(client, mode, found))?;
    results.extend(missing.into_iter().map(BulkResult::from));
    bulk::report(ctx, &results, "comment thread", verb)
}

async fn look_up_item(
    client: &LinearClient,
    mode: Mode,
    id: String,
) -> std::result::Result<Found<Thread>, Skipped> {
    let checked = match fetch(client, &id).await {
        Ok(comment) => thread(mode, &id, comment),
        Err(error) => Err(error),
    };
    match checked {
        Ok(thread) => {
            let mut name = thread.name();
            if thread.unchanged(mode, None) {
                name.push_str(&format!(" (already {})", mode.state()));
            }
            Ok(Found {
                original: id,
                name,
                item: thread,
            })
        }
        Err(error) => Err(Skipped::failed(id, &error)),
    }
}

/// Resolves or reopens one looked-up thread; one already in that state is
/// left as it is and succeeds.
async fn apply_item(client: &LinearClient, mode: Mode, found: Found<Thread>) -> BulkResult {
    if found.item.unchanged(mode, None) {
        return found.result(Ok(()));
    }
    let outcome = change(client, mode, &found.item.id, None).await;
    found.result(outcome)
}
