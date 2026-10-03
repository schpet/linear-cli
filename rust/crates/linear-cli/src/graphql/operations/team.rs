//! Team operations: lists, members, workflow states, lookup, create and delete.

use serde::Serialize;

use super::user::User;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

pub use resolve_team::ResolveTeam;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateTeamVariables {
    pub input: TeamCreateInput,
}

/// Absent optionals are omitted and `private` is sent only as `true`.
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamCreateInput")]
pub struct TeamCreateInput {
    pub name: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateTeamVariables"
)]
pub struct CreateTeam {
    #[arguments(input: $input)]
    pub team_create: CreateTeamPayload,
}

/// `team` is nullable in the schema; the command reports a null team as a
/// failed create rather than decoding it away.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamPayload")]
pub struct CreateTeamPayload {
    pub success: bool,
    pub team: Option<CreatedTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct CreatedTeam {
    pub id: cynic::Id,
    pub name: String,
    pub key: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct MovePageVariables {
    pub team_id: String,
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "MovePageVariables"
)]
pub struct GetTeamIssuesForMove {
    #[arguments(id: $team_id)]
    pub team: Option<MoveTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Team",
    variables = "MovePageVariables"
)]
pub struct MoveTeam {
    #[arguments(first: $first, after: $after)]
    pub issues: MoveIssues,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct MoveIssues {
    pub nodes: Vec<MoveIssue>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct MoveIssue {
    pub id: cynic::Id,
    pub identifier: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct MoveVariables {
    pub id: String,
    pub team_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "MoveVariables"
)]
pub struct MoveIssueToTeam {
    #[arguments(id: $id, input: { teamId: $team_id })]
    pub issue_update: MovePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssuePayload")]
pub struct MovePayload {
    pub success: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteTeam {
    #[arguments(id: $id)]
    pub team_delete: DeletePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletePayload {
    pub success: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTeamMembersVariables {
    pub team_key: String,
    pub include_disabled: bool,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTeamMembersVariables"
)]
pub struct GetTeamMembers {
    #[arguments(id: $team_key)]
    pub team: MembersTeam,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Team",
    variables = "GetTeamMembersVariables"
)]
pub struct MembersTeam {
    #[arguments(includeDisabled: $include_disabled, first: $first, after: $after)]
    pub members: MemberConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "UserConnection",
    variables = "GetTeamMembersVariables"
)]
pub struct MemberConnection {
    pub nodes: Vec<User>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ResolveTeamVariables {
    pub reference: String,
    pub id: Option<cynic::Id>,
    pub is_uuid: bool,
}

#[allow(non_snake_case)]
mod resolve_team {
    use super::*;

    #[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
    #[cynic(
        schema = "linear",
        graphql_type = "Query",
        variables = "ResolveTeamVariables"
    )]
    pub struct ResolveTeam {
        #[arguments(filter: { or: [{ key: { eqIgnoreCase: $reference } }, { name: { eqIgnoreCase: $reference } }] })]
        pub teams: TeamNodes,
        #[arguments(filter: { id: { eq: $id } })]
        #[directives(include(if: $is_uuid))]
        #[cynic(rename = "teams", alias)]
        pub teamById: Option<TeamNodes>,
    }
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetAllTeamsVariables {
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetAllTeamsVariables"
)]
pub struct GetAllTeams {
    #[arguments(first: $first, after: $after)]
    pub teams: TeamPage,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamNodes {
    pub nodes: Vec<TeamNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamPage {
    pub nodes: Vec<TeamNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct TeamNode {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}

/// Variables for [`GetTeams`]. `first` is the document's nullable `Int`;
/// `team list` sends `100`, and an unset `first` is omitted rather than sent
/// as `null`.
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTeamsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<TeamFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// A subset of the schema's `TeamFilter` input object.
///
/// No command sends a team filter yet; unset keys are omitted. Extend field
/// by field as commands need them.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamFilter")]
pub struct TeamFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringComparator>,
}

/// A subset of the schema's `StringComparator` input object.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "StringComparator")]
pub struct StringComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub contains_ignore_case: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTeamsVariables"
)]
pub struct GetTeams {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub teams: TeamConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct TeamConnection {
    pub nodes: Vec<Team>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: cynic::Id,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub cycles_enabled: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub archived_at: Option<DateTime>,
    pub organization: Organization,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct Organization {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetWorkflowStatesVariables {
    pub team_key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetWorkflowStatesVariables"
)]
pub struct GetWorkflowStates {
    #[arguments(id: $team_key)]
    pub team: WorkflowTeam,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct WorkflowTeam {
    pub states: WorkflowStateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear")]
pub struct WorkflowStateConnection {
    pub nodes: Vec<WorkflowState>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct WorkflowState {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub state_type: String,
    pub position: crate::graphql::scalars::Float,
}
