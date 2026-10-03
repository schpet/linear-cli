//! `GetIssueLabels`: the paginated connection used by `label list`.
//!
//! An empty filter and the first-page cursor are omitted from the variables
//! object when `None` rather than sent as `null`.

use serde::Serialize;

use crate::graphql::operations::teams::StringComparator;
use crate::graphql::pagination::PageInfo;
use crate::graphql::schema;

/// Variables for [`GetIssueLabels`]. `first` is the document's nullable
/// `Int`; `label list` sends `100`.
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetIssueLabelsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<IssueLabelFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// The subset of the schema's `IssueLabelFilter` that `label list` sends.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabelFilter")]
pub struct IssueLabelFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub or: Option<Vec<IssueLabelFilter>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team: Option<NullableTeamFilter>,
}

/// The subset of the schema's `NullableTeamFilter` that `label list` sends.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "NullableTeamFilter")]
pub struct NullableTeamFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub null: Option<bool>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueLabelsVariables"
)]
pub struct GetIssueLabels {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub issue_labels: IssueLabelConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct IssueLabelConnection {
    pub nodes: Vec<IssueLabel>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct IssueLabel {
    pub id: cynic::Id,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub team: Option<Team>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct Team {
    pub key: String,
    pub name: String,
}
