//! Typed `GetOrganizationMembers` selection for `user list`.

use serde::Serialize;

use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetOrganizationMembersVariables {
    pub include_disabled: bool,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetOrganizationMembersVariables"
)]
pub struct GetOrganizationMembers {
    pub viewer: Viewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "User",
    variables = "GetOrganizationMembersVariables"
)]
pub struct Viewer {
    pub organization: Organization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", variables = "GetOrganizationMembersVariables")]
pub struct Organization {
    #[arguments(includeDisabled: $include_disabled, first: $first, after: $after)]
    pub users: UserConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct UserConnection {
    pub nodes: Vec<User>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct User {
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

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}
