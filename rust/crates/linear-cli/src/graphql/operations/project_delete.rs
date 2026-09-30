//! The source project-delete mutation, including nullable entity fallback.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteProjectVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteProjectVariables"
)]
pub struct DeleteProject {
    #[arguments(id: $id)]
    pub project_delete: ProjectDeletePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectArchivePayload")]
pub struct ProjectDeletePayload {
    pub success: bool,
    pub entity: Option<DeletedProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct DeletedProject {
    pub id: cynic::Id,
    pub name: String,
}
