//! The frozen `AuthStatus` document for `auth whoami`.

use crate::graphql::schema;

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
    pub logo_url: Option<String>,
}
