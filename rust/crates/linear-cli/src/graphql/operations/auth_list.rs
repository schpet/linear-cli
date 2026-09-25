//! The frozen `AuthListViewer` document for `auth list`.

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
