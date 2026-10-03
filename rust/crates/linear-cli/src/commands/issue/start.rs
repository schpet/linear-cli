//! `issue start`: create or switch the VCS branch, then move the issue to a started state.
use crate::client::LinearClient;
use crate::{
    cli::issue::IssueStart,
    commands::{issue::read as issue_read, team_key::configured_team_key},
    config::{ChildEnvOverlay, Vcs},
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::{
        issue::{UpdateIssueState, UpdateIssueStateVariables},
        issue_read::*,
        team::WorkflowState,
    },
    platform::{
        prompt::Choice,
        vcs_script::{CommandSpec, NativeProcessRunner, ProcessRunner, Program, decoded_trim},
    },
    refs::{IssueReference, prepare_issue_reference},
};
use std::{io::Write, path::Path};
pub fn run(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    start(ctx, args).context("Failed to start issue")
}

fn start(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    let team = configured_team_key(ctx.options());
    let team = team_and_flags(team.as_deref(), args.all_assignees, args.unassigned)?;
    // Start never infers the issue from the VCS: without one it offers a picker.
    let identifier = match args.issue_id.as_deref().filter(|value| !value.is_empty()) {
        Some(input) => match prepare_issue_reference(Some(input), Some(team), &ctx.scope()?)? {
            IssueReference::Identifier(identifier) => Some(identifier),
            IssueReference::Unresolved => None,
            IssueReference::Inferred => unreachable!("a given reference is never inferred"),
        },
        None => None,
    };
    let identifier = match identifier {
        Some(identifier) => identifier,
        None => pick(ctx, team, args)?,
    };
    work_on(
        ctx,
        &identifier,
        team,
        args.branch.as_deref(),
        args.from_ref.as_deref(),
    )
}

/// Asks which of the team's unstarted issues to start.
fn pick(ctx: &Ctx, team: &str, args: &IssueStart) -> Result<String> {
    ctx.require_tty("an issue ID")?;
    let priority = ctx.options().issue_sort(None).0 == crate::config::IssueSort::Priority;
    let client = ctx.client()?;
    let issues = ctx.spin(
        true,
        list(
            client,
            filter(team, args.all_assignees, args.unassigned),
            priority,
        ),
    )?;
    ctx.prompter()?
        .select("Select an issue to start:", choices(&issues, team)?)
}

