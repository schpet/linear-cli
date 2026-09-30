//! Exact selections for initiative association commands; local display lookups.
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SlugVariables {
    pub slug_id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct NameVariables {
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeNameById {
    #[arguments(id: $id)]
    pub initiative: Option<InitiativeName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForAddProject {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub initiatives: InitiativeSlugResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForAddProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: InitiativeNameResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeNameByIdForRemove {
    #[arguments(id: $id)]
    pub initiative: Option<InitiativeName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForRemoveProject {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub initiatives: InitiativeSlugResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForRemoveProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: InitiativeNameResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeNameResults {
    pub nodes: Vec<InitiativeName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeName {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeSlugResults {
    pub nodes: Vec<InitiativeSlug>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeSlug {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetProjectNameById {
    #[arguments(id: $id)]
    pub project: Option<ProjectName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetProjectBySlugForAddProject {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub projects: ProjectSlugResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetProjectByNameForAddProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub projects: ProjectNameResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetProjectNameByIdForRemove {
    #[arguments(id: $id)]
    pub project: Option<ProjectName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetProjectBySlugForRemoveProject {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub projects: ProjectSlugResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetProjectByNameForRemoveProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub projects: ProjectNameResults,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct ProjectNameResults {
    pub nodes: Vec<ProjectName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectName {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct ProjectSlugResults {
    pub nodes: Vec<ProjectSlug>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectSlug {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
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
    pub initiative_to_project: LinkId,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct LinkId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LinksVariables {
    pub first: Option<i32>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LinksVariables"
)]
pub struct GetInitiativeToProjects {
    #[arguments(first: $first)]
    pub initiative_to_projects: Links,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectConnection")]
pub struct Links {
    pub nodes: Vec<Link>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct Link {
    pub id: cynic::Id,
    pub initiative: Option<InitiativeId>,
    pub project: Option<ProjectId>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectId {
    pub id: cynic::Id,
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
