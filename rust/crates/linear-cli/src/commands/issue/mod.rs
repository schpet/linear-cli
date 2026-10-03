//! `linear issue`: issues and everything attached to them.
pub mod agent_session;
pub mod archive;
pub mod attach;
pub mod comment;
pub mod commits;
pub mod create;
pub mod create_prompt;
pub mod describe;
pub mod details;
pub mod id;
pub mod link;
mod mine;
pub mod pull_request;
mod query;
pub mod read;
pub mod relation;
pub mod start;
pub mod update;
pub mod view;
pub mod write;
pub mod write_network;

use crate::cli::issue::{
    IssueAgentSessionCommand, IssueCommand, IssueCommentCommand, IssueRelationCommand,
};
use crate::commands::team_key::configured_team_key;
use crate::config::Vcs;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::platform::vcs_script::{self, NativeProcessRunner};
use crate::refs::{IssueReference, prepare_issue_reference};

pub fn run(ctx: &Ctx, command: &IssueCommand) -> Result<()> {
    match command {
        IssueCommand::Id(_) => id::run(ctx),
        IssueCommand::Mine(args) => mine::run(ctx, args),
        IssueCommand::Query(args) => query::run(ctx, args),
        IssueCommand::Title(args) => {
            details::run(ctx, args.issue_id.as_deref(), details::Field::Title)
        }
        IssueCommand::Url(args) => details::run(ctx, args.issue_id.as_deref(), details::Field::Url),
        IssueCommand::Start(args) => start::run(ctx, args),
        IssueCommand::View(args) => view::run(ctx, args),
        IssueCommand::Describe(args) => describe::run(ctx, args),
        IssueCommand::Commits(args) => commits::run(ctx, args),
        IssueCommand::PullRequest(args) => pull_request::run(ctx, args),
        IssueCommand::Archive(args) => archive::archive(ctx, args),
        IssueCommand::Delete(args) => archive::delete(ctx, args),
        IssueCommand::Create(args) => create::run(ctx, args),
        IssueCommand::Update(args) => update::run(ctx, args),
        IssueCommand::Comment(group) => match &group.command {
            IssueCommentCommand::Add(args) => comment::add::run(ctx, args),
            IssueCommentCommand::Delete(args) => comment::delete::run(ctx, args),
            IssueCommentCommand::Update(args) => comment::update::run(ctx, args),
            IssueCommentCommand::List(args) => comment::list::run(ctx, args),
        },
        IssueCommand::Attach(args) => attach::run(ctx, args),
        IssueCommand::Link(args) => link::run(ctx, args),
        IssueCommand::Relation(group) => match &group.command {
            IssueRelationCommand::Add(args) => relation::add(ctx, args),
            IssueRelationCommand::Delete(args) => relation::delete(ctx, args),
            IssueRelationCommand::List(args) => relation::list(ctx, args),
        },
        IssueCommand::AgentSession(group) => match &group.command {
            IssueAgentSessionCommand::List(args) => agent_session::list(ctx, args),
            IssueAgentSessionCommand::View(args) => agent_session::view(ctx, args),
        },
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
    match prepare_issue_reference(Some(input), team.as_deref(), &ctx.scope()?)? {
        IssueReference::Identifier(identifier) => Ok(Some(identifier)),
        IssueReference::Unresolved => Ok(None),
        IssueReference::Inferred => infer(ctx),
    }
}

/// Like [`resolve`], failing when no issue is identified.
pub(crate) fn require(ctx: &Ctx, input: Option<&str>) -> Result<String> {
    resolve(ctx, input)?.ok_or_else(|| unresolved(ctx))
}

/// The issue the current git branch name or jj change trailers name.
pub(crate) fn infer(ctx: &Ctx) -> Result<Option<String>> {
    vcs_script::infer_issue(
        &mut NativeProcessRunner,
        vcs(ctx),
        ctx.cwd(),
        &ctx.config().child_env,
    )
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
