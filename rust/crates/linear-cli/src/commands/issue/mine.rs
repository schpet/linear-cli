//! `issue list` (alias `issue mine`): one team's issues, assigned to you by
//! default.
use std::time::SystemTime;

use crate::cli::issue::IssueList;
use crate::commands::team_key::{configured_team_key, no_team};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue_read::IssueFilter;
use crate::refs::is_linear_uuid;

use super::{filter, list_view, read};

/// Linear's "assigned to me" view filter, base64 JSON.
const ASSIGNED_TO_ME: &str =
    "eyJhbmQiOlt7ImFzc2lnbmVlIjp7Im9yIjpbeyJpc01lIjp7ImVxIjp0cnVlfX1dfX1dfQ";

pub fn run(ctx: &Ctx, args: &IssueList) -> Result<()> {
    list(ctx, args).context("Failed to list issues")
}

fn list(ctx: &Ctx, args: &IssueList) -> Result<()> {
    let filters = &args.filters;
    let mine = filters.assignee.is_none() && !filters.unassigned && !filters.all_assignees;
    if args.team.is_none() && configured_team_key(ctx.options()).is_none() {
        return Err(no_team());
    }
    if filters
        .milestone
        .as_deref()
        .is_some_and(|milestone| !is_linear_uuid(milestone))
        && filters.project.is_none()
    {
        return Err(Error::new("--milestone requires --project to be set").with_hint(
            "Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly.",
        ));
    }
    let explicit = args
        .team
        .as_deref()
        .map(|team| filter::resolve_team(ctx, ctx.client()?, team))
        .transpose()?;
    let team = match &explicit {
        Some(team) => team.key.clone(),
        None => configured_team_key(ctx.options()).expect("checked above"),
    };
    if args.web || args.app {
        let path = if mine {
            format!("team/{team}/active?filter={ASSIGNED_TO_ME}")
        } else {
            format!("team/{team}/active")
        };
        return ctx.open_in_linear(&path, args.app);
    }
    let client = ctx.client()?;
    let priority = read::priority_sort(ctx, args.sort);
    let project = filter::resolve_project(ctx, client, filters.project.as_deref())?;
    let cycle = filter::resolve_cycle(
        ctx,
        client,
        filters.cycle.as_deref(),
        Some(&team),
        explicit.as_ref().map(|team| team.id.as_str()),
    )?;
    let milestone = filters
        .milestone
        .as_deref()
        .map(|milestone| ctx.block_on(filter::milestone_id(client, milestone, project.as_deref())))
        .transpose()?;
    let rows = ctx.spin(true, async {
        let teams = std::slice::from_ref(&team);
        let mut filter = IssueFilter {
            team: Some(filter::team_filter(teams, true)),
            state: if args.all_states {
                None
            } else {
                filter::state_filter(client, &args.state, Some(teams)).await?
            },
            assignee: filter::assignee_filter(
                client,
                filters.assignee.as_deref(),
                filters.unassigned,
                mine,
            )
            .await?,
            ..Default::default()
        };
        filter::entity_filters(
            &mut filter,
            project,
            filters.project_label.as_deref(),
            cycle,
            milestone,
            &filters.label,
        );
        filter::apply_dates(&mut filter, filters.created_after, filters.updated_after);
        read::mine(client, filter, priority, filters.limit.max()).await
    })?;
    let table = list_view::table(&rows, false, filters.all_assignees, SystemTime::now());
    list_view::print_table(ctx, &table, !args.no_pager)
}
