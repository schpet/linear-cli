//! Typed document list/view selections in `--json` output order.
use crate::graphql::operations::{initiatives::IDComparator, number::WholeNumber, teams::PageInfo};
use crate::graphql::{scalars::DateTime, schema};
use serde::Serialize;

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
#[cynic(schema = "linear", graphql_type = "Team")]
#[serde(rename_all = "camelCase")]
pub struct CycleTeam {
    pub key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
#[serde(rename_all = "camelCase")]
pub struct DocumentCycle {
    pub name: Option<String>,
    pub number: WholeNumber,
    pub team: CycleTeam,
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
#[serde(rename_all = "camelCase")]
pub struct DocumentCommentsConnection {
    pub nodes: Vec<DocumentComment>,
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

#[derive(cynic::QueryFragment, Clone, Debug, Serialize)]
#[cynic(schema = "linear", graphql_type = "DocumentConnection")]
#[serde(rename_all = "camelCase")]
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
    #[arguments(filter: $filter, first: $first)]
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
