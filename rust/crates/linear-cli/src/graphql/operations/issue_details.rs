//! An issue's title, URL and branch name, for `issue title`, `issue url` and
//! the VCS commands.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct Variables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "Variables")]
pub struct GetIssueDetails {
    #[arguments(id: $id)]
    pub issue: IssueDetails,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueDetails {
    pub title: String,
    pub url: String,
    pub branch_name: String,
}
