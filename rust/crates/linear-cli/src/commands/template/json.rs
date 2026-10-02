//! The `--json` shape shared by `template list` and `template view`.

use serde::Serialize;

use crate::error::Error;
use crate::graphql::operations::number::Float;
use crate::graphql::operations::templates::{
    InheritedTemplate, Template, TemplateCreator, TemplateTeam,
};
use crate::graphql::scalars::{DateTime, Json};

/// The GraphQL field names, nesting and nulls.
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
    team: Option<&'a TemplateTeam>,
    inherited_from: Option<&'a InheritedTemplate>,
    creator: Option<&'a TemplateCreator>,
    template_data: &'a Json,
}

/// Serialize a template list in GraphQL field order.
pub fn render_list(templates: &[Template]) -> Result<Vec<u8>, Error> {
    let projected = templates
        .iter()
        .map(project)
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = serde_json::to_vec_pretty(&projected)
        .map_err(|error| Error::new("could not serialize templates").with_source(error))?;
    output.push(b'\n');
    Ok(output)
}

/// Serialize one template with the same field order and number spelling as the list.
pub fn render_one(template: &Template) -> Result<Vec<u8>, Error> {
    let projected = project(template)?;
    let mut output = serde_json::to_vec_pretty(&projected)
        .map_err(|error| Error::new("could not serialize template").with_source(error))?;
    output.push(b'\n');
    Ok(output)
}

fn project(template: &Template) -> Result<JsonTemplate<'_>, Error> {
    Ok(JsonTemplate {
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
    })
}
