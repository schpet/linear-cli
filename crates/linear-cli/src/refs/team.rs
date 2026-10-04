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
        NONE_ACCESSIBLE.to_owned()
    } else {
        teams.sort_by(|left, right| collation::compare(&left.key, &right.key));
        valid_keys_hint(&teams)
    };
    Err(Error::not_found("Team", &reference.input).with_hint(hint))
}

/// The most teams a not-found hint names before pointing at `team list`.
const LISTED_TEAMS: usize = 10;

fn valid_keys_hint(teams: &[ResolvedTeam]) -> String {
    let listed: Vec<String> = teams
        .iter()
        .take(LISTED_TEAMS)
        .map(|team| format!("{} ({})", team.key, team.name))
        .collect();
    match teams
        .len()
        .checked_sub(LISTED_TEAMS)
        .filter(|more| *more > 0)
    {
        Some(more) => format!(
            "Valid team keys include {}, and {more} more. Run `linear team list` to see them all.",
            listed.join(", ")
        ),
        None => format!(
            "Valid team keys: {}. Run `linear team list` to see all teams.",
            listed.join(", ")
        ),
    }
}

const NONE_ACCESSIBLE: &str = "This workspace has no teams you can access.";

/// The error for a picker with no team to offer.
pub fn none_accessible() -> Error {
    Error::new(NONE_ACCESSIBLE.trim_end_matches('.'))
        .with_hint("Ask a workspace admin to add you to a team, or check the API key's workspace.")
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

#[cfg(test)]
mod tests {
    use super::{ResolvedTeam, valid_keys_hint};

    fn teams(count: usize) -> Vec<ResolvedTeam> {
        (1..=count)
            .map(|n| ResolvedTeam {
                id: format!("id-{n}"),
                key: format!("T{n:02}"),
                name: format!("Team {n}"),
            })
            .collect()
    }

    #[test]
    fn the_not_found_hint_names_at_most_ten_teams() {
        assert_eq!(
            valid_keys_hint(&teams(2)),
            "Valid team keys: T01 (Team 1), T02 (Team 2). Run `linear team list` to see all teams."
        );
        let many = valid_keys_hint(&teams(77));
        assert!(
            many.starts_with("Valid team keys include T01 (Team 1), "),
            "{many}"
        );
        assert!(
            many.ends_with("T10 (Team 10), and 67 more. Run `linear team list` to see them all."),
            "{many}"
        );
        assert!(!many.contains("T11"), "{many}");
        assert!(valid_keys_hint(&teams(10)).contains("T10 (Team 10). Run"));
    }
}
