//! Cycle operations: team cycle lists, lookups and details.

use serde::Serialize;

use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LookupVariables {
    pub team_id: String,
    // Sent as an explicit null on the first page.
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LookupVariables"
)]
pub struct GetTeamCyclesForLookup {
    #[arguments(id: $team_id)]
    pub team: Option<LookupTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Team",
    variables = "LookupVariables"
)]
pub struct LookupTeam {
    pub key: String,
    pub cycles_enabled: bool,
    // Non-null in the schema, but Linear sends null for a team without
    // cycles; that is reported as such rather than as a decode failure.
    #[arguments(first: 250, after: $after)]
    pub cycles: Option<LookupConnection>,
    pub active_cycle: Option<ActiveCycle>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "CycleConnection")]
pub struct LookupConnection {
    pub nodes: Vec<LookupCycle>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
pub struct LookupCycle {
    pub id: cynic::Id,
    pub number: crate::graphql::scalars::WholeNumber,
    pub name: Option<String>,
    pub starts_at: DateTime,
    pub is_next: bool,
    pub is_previous: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
pub struct ActiveCycle {
    pub id: cynic::Id,
    pub number: crate::graphql::scalars::WholeNumber,
    pub name: Option<String>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DetailVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetCycleDetails {
    #[arguments(id: $id)]
    pub cycle: Option<DetailCycle>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
pub struct DetailCycle {
    pub id: cynic::Id,
    pub number: crate::graphql::scalars::WholeNumber,
    pub name: Option<String>,
    pub description: Option<String>,
    pub starts_at: DateTime,
    pub ends_at: DateTime,
    pub completed_at: Option<DateTime>,
    pub is_active: bool,
    pub is_future: bool,
    pub is_past: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub team: DetailTeam,
    pub issues: DetailIssues,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct DetailTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct DetailIssues {
    pub nodes: Vec<DetailIssue>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DetailIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    pub state: DetailState,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
pub struct DetailState {
    pub name: String,
    #[cynic(rename = "type")]
    pub state_type: String,
}

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
    pub number: crate::graphql::scalars::WholeNumber,
    pub name: Option<String>,
    pub starts_at: DateTime,
    pub ends_at: DateTime,
    pub completed_at: Option<DateTime>,
    pub is_active: bool,
    pub is_future: bool,
    pub is_past: bool,
}
