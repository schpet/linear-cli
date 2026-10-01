use crate::graphql::{edit::Edit, scalars::TimelessDate, schema};
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueCreateInput")]
pub struct IssueCreateInput {
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub title: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub assignee_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub due_date: Edit<TimelessDate>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub parent_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub priority: Edit<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub estimate: Edit<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
    pub team_id: String,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_milestone_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub cycle_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub state_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub template_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub use_default_template: Edit<bool>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub description: Edit<String>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CreateIssueVariables {
    pub input: IssueCreateInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateIssueVariables"
)]
pub struct CreateIssue {
    #[arguments(input:$input)]
    pub issue_create: CreatePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssuePayload")]
pub struct CreatePayload {
    pub success: bool,
    pub issue: Option<CreatedIssue>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct CreatedIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub url: String,
    pub team: CreatedTeam,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct CreatedTeam {
    pub key: String,
}
