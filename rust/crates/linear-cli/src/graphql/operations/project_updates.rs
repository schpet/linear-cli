//! Status updates of a resolved project, newest first.

use crate::graphql::operations::projects::ProjectUpdateHealthType;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ListProjectUpdatesVariables {
    pub id: String,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ListProjectUpdatesVariables"
)]
pub struct ListProjectUpdates {
    #[arguments(id: $id)]
    pub project: Option<UpdateProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "ListProjectUpdatesVariables"
)]
pub struct UpdateProject {
    pub name: String,
    pub slug_id: String,
    #[arguments(first: $first, after: $after)]
    pub project_updates: UpdateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdateConnection")]
pub struct UpdateConnection {
    pub nodes: Vec<UpdateNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdate")]
pub struct UpdateNode {
    pub id: cynic::Id,
    pub body: String,
    pub health: Option<ProjectUpdateHealthType>,
    pub url: String,
    pub created_at: DateTime,
    pub user: Option<UpdateUser>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct UpdateUser {
    pub name: String,
    pub display_name: String,
}
