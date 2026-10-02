//! `team delete`: delete a team after moving its issues to another team.
use cynic::{MutationBuilder, QueryBuilder};

use crate::cli::team::TeamDelete;
use crate::commands::confirm;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_delete::{
    DeleteTeam, GetTeamIssuesForMove, IdVariables, MoveIssue, MoveIssueToTeam, MovePageVariables,
    MoveVariables,
};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome};
use crate::refs::{
    ResolvedTeam, fetch_all_teams_with_transport, prepare_team_lookup, resolve_team_with_transport,
};

pub fn run(ctx: &Ctx, args: &TeamDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete team")
}

fn delete(ctx: &Ctx, args: &TeamDelete) -> Result<()> {
    if !args.force {
        ctx.require_tty("--force")?;
    }
    let scope = ctx.scope()?;
    let source = prepare_team_lookup(&args.team, &scope)?;
    let target = args
        .move_issues
        .as_deref()
        .map(|target| prepare_team_lookup(target, &scope))
        .transpose()?;
    let client = ctx.client()?;
    let (team, target, issues) = ctx.spin(true, async {
        let team = resolve_team_with_transport(&source, client).await?;
        let target = match &target {
            Some(target) => Some(resolve_team_with_transport(target, client).await?),
            None => None,
        };
        if target.as_ref().is_some_and(|target| target.id == team.id) {
            return Err(Error::new("Cannot move issues to the team being deleted"));
        }
        let issues = team_issues(client, &team).await?;
        Ok((team, target, issues))
    })?;
    let target = match target {
        _ if issues.is_empty() => None,
        Some(target) => Some(target),
        None => Some(choose_target(ctx, client, &team, issues.len())?),
    };
    let question = match &target {
        Some(target) => format!(
            "Move {} issue(s) to {} and delete team \"{}: {}\"?",
            issues.len(),
            target.key,
            team.key,
            team.name
        ),
        None => format!(
            "Are you sure you want to delete team \"{}: {}\"?",
            team.key, team.name
        ),
    };
    if !confirm::deletion(ctx, args.force, &question)? {
        return Ok(());
    }
    if let Some(target) = &target {
        move_issues(ctx, client, &team, target, &issues)?;
    }
    let request = GraphQlRequest::with_variables(DeleteTeam::build(IdVariables {
        id: team.id.clone(),
    }));
    let result: DeleteTeam = ctx.spin(true, client.execute(&request))?;
    if !result.team_delete.success {
        return Err(Error::new("Linear did not delete the team"));
    }
    ctx.print(format!("✓ Deleted team {}: {}\n", team.key, team.name))
}

/// Every issue of the team.
async fn team_issues(client: &GraphQlTransport, team: &ResolvedTeam) -> Result<Vec<MoveIssue>> {
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetTeamIssuesForMove::build(MovePageVariables {
                team_id: team.id.clone(),
                first: 100,
                after,
            }));
        async move {
            let data: GetTeamIssuesForMove = client.execute(&request).await?;
            let issues = data
                .team
                .ok_or_else(|| Error::not_found("Team", &team.key))?
                .issues;
            Ok::<Page<MoveIssue>, Error>(Page {
                nodes: issues.nodes,
                page_info: issues.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more team issues but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a team issue pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    Ok(result.nodes)
}

/// Asks which team gets the issues of the team being deleted.
fn choose_target(
    ctx: &Ctx,
    client: &GraphQlTransport,
    team: &ResolvedTeam,
    count: usize,
) -> Result<ResolvedTeam> {
    if !ctx.stdin_tty() {
        return Err(Error::new(format!(
            "Team {} has {count} issue(s) to move before it can be deleted",
            team.key
        ))
        .with_hint("Pass --move-issues <team> to choose where they go."));
    }
    ctx.print(format!(
        "Team {} ({}) has {count} issue(s). They must move to another team before it is deleted.\n",
        team.key, team.name
    ))?;
    let mut teams = ctx.spin(true, fetch_all_teams_with_transport(client))?;
    teams.retain(|other| other.id != team.id);
    if teams.is_empty() {
        return Err(Error::new("There is no other team to move the issues to"));
    }
    let options: Vec<_> = teams
        .iter()
        .enumerate()
        .map(|(index, team)| PlainOption {
            label: format!("{} ({})", team.name, team.key),
            value: index.to_string(),
            script_token: team.key.clone(),
        })
        .collect();
    let mut session = ctx.prompts()?;
    let selected = session.select(&PlainSelect {
        message: "Select a team to move issues to:",
        options: &options,
        default_index: 0,
        default_hint: None,
    });
    let index = match session.finish_result(selected)? {
        PromptOutcome::Submitted(index) => index,
        PromptOutcome::Interrupted => return Err(Error::cancelled()),
        PromptOutcome::EndOfInput => {
            return Err(Error::new("Input ended before a team was selected"));
        }
    };
    let index: usize = index
        .parse()
        .expect("the team prompt answers with an option index");
    Ok(teams.swap_remove(index))
}

/// Moves every issue to `target`, one at a time. Any failure stops the
/// delete, after reporting which issues moved and which did not.
fn move_issues(
    ctx: &Ctx,
    client: &GraphQlTransport,
    team: &ResolvedTeam,
    target: &ResolvedTeam,
    issues: &[MoveIssue],
) -> Result<()> {
    let message = format!("Moving {} issue(s) to {}...", issues.len(), target.key);
    let failures = ctx.spin_with(&message, async {
        let mut failures = Vec::new();
        for issue in issues {
            let request = GraphQlRequest::with_variables(MoveIssueToTeam::build(MoveVariables {
                id: issue.id.inner().to_owned(),
                team_id: target.id.clone(),
            }));
            let reason = match client.execute::<MoveIssueToTeam, _>(&request).await {
                Ok(result) if result.issue_update.success => continue,
                Ok(_) => "Linear did not move the issue".to_owned(),
                Err(error) => Error::from(error).to_string(),
            };
            failures.push((issue.identifier.as_str(), reason));
        }
        failures
    });
    let moved = issues.len() - failures.len();
    if failures.is_empty() {
        return ctx.print(format!("✓ Moved {moved} issue(s) to {}\n", target.key));
    }
    let mut report = format!(
        "Moved {moved} of {} issue(s) to {}. These could not be moved:\n",
        issues.len(),
        target.key
    );
    for (identifier, reason) in &failures {
        report.push_str(&format!("  - {identifier}: {reason}\n"));
    }
    ctx.print(report)?;
    Err(Error::new(format!(
        "{} issue(s) could not be moved, so team {} was not deleted",
        failures.len(),
        team.key
    ))
    .with_hint("Run the command again to retry."))
}
