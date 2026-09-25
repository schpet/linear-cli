//! The typed `GetTeamMembers` selection used by `team members`.
use serde::Serialize;

use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTeamMembersVariables {
    #[cynic(rename = "teamKey")]
    pub team_key: String,
    #[cynic(rename = "includeDisabled")]
    pub include_disabled: bool,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTeamMembersVariables"
)]
pub struct GetTeamMembers {
    #[arguments(id: $team_key)]
    pub team: Team,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", variables = "GetTeamMembersVariables")]
pub struct Team {
    #[arguments(includeDisabled: $include_disabled, first: $first, after: $after)]
    pub members: MemberConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(
    schema = "linear",
    graphql_type = "UserConnection",
    variables = "GetTeamMembersVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct MemberConnection {
    pub nodes: Vec<Member>,
    pub page_info: super::teams::PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub active: bool,
    pub initials: String,
    pub description: Option<String>,
    pub timezone: Option<String>,
    pub last_seen: Option<DateTime>,
    pub status_emoji: Option<String>,
    pub status_label: Option<String>,
    pub guest: bool,
    pub is_assignable: bool,
    pub admin: bool,
    pub owner: bool,
    pub is_me: bool,
    pub url: String,
}
