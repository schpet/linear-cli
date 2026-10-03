//! `issue mine` (alias `issue list`): your issues in one team.
use std::process::{Command, Stdio};
use std::time::SystemTime;

use crate::cli::issue::IssueMine;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue_read::IssueFilter;
use crate::refs::is_linear_uuid;

use super::read;

/// Linear's "assigned to me" view filter, base64 JSON.
const ASSIGNED_TO_ME: &str =
    "eyJhbmQiOlt7ImFzc2lnbmVlIjp7Im9yIjpbeyJpc01lIjp7ImVxIjp0cnVlfX1dfX1dfQ";

pub fn run(ctx: &Ctx, args: &IssueMine) -> Result<()> {
    list(ctx, args).context("Failed to list issues")
}

fn list(ctx: &Ctx, args: &IssueMine) -> Result<()> {
    let removed = if args.assignee.is_some() {
        Some("--assignee")
    } else if args.all_assignees {
        Some("--all-assignees")
    } else if args.unassigned {
        Some("--unassigned")
    } else {
        None
    };
    if let Some(flag) = removed {
        return Err(
            Error::new(format!("{flag} has been removed from 'issue mine'")).with_hint(format!(
                "Use 'linear issue query {flag}' for assignee filtering."
            )),
        );
    }
    if args.team.is_none() && configured_team_key(ctx.options()).is_none() {
        return Err(no_team(ctx));
    }
    if args
        .milestone
        .as_deref()
        .is_some_and(|milestone| !is_linear_uuid(milestone))
        && args.project.is_none()
    {
        return Err(Error::new("--milestone requires --project to be set").with_hint(
            "Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly.",
        ));
    }
    let explicit = args
        .team
        .as_deref()
        .map(|team| read::resolve_team(ctx, ctx.client()?, team))
        .transpose()?;
    let team = match &explicit {
        Some(team) => team.key.clone(),
        None => configured_team_key(ctx.options()).expect("checked above"),
    };
    if args.web || args.app {
        let path = format!("team/{team}/active?filter={ASSIGNED_TO_ME}");
        return ctx.open_in_linear(&path, args.app);
    }
    let client = ctx.client()?;
    let priority = read::priority_sort(ctx, args.sort);
    let project = read::resolve_project(ctx, client, args.project.as_deref())?;
    let cycle = read::resolve_cycle(
        ctx,
        client,
        args.cycle.as_deref(),
        Some(&team),
        explicit.as_ref().map(|team| team.id.as_str()),
    )?;
    let milestone = args
        .milestone
        .as_deref()
        .map(|milestone| ctx.block_on(read::milestone_id(client, milestone, project.as_deref())))
        .transpose()?;
    let rows = ctx.spin(true, async {
        let teams = std::slice::from_ref(&team);
        let mut filter = IssueFilter {
            team: Some(read::team_filter(teams, true)),
            state: if args.all_states {
                None
            } else {
                read::state_filter(client, &args.state, Some(teams)).await?
            },
            assignee: read::assignee_filter(client, None, false, true).await?,
            ..Default::default()
        };
        read::entity_filters(
            &mut filter,
            project,
            args.project_label.as_deref(),
            cycle,
            milestone,
            &args.label,
        );
        read::apply_dates(&mut filter, args.created_after, args.updated_after);
        read::mine(client, filter, priority, args.limit.max()).await
    })?;
    let table = read::table(&rows, false, false, SystemTime::now());
    read::print_table(ctx, &table, !args.no_pager)
}

fn no_team(ctx: &Ctx) -> Error {
    let in_repository = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(ctx.cwd())
        .stdin(Stdio::null())
        .envs(ctx.config().child_env.iter())
        .output()
        .is_ok_and(|output| output.status.success());
    let hint = if in_repository {
        "Use --team <key, name, or ID> to specify a team, or run `linear config` to link this repository to a team."
    } else {
        "Use --team <key, name, or ID> to specify a team."
    };
    Error::new("No default team configured and no team scope provided").with_hint(hint)
}
