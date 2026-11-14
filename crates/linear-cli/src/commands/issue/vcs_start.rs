//! `issue git start` and `issue jj start`: prepare a git branch or a jj
//! change for an issue, then start the issue as `issue start` does. Each
//! thing done is said on stdout as soon as it is done, so that whoever runs
//! it (often an agent) knows what changed in the repository and in Linear,
//! even when a later step fails.
use std::process::Command;

use serde::Deserialize;

use crate::{
    cli::issue::{IssueGitStart, IssueJjStart},
    commands::{
        issue::transition::{self, Outcome, Settled, Verb},
        json::quoted,
    },
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue::IssueTransitionDetails,
    platform::{
        jj_pr::{malformed, parse_lines},
        process,
        prompt::Choice,
        terminal_text::single_line,
        vcs::{self, RepositoryKind},
    },
};

pub fn run_git(ctx: &Ctx, args: &IssueGitStart) -> Result<()> {
    let prepare = Prepare::Git {
        branch: args.branch.as_deref(),
        from_ref: args.from_ref.as_deref(),
    };
    super::start::target(ctx, &args.target)
        .and_then(|identifier| work_on(ctx, identifier, prepare))
        .context("Failed to start issue with git")
}

pub fn run_jj(ctx: &Ctx, args: &IssueJjStart) -> Result<()> {
    super::start::target(ctx, &args.target)
        .and_then(|identifier| work_on(ctx, identifier, Prepare::Jj))
        .context("Failed to start issue with jj")
}

/// What to prepare in the working copy. The command names the tool.
#[derive(Clone, Copy)]
enum Prepare<'a> {
    Git {
        branch: Option<&'a str>,
        from_ref: Option<&'a str>,
    },
    Jj,
}

impl Prepare<'_> {
    fn kind(self) -> RepositoryKind {
        match self {
            Self::Git { .. } => RepositoryKind::Git,
            Self::Jj => RepositoryKind::Jj,
        }
    }
}

/// Switches the working copy to the issue (a git branch or a jj change), then
/// starts the issue. The working copy is never rolled back when the status
/// change fails.
fn work_on(ctx: &Ctx, identifier: String, prepare: Prepare<'_>) -> Result<()> {
    let client = ctx.client()?;
    let issue = ctx.spin(
        true,
        super::details::fetch_for_transition(client, identifier),
    )?;
    let repo = Repo {
        ctx,
        kind: prepare.kind(),
    };
    ctx.flush()?;
    match prepare {
        Prepare::Git { branch, from_ref } => repo.git_branch(&issue, branch, from_ref)?,
        Prepare::Jj => {
            let prepared = repo.jj_prepare()?;
            repo.jj_describe(&prepared, &issue)?;
        }
    }
    let outcome = ctx
        .spin(true, transition::apply(client, Verb::Start, &issue))
        .map_err(|error| prepared(error, prepare.kind(), &issue.identifier))?;
    ctx.print(status_line(&issue, &outcome))
}

/// Text from git, jj or Linear as a result line shows it: JSON-quoted, with
/// nothing a terminal would act on.
fn shown(text: &str) -> String {
    single_line(&quoted(text)).into_owned()
}

/// What happened to the issue's status, after the repository lines.
fn status_line(issue: &IssueTransitionDetails, outcome: &Outcome) -> String {
    let identifier = single_line(&issue.identifier);
    match outcome {
        Outcome::Moved { to } => format!(
            "✓ Moved Linear issue {identifier} from {} to {}\n",
            shown(&issue.state.name),
            shown(to)
        ),
        Outcome::Unchanged {
            why: Settled::Already,
            state,
        } => format!(
            "✓ Left Linear issue {identifier} in {} (already started)\n",
            shown(state)
        ),
        Outcome::Unchanged {
            why: Settled::Closed,
            state,
        } => format!(
            "✓ Left Linear issue {identifier} in {} (closed; not reopened)\n",
            shown(state)
        ),
    }
}

/// `error` from starting the issue once the working copy is ready, saying so
/// after whatever the error already suggests.
fn prepared(error: Error, kind: RepositoryKind, identifier: &str) -> Error {
    let ready = match kind {
        RepositoryKind::Git => "The branch is ready",
        RepositoryKind::Jj => "The jj change is ready",
    };
    let recovery =
        format!("{ready}; set the status with `linear issue update {identifier} --state <state>`.");
    with_recovery(error, &recovery).context("Could not move the issue to a started status")
}

