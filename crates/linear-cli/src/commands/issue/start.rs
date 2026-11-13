//! `issue start`: move an issue to its team's first started status. The
//! picker it shares with `issue git start` and `issue jj start` lives here.
use crate::{
    cli::{
        issue::{IssueStart, StartTarget},
        values::IssueRef,
    },
    commands::{
        issue::{
            list_view, read as issue_read,
            transition::{self, Verb},
        },
        team_key::configured_team_key,
    },
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue_read::*,
    platform::prompt::Choice,
    refs::{self, prepare_issue_reference, team::TeamReference},
};

pub fn run(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    start(ctx, args).context("Failed to start issue")
}

fn start(ctx: &Ctx, args: &IssueStart) -> Result<()> {
    if args.branch.is_some() || args.from_ref.is_some() {
        return Err(
            Error::invalid("--branch and --from-ref moved to `issue git start`").with_hint(
                "`issue start` only changes the issue's status. Run `linear issue git start <ISSUE> --branch <NAME> --from-ref <REF>` to create a branch too.",
            ),
        );
    }
    let identifier = target(ctx, &args.target)?;
    transition::run_for(ctx, Verb::Start, identifier)
}

/// The issue to start: the one named, or one picked from the team's
/// unstarted issues.
pub(super) fn target(ctx: &Ctx, args: &StartTarget) -> Result<String> {
    if args.issue_id.is_none() && !ctx.interactive() {
        return Err(ctx.missing_value("No issue to start", "an issue ID"));
    }
    let named = match &args.issue_id {
        // A reference reads a full identifier, which needs no team.
        Some(reference @ (IssueRef::Git | IssueRef::Jj)) => return super::require(ctx, reference),
        Some(IssueRef::Named(input)) => Some(input),
        None => None,
    };
    let team = match &args.team {
        Some(team) => {
            let reference = TeamReference::parse(team, &ctx.scope()?)?;
            Some(
                ctx.spin(true, refs::team::resolve(ctx.client()?, &reference))?
                    .key,
            )
        }
        None => configured_team_key(ctx.options()),
    };
    match named {
        Some(input) => prepare_issue_reference(input, team.as_deref(), &ctx.scope()?)?
            .ok_or_else(|| Error::new(format!("Not an issue ID: {input}")).with_hint(
                "Pass an issue ID like ENG-123, an issue URL, or an issue number in the configured team or --team.",
            )),
        None => {
            let team = team.ok_or_else(|| {
                Error::invalid("No team to pick an issue from").with_hint(
                    "Pass an issue ID or --team, or run `linear config` to set a default team.",
                )
            })?;
            pick(ctx, &team, args)
        }
    }
}

/// Asks which of the team's unstarted issues to start.
fn pick(ctx: &Ctx, team: &str, args: &StartTarget) -> Result<String> {
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
        .select("Issue to start:", choices(&issues, team, args)?)
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
fn choices(issues: &[ListedIssue], team: &str, args: &StartTarget) -> Result<Vec<Choice<String>>> {
    if issues.is_empty() {
        return Err(no_issues(team, args));
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
/// The error for a picker with nothing to offer, naming the filter it used.
fn no_issues(team: &str, args: &StartTarget) -> Error {
    if args.all_assignees {
        Error::new(format!("{team} has no unstarted issues"))
    } else if args.unassigned {
        Error::new(format!("{team} has no unassigned unstarted issues"))
    } else {
        Error::new(format!("{team} has no unstarted issues assigned to you"))
            .with_hint("Pass -A to choose from every assignee's issues, or -U for unassigned ones.")
    }
}

#[cfg(test)]
mod tests;
