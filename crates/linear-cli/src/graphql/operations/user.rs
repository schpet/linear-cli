//! The viewer, workspace members and user lookups.

use serde::Serialize;

use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

/// Who an API key belongs to, for `auth login` and `auth list`.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetViewerAccount {
    pub viewer: ViewerAccount,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct ViewerAccount {
    pub name: String,
    pub email: String,
    pub organization: OrganizationName,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct AuthStatus {
    pub viewer: AuthViewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct AuthViewer {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub admin: bool,
    pub guest: bool,
    pub organization: OrganizationName,
}

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
    pub viewer: MembersViewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "User",
    variables = "GetOrganizationMembersVariables"
)]
pub struct MembersViewer {
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

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetViewer {
    pub viewer: Viewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct Viewer {
    pub organization: ViewerOrganization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct ViewerOrganization {
    pub url_key: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct UserRef {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct OrganizationName {
    pub name: String,
    pub url_key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetViewerId {
    pub viewer: ViewerId,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct ViewerId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LookupUserVariables {
    pub input: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LookupUserVariables"
)]
pub struct LookupUser {
    #[arguments(filter: { or: [{ email: { eqIgnoreCase: $input } }, { displayName: { eqIgnoreCase: $input } }, { name: { containsIgnoreCaseAndAccent: $input } }] })]
    pub users: LookupUsers,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "UserConnection")]
pub struct LookupUsers {
    pub nodes: Vec<LookupUserNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct LookupUserNode {
    pub id: cynic::Id,
    pub email: String,
    pub display_name: String,
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ListAgentUsersVariables {
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// The workspace's agent (app) users, the only users an issue can be
/// delegated to.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ListAgentUsersVariables"
)]
pub struct ListAgentUsers {
    #[arguments(filter: { app: { eq: true } }, first: $first, after: $after)]
    pub users: AgentUsers,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "UserConnection",
    variables = "ListAgentUsersVariables"
)]
pub struct AgentUsers {
    pub nodes: Vec<AgentUser>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct AgentUser {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub is_me: bool,
}
