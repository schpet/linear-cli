//! Source start sequencing, captured VCS steps and best-effort state update.
use crate::{
    commands::{issue_read, team_states},
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
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
        vcs_script::{CommandSpec, ProcessRunner, Program, decoded_trim},
    },
};
use cynic::{MutationBuilder, QueryBuilder};
use std::{io::Write, path::Path};
pub const CONTEXT: &str = "Failed to start issue";
pub fn team_and_flags(team: Option<&str>, all: bool, unassigned: bool) -> Result<&str, AppError> {
    let team = team
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::new(AppErrorKind::Validation, "Could not determine team ID"))?;
    if all && unassigned {
        return Err(AppError::new(
            AppErrorKind::Validation,
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
) -> Result<Vec<GetIssuesForStateIssuesNodes>, AppError> {
    issue_read::mine_with_requests(transport, filter, priority, None, list_request).await
}
pub fn choices(
    issues: &[GetIssuesForStateIssuesNodes],
    team: &str,
) -> Result<Vec<SelectOption>, AppError> {
    if issues.is_empty() {
        return Err(AppError::new(
            AppErrorKind::NotFound,
            format!("Unstarted issues not found: {team}"),
        ));
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
pub fn existing_branch(value: &str) -> Result<ExistingBranch, AppError> {
    match value {
        "switch" => Ok(ExistingBranch::Switch),
        "create" => Ok(ExistingBranch::Suffix),
        _ => Err(AppError::new(
            AppErrorKind::Invariant,
            "branch menu returned an unknown action",
        )),
    }
}
pub fn stage<T>(outcome: PromptOutcome<T>, name: &str) -> Result<PromptOutcome<T>, AppError> {
    match outcome {
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            format!("unexpected EOF while selecting {name}"),
        )),
        outcome => Ok(outcome),
    }
}
pub fn check_prompt_topology(stdin_tty: bool, stdout_fifo: bool) -> Result<(), AppError> {
    if stdin_tty && stdout_fifo {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "issue start prompts require terminal or regular-file stdout when stdin is a terminal",
        )
        .with_suggestion(
            "Run without piping stdout, or use a branch that does not require a selection.",
        ));
    }
    Ok(())
}
#[cfg(unix)]
pub fn stdout_is_fifo() -> Result<bool, AppError> {
    use rustix::fs::{FileType, fstat};
    let stat = fstat(std::io::stdout()).map_err(|error| {
        AppError::new(
            AppErrorKind::IoProcess,
            format!("could not inspect stdout: {error}"),
        )
        .with_source(error)
    })?;
    Ok(FileType::from_raw_mode(stat.st_mode) == FileType::Fifo)
}
#[cfg(not(unix))]
pub fn stdout_is_fifo() -> Result<bool, AppError> {
    Ok(false)
}
pub fn branch_name<'a>(custom: Option<&'a str>, returned: &'a str) -> &'a str {
    custom.filter(|value| !value.is_empty()).unwrap_or(returned)
}
pub fn verify(
    runner: &mut impl ProcessRunner,
    branch: &str,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<bool, AppError> {
    runner
        .capture(
            &CommandSpec::new(Program::Git, &["rev-parse", "--verify", branch]),
            cwd,
            env,
        )
        .map(|captured| captured.outcome.success())
        .map_err(|error| error.with_context("Failed to check if branch exists"))
}
pub fn create_branch(
    runner: &mut impl ProcessRunner,
    branch: &str,
    from: Option<&str>,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<Vec<u8>, AppError> {
    let from = from.filter(|value| !value.is_empty()).unwrap_or("HEAD");
    let result = runner.capture(
        &CommandSpec::new(Program::Git, &["checkout", "-b", branch, from]),
        cwd,
        env,
    )?;
    if !result.outcome.success() {
        return Err(AppError::new(
            AppErrorKind::IoProcess,
            format!(
                "Failed to create branch '{branch}': {}",
                decoded_trim(&result.stderr)
            ),
        ));
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
) -> Result<Vec<u8>, AppError> {
    match action {
        ExistingBranch::Switch => {
            let result = runner.capture(
                &CommandSpec::new(Program::Git, &["checkout", branch]),
                cwd,
                env,
            )?;
            if !result.outcome.success() {
                return Err(AppError::new(
                    AppErrorKind::IoProcess,
                    format!(
                        "Failed to switch to branch '{branch}': {}",
                        decoded_trim(&result.stderr)
                    ),
                ));
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
                suffix = suffix.checked_add(1).ok_or_else(|| {
                    AppError::new(AppErrorKind::Invariant, "branch suffix counter exhausted")
                })?;
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
) -> Result<(), AppError> {
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
            writeln!(stderr, "{}", decoded(&result.stderr)).map_err(|error| {
                AppError::new(AppErrorKind::IoProcess, "could not write jj failure")
                    .with_source(error)
            })?;
            return Err(AppError::new(
                AppErrorKind::IoProcess,
                "Failed to create new jj change",
            ));
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
) -> Result<Vec<u8>, AppError> {
    let description = format!(
        "{identifier} {title}\n\nLinear-issue: Fixes {identifier}\nLinear-issue-url: {url}"
    );
    let result = runner.capture(
        &CommandSpec::new(Program::Jj, &["describe", "-m", &description]),
        cwd,
        env,
    )?;
    if !result.outcome.success() {
        writeln!(stderr, "{}", decoded(&result.stderr)).map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "could not write jj failure").with_source(error)
        })?;
        return Err(AppError::new(
            AppErrorKind::IoProcess,
            "Failed to set jj description",
        ));
    }
    Ok(format!("✓ Prepared jj change for issue {identifier}\n").into_bytes())
}
pub fn started(mut states: Vec<WorkflowState>) -> Result<WorkflowState, AppError> {
    crate::workflow_states::sort(&mut states)?;
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
    selected.ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            "No 'started' state found in workflow",
        )
    })
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
        ObservedExchangeFailure::Strict(error) => format!("Error: {}", error.display_message()),
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
    let state = started(response.team.states.nodes)
        .map_err(|error| format!("Error: {}", error.display_message()))?;
    let response: UpdateIssueState =
        bulk_error::execute_observed(transport, &update_request(identifier, state.id.inner()))
            .await
            .map_err(post_failure)?;
    // The source intentionally ignores both true and false, but typed decoding is full.
    let _reported_success = response.issue_update.success;
    Ok(format!("✓ Issue state updated to '{}'\n", state.name).into_bytes())
}