/// `error`, its hint followed by `recovery`.
fn with_recovery(error: Error, recovery: &str) -> Error {
    let hint = match error.hint() {
        Some(hint) => format!("{hint} {recovery}"),
        None => recovery.to_owned(),
    };
    error.with_hint(hint)
}

fn choose_existing(ctx: &Ctx, branch: &str) -> Result<ExistingBranch> {
    ctx.require_tty(
        "what to do with the existing branch",
        "--branch with a new name",
    )?;
    ctx.eprint(format!("Branch {branch} already exists.\n"))?;
    ctx.prompter()?.select(
        "Existing branch:",
        vec![
            Choice::new("Switch to it", ExistingBranch::Switch),
            Choice::new("Create a new branch with a suffix", ExistingBranch::Suffix),
        ],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExistingBranch {
    Switch,
    Suffix,
}

/// `branch-1`, `branch-2`, …: the first name that does not `exist` yet.
fn free_suffix(branch: &str, mut exists: impl FnMut(&str) -> Result<bool>) -> Result<String> {
    for suffix in 1_u64.. {
        let candidate = format!("{branch}-{suffix}");
        if !exists(&candidate)? {
            return Ok(candidate);
        }
    }
    unreachable!("some branch suffix is free")
}

/// What was checked out before `issue git start` changed it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Previous {
    Branch(String),
    /// A detached HEAD, at this abbreviated commit.
    Detached(String),
}

/// What `issue git start` did to the git checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
enum GitDone {
    Created {
        branch: String,
        /// The ref the branch was created from, as given or `HEAD`.
        base: String,
        /// That ref's commit, abbreviated.
        commit: String,
        previous: Previous,
    },
    Switched {
        branch: String,
        previous: Previous,
        /// A --from-ref given that switching did not use.
        unused_base: Option<String>,
    },
}

impl GitDone {
    /// The lines that report it on stdout.
    fn report(&self) -> String {
        match self {
            Self::Created {
                branch,
                base,
                commit,
                previous,
            } => {
                let previous = match previous {
                    Previous::Branch(name) => format!("previous branch: {}", shown(name)),
                    Previous::Detached(commit) => {
                        format!("previous HEAD: detached at {}", single_line(commit))
                    }
                };
                format!(
                    "✓ Created and switched to git branch {} from {} ({}); {previous}\n",
                    shown(branch),
                    shown(base),
                    single_line(commit)
                )
            }
            Self::Switched {
                branch,
                previous,
                unused_base,
            } => {
                let mut lines = match previous {
                    Previous::Branch(name) if name == branch => {
                        format!("✓ Kept git branch {} checked out\n", shown(branch))
                    }
                    Previous::Branch(name) => format!(
                        "✓ Switched from git branch {} to {}\n",
                        shown(name),
                        shown(branch)
                    ),
                    Previous::Detached(commit) => format!(
                        "✓ Switched from detached git HEAD at {} to branch {}\n",
                        single_line(commit),
                        shown(branch)
                    ),
                };
                if let Some(base) = unused_base {
                    lines.push_str(&format!(
                        "✓ Left existing branch history unchanged; --from-ref {} was not used\n",
                        shown(base)
                    ));
                }
                lines
            }
        }
    }
}

/// The jj change `issue jj start` describes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum JjPrepared {
    /// `jj new` made it on the change that was checked out.
    Created { change: String },
    /// The checked-out change, which was empty and undescribed.
    Reused { change: String },
}

impl JjPrepared {
    fn change(&self) -> &str {
        match self {
            Self::Created { change } | Self::Reused { change } => change,
        }
    }

    /// Why a failed `jj describe` leaves this change as it is.
    fn describe_failed(&self) -> String {
        let (change, done) = match self {
            Self::Created { change } => (change, "created"),
            Self::Reused { change } => (change, "reused"),
        };
        format!(
            "The jj change {change} was {done}, but setting its description failed. Inspect it with jj log -r {change} before retrying."
        )
    }
}

/// The checked-out jj change, as `issue jj start` needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    /// The full change ID, which describing it does not change.
    id: String,
    /// Empty and undescribed: the scratch change of the squash workflow.
    scratch: bool,
}

#[derive(Deserialize)]
struct ChangeFields {
    change_id: String,
    description: String,
}

/// The working-copy change: its fields, and whether it is empty.
const CHANGE: &str = r#""[" ++ json(self) ++ "," ++ json(empty) ++ "]\n""#;

