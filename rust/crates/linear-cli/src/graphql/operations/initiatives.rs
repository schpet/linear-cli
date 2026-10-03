//! Typed documents for `initiative list`.

use serde::Serialize;

use crate::graphql::operations::teams::StringComparator;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetInitiativesVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<InitiativeFilter>,
    pub include_archived: Option<bool>,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct InitiativeFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner: Option<NullableUserFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct NullableUserFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IDComparator")]
pub struct IDComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<cynic::Id>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetInitiativesVariables"
)]
pub struct GetInitiatives {
    // The schema declares this non-null, but a null response is handled
    // rather than treated as a decode failure.
    #[arguments(filter: $filter, includeArchived: $include_archived, first: $first, after: $after)]
    pub initiatives: Option<InitiativeConnection>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct InitiativeConnection {
    pub nodes: Vec<Initiative>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct Initiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: InitiativeStatus,
    pub target_date: Option<TimelessDate>,
    pub health: Option<InitiativeUpdateHealthType>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub url: String,
    pub archived_at: Option<DateTime>,
    pub owner: Option<InitiativeOwner>,
    pub projects: InitiativeProjects,
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", non_exhaustive)]
pub enum InitiativeStatus {
    #[cynic(rename = "Active")]
    Active,
    #[cynic(rename = "Canceled")]
    Canceled,
    #[cynic(rename = "Completed")]
    Completed,
    #[cynic(rename = "Planned")]
    Planned,
    #[cynic(rename = "Proposed")]
    Proposed,
    #[cynic(fallback)]
    Unknown(String),
}

impl InitiativeStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "Active",
            Self::Canceled => "Canceled",
            Self::Completed => "Completed",
            Self::Planned => "Planned",
            Self::Proposed => "Proposed",
            Self::Unknown(value) => value,
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Planned => 2,
            Self::Completed => 3,
            Self::Canceled | Self::Proposed | Self::Unknown(_) => 255,
        }
    }
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum InitiativeUpdateHealthType {
    AtRisk,
    OffTrack,
    OnTrack,
    #[cynic(fallback)]
    Unknown(String),
}

impl InitiativeUpdateHealthType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::AtRisk => "atRisk",
            Self::OffTrack => "offTrack",
            Self::OnTrack => "onTrack",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct InitiativeOwner {
    pub id: cynic::Id,
    pub display_name: String,
    pub initials: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
#[serde(transparent)]
pub struct InitiativeProjects {
    pub nodes: Vec<InitiativeProject>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct InitiativeProject {
    pub id: cynic::Id,
    pub name: String,
    pub status: InitiativeProjectStatus,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct InitiativeProjectStatus {
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetViewerIdVariables {}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetViewerIdVariables"
)]
pub struct GetViewerId {
    pub viewer: ViewerId,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct ViewerId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LookupUserVariables {
    pub input: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LookupUserVariables"
)]
pub struct LookupUser {
    #[arguments(filter: { or: [{ email: { eqIgnoreCase: $input } }, { displayName: { eqIgnoreCase: $input } }, { name: { containsIgnoreCaseAndAccent: $input } }] })]
    pub users: LookupUsers,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "UserConnection")]
pub struct LookupUsers {
    pub nodes: Vec<LookupUserNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct LookupUserNode {
    pub id: cynic::Id,
    pub email: String,
    pub display_name: String,
    pub name: String,
}
