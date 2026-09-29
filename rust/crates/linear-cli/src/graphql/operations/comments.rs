//! Shared typed selections for comment-list connections.

use super::teams::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

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
    pub user: Option<CommentUser>,
    pub external_user: Option<CommentExternalUser>,
    pub bot_actor: Option<CommentBotActor>,
    pub parent: Option<CommentParent>,
}

#[derive(cynic::QueryFragment, serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct CommentUser {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ExternalUser")]
#[serde(rename_all = "camelCase")]
pub struct CommentExternalUser {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, serde::Serialize, Clone, Debug, PartialEq, Eq)]
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

#[derive(cynic::QueryFragment, serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct CommentParent {
    pub id: cynic::Id,
}
