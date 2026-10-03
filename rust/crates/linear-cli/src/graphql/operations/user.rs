//! The viewer, workspace members and user lookups.

use serde::Serialize;

use crate::client::LinearClient;
use crate::error::Result;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct AuthListViewer {
    pub viewer: AuthListUser,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct AuthListUser {
    pub name: String,
    pub email: String,
    pub organization: AuthListOrganization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct AuthListOrganization {
    pub name: String,
    pub url_key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct AuthLoginViewer {
    pub viewer: LoginViewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct LoginViewer {
    pub name: String,
    pub email: String,
    pub organization: LoginOrganization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct LoginOrganization {
    pub name: String,
    pub url_key: String,
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
    pub organization: AuthOrganization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct AuthOrganization {
    pub name: String,
    pub url_key: String,
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

/// The URL key of the workspace the client's API key belongs to.
pub async fn url_key(client: &LinearClient) -> Result<String> {
    let result: GetViewer = client.query(()).await?;
    Ok(result.viewer.organization.url_key)
}
