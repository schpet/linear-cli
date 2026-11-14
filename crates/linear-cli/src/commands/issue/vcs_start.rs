//! `issue git start` and `issue jj start`: prepare a git branch or a jj
//! change for an issue, then start the issue as `issue start` does.
use std::process::Command;

use crate::{
    cli::issue::{IssueGitStart, IssueJjStart},
    commands::issue::transition::{self, Verb},
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue::IssueTransitionDetails,
    platform::{
        process,
        prompt::Choice,
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
    let output = match prepare {
        Prepare::Git { branch, from_ref } => repo.git_branch(&issue, branch, from_ref)?,
        Prepare::Jj => {
            repo.jj_prepare()?;
            repo.jj_describe(&issue.identifier, &issue.title, &issue.url)?
        }
    };
    ctx.print(output)?;
    let outcome = ctx
        .spin(true, transition::apply(client, Verb::Start, &issue))
        .map_err(|error| prepared(error, prepare.kind(), &issue.identifier))?;
    ctx.print(transition::report(Verb::Start, &issue.identifier, &outcome))
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
    let hint = match error.hint() {
        Some(hint) => format!("{hint} {recovery}"),
        None => recovery,
    };
    error
        .context("Could not move the issue to a started status")
        .with_hint(hint)
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

/// git and jj run in the working directory with the child environment.
struct Repo<'a> {
    ctx: &'a Ctx,
    kind: RepositoryKind,
}

impl Repo<'_> {
    /// Creates the issue's branch, or `branch`, from `from_ref`; an existing
    /// branch is switched to or given a suffix, as the user chooses.
    fn git_branch(
        &self,
        issue: &IssueTransitionDetails,
        branch: Option<&str>,
        from_ref: Option<&str>,
    ) -> Result<Vec<u8>> {
        let branch = branch
            .filter(|value| !value.is_empty())
            .unwrap_or(&issue.branch_name);
        if !self.git_branch_exists(branch)? {
            return self.git_create(branch, from_ref);
        }
        match choose_existing(self.ctx, branch)? {
            ExistingBranch::Switch => self.git_switch(branch),
            ExistingBranch::Suffix => {
                let branch = free_suffix(branch, |name| self.git_branch_exists(name))?;
                self.git_create(&branch, from_ref)
            }
        }
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

    fn git_switch(&self, branch: &str) -> Result<Vec<u8>> {
        process::checked_output(self.command("git").args(["checkout", branch]))
            .context(format!("Failed to switch to branch '{branch}'"))?;
        Ok(format!("✓ Switched to '{branch}'\n").into_bytes())
    }

    fn git_create(&self, branch: &str, from: Option<&str>) -> Result<Vec<u8>> {
        let from = from.filter(|value| !value.is_empty()).unwrap_or("HEAD");
        process::checked_output(self.command("git").args(["checkout", "-b", branch, from]))
            .context(format!("Failed to create branch '{branch}'"))?;
        Ok(format!("✓ Created and switched to branch '{branch}'\n").into_bytes())
    }

    /// Starts a new change unless `@` is already empty and undescribed.
    fn jj_prepare(&self) -> Result<()> {
        let probe = process::checked_output(self.command("jj").args([
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "if(description, \"occupied\", if(empty, \"empty\", \"occupied\"))",
        ]))
        .map_err(|error| {
            self.outside_repository(error.context("Failed to inspect the working-copy change"))
        })?;
        match process::text(&probe.stdout).as_str() {
            "empty" => Ok(()),
            "occupied" => {
                process::checked_output(self.command("jj").arg("new"))
                    .context("Failed to create new jj change")?;
                Ok(())
            }
            other => Err(Error::new(format!(
                "Unexpected output from `jj log`: {other:?}"
            ))),
        }
    }

    fn jj_describe(&self, identifier: &str, title: &str, url: &str) -> Result<Vec<u8>> {
        let description = format!(
            "{identifier} {title}\n\nLinear-issue: Fixes {identifier}\nLinear-issue-url: {url}"
        );
        process::checked_output(self.command("jj").args(["describe", "-m", &description]))
            .context("Failed to set jj description")?;
        Ok(format!("✓ Prepared jj change for issue {identifier}\n").into_bytes())
    }
}

#[cfg(test)]
mod tests;