/// Reads the [`CHANGE`] template's output: exactly one change, with an ID.
fn parse_change(stdout: &[u8]) -> Result<Change> {
    let mut records: Vec<(ChangeFields, bool)> = parse_lines(stdout, "jj log")?;
    let (Some((fields, empty)), true) = (records.pop(), records.is_empty()) else {
        return Err(malformed(
            "jj log",
            "expected exactly one working-copy change",
        ));
    };
    if fields.change_id.trim().is_empty() {
        return Err(malformed(
            "jj log",
            "the working-copy change has no change ID",
        ));
    }
    Ok(Change {
        id: fields.change_id,
        scratch: empty && fields.description.is_empty(),
    })
}

/// git and jj run in the working directory with the child environment.
struct Repo<'a> {
    ctx: &'a Ctx,
    kind: RepositoryKind,
}

impl Repo<'_> {
    /// Creates the issue's branch, or `branch`, from `from_ref`; an existing
    /// branch is switched to or given a suffix, as the user chooses. What
    /// was done is printed once it is done.
    fn git_branch(
        &self,
        issue: &IssueTransitionDetails,
        branch: Option<&str>,
        from_ref: Option<&str>,
    ) -> Result<()> {
        let branch = branch
            .filter(|value| !value.is_empty())
            .unwrap_or(&issue.branch_name);
        let from_ref = from_ref.filter(|value| !value.is_empty());
        let done = if !self.git_branch_exists(branch)? {
            self.git_create(branch, from_ref)?
        } else {
            match choose_existing(self.ctx, branch)? {
                ExistingBranch::Switch => self.git_switch(branch, from_ref)?,
                ExistingBranch::Suffix => {
                    let branch = free_suffix(branch, |name| self.git_branch_exists(name))?;
                    self.git_create(&branch, from_ref)?
                }
            }
        };
        self.say(done.report())
    }

    /// Prints `lines` on stdout now, before anything else is tried.
    fn say(&self, lines: impl AsRef<[u8]>) -> Result<()> {
        self.ctx.print(lines)?;
        self.ctx.flush()
    }

    fn command(&self, program: &str) -> Command {
        process::command(program, self.ctx.cwd(), &self.ctx.config().child_env)
    }

    /// `error` from the first git or jj command, replaced by a plain
    /// explanation when the working directory is in no repository at all.
    fn outside_repository(&self, error: Error) -> Error {
        if vcs::in_repository(self.kind, self.ctx.cwd()) {
            return error;
        }
        vcs::not_in_repository(self.kind).with_hint(
            "Run it from your repository. To only change the issue's status, use `linear issue start <ISSUE>`.",
        )
    }

    fn git_branch_exists(&self, branch: &str) -> Result<bool> {
        let mut command = self.command("git");
        let reference = format!("refs/heads/{branch}");
        command.args(["show-ref", "--verify", "--quiet", &reference]);
        let output =
            process::output(&mut command).map_err(|error| self.outside_repository(error))?;
        // Missing or invalid local ref names exit 1 with --verify --quiet.
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(self.outside_repository(
                process::failed(&command, output.status, &output.stderr)
                    .context("Failed to check if branch exists"),
            )),
        }
    }

    /// The commit `revision` names, abbreviated as git does but to 12
    /// characters at least; `None` when it names no commit.
    fn git_commit(&self, revision: &str) -> Result<Option<String>> {
        let mut command = self.command("git");
        let commit = format!("{revision}^{{commit}}");
        command.args([
            "rev-parse",
            "--verify",
            "--quiet",
            "--short=12",
            "--end-of-options",
            &commit,
        ]);
        let output = process::output(&mut command)?;
        match output.status.code() {
            Some(0) => {
                let commit = process::text(&output.stdout);
                if commit.is_empty() {
                    return Err(Error::new(format!(
                        "git rev-parse named no commit for {}",
                        quoted(revision)
                    )));
                }
                Ok(Some(commit))
            }
            Some(1) => Ok(None),
            _ => Err(process::failed(&command, output.status, &output.stderr)),
        }
    }

    /// The branch checked out now, or the commit of a detached HEAD.
    fn git_previous(&self) -> Result<Previous> {
        let mut command = self.command("git");
        command.args(["symbolic-ref", "--quiet", "--short", "HEAD"]);
        let output = process::output(&mut command)
            .map_err(|error| error.context("Failed to read the current git branch"))?;
        match output.status.code() {
            Some(0) => {
                let branch = process::text(&output.stdout);
                if branch.is_empty() {
                    return Err(Error::new("git named no current branch"));
                }
                Ok(Previous::Branch(branch))
            }
            // A detached HEAD has no branch.
            Some(1) => self
                .git_commit("HEAD")
                .context("Failed to read the detached git HEAD")?
                .map(Previous::Detached)
                .ok_or_else(|| Error::new("git HEAD is detached but names no commit")),
            _ => Err(process::failed(&command, output.status, &output.stderr)
                .context("Failed to read the current git branch")),
        }
    }

    fn git_switch(&self, branch: &str, from_ref: Option<&str>) -> Result<GitDone> {
        let previous = self.git_previous()?;
        process::checked_output(self.command("git").args(["checkout", branch]))
            .context(format!("Failed to switch to branch '{branch}'"))?;
        Ok(GitDone::Switched {
            branch: branch.to_owned(),
            previous,
            unused_base: from_ref.map(str::to_owned),
        })
    }

    fn git_create(&self, branch: &str, from: Option<&str>) -> Result<GitDone> {
        let base = from.unwrap_or("HEAD");
        let previous = self.git_previous()?;
        let commit = self
            .git_commit(base)
            .context(format!("Failed to resolve {}", quoted(base)))?
            .ok_or_else(|| {
                Error::invalid(format!(
                    "Cannot create branch '{branch}' from {}: it names no commit",
                    quoted(base)
                ))
                .with_hint(
                    "Pass --from-ref a branch, tag or commit that exists, or leave it out to branch from HEAD.",
                )
            })?;
        process::checked_output(self.command("git").args(["checkout", "-b", branch, base]))
            .context(format!("Failed to create branch '{branch}'"))?;
        Ok(GitDone::Created {
            branch: branch.to_owned(),
            base: base.to_owned(),
            commit,
            previous,
        })
    }

    /// The working-copy change, read after snapshotting the working copy.
    fn jj_current(&self) -> Result<Change> {
        let output = process::checked_output(self.command("jj").args([
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            CHANGE,
        ]))?;
        parse_change(&output.stdout)
    }

    /// Starts a new change unless `@` is already empty and undescribed, and
    /// says which change it is.
    fn jj_prepare(&self) -> Result<JjPrepared> {
        let current = self.jj_current().map_err(|error| {
            self.outside_repository(error.context("Failed to inspect the working-copy change"))
        })?;
        if current.scratch {
            self.say(format!(
                "✓ Reused empty, undescribed jj change {}\n",
                single_line(&current.id)
            ))?;
            return Ok(JjPrepared::Reused { change: current.id });
        }
        process::checked_output(self.command("jj").arg("new"))
            .context("Failed to create new jj change")?;
        let previous = single_line(&current.id).into_owned();
        match self.jj_current() {
            Ok(created) => {
                self.say(format!(
                    "✓ Created and switched to jj change {} from {previous}\n",
                    single_line(&created.id)
                ))?;
                Ok(JjPrepared::Created { change: created.id })
            }
            Err(error) => {
                self.say(format!(
                    "✓ Created and switched to a new jj change from {previous}\n"
                ))?;
                Err(with_recovery(
                    error.context("Failed to inspect the new jj change"),
                    "A new jj change was created, but its ID could not be read. Inspect it with jj log -r @ before retrying.",
                ))
            }
        }
    }

    /// Describes the prepared change with the issue title and its
    /// `Linear-issue` trailers, then says exactly what was written.
    fn jj_describe(&self, prepared: &JjPrepared, issue: &IssueTransitionDetails) -> Result<()> {
        let IssueTransitionDetails {
            identifier,
            title,
            url,
            ..
        } = issue;
        let change = prepared.change();
        let description = format!(
            "{identifier} {title}\n\nLinear-issue: Fixes {identifier}\nLinear-issue-url: {url}"
        );
        // `@` is the change just probed. Its change ID is only for display: it
        // names several commits when the change is divergent, and jj then
        // refuses it as a revision.
        process::checked_output(self.command("jj").args([
            "describe",
            "-m",
            &description,
            "-r",
            "@",
        ]))
        .map_err(|error| {
            with_recovery(
                error.context("Failed to set jj description"),
                &prepared.describe_failed(),
            )
        })?;
        self.say(format!(
            "✓ Wrote jj change {} description: {}\n\
             ✓ Wrote trailers (keep both when rewording to use @jj):\n\
             \x20 Linear-issue: Fixes {}\n\
             \x20 Linear-issue-url: {}\n",
            single_line(change),
            shown(&format!("{identifier} {title}")),
            single_line(identifier),
            single_line(url),
        ))
    }
}

#[cfg(test)]
mod tests;
