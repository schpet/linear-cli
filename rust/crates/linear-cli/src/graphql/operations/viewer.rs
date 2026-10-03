//! The authenticated user's workspace.
use crate::client::LinearClient;
use crate::error::Result;
use crate::graphql::schema;

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
