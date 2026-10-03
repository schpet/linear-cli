//! Teams, referenced by key, name, UUID or team URL.

use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::team::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables, TeamRef,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::collation;

use super::uuid::is_linear_uuid;
use super::workspace::{WorkspaceScope, expect_team_url};

/// A team argument, checked locally: a team URL is reduced to its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamReference {
    input: String,
    lookup: String,
}

impl TeamReference {
    pub fn parse(input: &str, scope: &WorkspaceScope<'_>) -> Result<Self> {
        if input.trim().is_empty() {
            return Err(Error::new("Team reference is empty")
                .with_hint("Pass a team key, name, or ID, e.g. --team ENG."));
        }
        let lookup = expect_team_url(input, scope)?.unwrap_or_else(|| input.to_owned());
        Ok(Self {
            input: input.to_owned(),
            lookup,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTeam {
    pub id: String,
    pub key: String,
    pub name: String,
}

impl From<TeamRef> for ResolvedTeam {
    fn from(node: TeamRef) -> Self {
        Self {
            id: node.id.into_inner(),
            key: node.key,
            name: node.name,
        }
    }
}

/// The team `reference` names, by key, then UUID, then exact name (all
/// case-insensitive), or `None` when nothing matches.
pub async fn find(
    client: &LinearClient,
    reference: &TeamReference,
) -> Result<Option<ResolvedTeam>> {
    let lookup = &reference.lookup;
    let is_uuid = is_linear_uuid(lookup);
    let data: ResolveTeam = client
        .query(ResolveTeamVariables {
            reference: lookup.clone(),
            id: is_uuid.then(|| cynic::Id::new(lookup.clone())),
            is_uuid,
        })
        .await?;
    let wanted = lookup.to_lowercase();
    let candidates: Vec<ResolvedTeam> = data
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
    if let Some(team) = data
        .team_by_id
        .and_then(|teams| teams.nodes.into_iter().next())
    {
        return Ok(Some(team.into()));
    }
    let mut by_name: Vec<ResolvedTeam> = candidates
        .into_iter()
        .filter(|team| team.name.to_lowercase() == wanted)
        .collect();
    if by_name.len() > 1 {
        return Err(super::ambiguous(
            "Team",
            &reference.input,
            by_name
                .iter()
                .map(|team| format!("{} ({})", team.key, team.name)),
        )
        .with_hint("Use the team key instead of the name."));
    }
    Ok(by_name.pop())
}

/// The team `reference` names; when nothing matches, the error lists every
/// team key.
pub async fn resolve(client: &LinearClient, reference: &TeamReference) -> Result<ResolvedTeam> {
    if let Some(team) = find(client, reference).await? {
        return Ok(team);
    }
    let mut teams = fetch_all(client).await?;
    let hint = if teams.is_empty() {
        "This workspace has no teams you can access.".to_owned()
    } else {
        teams.sort_by(|left, right| collation::compare(&left.key, &right.key));
        format!(
            "Valid team keys: {}. Run `linear team list` to see all teams.",
            teams
                .iter()
                .map(|team| format!("{} ({})", team.key, team.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(Error::not_found("Team", &reference.input).with_hint(hint))
}

/// Every team the key can access, sorted by name.
pub async fn fetch_all(client: &LinearClient) -> Result<Vec<ResolvedTeam>> {
    let mut teams = pagination::collect(None, |after, first| async move {
        let data: GetAllTeams = client
            .query(GetAllTeamsVariables {
                first: Some(first),
                after,
            })
            .await?;
        Ok(Page {
            nodes: data
                .teams
                .nodes
                .into_iter()
                .map(ResolvedTeam::from)
                .collect(),
            page_info: data.teams.page_info,
        })
    })
    .await?;
    teams.sort_by(|left, right| {
        collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
    });
    Ok(teams)
}
