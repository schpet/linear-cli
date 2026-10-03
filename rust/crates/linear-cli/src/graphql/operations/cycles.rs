//! The exact `GetTeamCycles` selection used by `cycle list`.

use serde::Serialize;

use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

use crate::graphql::pagination::PageInfo;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTeamCyclesVariables {
    pub team_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTeamCyclesVariables"
)]
pub struct GetTeamCycles {
    #[arguments(id: $team_id)]
    pub team: TeamCycles,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Team",
    variables = "GetTeamCyclesVariables"
)]
pub struct TeamCycles {
    pub id: cynic::Id,
    pub name: String,
    #[arguments(first: $first, after: $after)]
    pub cycles: CycleConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct CycleConnection {
    pub nodes: Vec<Cycle>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub id: cynic::Id,
    pub number: crate::graphql::operations::number::WholeNumber,
    pub name: Option<String>,
    pub starts_at: DateTime,
    pub ends_at: DateTime,
    pub completed_at: Option<DateTime>,
    pub is_active: bool,
    pub is_future: bool,
    pub is_past: bool,
}
