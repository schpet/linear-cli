//! Minimal source lookup shared by relation mutations and URL links.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct Variables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "Variables")]
pub struct GetIssueId {
    #[arguments(id: $id)]
    pub issue: IssueId,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueId {
    pub id: cynic::Id,
}
