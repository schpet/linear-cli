//! `GetAgentSessionDetails`: the agent session document with the
//! `AgentActivityContent` union.
//!
//! Cynic adds `__typename` to the union selection so it can dispatch
//! variants; the `--json` shape below never emits it.

use std::error::Error as StdError;
use std::fmt;

use serde::Serialize;
use serde::ser::{Error as _, Serializer};

use crate::error::Error;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetAgentSessionDetailsVariables {
    pub id: String,
    pub first: i32,
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetAgentSessionDetailsVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct GetAgentSessionDetails {
    #[arguments(id: $id)]
    pub agent_session: AgentSession,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", variables = "GetAgentSessionDetailsVariables")]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: cynic::Id,
    pub status: AgentSessionStatus,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub session_type: Option<AgentSessionType>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub started_at: Option<DateTime>,
    pub ended_at: Option<DateTime>,
    pub dismissed_at: Option<DateTime>,
    pub summary: Option<String>,
    pub external_link: Option<String>,
    pub creator: Option<UserName>,
    pub app_user: UserName,
    pub dismissed_by: Option<UserName>,
    pub issue: Option<IssueReference>,
    #[arguments(first: $first, after: $after)]
    pub activities: AgentActivityConnection,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct UserName {
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueReference {
    pub identifier: String,
    pub title: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(transparent)]
pub struct AgentActivityConnection {
    pub nodes: Vec<AgentActivity>,
    #[serde(skip)]
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    pub id: cynic::Id,
    pub created_at: DateTime,
    pub content: AgentActivityContent,
}

/// Wire spellings are lowercase: `action`, `thought`, and so on.
#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "AgentActivityType",
    rename_all = "lowercase"
)]
pub enum AgentActivityType {
    Action,
    Elicitation,
    Error,
    Prompt,
    Response,
    Thought,
}

/// Wire spellings are camelCase: `awaitingInput`.
#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "AgentSessionStatus",
    rename_all = "camelCase"
)]
pub enum AgentSessionStatus {
    Active,
    AwaitingInput,
    Complete,
    Error,
    Pending,
    Stale,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "AgentSessionType",
    rename_all = "camelCase"
)]
pub enum AgentSessionType {
    CommentThread,
}

/// All six `AgentActivityContent` members.
///
/// Cynic matches each variant to a union member by the variant's name, so
/// variants carry the schema type names verbatim.
///
/// `exhaustive` makes the build fail if the pinned schema gains a member. The
/// fallback still exists because Cynic requires one; it carries the concrete
/// `__typename` so [`AgentActivityContent::ensure_supported`] can name it.
#[derive(cynic::InlineFragments, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityContent", exhaustive)]
pub enum AgentActivityContent {
    AgentActivityThoughtContent(ThoughtContent),
    AgentActivityActionContent(ActionContent),
    AgentActivityResponseContent(ResponseContent),
    AgentActivityPromptContent(PromptContent),
    AgentActivityErrorContent(ErrorContent),
    AgentActivityElicitationContent(ElicitationContent),
    #[cynic(fallback)]
    Unsupported(String),
}

impl AgentActivityContent {
    /// Rejects a union member this build does not model.
    ///
    /// Call this at the application boundary before rendering; the `--json`
    /// serializer refuses the fallback as well.
    pub fn ensure_supported(&self) -> Result<(), UnsupportedActivityContent> {
        match self {
            Self::AgentActivityThoughtContent(_)
            | Self::AgentActivityActionContent(_)
            | Self::AgentActivityResponseContent(_)
            | Self::AgentActivityPromptContent(_)
            | Self::AgentActivityErrorContent(_)
            | Self::AgentActivityElicitationContent(_) => Ok(()),
            Self::Unsupported(typename) => Err(UnsupportedActivityContent {
                typename: typename.clone(),
            }),
        }
    }
}

impl Serialize for AgentActivityContent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::AgentActivityThoughtContent(content) => content.serialize(serializer),
            Self::AgentActivityActionContent(content) => content.serialize(serializer),
            Self::AgentActivityResponseContent(content) => content.serialize(serializer),
            Self::AgentActivityPromptContent(content) => content.serialize(serializer),
            Self::AgentActivityErrorContent(content) => content.serialize(serializer),
            Self::AgentActivityElicitationContent(content) => content.serialize(serializer),
            Self::Unsupported(typename) => Err(S::Error::custom(format!(
                "unsupported AgentActivityContent type: {typename}"
            ))),
        }
    }
}

/// The server returned an `AgentActivityContent` member this build does not model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedActivityContent {
    pub typename: String,
}

impl fmt::Display for UnsupportedActivityContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported agent activity content type: {}",
            self.typename
        )
    }
}

impl StdError for UnsupportedActivityContent {}

impl From<UnsupportedActivityContent> for Error {
    fn from(error: UnsupportedActivityContent) -> Self {
        Error::new(error.to_string())
    }
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityThoughtContent")]
pub struct ThoughtContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub body: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityActionContent")]
pub struct ActionContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub action: String,
    pub parameter: String,
    pub result: Option<String>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityResponseContent")]
pub struct ResponseContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub body: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityPromptContent")]
pub struct PromptContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub body: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityErrorContent")]
pub struct ErrorContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub body: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentActivityElicitationContent")]
pub struct ElicitationContent {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub activity_type: AgentActivityType,
    pub body: String,
}

/// Schema check for the embedded `JSONObject` scalar.
///
/// Not selected by any command; checks that `resultData: JSONObject` aligns
/// with [`JsonObject`].
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetIssueAgentSessionsVariables {
    pub issue_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    pub first: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueAgentSessionsVariables"
)]
pub struct GetIssueAgentSessions {
    #[arguments(id: $issue_id)]
    pub issue: SessionIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "GetIssueAgentSessionsVariables"
)]
pub struct SessionIssue {
    #[arguments(first: $first, after: $after)]
    pub comments: SessionComments,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentConnection")]
pub struct SessionComments {
    pub nodes: Vec<SessionComment>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct SessionComment {
    pub agent_session: Option<ListSession>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AgentSession")]
#[serde(rename_all = "camelCase")]
pub struct ListSession {
    pub id: cynic::Id,
    pub status: AgentSessionStatus,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub session_type: Option<AgentSessionType>,
    pub created_at: DateTime,
    pub started_at: Option<DateTime>,
    pub ended_at: Option<DateTime>,
    pub summary: Option<String>,
    pub creator: Option<UserName>,
    pub app_user: UserName,
}

#[cfg(test)]
mod tests;
