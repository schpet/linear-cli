//! Label lookup and delete selections.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct NameVariables {
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
    pub issue_label_delete: DeleteLabelPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeleteLabelPayload {
    pub success: bool,
}
