//! `team delete`: delete a team after moving its issues to another team.
use crate::cli::team::TeamDelete;
use crate::client::LinearClient;
use crate::commands::bulk::{self, BulkOutcome, BulkResult, Verb};
use crate::commands::confirm;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::team::{
    DeleteTeam, GetTeamIssuesForMove, MoveIssue, MoveIssueToTeam, MovePageVariables, MoveVariables,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::prompt::Choice;
use crate::refs::{self, team::ResolvedTeam, team::TeamReference};

pub fn run(ctx: &Ctx, args: &TeamDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete team")
}

fn delete(ctx: &Ctx, args: &TeamDelete) -> Result<()> {
    if !args.confirm.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    let scope = ctx.scope()?;
    let source = TeamReference::parse(&args.team, &scope)?;
    let target = args
        .move_issues
        .as_deref()
        .map(|target| TeamReference::parse(target, &scope))
        .transpose()?;
    let client = ctx.client()?;
    let (team, target, issues) = ctx.spin(true, async {
        let team = refs::team::resolve(client, &source).await?;
        let target = match &target {
            Some(target) => Some(refs::team::resolve(client, target).await?),
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
    if !confirm::proceed(ctx, args.confirm.yes, &question)? {
        return Ok(());
    }
    if let Some(target) = &target {
        move_issues(ctx, client, &team, target, &issues)?;
    }
    let result: DeleteTeam = ctx.spin(
        true,
        client.mutate(IdVariables {
            id: team.id.clone(),
        }),
    )?;
    if !result.team_delete.success {
        return Err(Error::new("Linear did not delete the team"));
    }
    ctx.print(outcome::done(
        "Deleted",
        "team",
        &format!("{}: {}", team.key, team.name),
        None,
    ))
}

/// Every issue of the team.
async fn team_issues(client: &LinearClient, team: &ResolvedTeam) -> Result<Vec<MoveIssue>> {
    pagination::collect(None, |after, first| {
        let variables = MovePageVariables {
            team_id: team.id.clone(),
            first,
            after,
        };
        async move {
            let data: GetTeamIssuesForMove = client.query(variables).await?;
            let issues = data
                .team
                .ok_or_else(|| Error::not_found("Team", &team.key))?
                .issues;
            Ok::<Page<MoveIssue>, Error>(Page {
                nodes: issues.nodes,
                page_info: issues.page_info,
            })
        }
    })
    .await
}

/// Asks which team gets the issues of the team being deleted.
fn choose_target(
    ctx: &Ctx,
    client: &LinearClient,
    team: &ResolvedTeam,
    count: usize,
) -> Result<ResolvedTeam> {
    if !ctx.interactive() {
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
    let mut teams = ctx.spin(true, refs::team::fetch_all(client))?;
    teams.retain(|other| other.id != team.id);
    if teams.is_empty() {
        return Err(Error::new("There is no other team to move the issues to"));
    }
    let choices = teams
        .into_iter()
        .map(|team| Choice::new(format!("{} ({})", team.name, team.key), team))
        .collect();
    ctx.prompter()?
        .select("Select a team to move issues to:", choices)
}

/// Moves every issue to `target`. Any failure stops the delete, after
/// reporting which issues did not move.
fn move_issues(
    ctx: &Ctx,
    client: &LinearClient,
    team: &ResolvedTeam,
    target: &ResolvedTeam,
    issues: &[MoveIssue],
) -> Result<()> {
    ctx.print(format!(
        "Moving {} issue(s) to {}...\n",
        issues.len(),
        target.key
    ))?;
    let results = bulk::run(ctx, issues.iter().collect(), |issue| async move {
        let variables = MoveVariables {
            id: issue.id.inner().to_owned(),
            team_id: target.id.clone(),
        };
        let outcome = match client.mutate::<MoveIssueToTeam, _>(variables).await {
            Ok(result) if result.issue_update.success => BulkOutcome::Succeeded,
            Ok(_) => BulkOutcome::Failed("Linear did not move the issue".to_owned()),
            Err(error) => BulkOutcome::Failed(Error::from(error).to_string()),
        };
        BulkResult {
            id: issue.identifier.clone(),
            name: None,
            outcome,
        }
    })?;
    let moved = Verb {
        present: "move",
        past: "moved",
    };
    ctx.print(bulk::summary(&results, "issue", moved).0)?;
    let failed = results.iter().filter(|row| !row.succeeded()).count();
    if failed == 0 {
        return Ok(());
    }
    Err(Error::new(format!(
        "{failed} issue(s) could not be moved, so team {} was not deleted",
        team.key
    ))
    .with_hint("Run the command again to retry."))
}
