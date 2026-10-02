//! Team delete and the lookups it needs.
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::{edit::Edit, schema};
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetTeamDetails {
    #[arguments(id: $id)]
    pub team: Option<TeamDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct TeamDetails {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
    pub issues: DetailIssues,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct DetailIssues {
    pub nodes: Vec<DetailIssue>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DetailIssue {
    pub id: cynic::Id,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct MovePageVariables {
    pub team_id: String,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub after: Edit<String>,
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
