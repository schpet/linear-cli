//! Issue and project templates.

use serde::Serialize;

use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::Json;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetTemplates {
    pub templates: Vec<Template>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTemplateVariables {
    pub id: String,
}

/// `template(id:)` is non-null in the schema: Linear reports a missing ID as a
/// GraphQL error, and a `null` field fails the typed decode.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTemplateVariables"
)]
pub struct GetTemplate {
    #[arguments(id: $id)]
    pub template: Template,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct Template {
    pub id: cynic::Id,
    pub name: String,
    pub description: Option<String>,
    /// Linear stores the type as a plain string; unknown types pass through.
    #[cynic(rename = "type")]
    pub template_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub has_form_fields: bool,
    pub last_applied_at: Option<DateTime>,
    pub sort_order: crate::graphql::scalars::Float,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub team: Option<TemplateTeam>,
    pub inherited_from: Option<InheritedTemplate>,
    pub creator: Option<TemplateCreator>,
    /// Stringified JSON, retained verbatim.
    pub template_data: Json,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct TemplateTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Template")]
pub struct InheritedTemplate {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct TemplateCreator {
    pub id: cynic::Id,
    pub name: String,
}
