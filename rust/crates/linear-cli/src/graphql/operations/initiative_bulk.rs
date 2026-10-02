//! Details and mutations for `initiative archive` and `initiative delete`.
use crate::graphql::{scalars::DateTime, schema};

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeForArchive {
    #[arguments(id: $id)]
    pub initiative: Option<ArchiveDetail>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct ArchiveDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub archived_at: Option<DateTime>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeForDelete {
    #[arguments(id: $id)]
    pub initiative: Option<DeleteDetail>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct DeleteDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub projects: Option<LinkedProjects>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct LinkedProjects {
    pub nodes: Vec<LinkedProject>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct LinkedProject {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct ArchiveInitiative {
    #[arguments(id: $id)]
    pub initiative_archive: ArchivePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeArchivePayload")]
pub struct ArchivePayload {
    pub success: bool,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteInitiative {
    #[arguments(id: $id)]
    pub initiative_delete: DeletePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletePayload {
    pub success: bool,
}
