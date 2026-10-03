//! The `--json` shape shared by `template list` and `template view`.

use crate::graphql::operations::team::TeamRef;
use serde::Serialize;

use crate::commands::json;
use crate::graphql::operations::template::{InheritedTemplate, Template, TemplateCreator};
use crate::graphql::scalars::Float;
use crate::graphql::scalars::{DateTime, Json};

/// A template with its GraphQL field names, nesting and nulls.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTemplate<'a> {
    id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    #[serde(rename = "type")]
    template_type: &'a str,
    icon: Option<&'a str>,
    color: Option<&'a str>,
    has_form_fields: bool,
    last_applied_at: Option<&'a DateTime>,
    sort_order: &'a Float,
    created_at: &'a DateTime,
    updated_at: &'a DateTime,
    team: Option<&'a TeamRef>,
    inherited_from: Option<&'a InheritedTemplate>,
    creator: Option<&'a TemplateCreator>,
    template_data: &'a Json,
}

pub fn render_list(templates: &[Template]) -> Vec<u8> {
    let projected: Vec<_> = templates.iter().map(project).collect();
    json::render(&projected)
}

pub fn render_one(template: &Template) -> Vec<u8> {
    json::render(&project(template))
}

fn project(template: &Template) -> JsonTemplate<'_> {
    JsonTemplate {
        id: template.id.inner(),
        name: &template.name,
        description: template.description.as_deref(),
        template_type: &template.template_type,
        icon: template.icon.as_deref(),
        color: template.color.as_deref(),
        has_form_fields: template.has_form_fields,
        last_applied_at: template.last_applied_at.as_ref(),
        sort_order: &template.sort_order,
        created_at: &template.created_at,
        updated_at: &template.updated_at,
        team: template.team.as_ref(),
        inherited_from: template.inherited_from.as_ref(),
        creator: template.creator.as_ref(),
        template_data: &template.template_data,
    }
}
