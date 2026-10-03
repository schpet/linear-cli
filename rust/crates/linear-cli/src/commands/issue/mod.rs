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
mod id;
mod link;
mod list;
mod list_view;
mod pull_request;
mod query;
mod read;
mod relation;
mod start;
mod title;
mod update;
mod url;
mod view;
mod write;
mod write_network;

use crate::cli::issue::IssueCommand;
use crate::commands::team_key::configured_team_key;
use crate::config::Vcs;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::refs::prepare_issue_reference;

pub fn run(ctx: &Ctx, command: &IssueCommand) -> Result<()> {
    match command {
        IssueCommand::Id(_) => id::run(ctx),
        IssueCommand::List(args) => list::run(ctx, args),
        IssueCommand::Query(args) => query::run(ctx, args),
        IssueCommand::Title(args) => title::run(ctx, args),
        IssueCommand::Url(args) => url::run(ctx, args),
        IssueCommand::Start(args) => start::run(ctx, args),
        IssueCommand::View(args) => view::run(ctx, args),
        IssueCommand::Describe(args) => describe::run(ctx, args),
        IssueCommand::Commits(args) => commits::run(ctx, args),
        IssueCommand::PullRequest(args) => pull_request::run(ctx, args),
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

/// The issue identifier `input` names (an identifier, a URL, or a bare number
/// in the configured team), or, when `input` is omitted, the one the current
/// git branch or jj change names. `None` when nothing identifies an issue.
pub(crate) fn resolve(ctx: &Ctx, input: Option<&str>) -> Result<Option<String>> {
    let Some(input) = input else {
        return infer(ctx);
    };
    let team = configured_team_key(ctx.options());
    prepare_issue_reference(input, team.as_deref(), &ctx.scope()?)
}

/// Like [`resolve`], failing when no issue is identified.
pub(crate) fn require(ctx: &Ctx, input: Option<&str>) -> Result<String> {
    resolve(ctx, input)?.ok_or_else(|| unresolved(ctx))
}

/// The issue the current git branch name or jj change trailers name.
pub(crate) fn infer(ctx: &Ctx) -> Result<Option<String>> {
    crate::platform::vcs::infer_issue(vcs(ctx), ctx.cwd(), &ctx.config().child_env)
}

pub(crate) fn vcs(ctx: &Ctx) -> Vcs {
    ctx.options().vcs()
}

pub(crate) fn unresolved(ctx: &Ctx) -> Error {
    let hint = match vcs(ctx) {
        Vcs::Git => {
            "Pass an issue ID like ENG-123, or run from a git branch whose name contains one."
        }
        Vcs::Jj => {
            "Pass an issue ID like ENG-123, or run from a jj change with a Linear-issue trailer."
        }
    };
    Error::new("Could not determine issue ID").with_hint(hint)
}
