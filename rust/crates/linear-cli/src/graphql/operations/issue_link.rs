//! URL attachment mutation; omitted title remains absent from wire variables.
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct Variables {
    pub issue_id: String,
    pub url: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Mutation", variables = "Variables")]
pub struct AttachmentLinkURL {
    #[arguments(issueId: $issue_id, url: $url, title: $title)]
    #[cynic(rename = "attachmentLinkURL")]
    pub attachment_link_url: LinkedPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AttachmentPayload")]
pub struct LinkedPayload {
    pub success: bool,
    pub attachment: LinkedAttachment,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Attachment")]
pub struct LinkedAttachment {
    pub id: cynic::Id,
    pub title: String,
    pub url: String,
}
