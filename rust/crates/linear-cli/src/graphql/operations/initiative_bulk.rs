//! Exact archive/delete selections; optional display reads remain separate from required details.
use crate::graphql::{scalars::DateTime, schema};

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct SlugVariables {
    pub slug_id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct NameVariables {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForArchive {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub initiatives: SlugResults,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForDelete {
    #[arguments(filter: { slugId: { eq: $slug_id } }, includeArchived: true)]
    pub initiatives: SlugResults,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForArchive {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: NameResults,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForDelete {
    #[arguments(filter: { name: { eqIgnoreCase: $name } }, includeArchived: true)]
    pub initiatives: NameResults,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct SlugResults {
    pub nodes: Vec<SlugNode>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct SlugNode {
    pub id: cynic::Id,
    pub slug_id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct NameResults {
    pub nodes: Vec<NameNode>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct NameNode {
    pub id: cynic::Id,
    pub name: String,
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
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeNameForBulkArchive {
    #[arguments(id: $id)]
    pub initiative: Option<BulkArchiveDetail>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct BulkArchiveDetail {
    pub id: cynic::Id,
    pub name: String,
    pub archived_at: Option<DateTime>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeNameForBulkDelete {
    #[arguments(id: $id)]
    pub initiative: Option<BulkDeleteDetail>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct BulkDeleteDetail {
    pub id: cynic::Id,
    pub name: String,
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
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct BulkArchiveInitiative {
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
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct BulkDeleteInitiative {
    #[arguments(id: $id)]
    pub initiative_delete: DeletePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletePayload {
    pub success: bool,
}