/// Switches the working copy to the issue (a git branch or a jj change), then
/// moves the issue to a started state. The state change is best effort: the
/// VCS work is already done and is never rolled back.
pub(crate) fn work_on(
    ctx: &Ctx,
    identifier: &str,
    team: &str,
    branch: Option<&str>,
    from_ref: Option<&str>,
) -> Result<()> {
    let client = ctx.client()?;
    let details = ctx.spin(true, super::details::fetch(client, identifier.to_owned()))?;
    let mut runner = NativeProcessRunner;
    let cwd = ctx.cwd();
    let env = &ctx.config().child_env;
    ctx.flush()?;
    let output = match super::vcs(ctx) {
        Vcs::Git => {
            let branch = branch_name(branch, &details.branch_name);
            let choice = if verify(&mut runner, branch, cwd, env)? {
                Some(choose_existing(ctx, branch)?)
            } else {
                None
            };
            match choice {
                Some(choice) => existing_git(&mut runner, choice, branch, from_ref, cwd, env)?,
                None => create_branch(&mut runner, branch, from_ref, cwd, env)?,
            }
        }
        Vcs::Jj => {
            let mut stderr = std::io::stderr();
            prepare_jj(&mut runner, cwd, env, &mut stderr)?;
            describe_jj(
                &mut runner,
                identifier,
                &details.title,
                &details.url,
                cwd,
                env,
                &mut stderr,
            )?
        }
    };
    ctx.print(output)?;
    match ctx.spin(true, update_state(client, team, identifier)) {
        Ok(output) => ctx.print(output),
        Err(message) => ctx.eprint(format!("Failed to update issue state: {message}\n")),
    }
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
pub fn team_and_flags(team: Option<&str>, all: bool, unassigned: bool) -> Result<&str, Error> {
    let team = team
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::new("Could not determine team ID"))?;
    if all && unassigned {
        return Err(Error::new(
            "Cannot specify both --all-assignees and --unassigned",
        ));
    }
    Ok(team)
}
pub fn filter(team: &str, all: bool, unassigned: bool) -> IssueFilter {
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
pub async fn list(
    client: &LinearClient,
    filter: IssueFilter,
    priority: bool,
) -> Result<Vec<ListedIssue>, Error> {
    issue_read::mine(client, filter, priority, None).await
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
                issue_read::priority(issue.priority),
                issue.identifier,
                issue.title
            );
            Choice::new(label, issue.identifier.clone())
        })
        .collect())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExistingBranch {
    Switch,
    Suffix,
}
pub fn branch_name<'a>(custom: Option<&'a str>, returned: &'a str) -> &'a str {
    custom.filter(|value| !value.is_empty()).unwrap_or(returned)
}
pub fn verify(
    runner: &mut impl ProcessRunner,
    branch: &str,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<bool, Error> {
    runner
        .capture(
            &CommandSpec::new(Program::Git, &["rev-parse", "--verify", branch]),
            cwd,
            env,
        )
        .map(|captured| captured.outcome.success())
        .context("Failed to check if branch exists")
}
pub fn create_branch(
    runner: &mut impl ProcessRunner,
    branch: &str,
    from: Option<&str>,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<Vec<u8>, Error> {
    let from = from.filter(|value| !value.is_empty()).unwrap_or("HEAD");
    let result = runner.capture(
        &CommandSpec::new(Program::Git, &["checkout", "-b", branch, from]),
        cwd,
        env,
    )?;
    if !result.outcome.success() {
        return Err(Error::new(format!(
            "Failed to create branch '{branch}': {}",
            decoded_trim(&result.stderr)
        )));
    }
    Ok(format!("✓ Created and switched to branch '{branch}'\n").into_bytes())
}
pub fn existing_git(
    runner: &mut impl ProcessRunner,
    action: ExistingBranch,
    branch: &str,
    from: Option<&str>,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<Vec<u8>, Error> {
    match action {
        ExistingBranch::Switch => {
            let result = runner.capture(
                &CommandSpec::new(Program::Git, &["checkout", branch]),
                cwd,
                env,
            )?;
            if !result.outcome.success() {
                return Err(Error::new(format!(
                    "Failed to switch to branch '{branch}': {}",
                    decoded_trim(&result.stderr)
                )));
            }
            Ok(format!("✓ Switched to '{branch}'\n").into_bytes())
        }
        ExistingBranch::Suffix => {
            let mut suffix = 1_u64;
            loop {
                let candidate = format!("{branch}-{suffix}");
                if !verify(runner, &candidate, cwd, env)? {
                    return create_branch(runner, &candidate, from, cwd, env);
                }
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| Error::new("branch suffix counter exhausted"))?;
            }
        }
    }
}
fn decoded(bytes: &[u8]) -> String {
    let value = String::from_utf8_lossy(bytes);
    value.strip_prefix('\u{feff}').unwrap_or(&value).to_owned()
}
pub fn prepare_jj(
    runner: &mut impl ProcessRunner,
    cwd: &Path,
    env: &ChildEnvOverlay,
    stderr: &mut (impl Write + ?Sized),
) -> Result<(), Error> {
    let description = runner.capture(
        &CommandSpec::new(
            Program::Jj,
            &["log", "-r", "@", "-T", "description", "--no-graph"],
        ),
        cwd,
        env,
    )?;
    let needs_new = if !decoded_trim(&description.stdout).is_empty() {
        true
    } else {
        let diff = runner.capture(
            &CommandSpec::new(
                Program::Jj,
                &["log", "-p", "-r", "@", "--git", "--no-graph"],
            ),
            cwd,
            env,
        )?;
        decoded(&diff.stdout).contains("diff --git")
    };
    if needs_new {
        let result = runner.capture(&CommandSpec::new(Program::Jj, &["new"]), cwd, env)?;
        if !result.outcome.success() {
            writeln!(stderr, "{}", decoded(&result.stderr))
                .map_err(|error| Error::new("could not write jj failure").with_source(error))?;
            return Err(Error::new("Failed to create new jj change"));
        }
    }
    Ok(())
}
pub fn describe_jj(
    runner: &mut impl ProcessRunner,
    identifier: &str,
    title: &str,
    url: &str,
    cwd: &Path,
    env: &ChildEnvOverlay,
    stderr: &mut (impl Write + ?Sized),
) -> Result<Vec<u8>, Error> {
    let description = format!(
        "{identifier} {title}\n\nLinear-issue: Fixes {identifier}\nLinear-issue-url: {url}"
    );
    let result = runner.capture(
        &CommandSpec::new(Program::Jj, &["describe", "-m", &description]),
        cwd,
        env,
    )?;
    if !result.outcome.success() {
        writeln!(stderr, "{}", decoded(&result.stderr))
            .map_err(|error| Error::new("could not write jj failure").with_source(error))?;
        return Err(Error::new("Failed to set jj description"));
    }
    Ok(format!("✓ Prepared jj change for issue {identifier}\n").into_bytes())
}
pub fn started(mut states: Vec<WorkflowState>) -> Result<WorkflowState, Error> {
    crate::refs::workflow_states::sort(&mut states);
    let mut selected: Option<WorkflowState> = None;
    for state in states {
        if state.state_type == "started"
            && selected
                .as_ref()
                .is_none_or(|previous| state.position.get() < previous.position.get())
        {
            selected = Some(state);
        }
    }
    selected.ok_or_else(|| Error::new("No 'started' state found in workflow"))
}

pub async fn update_state(
    client: &LinearClient,
    team: &str,
    identifier: &str,
) -> Result<Vec<u8>, String> {
    let states = crate::refs::workflow_states::fetch(client, team.to_owned())
        .await
        .map_err(|failure| Error::from(failure).to_string())?;
    let state = started(states).map_err(|error| error.to_string())?;
    let response: UpdateIssueState = client
        .mutate(UpdateIssueStateVariables {
            issue_id: identifier.to_owned(),
            state_id: state.id.inner().to_owned(),
        })
        .await
        .map_err(|failure| Error::from(failure).to_string())?;
    // The `success` flag is not reported; the whole payload is still decoded.
    let _reported_success = response.issue_update.success;
    Ok(format!("✓ Issue state updated to '{}'\n", state.name).into_bytes())
}

#[cfg(test)]
mod tests;
