//! `template list`: every template, filtered by type and team, as a table or
//! JSON.
use crate::cli::TemplateType;
use crate::cli::template::TemplateList;
use crate::commands::table::{Cell, Column, Table};
use crate::commands::template::json as template_json;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::templates::{GetTemplates, Template};
use crate::platform::collation;
use crate::refs::{prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, args: &TemplateList) -> Result<()> {
    list(ctx, args).context("Failed to list templates")
}

fn list(ctx: &Ctx, args: &TemplateList) -> Result<()> {
    let team = args
        .team
        .as_deref()
        .map(|team| prepare_team_lookup(team, &ctx.scope()?))
        .transpose()?;
    let client = ctx.client()?;
    let (templates, team_id) = ctx.spin(!args.json, async {
        let team_id = match &team {
            Some(lookup) => Some(resolve_team_with_transport(lookup, client).await?.id),
            None => None,
        };
        let data: GetTemplates = client.query(()).await?;
        Ok::<_, Error>((data.templates, team_id))
    })?;
    let mut templates = select(templates, args.r#type, team_id.as_deref());
    args.limit.apply(&mut templates);
    if args.json {
        ctx.print(template_json::render_list(&templates))
    } else if templates.is_empty() {
        ctx.print("No templates found.\n")
    } else {
        ctx.print(render_text(&templates).render_for(ctx))
    }
}

fn type_name(template_type: TemplateType) -> &'static str {
    match template_type {
        TemplateType::Issue => "issue",
        TemplateType::Project => "project",
        TemplateType::Document => "document",
    }
}

/// Filter by type, then keep workspace templates and the resolved team's,
/// then stable-sort by type, lowercased name, workspace first, and team key.
fn select(
    templates: Vec<Template>,
    template_type: Option<TemplateType>,
    team_id: Option<&str>,
) -> Vec<Template> {
    let mut selected: Vec<Template> = templates
        .into_iter()
        .filter(|template| {
            template_type.is_none_or(|wanted| template.template_type == type_name(wanted))
        })
        .filter(|template| {
            team_id.is_none_or(|id| {
                template
                    .team
                    .as_ref()
                    .is_none_or(|team| team.id.inner() == id)
            })
        })
        .collect();
    selected.sort_by(|left, right| {
        collation::compare(&left.template_type, &right.template_type)
            .then_with(|| collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase()))
            .then_with(|| left.team.is_some().cmp(&right.team.is_some()))
            .then_with(|| collation::compare(team_key(left), team_key(right)))
    });
    selected
}

fn team_key(template: &Template) -> &str {
    template.team.as_ref().map_or("", |team| team.key.as_str())
}

fn scope_label(template: &Template) -> &str {
    template
        .team
        .as_ref()
        .map_or("Workspace", |team| team.key.as_str())
}

fn type_cell(template: &Template) -> String {
    if template.has_form_fields {
        format!("{} (form)", template.template_type)
    } else {
        template.template_type.clone()
    }
}

fn render_text(templates: &[Template]) -> Table {
    let mut table = Table::new([
        Column::fixed("ID"),
        Column::flexible("NAME"),
        Column::fixed("TYPE"),
        Column::fixed("TEAM"),
    ]);
    for template in templates {
        table.row([
            Cell::from(template.id.inner()),
            Cell::from(template.name.as_str()),
            Cell::from(type_cell(template)),
            Cell::from(scope_label(template)),
        ]);
    }
    table
}
