//! Comment operations: lists for every commentable entity, create, update,
//! delete, resolve and unresolve.

use serde::Serialize;

use super::common::DeletePayload;
use super::project::ProjectRef;
use super::user::UserRef;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct AddCommentVariables {
    pub input: CommentCreateInput,
}

/// Absent optional keys are omitted. Callers construct this only through `comment_add::build_input`, which
/// sets exactly one target field.
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentCreateInput")]
pub struct CommentCreateInput {
    pub body: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub document_content_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub initiative_id: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "AddCommentVariables"
)]
pub struct AddComment {
    #[arguments(input: $input)]
    pub comment_create: AddCommentPayload,
}

/// `comment` is non-null in the schema; a null or missing comment is a decode
/// failure.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentPayload")]
pub struct AddCommentPayload {
    pub success: bool,
    pub comment: CreatedComment,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct CreatedComment {
    pub id: cynic::Id,
    pub url: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DocumentCommentTargetVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DocumentCommentTargetVariables"
)]
pub struct GetDocumentCommentTarget {
    #[arguments(id: $id)]
    pub document: DocumentCommentTarget,
}

/// `documentContentId` is the schema's one nullable field here; null is a
/// business error for the caller, never a decode failure.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct DocumentCommentTarget {
    pub id: cynic::Id,
    pub title: String,
    pub document_content_id: Option<String>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteCommentVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteCommentVariables"
)]
pub struct DeleteComment {
    #[arguments(id: $id)]
    pub comment_delete: DeletePayload,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetCommentVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetCommentVariables"
)]
pub struct GetComment {
    #[arguments(id: $id)]
    pub comment: Option<ExistingComment>,
}

/// The comment `--reply-to` names: whether it is itself a reply, and what
/// it is on.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetCommentVariables"
)]
pub struct GetReplyParent {
    #[arguments(id: $id)]
    pub comment: Option<ReplyParent>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct ReplyParent {
    pub parent_id: Option<String>,
    pub issue_id: Option<String>,
    pub project_id: Option<String>,
    pub initiative_id: Option<String>,
    pub document_content_id: Option<String>,
}

#[derive(cynic::QueryFragment, serde::Deserialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment", no_deserialize)]
pub struct ExistingComment {
    // A missing or null body is treated as empty.
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentUpdateInput")]
pub struct CommentUpdateInput {
    pub body: String,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateCommentVariables {
    pub id: String,
    pub input: CommentUpdateInput,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateCommentVariables"
)]
pub struct UpdateComment {
    #[arguments(id: $id, input: $input)]
    pub comment_update: UpdatedPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CommentPayload")]
pub struct UpdatedPayload {
    pub success: bool,
    pub comment: Option<UpdatedComment>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct UpdatedComment {
    pub url: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentConnection")]
pub struct CommentConnection {
    pub nodes: Vec<CommentNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct CommentNode {
    pub id: cynic::Id,
    pub body: String,
    pub quoted_text: Option<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub edited_at: Option<DateTime>,
    pub url: String,
    pub user: Option<UserRef>,
    pub external_user: Option<CommentExternalUser>,
    pub bot_actor: Option<CommentBotActor>,
    pub parent: Option<CommentParent>,
    /// Set only on a resolved thread's top-level comment.
    pub resolved_at: Option<DateTime>,
    pub resolving_comment_id: Option<String>,
    pub resolving_user: Option<UserRef>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ExternalUser")]
#[serde(rename_all = "camelCase")]
pub struct CommentExternalUser {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ActorBot")]
#[serde(rename_all = "camelCase")]
pub struct CommentBotActor {
    pub id: Option<cynic::Id>,
    pub name: Option<String>,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub bot_type: String,
    pub sub_type: Option<String>,
}

/// A reply's top-level comment; its `resolved_at` is the reply's thread state.
#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Comment")]
#[serde(rename_all = "camelCase")]
pub struct CommentParent {
    pub id: cynic::Id,
    pub resolved_at: Option<DateTime>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetDocumentCommentsVariables {
    pub id: String,
    pub after: Option<String>,
    pub first: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetDocumentCommentsVariables"
)]
pub struct GetDocumentComments {
    #[arguments(id: $id)]
    pub document: Option<CommentDocument>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Document",
    variables = "GetDocumentCommentsVariables"
)]
pub struct CommentDocument {
    pub id: cynic::Id,
    #[arguments(first: $first, after: $after, orderBy: createdAt)]
    pub comments: CommentConnection,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetInitiativeCommentsVariables {
    pub id: String,
    pub filter_id: cynic::Id,
    pub after: Option<String>,
    pub first: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetInitiativeCommentsVariables"
)]
pub struct GetInitiativeComments {
    #[arguments(id: $id)]
    pub initiative: Option<CommentInitiative>,
    #[arguments(first: $first, after: $after, orderBy: createdAt, filter: { initiative: { id: { eq: $filter_id } } })]
    pub comments: CommentConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CommentInitiative {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetIssueCommentsVariables {
    pub id: String,
    pub after: Option<String>,
    pub first: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueCommentsVariables"
)]
pub struct GetIssueComments {
    #[arguments(id: $id)]
    pub issue: Option<CommentIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "GetIssueCommentsVariables"
)]
pub struct CommentIssue {
    #[arguments(first: $first, after: $after, orderBy: createdAt)]
    pub comments: CommentConnection,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectCommentsVariables {
    pub id: String,
    pub filter_id: cynic::Id,
    pub after: Option<String>,
    pub first: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectCommentsVariables"
)]
pub struct GetProjectComments {
    #[arguments(id: $id)]
    pub project: Option<ProjectRef>,
    #[arguments(first: $first, after: $after, orderBy: createdAt, filter: { project: { id: { eq: $filter_id } } })]
    pub comments: CommentConnection,
}

/// What `issue comment delete` shows before asking: where the comment is and
/// how it starts.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetCommentVariables"
)]
pub struct GetCommentForDelete {
    #[arguments(id: $id)]
    pub comment: Option<CommentForDelete>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct CommentForDelete {
    pub body: String,
    pub issue: Option<CommentForDeleteIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct CommentForDeleteIssue {
    pub identifier: String,
}

/// What `issue comment resolve` and `unresolve` check before changing a
/// thread: whether the comment is a reply, what it is on, and its state.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetCommentVariables"
)]
pub struct GetCommentForResolution {
    #[arguments(id: $id)]
    pub comment: Option<CommentForResolution>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct CommentForResolution {
    pub url: String,
    pub parent_id: Option<String>,
    pub resolved_at: Option<DateTime>,
    pub resolving_comment_id: Option<String>,
    pub issue: Option<CommentForDeleteIssue>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ResolveCommentVariables {
    pub id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub resolving_comment_id: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "ResolveCommentVariables"
)]
pub struct ResolveComment {
    #[arguments(id: $id, resolvingCommentId: $resolving_comment_id)]
    pub comment_resolve: ResolutionPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "GetCommentVariables"
)]
pub struct UnresolveComment {
    #[arguments(id: $id)]
    pub comment_unresolve: ResolutionPayload,
}

/// `comment` is non-null in the schema; a null or missing comment is a decode
/// failure.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CommentPayload")]
pub struct ResolutionPayload {
    pub success: bool,
    pub comment: ResolvedComment,
}

/// The thread's top-level comment after the change.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct ResolvedComment {
    pub id: cynic::Id,
    pub resolved_at: Option<DateTime>,
    pub resolving_comment_id: Option<String>,
}
