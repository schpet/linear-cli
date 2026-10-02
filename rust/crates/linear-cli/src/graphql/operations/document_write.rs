//! Document create, update and attachment-target operations.
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::{scalars::DateTime, schema};
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DocumentCreateInput")]
pub struct DocumentCreateInput {
    pub title: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub initiative_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub cycle_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<String>,
}
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DocumentUpdateInput")]
pub struct DocumentUpdateInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub initiative_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub cycle_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<String>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CreateDocumentVariables {
    pub input: DocumentCreateInput,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateDocumentVariables {
    pub id: String,
    pub input: DocumentUpdateInput,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct DocumentEditVariables {
    pub id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct DocumentGuardVariables {
    pub id: String,
    pub after: Option<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateDocumentVariables"
)]
pub struct CreateDocument {
    #[arguments(input: $input)]
    pub document_create: CreateDocumentPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateDocumentVariables"
)]
pub struct UpdateDocument {
    #[arguments(id: $id, input: $input)]
    pub document_update: UpdateDocumentPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentPayload")]
pub struct CreateDocumentPayload {
    pub success: bool,
    pub document: CreatedDocument,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentPayload")]
pub struct UpdateDocumentPayload {
    pub success: bool,
    pub document: UpdatedDocument,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct CreatedDocument {
    pub id: cynic::Id,
    pub slug_id: String,
    pub title: String,
    pub url: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct UpdatedDocument {
    pub id: cynic::Id,
    pub slug_id: String,
    pub title: String,
    pub url: String,
    pub updated_at: DateTime,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DocumentEditVariables"
)]
pub struct GetDocumentForEdit {
    #[arguments(id: $id)]
    pub document: Option<DocumentForEdit>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct DocumentForEdit {
    pub id: cynic::Id,
    pub title: String,
    pub content: Option<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DocumentGuardVariables"
)]
pub struct DocumentInlineCommentGuard {
    #[arguments(id: $id)]
    pub document: Option<DocumentGuard>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Document",
    variables = "DocumentGuardVariables"
)]
pub struct DocumentGuard {
    pub id: cynic::Id,
    #[arguments(first: 50, after: $after, orderBy: createdAt)]
    pub comments: DocumentGuardComments,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CommentConnection")]
pub struct DocumentGuardComments {
    pub nodes: Vec<GuardComment>,
    pub page_info: PageInfo,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct GuardComment {
    pub id: cynic::Id,
    pub quoted_text: Option<String>,
    pub resolved_at: Option<DateTime>,
    pub archived_at: Option<DateTime>,
}
