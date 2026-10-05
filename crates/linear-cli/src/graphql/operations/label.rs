//! Issue label operations: list, create and delete.

use serde::Serialize;

use super::common::DeletePayload;
use super::common::IdVariablesFields;
use super::common::NameVariablesFields;
use super::team::StringComparator;
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

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateIssueLabelVariables {
    pub input: IssueLabelCreateInput,
}

#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabelCreateInput")]
pub struct IssueLabelCreateInput {
    pub name: String,
    pub color: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateIssueLabelVariables"
)]
pub struct CreateIssueLabel {
    #[arguments(input: $input)]
    pub issue_label_create: CreateIssueLabelPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabelPayload")]
pub struct CreateIssueLabelPayload {
    pub success: bool,
    // Non-null in the SDL: malformed null labels fail typed decoding.
    pub issue_label: CreatedIssueLabel,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct CreatedIssueLabel {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub team: Option<CreatedLabelTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct CreatedLabelTeam {
    pub key: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetLabelById {
    #[arguments(id: $id)]
    pub issue_label: Label,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetLabelByName {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub issue_labels: LabelConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
pub struct LabelConnection {
    pub nodes: Vec<Label>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct Label {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    pub team: Option<LabelTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct LabelTeam {
    pub key: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteIssueLabel {
    #[arguments(id: $id)]
    pub issue_label_delete: DeletePayload,
}
