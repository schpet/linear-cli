//! Exact name-or-version release lookup with complete pages.
use crate::graphql::{operations::teams::PageInfo, schema};
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ResolveReleasesVariables {
    pub input: String,
    pub after: Option<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ResolveReleasesVariables"
)]
pub struct ResolveReleases {
    #[arguments(filter: { or: [{ name: { eqIgnoreCase: $input } }, { version: { eq: $input } }] }, first: 100, after: $after)]
    pub releases: ReleaseConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ReleaseConnection")]
pub struct ReleaseConnection {
    pub nodes: Vec<ReleaseNode>,
    pub page_info: PageInfo,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Release")]
pub struct ReleaseNode {
    pub id: cynic::Id,
    pub name: String,
    pub version: Option<String>,
}
