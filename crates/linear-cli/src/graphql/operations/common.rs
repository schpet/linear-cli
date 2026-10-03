//! Variables and selections shared by operations on several entities.

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
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletePayload {
    pub success: bool,
}
