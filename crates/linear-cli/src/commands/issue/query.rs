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

use super::{filter, list_view, read};

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
    let filters = &args.filters;
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
    let search = args.search.as_deref().map(str::trim);
    if search.is_some_and(str::is_empty) {
        return Err(Error::invalid("--search term cannot be empty"));
    }
    let default_team = if args.all_teams || !args.team.is_empty() {
        None
    } else {
        Some(default_team(ctx)?)
    };
    let client = ctx.client()?;
    // One spinner covers every lookup and the query; prompts hide it.
    let spinner = ctx.spinner(true, "");
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
    if filters.cycle.is_some() && scope.several() {
        return Err(Error::new("--cycle requires a single team scope").with_hint(
            "Use --team <key, name, or ID> to specify exactly one team when filtering by cycle.",
        ));
    }
    let state = ctx.block_on(filter::state_filter(
        client,
        &args.state,
        scope.keys.as_deref(),
    ))?;
    let project = filter::resolve_project(ctx, client, filters.project.as_deref())?;
    let cycle = filter::resolve_cycle(
        ctx,
        client,
        filters.cycle.as_deref(),
        scope
            .keys
            .as_ref()
            .and_then(|keys| keys.first())
            .map(String::as_str),
        scope.team_id.as_deref(),
    )?;
    let milestone = filters
        .milestone
        .as_deref()
        .map(|milestone| ctx.block_on(filter::milestone_id(client, milestone, project.as_deref())))
        .transpose()?;
    let sort = ctx.options().issue_sort(args.sort).0;
    let show_team = scope.several();
    let show_assignee = filters.assignee.is_none() && !filters.unassigned;
    let output = ctx.block_on(async {
        filter::check_labels(client, &filters.label, scope.keys.as_deref()).await?;
        let mut filter = IssueFilter {
            team: scope.keys.as_deref().map(filter::query_team_filter),
            state,
            assignee: filter::assignee_filter(
                client,
                filters.assignee.as_ref(),
                filters.unassigned,
                false,
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
                    filters.limit.max(),
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
                    sort,
                    filters.limit.max(),
                    args.include_archived,
                )
                .await?;
                if args.json {
                    return Ok(Output::Json(json::render(&data)));
                }
                data
            }
        };
        Ok::<_, Error>(Output::Table(list_view::table(
            &rows,
            show_team,
            show_assignee,
            SystemTime::now(),
        )))
    })?;
    drop(spinner);
    match output {
        Output::Json(json) => ctx.print(json),
        Output::Table(table) => list_view::print_table(ctx, &table, !args.no_pager),
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
    let implicit = ctx.options().team_key_source().is_some_and(|source| {
        matches!(
            source,
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
        let team = filter::resolve_team(ctx, ctx.client()?, reference)?;
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
