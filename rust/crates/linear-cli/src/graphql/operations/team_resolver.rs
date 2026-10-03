//! Minimal typed team-reference queries, separate from the broader `GetTeams`
//! projection used by `team list`.

use crate::graphql::pagination::PageInfo;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ResolveTeamVariables {
    pub reference: String,
    pub id: Option<cynic::Id>,
    pub is_uuid: bool,
}

#[allow(non_snake_case)]
mod resolve_team {
    use super::*;

    #[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
    #[cynic(
        schema = "linear",
        graphql_type = "Query",
        variables = "ResolveTeamVariables"
    )]
    pub struct ResolveTeam {
        #[arguments(filter: { or: [{ key: { eqIgnoreCase: $reference } }, { name: { eqIgnoreCase: $reference } }] })]
        pub teams: TeamNodes,
        #[arguments(filter: { id: { eq: $id } })]
        #[directives(include(if: $is_uuid))]
        #[cynic(rename = "teams", alias)]
        pub teamById: Option<TeamNodes>,
    }
}

pub use resolve_team::ResolveTeam;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetAllTeamsVariables {
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetAllTeamsVariables"
)]
pub struct GetAllTeams {
    #[arguments(first: $first, after: $after)]
    pub teams: TeamPage,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamNodes {
    pub nodes: Vec<TeamNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamPage {
    pub nodes: Vec<TeamNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct TeamNode {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}
