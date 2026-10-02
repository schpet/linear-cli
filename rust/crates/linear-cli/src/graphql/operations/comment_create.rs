//! The `AddComment` mutation shared by every `comment add` command, and
//! the document command's `GetDocumentCommentTarget` content-record lookup.
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
