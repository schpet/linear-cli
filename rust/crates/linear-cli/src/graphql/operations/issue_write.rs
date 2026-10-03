use crate::graphql::{
    edit::Edit, operations::projects::ProjectFilter, pagination::PageInfo, schema,
};
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetUserSettings {
    pub user_settings: UserSettings,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear")]
pub struct UserSettings {
    pub auto_assign_to_self: bool,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TeamKey {
    pub team_key: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "TeamKey")]
pub struct GetLabelsForTeam {
    #[arguments(id:$team_key)]
    pub team: Option<LabelTeam>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct LabelTeam {
    pub labels: LabelConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
pub struct LabelConnection {
    pub nodes: Vec<Label>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct Label {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LabelVariables {
    pub name: String,
    pub team_key: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LabelVariables"
)]
pub struct GetIssueLabelIdByNameForTeam {
    #[arguments(filter:{name:{eqIgnoreCase:$name},or:[{team:{key:{eq:$team_key}}},{team:{null:true}}]})]
    pub issue_labels: LookupLabelConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LabelVariables"
)]
pub struct GetIssueLabelIdOptionsByNameForTeam {
    #[arguments(filter:{name:{containsIgnoreCase:$name},or:[{team:{key:{eq:$team_key}}},{team:{null:true}}]})]
    pub issue_labels: LookupLabelConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
pub struct LookupLabelConnection {
    pub nodes: Vec<LookupLabel>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct LookupLabel {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct NamedProject {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ProjectsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ProjectFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub after: Edit<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectsVariables"
)]
pub struct GetProjectsForTeam {
    #[arguments(filter:$filter,first:$first,after:$after)]
    pub projects: ProjectsPage,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct ProjectsPage {
    pub nodes: Vec<NamedProject>,
    pub page_info: PageInfo,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IssueVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssueVariables"
)]
pub struct GetParentIssueData {
    #[arguments(id:$id)]
    pub issue: Option<ParentIssue>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ParentIssue {
    pub title: String,
    pub identifier: String,
    pub project: Option<ProjectId>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssueVariables"
)]
pub struct GetIssueProjectId {
    #[arguments(id:$id)]
    pub issue: Option<IssueProject>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueProject {
    pub project: Option<ProjectId>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TeamSubstring {
    pub team: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "TeamSubstring")]
pub struct GetTeamIdOptionsByKey {
    #[arguments(filter:{key:{containsIgnoreCase:$team}})]
    pub teams: TeamNames,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamNames {
    pub nodes: Vec<NamedTeam>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct NamedTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}
