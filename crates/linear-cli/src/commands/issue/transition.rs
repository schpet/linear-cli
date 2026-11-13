//! `issue start`, `issue complete` and `issue cancel`: move an issue to the
//! status of a type, the way Linear's own automations do. Nothing here
//! touches the repository.
use crate::cli::issue::IssueTransition;
use crate::cli::values::IssueRef;
use crate::client::LinearClient;
use crate::commands::json::quoted;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue::{
    IssueTransitionDetails, UpdateIssueState, UpdateIssueStateVariables,
};
use crate::graphql::operations::team::WorkflowState;
use crate::platform::terminal_text::single_line;

/// The status move a command asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verb {
    Start,
    Complete,
    Cancel,
}

impl Verb {
    /// The workflow state type the verb moves an issue to.
    fn target(self) -> &'static str {
        match self {
            Self::Start => "started",
            Self::Complete => "completed",
            Self::Cancel => "canceled",
        }
    }

    fn failure(self) -> &'static str {
        match self {
            Self::Start => "Failed to start issue",
            Self::Complete => "Failed to complete issue",
            Self::Cancel => "Failed to cancel issue",
        }
    }
}

/// The types of workflow state Linear has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Triage,
    Backlog,
    Unstarted,
    Started,
    Completed,
    Canceled,
    Duplicate,
}

impl Kind {
    fn parse(state_type: &str) -> Option<Self> {
        Some(match state_type {
            "triage" => Self::Triage,
            "backlog" => Self::Backlog,
            "unstarted" => Self::Unstarted,
            "started" => Self::Started,
            "completed" => Self::Completed,
            "canceled" => Self::Canceled,
            "duplicate" => Self::Duplicate,
            _ => return None,
        })
    }
}

/// Why an issue stays in the status it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Settled {
    /// It is already in a status of the type asked for.
    Already,
    /// It is completed, canceled or a duplicate, which `start` does not reopen.
    Closed,
}

/// What happened to the issue's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Moved { to: String },
    Unchanged { why: Settled, state: String },
}

/// Whether an issue in a state of type `current` stays there when `verb` is
/// asked of it. `start` leaves started and closed issues alone, as Linear's
/// "Move issue to started status" does; the others only skip an issue that
/// is already where they would put it.
fn settled(verb: Verb, current: Kind) -> Option<Settled> {
    match (verb, current) {
        (Verb::Start, Kind::Started)
        | (Verb::Complete, Kind::Completed)
        | (Verb::Cancel, Kind::Canceled) => Some(Settled::Already),
        (Verb::Start, Kind::Completed | Kind::Canceled | Kind::Duplicate) => Some(Settled::Closed),
        (Verb::Start, Kind::Triage | Kind::Backlog | Kind::Unstarted)
        | (
            Verb::Complete,
            Kind::Triage
            | Kind::Backlog
            | Kind::Unstarted
            | Kind::Started
            | Kind::Canceled
            | Kind::Duplicate,
        )
        | (
            Verb::Cancel,
            Kind::Triage
            | Kind::Backlog
            | Kind::Unstarted
            | Kind::Started
            | Kind::Completed
            | Kind::Duplicate,
        ) => None,
    }
}

/// The state `verb` moves an issue of `team` to. Starting takes the first
/// started status, as Linear does. Linear has no rule for which completed or
/// canceled status to use, so those need the team to have exactly one.
fn destination(
    verb: Verb,
    team: &str,
    identifier: &str,
    states: Vec<WorkflowState>,
) -> Result<WorkflowState> {
    let kind = verb.target();
    let mut candidates: Vec<WorkflowState> = states
        .into_iter()
        .filter(|state| state.state_type == kind)
        .collect();
    candidates.sort_by(|a, b| a.position.get().total_cmp(&b.position.get()));
    let several = candidates.len() > 1;
    let names = candidates
        .iter()
        .map(|state| quoted(&state.name))
        .collect::<Vec<_>>()
        .join(", ");
    let first = candidates.into_iter().next().ok_or_else(|| {
        Error::new(format!("Team {team} has no {kind} status")).with_hint(format!(
            "Run `linear team states {team}` to see its statuses, then `linear issue update {identifier} --state <state>`."
        ))
    })?;
    match verb {
        Verb::Start => Ok(first),
        Verb::Complete | Verb::Cancel if several => Err(Error::new(format!(
            "Team {team} has several {kind} statuses: {names}"
        ))
        .with_hint(format!(
            "Choose one with `linear issue update {identifier} --state <name>`."
        ))),
        Verb::Complete | Verb::Cancel => Ok(first),
    }
}

/// Moves `issue` as `verb` asks, unless it is settled where it is.
pub(super) async fn apply(
    client: &LinearClient,
    verb: Verb,
    issue: &IssueTransitionDetails,
) -> Result<Outcome> {
    let identifier = &issue.identifier;
    let current = Kind::parse(&issue.state.state_type).ok_or_else(|| {
        Error::new(format!(
            "Issue {identifier} is in a status of unknown type {}",
            quoted(&issue.state.state_type)
        ))
        .with_hint(format!(
            "Set the status with `linear issue update {identifier} --state <state>`."
        ))
    })?;
    if let Some(why) = settled(verb, current) {
        return Ok(Outcome::Unchanged {
            why,
            state: issue.state.name.clone(),
        });
    }
    let team = &issue.team.key;
    let states = crate::refs::workflow_states::fetch(client, team.clone()).await?;
    let state = destination(verb, team, identifier, states)?;
    let response: UpdateIssueState = client
        .mutate(UpdateIssueStateVariables {
            issue_id: identifier.clone(),
            state_id: state.id.inner().to_owned(),
        })
        .await?;
    if !response.issue_update.success {
        return Err(Error::new("Linear did not update the issue"));
    }
    Ok(Outcome::Moved { to: state.name })
}

/// The line that reports `outcome` on stdout.
pub(super) fn report(verb: Verb, identifier: &str, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Moved { to } => format!("✓ Moved issue {identifier} to {}\n", single_line(to)),
        Outcome::Unchanged {
            why: Settled::Already,
            state,
        } => format!(
            "✓ Issue {identifier} is already {} ({})\n",
            verb.target(),
            single_line(state)
        ),
        Outcome::Unchanged {
            why: Settled::Closed,
            state,
        } => format!(
            "Issue {identifier} is closed ({}), so it was not started. To reopen it, run `linear issue update {identifier} --state <state>`.\n",
            single_line(state)
        ),
    }
}

/// Fetches the issue `identifier` names, moves it, and reports the result.
pub(super) fn run_for(ctx: &Ctx, verb: Verb, identifier: String) -> Result<()> {
    let client = ctx.client()?;
    let (issue, outcome) = ctx.spin(true, async {
        let issue = super::details::fetch_for_transition(client, identifier).await?;
        let outcome = apply(client, verb, &issue).await?;
        Ok::<_, Error>((issue, outcome))
    })?;
    ctx.print(report(verb, &issue.identifier, &outcome))
}

/// `issue complete` and `issue cancel`.
pub fn run(ctx: &Ctx, verb: Verb, args: &IssueTransition) -> Result<()> {
    named(ctx, verb, &args.issue_id).context(verb.failure())
}

fn named(ctx: &Ctx, verb: Verb, input: &IssueRef) -> Result<()> {
    run_for(ctx, verb, super::require(ctx, input)?)
}

#[cfg(test)]
mod tests;
