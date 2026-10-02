//! Team delete and the issue moves before it.
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::schema;

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
