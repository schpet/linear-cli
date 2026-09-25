//! `GetTemplates`: the unpaginated workspace template list used by
//! `template list`.
//!
//! Mirrors the `GetTemplates` document in `src/utils/templates.ts`
//! field-for-field and in selection order. The oracle sends no variables.
//! `sortOrder` is a `Float!`, so [`Template`] deliberately does not implement
//! `Serialize`: the command projects it through the JS number formatter
//! instead of Serde's `f64` spelling.

use serde::Serialize;

use crate::graphql::scalars::{DateTime, Json};
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetTemplates {
    pub templates: Vec<Template>,
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
    pub sort_order: f64,
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
