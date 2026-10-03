//! `issue query`: issues matching structured filters or a full-text search.
use std::time::SystemTime;

use crate::cli::issue::IssueQuery;
use crate::commands::json;
use crate::commands::team_key::configured_team_key;
use crate::config::OptionSource;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue_read::{IssueFilter, ListedIssue};
use crate::refs::{is_linear_uuid, team::ResolvedTeam};

use super::read;

pub fn run(ctx: &Ctx, args: &IssueQuery) -> Result<()> {
    query(ctx, args).context("Failed to query issues")
}

enum Output {
    Json(Vec<u8>),
    Table(crate::commands::table::Table),
}

/// Which teams the query covers.
struct Scope {
    /// Team keys; `None` for every team.
    keys: Option<Vec<String>>,
    /// The ID of the one team named by `--team`, when exactly one was.
    team_id: Option<String>,
}

impl Scope {
    fn several(&self) -> bool {
        self.keys.as_ref().is_none_or(|keys| keys.len() > 1)
    }
}

fn query(ctx: &Ctx, args: &IssueQuery) -> Result<()> {
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
    let search = args.search.as_deref().map(str::trim);
    if search.is_some_and(str::is_empty) {
        return Err(Error::new("--search term cannot be empty"));
    }
    let default_team = if args.all_teams || !args.team.is_empty() {
        None
    } else {
        Some(default_team(ctx)?)
    };
    let client = ctx.client()?;
    let scope = if args.all_teams {
        Scope {
            keys: None,
            team_id: None,
        }
    } else if let Some(team) = default_team {
        Scope {
            keys: Some(vec![team]),
            team_id: None,
        }
    } else {
        explicit_teams(ctx, args)?
    };
    if args.cycle.is_some() && scope.several() {
        return Err(Error::new("--cycle requires a single team scope").with_hint(
            "Use --team <key, name, or ID> to specify exactly one team when filtering by cycle.",
        ));
    }
    let state = ctx.block_on(read::state_filter(
        client,
        &args.state,
        scope.keys.as_deref(),
    ))?;
    let project = read::resolve_project(ctx, client, args.project.as_deref())?;
    let cycle = read::resolve_cycle(
        ctx,
        client,
        args.cycle.as_deref(),
        scope
            .keys
            .as_ref()
            .and_then(|keys| keys.first())
            .map(String::as_str),
        scope.team_id.as_deref(),
    )?;
    let milestone = args
        .milestone
        .as_deref()
        .map(|milestone| ctx.block_on(read::milestone_id(client, milestone, project.as_deref())))
        .transpose()?;
    let priority = read::priority_sort(ctx, args.sort);
    let show_team = scope.several();
    let show_assignee = args.assignee.is_none() && !args.unassigned;
    let output = ctx.spin(!args.json, async {
        let mut filter = IssueFilter {
            team: scope.keys.as_deref().map(read::query_team_filter),
            state,
            assignee: read::assignee_filter(
                client,
                args.assignee.as_deref(),
                args.unassigned,
                false,
            )
            .await?,
            ..Default::default()
        };
        read::entity_filters(
            &mut filter,
            project,
            args.project_label.as_deref(),
            cycle,
            // Search does not filter by milestone.
            if search.is_some() { None } else { milestone },
            &args.label,
        );
        read::apply_dates(&mut filter, args.created_after, args.updated_after);
        // A filter without any condition is sent as no filter at all.
        let empty = serde_json::to_value(&filter)
            .expect("filters always serialize")
            .as_object()
            .is_some_and(serde_json::Map::is_empty);
        let filter = (!empty).then_some(filter);
        let rows = match search {
            Some(term) => {
                let data = read::search(
                    client,
                    filter,
                    term.to_owned(),
                    args.limit.max(),
                    args.include_archived,
                    args.search_comments,
                )
                .await?;
                if args.json {
                    return Ok(Output::Json(json::render(&data)));
                }
                data.into_iter().map(ListedIssue::from).collect::<Vec<_>>()
            }
            None => {
                let data = read::query(
                    client,
                    filter,
                    priority,
                    args.limit.max(),
                    args.include_archived,
                )
                .await?;
                if args.json {
                    return Ok(Output::Json(json::render(&data)));
                }
                data
            }
        };
        Ok::<_, Error>(Output::Table(read::table(
            &rows,
            show_team,
            show_assignee,
            SystemTime::now(),
        )))
    })?;
    match output {
        Output::Json(json) => ctx.print(json),
        Output::Table(table) => read::print_table(ctx, &table, !args.no_pager),
    }
}

/// The configured team, noting it on stderr when it came from the environment
/// or global config rather than this repository.
fn default_team(ctx: &Ctx) -> Result<String> {
    let team = configured_team_key(ctx.options()).ok_or_else(|| {
        Error::new("No default team configured and no team scope provided").with_hint(
            "Use --team <key, name, or ID> to specify a team, or --all-teams to query the whole workspace.",
        )
    })?;
    let implicit = ctx.options().team_id().is_some_and(|value| {
        matches!(
            value.source(),
            OptionSource::Env | OptionSource::GlobalConfig { .. }
        )
    });
    if implicit {
        ctx.eprint(format!(
            "Note: using default team {team}. Pass --team <key, name, or ID> or --all-teams to be explicit.\n"
        ))?;
    }
    Ok(team)
}

/// Every `--team`, resolved, without duplicates.
fn explicit_teams(ctx: &Ctx, args: &IssueQuery) -> Result<Scope> {
    let mut teams: Vec<ResolvedTeam> = vec![];
    for reference in &args.team {
        let team = read::resolve_team(ctx, ctx.client()?, reference)?;
        if !teams.iter().any(|known| known.id == team.id) {
            teams.push(team);
        }
    }
    let team_id = match teams.as_slice() {
        [only] => Some(only.id.clone()),
        _ => None,
    };
    Ok(Scope {
        keys: Some(teams.into_iter().map(|team| team.key).collect()),
        team_id,
    })
}
