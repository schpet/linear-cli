//! Links between initiatives and projects for `initiative add-project` and `remove-project`.
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LinksVariables {
    pub initiative_id: String,
    pub project_id: String,
    pub after: Option<String>,
}

/// Both names, and one page of the initiatives the project is linked to.
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LinksVariables"
)]
pub struct GetInitiativeProjectLinks {
    #[arguments(id: $initiative_id)]
    pub initiative: Named,
    #[arguments(id: $project_id)]
    pub project: LinkedProject,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct Named {
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "LinksVariables"
)]
pub struct LinkedProject {
    pub name: String,
    #[arguments(first: 100, after: $after)]
    pub initiative_to_projects: Links,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectConnection")]
pub struct Links {
    pub nodes: Vec<Link>,
    pub page_info: PageInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct Link {
    pub id: cynic::Id,
    pub initiative: LinkedInitiative,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct LinkedInitiative {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct AddVariables {
    pub input: InitiativeToProjectCreateInput,
}
#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectCreateInput")]
pub struct InitiativeToProjectCreateInput {
    pub initiative_id: String,
    pub project_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<f64>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "AddVariables"
)]
pub struct AddProjectToInitiative {
    #[arguments(input: $input)]
    pub initiative_to_project_create: AddPayload,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectPayload")]
pub struct AddPayload {
    pub success: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct RemoveProjectFromInitiative {
    #[arguments(id: $id)]
    pub initiative_to_project_delete: DeletePayload,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletePayload {
    pub success: bool,
}
