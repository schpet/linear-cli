//! Team-reference preparation and typed lookup, without command context or
//! credential/transport construction.

use std::collections::HashSet;
use std::future::Future;

use cynic::QueryBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::edit::Edit;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_resolver::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables, TeamNode,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;
use crate::text::js_space;

use super::workspace::{WorkspaceScope, expect_team_url};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedTeamLookup {
    original: String,
    lookup: String,
}

impl PreparedTeamLookup {
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn lookup(&self) -> &str {
        &self.lookup
    }
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
) -> Result<PreparedTeamLookup, AppError> {
    if original.trim_matches(js_space).is_empty() {
        return Err(
            AppError::new(AppErrorKind::Validation, "Team reference is empty")
                .with_suggestion("Pass a team key, name, or ID, e.g. --team ENG."),
        );
    }
    let lookup = expect_team_url(original, scope)?.unwrap_or_else(|| original.to_owned());
    Ok(PreparedTeamLookup {
        original: original.to_owned(),
        lookup,
    })
}

fn is_linear_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// One first-page `ResolveTeam` request. An absent result is not an error here.
pub async fn find_team<F, Fut>(
    prepared: &PreparedTeamLookup,
    fetch: F,
) -> Result<Option<ResolvedTeam>, AppError>
where
    F: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> Fut,
    Fut: Future<Output = Result<ResolveTeam, AppError>>,
{
    let is_uuid = is_linear_uuid(&prepared.lookup);
    let request = GraphQlRequest::with_variables(ResolveTeam::build(ResolveTeamVariables {
        reference: prepared.lookup.clone(),
        id: is_uuid.then(|| cynic::Id::new(prepared.lookup.clone())),
        is_uuid,
    }));
    let result = fetch(request).await?;
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
        return Err(AppError::new(
            AppErrorKind::Validation,
            format!(
                "Team name \"{}\" is ambiguous: {}",
                prepared.lookup,
                by_name
                    .iter()
                    .map(|team| format!("{} ({})", team.key, team.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with_suggestion("Use the team key instead of the name."));
    }
    Ok(by_name.into_iter().next())
}

/// Resolve a key, name or UUID, fetching every team only when the first query
/// misses. All request and decode failures pass through without command context.
pub async fn resolve_team<ResolveFetch, ResolveFuture, AllFetch, AllFuture>(
    prepared: &PreparedTeamLookup,
    resolve_fetch: ResolveFetch,
    mut all_fetch: AllFetch,
) -> Result<ResolvedTeam, AppError>
where
    ResolveFetch: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> ResolveFuture,
    ResolveFuture: Future<Output = Result<ResolveTeam, AppError>>,
    AllFetch: FnMut(GraphQlRequest<GetAllTeamsVariables>) -> AllFuture,
    AllFuture: Future<Output = Result<GetAllTeams, AppError>>,
{
    if let Some(team) = find_team(prepared, resolve_fetch).await? {
        return Ok(team);
    }

    let mut teams = Vec::new();
    let mut after = Edit::Unchanged;
    let mut seen: HashSet<Option<String>> = HashSet::new();
    let mut page = 1;
    loop {
        let request = GraphQlRequest::with_variables(GetAllTeams::build(GetAllTeamsVariables {
            first: Some(100),
            after,
        }));
        let response = all_fetch(request).await?;
        teams.extend(response.teams.nodes.into_iter().map(ResolvedTeam::from));
        if !response.teams.page_info.has_next_page {
            break;
        }
        let cursor = response.teams.page_info.end_cursor;
        if !seen.insert(cursor.clone()) {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("Linear repeated a team pagination cursor on page {page}"),
            )
            .with_suggestion("Retry the command."));
        }
        after = Edit::set_or_clear(cursor);
        page += 1;
    }

    let suggestion = if teams.is_empty() {
        "This workspace has no teams you can access.".to_owned()
    } else {
        let collator = collation::root()?;
        teams.sort_by(|left, right| {
            collator.compare(&left.name.to_lowercase(), &right.name.to_lowercase())
        });
        teams.sort_by(|left, right| collator.compare(&left.key, &right.key));
        format!(
            "Valid team keys: {}. Run `linear team list` to see all teams.",
            teams
                .iter()
                .map(|team| format!("{} ({})", team.key, team.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(AppError::not_found("Team", &prepared.original).with_suggestion(suggestion))
}

/// Execute through a client that the caller already built after preparation.
pub async fn resolve_team_with_transport(
    prepared: &PreparedTeamLookup,
    transport: &GraphQlTransport,
) -> Result<ResolvedTeam, AppError> {
    resolve_team(
        prepared,
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
    )
    .await
}
