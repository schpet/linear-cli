//! `linear issue`: issues and everything attached to them.
mod agent_session;
mod archive;
mod archive_or_delete;
mod attach;
mod comment;
mod create;
mod create_prompt;
mod delete;
mod describe;
mod details;
mod filter;
pub(crate) mod id;
pub(crate) mod lifecycle;
mod link;
mod list_view;
mod mine;
mod pull_request;
mod query;
mod read;
mod reference;
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

use minijinja::context;
use minijinja::value::{Serde, Value, merge_maps};
use serde::Serialize;

use crate::cli::issue::{IssueCommand, IssueGhCommand, IssueGitCommand, IssueJjCommand};
use crate::cli::values::IssueRef;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::refs::IssueReference;

pub fn run(ctx: &Ctx, command: &IssueCommand) -> Result<()> {
    match command {
        IssueCommand::Id(args) => id::run(ctx, args),
        IssueCommand::Mine(args) => mine::run(ctx, args),
        IssueCommand::RemovedList(_) => Err(Error::invalid("`issue list` was removed in 4.0")
            .with_hint(
                "Run `linear issue mine` for your own issues; it takes the same options. Use `linear issue query` to filter across assignees, states and teams.",
            )),
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
        IssueCommand::Gh(args) => match &args.command {
            IssueGhCommand::PullRequest(args) => pull_request::run(ctx, args),
        },
        IssueCommand::MovedPullRequest(_) => Err(Error::invalid(
            "`issue pull-request` moved to `issue gh pull-request`",
        )
        .with_hint(
            "Run `linear issue gh pr <ISSUE>`; gh options such as --draft or --base now go after `--`. From a jj repository, push a bookmark first and pass `-- --head <BRANCH>`.",
        )),
        IssueCommand::RemovedCommits(_) => {
            Err(Error::invalid("`issue commits` was removed in 4.0")
                .with_hint(REMOVED_COMMITS_HINT))
        }
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

/// What `issue commits` users run instead: jj's own search of the
/// `Linear-issue` trailers.
const REMOVED_COMMITS_HINT: &str = "Search the trailers with jj instead: \
     jj log -p -r \"description(regex:'(?mi)^Linear-issue:.*\\bENG-123\\b')\"";

/// The issue identifier `input` names: an identifier, a URL, or a bare number
/// in the configured team, or the one issue `@git` or `@jj` reads. `None`
/// when a named `input` is no issue. Nothing is read from the repository
/// unless `input` is such a reference.
pub(crate) fn resolve(ctx: &Ctx, input: &IssueRef) -> Result<Option<String>> {
    reference::resolve(ctx, input)
}

/// Like [`resolve`], failing when `input` names no issue.
pub(crate) fn require(ctx: &Ctx, input: &IssueRef) -> Result<String> {
    reference::require(ctx, input)
}

/// A named issue as typed, for a command that resolves it itself, or the one
/// issue `@git` or `@jj` reads.
pub(crate) fn as_typed(ctx: &Ctx, input: &IssueRef) -> Result<String> {
    reference::as_typed(ctx, input)
}

/// Enrich an issue for templates without changing its public JSON shape.
fn template_data(issue: &impl Serialize, project_url: Option<&str>) -> Value {
    let issue = Value::from(Serde(issue));
    let identifier = issue
        .get_attr("identifier")
        .expect("issues have an identifier");
    let url = issue.get_attr("url").expect("issues have a URL");
    let reference = IssueReference::new(
        identifier.as_str().expect("issue identifiers are strings"),
        Some(url.as_str().expect("issue URLs are strings")),
    );
    let fields = context! { workspace_url_key => reference.workspace_url_key };
    if let Some(url) = project_url {
        let project = issue.get_attr("project").expect("the issue has a project");
        let project = merge_maps([project, context! { url }]);
        merge_maps([issue, fields, context! { project }])
    } else {
        merge_maps([issue, fields])
    }
}
