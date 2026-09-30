//! The source `CreateTeam` mutation and the four input fields it can send.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateTeamVariables {
    pub input: TeamCreateInput,
}

/// Deno builds `{ name, description, key, private }` with `|| undefined`, so
/// absent optionals are omitted and `private` is sent only as `true`.
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamCreateInput")]
pub struct TeamCreateInput {
    pub name: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateTeamVariables"
)]
pub struct CreateTeam {
    #[arguments(input: $input)]
    pub team_create: CreateTeamPayload,
}

/// `team` is nullable in the schema; the command reports a null team as a
/// failed create rather than decoding it away.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamPayload")]
pub struct CreateTeamPayload {
    pub success: bool,
    pub team: Option<CreatedTeam>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct CreatedTeam {
    pub id: cynic::Id,
    pub name: String,
    pub key: String,
}
