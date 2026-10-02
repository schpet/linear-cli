//! `issue start`: create or switch the VCS branch, then move the issue to a started state.
use crate::{
    cli::issue::IssueStart,
    commands::{
        issue::read as issue_read, team::states as team_states, team_key::configured_team_key,
    },
    config::{ChildEnvOverlay, Vcs},
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{
        bulk_error::{self, ObservedExchangeFailure, SourceExceptionKind},
        envelope::GraphQlRequest,
        operations::{
            issue_read::*,
            issue_start_state::{UpdateIssueState, Variables},
            workflow_states::{GetWorkflowStates, WorkflowState},
        },
        transport::GraphQlTransport,
    },
    platform::{
        prompt::{PlainOption, PlainSelect, PromptOutcome},
        selector::SelectOption,
        vcs_script::{CommandSpec, NativeProcessRunner, ProcessRunner, Program, decoded_trim},
    },
    refs::{IssueReference, prepare_issue_reference},
};
use cynic::{MutationBuilder, QueryBuilder};
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
    let options = choices(&issues, team)?;
    let mut session = ctx.prompts()?;
    let picked = session.searchable_select_with_no_match(
        "Select an issue to start:",
        "Search issues",
        &options,
        "no issues match submitted search query",
    );
    match stage(session.finish_result(picked)?, "issue to start")? {
        PromptOutcome::Submitted(identifier) => Ok(identifier),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => unreachable!("stage reports end of input as an error"),
    }
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
    let details = ctx.spin(true, super::describe::fetch(client, identifier))?;
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
    let choices = branch_options();
    let message = crate::platform::prompt::escaped_display(&format!(
        "Branch {branch} already exists. What would you like to do?"
    ));
    let mut session = ctx.prompts()?;
    let answer = session.select(&branch_menu(&message, &choices));
    match stage(session.finish_result(answer)?, "existing branch action")? {
        PromptOutcome::Submitted(value) => existing_branch(&value),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => unreachable!("stage reports end of input as an error"),
    }
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
pub fn list_request(
    variables: GetIssuesForStateVariables,
) -> GraphQlRequest<GetIssuesForStateVariables> {
    let mut request = GraphQlRequest::with_variables(GetIssuesForState::build(variables));
    request.query = request.query.trim_end_matches('\n').to_owned();
    request
}
pub async fn list(
    transport: &GraphQlTransport,
    filter: IssueFilter,
    priority: bool,
) -> Result<Vec<GetIssuesForStateIssuesNodes>, Error> {
    issue_read::mine_with_requests(transport, filter, priority, None, list_request).await
}
pub fn choices(
    issues: &[GetIssuesForStateIssuesNodes],
    team: &str,
) -> Result<Vec<SelectOption>, Error> {
    if issues.is_empty() {
        return Err(Error::new(format!("Unstarted issues not found: {team}")));
    }
    issues
        .iter()
        .map(|issue| {
            Ok(SelectOption {
                label: format!(
                    "{} {}: {}",
                    issue_read::priority(issue.priority),
                    issue.identifier,
                    issue.title
                ),
                value: issue.identifier.clone(),
            })
        })
        .collect()
}
pub const BRANCH_CHOICES: [(&str, &str); 2] = [
    ("Switch to existing branch", "switch"),
    ("Create new branch with suffix", "create"),
];
pub fn branch_options() -> Vec<PlainOption> {
    BRANCH_CHOICES
        .iter()
        .map(|(label, value)| PlainOption {
            label: (*label).to_owned(),
            value: (*value).to_owned(),
            script_token: (*value).to_owned(),
        })
        .collect()
}
pub fn branch_menu<'a>(message: &'a str, choices: &'a [PlainOption]) -> PlainSelect<'a> {
    PlainSelect {
        message,
        options: choices,
        default_index: 0,
        default_hint: None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExistingBranch {
    Switch,
    Suffix,
}
pub fn existing_branch(value: &str) -> Result<ExistingBranch, Error> {
    match value {
        "switch" => Ok(ExistingBranch::Switch),
        "create" => Ok(ExistingBranch::Suffix),
        _ => Err(Error::new("branch menu returned an unknown action")),
    }
}
pub fn stage<T>(outcome: PromptOutcome<T>, name: &str) -> Result<PromptOutcome<T>, Error> {
    match outcome {
        PromptOutcome::EndOfInput => {
            Err(Error::new(format!("unexpected EOF while selecting {name}")))
        }
        outcome => Ok(outcome),
    }
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
    crate::workflow_states::sort(&mut states);
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
pub fn update_request(identifier: &str, state_id: &str) -> GraphQlRequest<Variables> {
    let mut request = GraphQlRequest::with_variables(UpdateIssueState::build(Variables {
        issue_id: identifier.to_owned(),
        state_id: state_id.to_owned(),
    }));
    request.query = request.query.trim_end_matches('\n').to_owned();
    request
}
pub fn post_failure(error: ObservedExchangeFailure) -> String {
    match error {
        ObservedExchangeFailure::Strict(error) => format!("Error: {}", error),
        ObservedExchangeFailure::Ordinary(error) => format!(
            "{}: {}",
            match error.kind {
                SourceExceptionKind::Plain => "Error",
                SourceExceptionKind::Client => "ClientError",
            },
            error.message
        ),
    }
}
pub async fn update_state(
    transport: &GraphQlTransport,
    team: &str,
    identifier: &str,
) -> Result<Vec<u8>, String> {
    let mut request = team_states::request(team.to_owned());
    request.query = request.query.trim_end_matches('\n').to_owned();
    let response: GetWorkflowStates = bulk_error::execute_observed(transport, &request)
        .await
        .map_err(post_failure)?;
    let state = started(response.team.states.nodes).map_err(|error| format!("Error: {}", error))?;
    let response: UpdateIssueState =
        bulk_error::execute_observed(transport, &update_request(identifier, state.id.inner()))
            .await
            .map_err(post_failure)?;
    // The `success` flag is not reported; the whole payload is still decoded.
    let _reported_success = response.issue_update.success;
    Ok(format!("✓ Issue state updated to '{}'\n", state.name).into_bytes())
}
