//! Single and bulk issue archive and delete operations.
use crate::graphql::{scalars::DateTime, schema};
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueArchiveDetails {
    #[arguments(id: $id)]
    pub issue: Option<ArchiveDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueDetailsForBulkArchive {
    #[arguments(id: $id)]
    pub issue: Option<ArchiveDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueDeleteDetails {
    #[arguments(id: $id)]
    pub issue: Option<DeleteDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueDetailsForBulkDelete {
    #[arguments(id: $id)]
    pub issue: Option<DeleteDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ArchiveDetails {
    pub identifier: String,
    pub title: String,
    pub archived_at: Option<DateTime>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DeleteDetails {
    pub title: String,
    pub identifier: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct ArchiveIssue {
    #[arguments(id: $id)]
    pub issue_archive: SuccessPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct BulkArchiveIssue {
    #[arguments(id: $id)]
    pub issue_archive: SuccessPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteIssue {
    #[arguments(id: $id)]
    pub issue_delete: SuccessPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct BulkDeleteIssue {
    #[arguments(id: $id)]
    pub issue_delete: SuccessPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueArchivePayload")]
pub struct SuccessPayload {
    pub success: bool,
}
