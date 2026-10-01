use crate::graphql::schema;
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
