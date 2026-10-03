//! The authenticated user's workspace.
use cynic::QueryBuilder;

use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::envelope::LegacyRequest;
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
    let request = LegacyRequest::without_variables(GetViewer::build(()));
    let result: GetViewer = client.execute_legacy(&request).await.map_err(Error::from)?;
    Ok(result.viewer.organization.url_key)
}
