//! Issue arguments: an issue named outright, or a reference that reads it
//! from the repository. `@git` is the first issue ID in the current git
//! branch's name; `@jj` is every issue the `Linear-issue` trailers of the
//! changes in `trunk()..@` name. This is the only place an issue is read from
//! the repository, and only an argument that says so reads it.
use std::cmp::Ordering;

use serde::Deserialize;

use crate::auth::ApiKeyInput;
use crate::cli::values::IssueRef;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::platform::process;
use crate::platform::vcs::{self, RepositoryKind};
use crate::refs::{self, WorkspaceScope};

/// How every reference error's hint starts: the issue can always be named.
const NAME_IT: &str = "Name the issue instead, like ENG-123";
/// [`NAME_IT`] in the middle of a sentence.
const OR_NAME_IT: &str = "or name the issue instead, like ENG-123";

/// `trunk()`, and whether it is the root commit jj falls back to when the
/// repository has no trunk bookmark it recognises.
const TRUNK: &str = r#"json(root) ++ "\n""#;
/// Each change in the range: its short change ID and its trailers as
/// `[key, value]` pairs, read by jj's own trailer parser.
const TRAILERS: &str = r#""[" ++ json(change_id.short()) ++ ",[" ++ trailers.map(|t| "[" ++ json(t.key()) ++ "," ++ json(t.value()) ++ "]").join(",") ++ "]]\n""#;
const RANGE: &str = "trunk()..@";

/// The issues a reference read from the repository, and where from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
    /// At least one, distinct and sorted.
    identifiers: Vec<String>,
    /// Where they were read, as the notice ends: `the git branch name "x"`.
    source: String,
}

impl Read {
    /// What is said on stderr before anything is sent.
    fn notice(&self) -> String {
        let (noun, list) = match self.identifiers.as_slice() {
            [one] => ("issue", one.clone()),
            several => ("issues", several.join(", ")),
        };
        format!("Using {noun} {list} from {}\n", self.source)
    }
}

/// An argument with any reference read.
enum Expanded<'a> {
    Named(&'a str),
    Read(Read),
}

fn expand<'a>(ctx: &Ctx, input: &'a IssueRef) -> Result<Expanded<'a>> {
    Ok(match input {
        IssueRef::Named(input) => Expanded::Named(input),
        IssueRef::Git => Expanded::Read(from_branch(&current_branch(ctx)?)?),
        IssueRef::Jj => Expanded::Read(read_jj(ctx)?),
    })
}

/// The one issue `input` names, or `None` when a named `input` is no issue
/// reference at all. A reference that names several issues is refused.
pub(crate) fn resolve(ctx: &Ctx, input: &IssueRef) -> Result<Option<String>> {
    match expand(ctx, input)? {
        Expanded::Named(named_input) => named(ctx, named_input),
        Expanded::Read(read) => {
            let identifier = one(input, &read)?;
            ctx.eprint(read.notice())?;
            Ok(Some(identifier))
        }
    }
}

/// Like [`resolve`], failing when a named `input` is no issue reference.
pub(crate) fn require(ctx: &Ctx, input: &IssueRef) -> Result<String> {
    resolve(ctx, input)?.ok_or_else(|| not_an_issue(input))
}

/// Every issue `input` names: one when it is named, all a reference reads.
pub(crate) fn require_all(ctx: &Ctx, input: &IssueRef) -> Result<Vec<String>> {
    match expand(ctx, input)? {
        Expanded::Named(named_input) => named(ctx, named_input)?
            .map(|identifier| vec![identifier])
            .ok_or_else(|| not_an_issue(input)),
        Expanded::Read(read) => {
            ctx.eprint(read.notice())?;
            Ok(read.identifiers)
        }
    }
}

/// [`require_all`] without the notice, for `issue id`, whose output is the
/// identifiers themselves.
pub(crate) fn identifiers(ctx: &Ctx, input: &IssueRef) -> Result<Vec<String>> {
    match expand(ctx, input)? {
        Expanded::Named(named_input) => named(ctx, named_input)?
            .map(|identifier| vec![identifier])
            .ok_or_else(|| not_an_issue(input)),
        Expanded::Read(read) => Ok(read.identifiers),
    }
}

/// `--parent` as the write commands take it: see [`as_typed`].
pub(crate) fn parent(ctx: &Ctx, input: Option<&IssueRef>) -> Result<Option<String>> {
    input.map(|input| as_typed(ctx, input)).transpose()
}

