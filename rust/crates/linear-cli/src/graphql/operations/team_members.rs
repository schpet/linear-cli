//! The typed `GetTeamMembers` selection used by `team members`.
use serde::Serialize;

use super::organization_members::User;
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
    pub nodes: Vec<User>,
    pub page_info: super::teams::PageInfo,
}
