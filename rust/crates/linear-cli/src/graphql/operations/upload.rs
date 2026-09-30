//! Exact source upload, issue lookup and sidebar attachment documents.
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct FileUploadVariables {
    pub content_type: String,
    pub filename: String,
    pub size: i32,
    pub make_public: Option<bool>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "FileUploadVariables"
)]
pub struct FileUpload {
    #[arguments(contentType: $content_type, filename: $filename, size: $size, makePublic: $make_public)]
    pub file_upload: UploadPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "UploadPayload")]
pub struct UploadPayload {
    pub success: bool,
    pub upload_file: Option<UploadFile>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "UploadFile")]
pub struct UploadFile {
    pub asset_url: String,
    pub upload_url: String,
    pub headers: Vec<UploadFileHeader>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "UploadFileHeader")]
pub struct UploadFileHeader {
    pub key: String,
    pub value: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetIssueIdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueIdVariables"
)]
pub struct GetIssueId {
    #[arguments(id: $id)]
    pub issue: Option<IssueId>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueId {
    pub id: cynic::Id,
}
#[derive(cynic::InputObject, Clone, Debug, Eq, PartialEq)]
#[cynic(schema = "linear", graphql_type = "AttachmentCreateInput")]
pub struct AttachmentCreateInput {
    pub issue_id: String,
    pub title: String,
    pub url: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub comment_body: Option<String>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct AttachmentCreateVariables {
    pub input: AttachmentCreateInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "AttachmentCreateVariables"
)]
pub struct AttachmentCreate {
    #[arguments(input: $input)]
    pub attachment_create: AttachmentPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "AttachmentPayload")]
pub struct AttachmentPayload {
    pub success: bool,
    pub attachment: CreatedAttachment,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Attachment")]
pub struct CreatedAttachment {
    pub id: cynic::Id,
    pub url: String,
    pub title: String,
}
