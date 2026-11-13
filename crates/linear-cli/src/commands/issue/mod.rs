//! `linear issue`: issues and everything attached to them.
mod agent_session;
mod archive;
mod archive_or_delete;
mod attach;
mod comment;
mod commits;
mod create;
mod create_prompt;
mod delete;
mod describe;
mod details;
mod filter;
pub(crate) mod id;
mod link;
mod list;
mod list_view;
mod pull_request;
mod query;
mod read;
mod relation;
mod start;
mod title;
mod transition;
mod update;
mod url;
mod vcs_start;
mod view;
mod write;
mod write_network;

use crate::cli::issue::{IssueCommand, IssueGhCommand, IssueGitCommand, IssueJjCommand};
use crate::commands::team_key::configured_team_key;
use crate::config::Vcs;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::refs::prepare_issue_reference;

pub fn run(ctx: &Ctx, command: &IssueCommand) -> Result<()> {
    match command {
        IssueCommand::RemovedId(_) => Err(Error::invalid(
            "`issue id` was removed in 4.0: linear no longer reads the issue from the git branch or jj change",
        )
        .with_hint("Pass the issue ID to each command, like `linear issue view ENG-123`.")),
        IssueCommand::List(args) => list::run(ctx, args),
        IssueCommand::Query(args) => query::run(ctx, args),
        IssueCommand::Title(args) => title::run(ctx, args),
        IssueCommand::Url(args) => url::run(ctx, args),
        IssueCommand::Start(args) => start::run(ctx, args),
        IssueCommand::Complete(args) => transition::run(ctx, transition::Verb::Complete, args),
        IssueCommand::Cancel(args) => transition::run(ctx, transition::Verb::Cancel, args),
        IssueCommand::Git(args) => match &args.command {
            IssueGitCommand::Start(args) => vcs_start::run_git(ctx, args),
        },
        IssueCommand::Jj(args) => match &args.command {
            IssueJjCommand::Start(args) => vcs_start::run_jj(ctx, args),
        },
        IssueCommand::View(args) => view::run(ctx, args),
        IssueCommand::Describe(args) => describe::run(ctx, args),
        IssueCommand::Commits(args) => commits::run(ctx, args),
        IssueCommand::Gh(args) => match &args.command {
            IssueGhCommand::PullRequest(args) => pull_request::run(ctx, args),
        },
        IssueCommand::MovedPullRequest(_) => Err(Error::invalid(
            "`issue pull-request` moved to `issue gh pull-request`",
        )
        .with_hint(
            "Run `linear issue gh pull-request` (or `linear issue gh pr`) with the same arguments.",
        )),
        IssueCommand::Archive(args) => archive::run(ctx, args),
        IssueCommand::Delete(args) => delete::run(ctx, args),
        IssueCommand::Create(args) => create::run(ctx, args),
        IssueCommand::Update(args) => update::run(ctx, args),
        IssueCommand::Comment(args) => comment::run(ctx, &args.command),
        IssueCommand::Attach(args) => attach::run(ctx, args),
        IssueCommand::Link(args) => link::run(ctx, args),
        IssueCommand::Relation(args) => relation::run(ctx, &args.command),
        IssueCommand::AgentSession(args) => agent_session::run(ctx, &args.command),
    }
}

/// The issue identifier `input` names: an identifier, a URL, or a bare number
/// in the configured team. `None` when it names no issue. The issue is always
/// named by the caller; nothing is read from the repository.
pub(crate) fn resolve(ctx: &Ctx, input: &str) -> Result<Option<String>> {
    let team = configured_team_key(ctx.options());
    prepare_issue_reference(input, team.as_deref(), &ctx.scope()?)
}

/// Like [`resolve`], failing when `input` names no issue.
pub(crate) fn require(ctx: &Ctx, input: &str) -> Result<String> {
    resolve(ctx, input)?.ok_or_else(|| {
        Error::invalid(format!("Not an issue ID: {input}")).with_hint(
            "Pass an issue ID like ENG-123, an issue URL, or an issue number in the configured team.",
        )
    })
}

pub(crate) fn vcs(ctx: &Ctx) -> Vcs {
    ctx.options().vcs()
}
