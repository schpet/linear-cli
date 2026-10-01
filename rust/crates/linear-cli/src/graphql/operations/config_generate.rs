//! Complete Config query: no variables field and no extra viewer/team selections.
use crate::graphql::schema;
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct Config {
    pub viewer: ConfigViewer,
    pub teams: ConfigTeams,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct ConfigViewer {
    pub organization: ConfigOrganization,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct ConfigOrganization {
    pub url_key: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct ConfigTeams {
    pub nodes: Vec<ConfigTeam>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct ConfigTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}