/// An issue for a command that resolves a named issue itself (`--parent`
/// with the workspace it writes to, `document --issue`, which also takes a
/// UUID): a named issue as typed, a reference as the one issue it reads,
/// with the notice.
pub(crate) fn as_typed(ctx: &Ctx, input: &IssueRef) -> Result<String> {
    match input {
        IssueRef::Named(input) => Ok(input.clone()),
        reference @ (IssueRef::Git | IssueRef::Jj) => require(ctx, reference),
    }
}

/// An identifier, a URL, or a bare number in the configured team. Only a URL
/// is checked against the workspace, so nothing else needs credentials.
fn named(ctx: &Ctx, input: &str) -> Result<Option<String>> {
    let team = configured_team_key(ctx.options());
    if refs::is_linear_url(input) {
        return refs::prepare_issue_reference(input, team.as_deref(), &ctx.scope()?);
    }
    // Not consulted: only a URL names a workspace.
    let unchecked = WorkspaceScope::new(None, ApiKeyInput::Absent);
    refs::prepare_issue_reference(input, team.as_deref(), &unchecked)
}

fn not_an_issue(input: &IssueRef) -> Error {
    Error::invalid(format!("Not an issue ID: {input}")).with_hint(
        "Pass an issue ID like ENG-123, an issue URL, an issue number in the configured team, @git, or @jj.",
    )
}

/// The only issue `read` names; several are refused, and listed.
fn one(reference: &IssueRef, read: &Read) -> Result<String> {
    match read.identifiers.as_slice() {
        [identifier] => Ok(identifier.clone()),
        several => Err(Error::invalid(format!(
            "{reference} names several issues: {}",
            several.join(", ")
        ))
        .with_hint(format!(
            "This command acts on one issue. Name the one you mean, like {}.",
            several.first().map_or("ENG-123", String::as_str)
        ))),
    }
}

/// The current git branch: `git symbolic-ref --quiet --short HEAD`.
fn current_branch(ctx: &Ctx) -> Result<String> {
    let mut command = process::command("git", ctx.cwd(), &ctx.config().child_env);
    command.args(["symbolic-ref", "--quiet", "--short", "HEAD"]);
    let outside = |error: Error| {
        if vcs::in_repository(RepositoryKind::Git, ctx.cwd()) {
            error.context("Failed to read the current git branch for @git")
        } else {
            vcs::not_in_repository(RepositoryKind::Git).with_hint(format!(
                "@git reads the current branch. Run it from your repository, {OR_NAME_IT}."
            ))
        }
    };
    let output = process::output(&mut command).map_err(outside)?;
    match output.status.code() {
        Some(0) => {
            let branch = process::text(&output.stdout);
            if branch.is_empty() {
                return Err(Error::new("git named no current branch for @git"));
            }
            Ok(branch)
        }
        // A detached HEAD has no branch.
        Some(1) => Err(
            Error::invalid("Cannot read @git: git HEAD is detached").with_hint(format!(
                "{NAME_IT}, or switch to a branch whose name contains its ID."
            )),
        ),
        _ => Err(outside(process::failed(
            &command,
            output.status,
            &output.stderr,
        ))),
    }
}

/// The issue a git branch name names: its first issue ID, as Linear's own
/// branch names put it first.
fn from_branch(branch: &str) -> Result<Read> {
    let identifier = scan(branch).into_iter().next().ok_or_else(|| {
        Error::invalid(format!("No issue ID in the git branch name {branch:?}")).with_hint(format!(
            "{NAME_IT}, or use @git on a branch whose name contains the issue ID."
        ))
    })?;
    Ok(Read {
        identifiers: vec![identifier],
        source: format!("the git branch name {branch:?}"),
    })
}

/// A jj command that reads the repository as it was last snapshotted: the
/// descriptions it reads do not depend on the files on disk.
fn jj(ctx: &Ctx) -> std::process::Command {
    let mut command = process::command("jj", ctx.cwd(), &ctx.config().child_env);
    command.args(["--ignore-working-copy", "--no-pager", "--color", "never"]);
    command
}

fn read_jj(ctx: &Ctx) -> Result<Read> {
    let run = |revset: &str, template: &str| {
        let mut command = jj(ctx);
        command.args(["log", "-r", revset, "--no-graph", "-T", template]);
        process::checked_output(&mut command).map_err(|error| {
            if vcs::in_repository(RepositoryKind::Jj, ctx.cwd()) {
                error.context("Failed to read @jj")
            } else {
                vcs::not_in_repository(RepositoryKind::Jj).with_hint(format!(
                    "@jj reads the changes in {RANGE}. Run it from your jj repository, {OR_NAME_IT}."
                ))
            }
        })
    };
    // Without a trunk, trunk()..@ is the whole history: never searched.
    if trunk_is_root(&run("trunk()", TRUNK)?.stdout)? {
        return Err(Error::invalid(format!(
            "Cannot read @jj: trunk() is the root commit, so {RANGE} would be the whole history"
        ))
        .with_hint(format!(
            "{NAME_IT}, or tell jj which bookmark is trunk, like `jj config set --repo 'revset-aliases.\"trunk()\"' main@origin`."
        )));
    }
    from_trailers(&run(RANGE, TRAILERS)?.stdout)
}

