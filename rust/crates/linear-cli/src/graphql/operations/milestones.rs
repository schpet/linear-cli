//! The exact `GetProjectMilestones` selection used by `milestone list`.

use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

use super::teams::PageInfo;

/// Deno declares `$first` and `$after` as nullable and omits `after` on the
/// first page, so both are skipped from the variables object when `None`.
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectMilestonesVariables {
    pub project_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// `Query.project` is non-null in the schema, but the source checks for a
/// null root and reports the project as not found, so only the root is
/// optional here. Every nested non-null field stays strict.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectMilestonesVariables"
)]
pub struct GetProjectMilestones {
    #[arguments(id: $project_id)]
    pub project: Option<MilestonesProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "GetProjectMilestonesVariables"
)]
pub struct MilestonesProject {
    pub id: cynic::Id,
    pub name: String,
    #[arguments(first: $first, after: $after)]
    pub project_milestones: ProjectMilestoneConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct ProjectMilestoneConnection {
    pub nodes: Vec<ProjectMilestone>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct ProjectMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub target_date: Option<TimelessDate>,
    pub sort_order: f64,
    pub project: MilestoneProjectRef,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct MilestoneProjectRef {
    pub id: cynic::Id,
    pub name: String,
}
