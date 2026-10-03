//! Team-reference preparation and typed lookup, without command context or
//! credential or client construction.

use std::future::Future;

use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::operations::team::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables, TeamNode,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::collation;

use super::uuid::is_linear_uuid;
use super::workspace::{WorkspaceScope, expect_team_url};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedTeamLookup {
    original: String,
    lookup: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTeam {
    pub id: String,
    pub key: String,
    pub name: String,
}

impl From<TeamNode> for ResolvedTeam {
    fn from(node: TeamNode) -> Self {
        Self {
            id: node.id.into_inner(),
            key: node.key,
            name: node.name,
        }
    }
}

/// Prepare before building a client: URL and workspace errors have priority.
pub fn prepare_team_lookup(
    original: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<PreparedTeamLookup, Error> {
    if original.trim().is_empty() {
        return Err(Error::new("Team reference is empty")
            .with_hint("Pass a team key, name, or ID, e.g. --team ENG."));
    }
    let lookup = expect_team_url(original, scope)?.unwrap_or_else(|| original.to_owned());
    Ok(PreparedTeamLookup {
        original: original.to_owned(),
        lookup,
    })
}

/// One first-page `ResolveTeam` request. An absent result is not an error here.
pub async fn find_team<F, Fut>(
    prepared: &PreparedTeamLookup,
    fetch: F,
) -> Result<Option<ResolvedTeam>, Error>
where
    F: FnOnce(ResolveTeamVariables) -> Fut,
    Fut: Future<Output = Result<ResolveTeam, Error>>,
{
    let is_uuid = is_linear_uuid(&prepared.lookup);
    let result = fetch(ResolveTeamVariables {
        reference: prepared.lookup.clone(),
        id: is_uuid.then(|| cynic::Id::new(prepared.lookup.clone())),
        is_uuid,
    })
    .await?;
    let wanted = prepared.lookup.to_lowercase();
    let candidates: Vec<ResolvedTeam> = result
        .teams
        .nodes
        .into_iter()
        .map(ResolvedTeam::from)
        .collect();
    if let Some(team) = candidates
        .iter()
        .find(|team| team.key.to_lowercase() == wanted)
    {
        return Ok(Some(team.clone()));
    }
    if let Some(team) = result
        .teamById
        .and_then(|teams| teams.nodes.into_iter().next())
    {
        return Ok(Some(team.into()));
    }
    let by_name: Vec<_> = candidates
        .into_iter()
        .filter(|team| team.name.to_lowercase() == wanted)
        .collect();
    if by_name.len() > 1 {
        return Err(Error::new(format!(
            "Team name \"{}\" is ambiguous: {}",
            prepared.lookup,
            by_name
                .iter()
                .map(|team| format!("{} ({})", team.key, team.name))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .with_hint("Use the team key instead of the name."));
    }
    Ok(by_name.into_iter().next())
}

/// Resolve a key, name or UUID, fetching every team only when the first query
/// misses. All request and decode failures pass through without command context.
pub async fn resolve_team<ResolveFetch, ResolveFuture, AllFetch, AllFuture>(
    prepared: &PreparedTeamLookup,
    resolve_fetch: ResolveFetch,
    all_fetch: AllFetch,
) -> Result<ResolvedTeam, Error>
where
    ResolveFetch: FnOnce(ResolveTeamVariables) -> ResolveFuture,
    ResolveFuture: Future<Output = Result<ResolveTeam, Error>>,
    AllFetch: FnMut(GetAllTeamsVariables) -> AllFuture,
    AllFuture: Future<Output = Result<GetAllTeams, Error>>,
{
    if let Some(team) = find_team(prepared, resolve_fetch).await? {
        return Ok(team);
    }

    let mut teams = fetch_all_teams(all_fetch).await?;

    let suggestion = if teams.is_empty() {
        "This workspace has no teams you can access.".to_owned()
    } else {
        teams.sort_by(|left, right| {
            collation::compare(&left.key, &right.key).then_with(|| {
                collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
            })
        });
        format!(
            "Valid team keys: {}. Run `linear team list` to see all teams.",
            teams
                .iter()
                .map(|team| format!("{} ({})", team.key, team.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(Error::not_found("Team", &prepared.original).with_hint(suggestion))
}

/// Execute through a client that the caller already built after preparation.
pub async fn resolve_team_with_transport(
    prepared: &PreparedTeamLookup,
    client: &LinearClient,
) -> Result<ResolvedTeam, Error> {
    resolve_team(
        prepared,
        |variables| async move { Ok(client.query(variables).await?) },
        |variables| async move { Ok(client.query(variables).await?) },
    )
    .await
}

/// Fetches every team, sorted by lowercased name.
pub async fn fetch_all_teams<F, Fut>(mut all_fetch: F) -> Result<Vec<ResolvedTeam>, Error>
where
    F: FnMut(GetAllTeamsVariables) -> Fut,
    Fut: Future<Output = Result<GetAllTeams, Error>>,
{
    let mut teams = pagination::collect(None, |after, first| {
        let response = all_fetch(GetAllTeamsVariables {
            first: Some(first),
            after,
        });
        async move {
            let teams = response.await?.teams;
            Ok(Page {
                nodes: teams.nodes.into_iter().map(ResolvedTeam::from).collect(),
                page_info: teams.page_info,
            })
        }
    })
    .await?;
    teams.sort_by(|left, right| {
        collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
    });
    Ok(teams)
}

pub async fn fetch_all_teams_with_transport(
    client: &LinearClient,
) -> Result<Vec<ResolvedTeam>, Error> {
    fetch_all_teams(|variables| async move { Ok(client.query(variables).await?) }).await
}

#[cfg(test)]
mod tests;