fn malformed(error: impl std::fmt::Display) -> Error {
    Error::new(format!("Unexpected output from `jj log` for @jj: {error}"))
        .with_hint("This needs jj 0.38 or newer; check `jj --version`.")
}

fn lines(stdout: &[u8]) -> Result<impl Iterator<Item = &str>> {
    let text = std::str::from_utf8(stdout).map_err(malformed)?;
    Ok(text.lines().filter(|line| !line.trim().is_empty()))
}

/// Reads the [`TRUNK`] template's output: one boolean.
fn trunk_is_root(stdout: &[u8]) -> Result<bool> {
    let mut found =
        lines(stdout)?.map(|line| serde_json::from_str::<bool>(line).map_err(malformed));
    match (found.next(), found.next()) {
        (Some(root), None) => root,
        _ => Err(malformed("expected trunk() to be one revision")),
    }
}

#[derive(Deserialize)]
struct Change(String, Vec<(String, String)>);

/// Reads the [`TRAILERS`] template's output: every issue the `Linear-issue`
/// trailers name, each of which must name one at least. The key is matched
/// without regard to case, as git does.
fn from_trailers(stdout: &[u8]) -> Result<Read> {
    let mut identifiers: Vec<String> = Vec::new();
    for line in lines(stdout)? {
        let Change(change, trailers) = serde_json::from_str(line).map_err(malformed)?;
        for (_, value) in trailers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case("Linear-issue"))
        {
            let named = scan(value);
            if named.is_empty() {
                return Err(Error::invalid(format!(
                    "Cannot read @jj: the Linear-issue trailer {value:?} of change {change} names no issue"
                ))
                .with_hint(format!("{NAME_IT}, or fix the trailer to name its ID.")));
            }
            identifiers.extend(named);
        }
    }
    identifiers.sort_by(|a, b| by_team_and_number(a, b));
    identifiers.dedup();
    if identifiers.is_empty() {
        return Err(Error::invalid(format!(
            "Cannot read @jj: no Linear-issue trailer in {RANGE} names an issue"
        ))
        .with_hint(format!(
            "{NAME_IT}, or add a `Linear-issue: Fixes ENG-123` trailer to a change's description."
        )));
    }
    Ok(Read {
        identifiers,
        source: format!("Linear-issue trailers in {RANGE}"),
    })
}

/// Orders `TEAM-123` identifiers by team, then by number. Numbers never
/// start with 0, so the shorter one is the smaller.
fn by_team_and_number(a: &str, b: &str) -> Ordering {
    let split = |id: &str| {
        let (team, number) = id.split_once('-').unwrap_or((id, ""));
        (team.to_owned(), number.len(), number.to_owned())
    };
    split(a).cmp(&split(b))
}

/// Every `TEAM-123` in `text` that starts and ends on a word boundary, in
/// order, where word bytes are ASCII letters, digits and `_`. The team is
/// uppercased.
fn scan(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut found = Vec::new();
    let mut start = 0;
    while let Some(&byte) = bytes.get(start) {
        let follows_word = start
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .is_some_and(|previous| word(*previous));
        let mut dash = start;
        while bytes.get(dash).is_some_and(u8::is_ascii_alphanumeric) {
            dash += 1;
        }
        let mut end = dash + 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        let numbered = bytes.get(dash) == Some(&b'-')
            && bytes
                .get(dash + 1)
                .is_some_and(|b| (b'1'..=b'9').contains(b));
        let ends_word = !bytes.get(end).is_some_and(|b| word(*b));
        // Both ends are ASCII bytes, hence UTF-8 boundaries.
        let parts = (text.get(start..dash), text.get(dash + 1..end));
        match parts {
            (Some(team), Some(number))
                if byte.is_ascii_alphanumeric() && !follows_word && numbered && ends_word =>
            {
                found.push(format!("{}-{number}", team.to_ascii_uppercase()));
                start = end;
            }
            _ => start += 1,
        }
    }
    found
}

#[cfg(test)]
mod tests;
