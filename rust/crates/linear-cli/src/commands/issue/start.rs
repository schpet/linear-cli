//! `issue start`: create or switch the VCS branch, then move the issue to a started state.
use crate::client::LinearClient;
use crate::{
    cli::issue::IssueStart,
    commands::{
        issue::{list_view, read as issue_read},
        team_key::configured_team_key,
    },
    config::Vcs,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::{
        issue::{UpdateIssueState, UpdateIssueStateVariables},
        issue_read::*,
        team::WorkflowState,
    },
    platform::{process, prompt::Choice},
    refs::prepare_issue_reference,
};
use std::process::Command;
pub fn run(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    start(ctx, args).context("Failed to start issue")
}

fn start(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    if args.all_assignees && args.unassigned {
        return Err(Error::new(
            "Cannot specify both --all-assignees and --unassigned",
        ));
    }
    let team = configured_team_key(ctx.options());
    // Start never infers the issue from the VCS: without one it offers a picker.
    let identifier = match args.issue_id.as_deref() {
        Some(input) => prepare_issue_reference(input, team.as_deref(), &ctx.scope()?)?
            .ok_or_else(|| Error::new(format!("Not an issue ID: {input}")).with_hint(
                "Pass an issue ID like ENG-123, an issue URL, or an issue number in the configured team.",
            ))?,
        None => {
            let team = team.ok_or_else(|| {
                Error::new("No team is configured to pick an issue from")
                    .with_hint("Pass an issue ID, or run `linear config` to set a team.")
            })?;
            pick(ctx, &team, args)?
        }
    };
    work_on(
        ctx,
        &identifier,
        args.branch.as_deref(),
        args.from_ref.as_deref(),
    )
}

/// Asks which of the team's unstarted issues to start.
fn pick(ctx: &Ctx, team: &str, args: &IssueStart) -> Result<String> {
    ctx.require_tty("an issue ID")?;
    let sort = ctx.options().issue_sort(None).0;
    let client = ctx.client()?;
    let issues = ctx.spin(
        true,
        issue_read::mine(
            client,
            filter(team, args.all_assignees, args.unassigned),
            sort,
            None,
        ),
    )?;
    ctx.prompter()?
        .select("Select an issue to start:", choices(&issues, team)?)
}

/// Switches the working copy to the issue (a git branch or a jj change), then
/// moves the issue to its team's first started state. The working copy is
/// never rolled back when the state change fails.
pub(crate) fn work_on(
    ctx: &Ctx,
    identifier: &str,
    branch: Option<&str>,
    from_ref: Option<&str>,
) -> Result<()> {
    let client = ctx.client()?;
    let details = ctx.spin(true, super::details::fetch(client, identifier.to_owned()))?;
    let repo = Repo::new(ctx);
    ctx.flush()?;
    let vcs = super::vcs(ctx);
    let output = match vcs {
        Vcs::Git => {
            let branch = branch
                .filter(|value| !value.is_empty())
                .unwrap_or(&details.branch_name);
            if repo.git_branch_exists(branch)? {
                match choose_existing(ctx, branch)? {
                    ExistingBranch::Switch => repo.git_switch(branch)?,
                    ExistingBranch::Suffix => {
                        let branch = free_suffix(branch, |name| repo.git_branch_exists(name))?;
                        repo.git_create(&branch, from_ref)?
                    }
                }
            } else {
                repo.git_create(branch, from_ref)?
            }
        }
        Vcs::Jj => {
            repo.jj_prepare()?;
            repo.jj_describe(identifier, &details.title, &details.url)?
        }
    };
    ctx.print(output)?;
    let state = ctx
        .spin(true, mark_started(client, &details.team.key, identifier))
        .map_err(|error| {
            let prepared = match vcs {
                Vcs::Git => "The branch is ready",
                Vcs::Jj => "The jj change is ready",
            };
            error
                .context("Could not move the issue to a started state")
                .with_hint(format!(
                    "{prepared}; set the state with `linear issue update {identifier} --state <state>`."
                ))
        })?;
    ctx.print(format!("✓ Issue state updated to '{state}'\n"))
}

fn choose_existing(ctx: &Ctx, branch: &str) -> Result<ExistingBranch> {
    ctx.require_tty("--branch with a new name")?;
    ctx.prompter()?.select(
        &format!("Branch {branch} already exists. What would you like to do?"),
        vec![
            Choice::new("Switch to existing branch", ExistingBranch::Switch),
            Choice::new("Create new branch with suffix", ExistingBranch::Suffix),
        ],
    )
}

fn filter(team: &str, all: bool, unassigned: bool) -> IssueFilter {
    IssueFilter {
        team: Some(TeamFilter {
            key: Some(StringComparator {
                eq: Some(team.to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        state: Some(WorkflowStateFilter {
            r#type: Some(StringComparator {
                r#in: Some(vec!["unstarted".to_owned()]),
                ..Default::default()
            }),
            ..Default::default()
        }),
        assignee: if all {
            None
        } else if unassigned {
            Some(NullableUserFilter {
                r#null: Some(true),
                ..Default::default()
            })
        } else {
            Some(NullableUserFilter {
                is_me: Some(BooleanComparator { eq: Some(true) }),
                ..Default::default()
            })
        },
        ..Default::default()
    }
}
fn choices(issues: &[ListedIssue], team: &str) -> Result<Vec<Choice<String>>> {
    if issues.is_empty() {
        return Err(Error::new(format!("Unstarted issues not found: {team}")));
    }
    Ok(issues
        .iter()
        .map(|issue| {
            let label = format!(
                "{} {}: {}",
                list_view::priority(issue.priority),
                issue.identifier,
                issue.title
            );
            Choice::new(label, issue.identifier.clone())
        })
        .collect())
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
}

impl<'a> Repo<'a> {
    fn new(ctx: &'a Ctx) -> Self {
        Self { ctx }
    }

    fn command(&self, program: &str) -> Command {
        process::command(program, self.ctx.cwd(), &self.ctx.config().child_env)
    }

    fn git_branch_exists(&self, branch: &str) -> Result<bool> {
        let mut command = self.command("git");
        command.args(["rev-parse", "--verify", "--quiet", branch]);
        let output = process::output(&mut command)?;
        // With --quiet, a missing ref exits 1 and nothing else does.
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(process::failed(&command, output.status, &output.stderr)
                .context("Failed to check if branch exists")),
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
        .context("Failed to inspect the working-copy change")?;
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

/// The started state with the lowest position: the first step of active work.
fn first_started(states: Vec<WorkflowState>) -> Result<WorkflowState> {
    states
        .into_iter()
        .filter(|state| state.state_type == "started")
        .min_by(|a, b| a.position.get().total_cmp(&b.position.get()))
        .ok_or_else(|| Error::new("The issue's team has no started workflow state"))
}

/// Moves the issue to its team's first started state, returning the state's name.
async fn mark_started(client: &LinearClient, team: &str, identifier: &str) -> Result<String> {
    let states = crate::refs::workflow_states::fetch(client, team.to_owned()).await?;
    let state = first_started(states)?;
    let response: UpdateIssueState = client
        .mutate(UpdateIssueStateVariables {
            issue_id: identifier.to_owned(),
            state_id: state.id.inner().to_owned(),
        })
        .await?;
    if !response.issue_update.success {
        return Err(Error::new("Linear did not update the issue"));
    }
    Ok(state.name)
}

#[cfg(test)]
mod tests;
