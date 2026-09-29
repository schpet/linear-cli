//! The source `CreateInitiative` mutation and its optional input fields.
use crate::graphql::operations::initiatives::InitiativeStatus;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateInitiativeVariables {
    pub input: InitiativeCreateInput,
}

#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeCreateInput")]
pub struct InitiativeCreateInput {
    pub name: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<InitiativeStatus>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateInitiativeVariables"
)]
pub struct CreateInitiative {
    #[arguments(input: $input)]
    pub initiative_create: CreateInitiativePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativePayload")]
pub struct CreateInitiativePayload {
    pub success: bool,
    pub initiative: CreatedInitiative,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CreatedInitiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}
