//! The detail and name-lookup selections used by `milestone view`.

use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DetailVariables {
    pub id: String,
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetMilestoneDetails {
    #[arguments(id: $id)]
    pub project_milestone: Option<DetailMilestone>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "ProjectMilestone",
    variables = "DetailVariables"
)]
pub struct DetailMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub description: Option<String>,
    pub target_date: Option<TimelessDate>,
    pub sort_order: crate::graphql::operations::number::Float,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub project: DetailProject,
    #[arguments(first: $first, after: $after)]
    pub issues: DetailIssues,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct DetailProject {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
    pub url: String,
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

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
pub struct DetailState {
    pub name: String,
    #[cynic(rename = "type")]
    pub state_type: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LookupVariables {
    pub project_id: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LookupVariables"
)]
pub struct GetProjectMilestonesForLookup {
    #[arguments(id: $project_id)]
    pub project: Option<LookupProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "LookupVariables"
)]
pub struct LookupProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub project_milestones: Option<LookupConnection>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestoneConnection")]
pub struct LookupConnection {
    pub nodes: Vec<LookupMilestone>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct LookupMilestone {
    pub id: cynic::Id,
    pub name: String,
}
