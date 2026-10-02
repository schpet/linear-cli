//! The `CreateIssueLabel` mutation and the fields `label create` sends.
use crate::graphql::schema;

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
