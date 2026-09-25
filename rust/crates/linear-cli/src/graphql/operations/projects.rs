//! Typed documents for `project list`. The project selection follows the
//! frozen Deno `GetProjects` document in field order.

use serde::Serialize;

use crate::graphql::operations::teams::{PageInfo, StringComparator, TeamFilter};
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ProjectFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct ProjectFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub accessible_teams: Option<TeamCollectionFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProjectStatusFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct TeamCollectionFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub some: Option<TeamFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct ProjectStatusFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<StringComparator>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectsVariables"
)]
pub struct GetProjects {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub projects: ProjectConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct ProjectConnection {
    pub nodes: Vec<Project>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct Project {
    pub id: cynic::Id,
    pub name: String,
    pub description: String,
    pub slug_id: String,
    pub icon: Option<String>,
    pub color: String,
    pub sort_order: f64,
    pub status: ProjectStatus,
    pub lead: Option<ProjectLead>,
    pub priority: i32,
    pub health: Option<ProjectUpdateHealthType>,
    pub start_date: Option<TimelessDate>,
    pub target_date: Option<TimelessDate>,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub canceled_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub url: String,
    pub teams: ProjectTeams,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub status_type: ProjectStatusType,
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum ProjectStatusType {
    Backlog,
    Canceled,
    Completed,
    Paused,
    Planned,
    Started,
    #[cynic(fallback)]
    Unknown(String),
}

impl ProjectStatusType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Backlog => "backlog",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Planned => "planned",
            Self::Started => "started",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum ProjectUpdateHealthType {
    AtRisk,
    OffTrack,
    OnTrack,
    #[cynic(fallback)]
    Unknown(String),
}

impl ProjectUpdateHealthType {
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
pub struct ProjectLead {
    pub name: String,
    pub display_name: String,
    pub initials: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct ProjectTeams {
    pub nodes: Vec<ProjectTeam>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct ProjectTeam {
    pub key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetViewer {
    pub viewer: Viewer,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct Viewer {
    pub organization: ViewerOrganization,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Organization")]
pub struct ViewerOrganization {
    pub url_key: String,
}
