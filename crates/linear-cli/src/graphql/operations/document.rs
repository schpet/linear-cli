//! Document operations: lists, details, create, update, delete and attachment targets.

use serde::Serialize;

use super::common::IdVariablesFields;
use super::initiative::IDComparator;
use super::team::TeamKey;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::WholeNumber;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetDocumentForDelete {
    #[arguments(id: $id)]
    pub document: Option<DocumentDetails>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct DocumentDetails {
    pub id: cynic::Id,
    pub title: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteDocument {
    #[arguments(id: $id)]
    pub document_delete: DeletePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentArchivePayload")]
pub struct DeletePayload {
    pub success: bool,
}

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
    pub title: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct UpdatedDocument {
    pub title: String,
    pub url: String,
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

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Project")]
#[serde(rename_all = "camelCase")]
pub struct DocumentProject {
    pub name: String,
    pub slug_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct DocumentIssue {
    pub identifier: String,
    pub title: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
#[serde(rename_all = "camelCase")]
pub struct DocumentInitiative {
    pub name: String,
    pub slug_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Team")]
#[serde(rename_all = "camelCase")]
pub struct DocumentTeam {
    pub name: String,
    pub key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
#[serde(rename_all = "camelCase")]
pub struct DocumentCycle {
    pub name: Option<String>,
    pub number: WholeNumber,
    pub team: TeamKey,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Release")]
#[serde(rename_all = "camelCase")]
pub struct DocumentRelease {
    pub name: String,
    pub version: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreatorName {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreator {
    pub name: String,
    pub email: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Comment")]
#[serde(rename_all = "camelCase")]
pub struct DocumentCommentParent {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Comment")]
#[serde(rename_all = "camelCase")]
pub struct DocumentComment {
    pub id: cynic::Id,
    pub body: String,
    pub quoted_text: Option<String>,
    pub document_content_id: Option<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub archived_at: Option<DateTime>,
    pub resolved_at: Option<DateTime>,
    pub url: String,
    pub user: Option<DocumentCreator>,
    pub parent: Option<DocumentCommentParent>,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "CommentConnection")]
#[serde(transparent)]
pub struct DocumentCommentsConnection {
    pub nodes: Vec<DocumentComment>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Document")]
#[serde(rename_all = "camelCase")]
pub struct ListedDocument {
    pub id: cynic::Id,
    pub title: String,
    pub slug_id: String,
    pub url: String,
    pub updated_at: DateTime,
    pub project: Option<DocumentProject>,
    pub issue: Option<DocumentIssue>,
    pub initiative: Option<DocumentInitiative>,
    pub team: Option<DocumentTeam>,
    pub cycle: Option<DocumentCycle>,
    pub release: Option<DocumentRelease>,
    pub creator: Option<DocumentCreatorName>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentConnection")]
pub struct DocumentConnection {
    pub nodes: Vec<ListedDocument>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Document")]
#[serde(rename_all = "camelCase")]
pub struct DocumentBody {
    pub id: cynic::Id,
    pub title: String,
    pub slug_id: String,
    pub content: Option<String>,
    pub url: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub creator: Option<DocumentCreator>,
    pub project: Option<DocumentProject>,
    pub issue: Option<DocumentIssue>,
    pub initiative: Option<DocumentInitiative>,
    pub team: Option<DocumentTeam>,
    pub cycle: Option<DocumentCycle>,
    pub release: Option<DocumentRelease>,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(
    schema = "linear",
    graphql_type = "Document",
    variables = "GetDocumentCommentsVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct DocumentWithComments {
    pub id: cynic::Id,
    pub title: String,
    pub slug_id: String,
    pub content: Option<String>,
    pub url: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub creator: Option<DocumentCreator>,
    pub project: Option<DocumentProject>,
    pub issue: Option<DocumentIssue>,
    pub initiative: Option<DocumentInitiative>,
    pub team: Option<DocumentTeam>,
    pub cycle: Option<DocumentCycle>,
    pub release: Option<DocumentRelease>,
    #[arguments(first: 50, after: $comments_after, orderBy: createdAt)]
    pub comments: DocumentCommentsConnection,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ListDocumentsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<DocumentFilter>,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetDocumentVariables {
    pub id: String,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetDocumentCommentsVariables {
    pub id: String,
    pub comments_after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ListDocumentsVariables"
)]
pub struct ListDocuments {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub documents: Option<DocumentConnection>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetDocumentVariables"
)]
pub struct GetDocument {
    #[arguments(id: $id)]
    pub document: Option<DocumentBody>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetDocumentCommentsVariables"
)]
pub struct GetDocumentWithComments {
    #[arguments(id: $id)]
    pub document: Option<DocumentWithComments>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetDocumentVariables"
)]
pub struct GetIssueForDocumentTarget {
    #[arguments(id: $id)]
    pub issue: Option<DocumentTargetIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DocumentTargetIssue {
    pub id: cynic::Id,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "DocumentFilter")]
pub struct DocumentFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project: Option<DocumentProjectFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub issue: Option<DocumentIssueFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub initiative: Option<DocumentInitiativeFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team: Option<DocumentTeamFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub cycle: Option<DocumentCycleFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub release: Option<DocumentReleaseFilter>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectFilter")]
pub struct DocumentProjectFilter {
    pub id: EntityIdentifierIDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueFilter")]
pub struct DocumentIssueFilter {
    pub id: IssueIDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeFilter")]
pub struct DocumentInitiativeFilter {
    pub id: EntityIdentifierIDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "NullableTeamFilter")]
pub struct DocumentTeamFilter {
    pub id: IDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CycleFilter")]
pub struct DocumentCycleFilter {
    pub id: IDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ReleaseFilter")]
pub struct DocumentReleaseFilter {
    pub id: IDComparator,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "EntityIdentifierIDComparator")]
pub struct EntityIdentifierIDComparator {
    pub eq: Option<cynic::Id>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueIDComparator")]
pub struct IssueIDComparator {
    pub eq: Option<cynic::Id>,
}
