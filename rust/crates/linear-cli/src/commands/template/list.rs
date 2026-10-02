//! `template list`: every template, filtered by type and team, as a table or
//! JSON.
use cynic::QueryBuilder;

use crate::cli::TemplateType;
use crate::cli::template::TemplateList;
use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::table;
use crate::commands::template::json as template_json;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::templates::{GetTemplates, Template};
use crate::platform::{collation, style};
use crate::refs::{prepare_team_lookup, resolve_team_with_transport};

const ID_WIDTH: usize = 36;
const MIN_COLUMN_WIDTH: usize = 4;
const MIN_TRUNCATED_NAME_WIDTH: usize = 20;
const MAX_TEAM_WIDTH: usize = 15;
const SPACE_WIDTH: usize = 3;

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
        let data: GetTemplates = client.execute(&request()).await?;
        Ok::<_, Error>((data.templates, team_id))
    })?;
    let templates = select(templates, args.r#type, team_id.as_deref());
    if args.json {
        ctx.print(template_json::render_list(&templates))
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(&templates, columns, ctx.color()))
    }
}

/// Every template in the workspace, unfiltered (the API takes no filter).
pub(super) fn request() -> GraphQlRequest<()> {
    GraphQlRequest::without_variables(GetTemplates::build(()))
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

/// The `ID NAME TYPE TEAM` table. NAME is truncated only when the widest name
/// does not fit, and then to no fewer than 20 columns; IDs, types and team
/// keys are padded but never truncated. Trailing padding is preserved.
fn render_text(templates: &[Template], columns: usize, color: bool) -> String {
    if templates.is_empty() {
        return "No templates found.\n".to_owned();
    }
    let type_cells: Vec<String> = templates.iter().map(type_cell).collect();
    let type_width = type_cells
        .iter()
        .map(|cell| display_width(cell))
        .fold(MIN_COLUMN_WIDTH, usize::max);
    let team_width = templates
        .iter()
        .map(|template| display_width(scope_label(template)))
        .fold(MIN_COLUMN_WIDTH, usize::max)
        .min(MAX_TEAM_WIDTH);
    let fixed = ID_WIDTH + type_width + team_width + SPACE_WIDTH;
    let max_name_width = templates
        .iter()
        .map(|template| display_width(&template.name))
        .fold(MIN_COLUMN_WIDTH, usize::max);
    let available_width = columns.saturating_sub(1).saturating_sub(fixed);
    let name_width = max_name_width.min(available_width.max(MIN_TRUNCATED_NAME_WIDTH));

    let header = [
        pad("ID", ID_WIDTH),
        pad("NAME", name_width),
        pad("TYPE", type_width),
        pad("TEAM", team_width),
    ];
    let mut output = format!("{}\n", style::underline(&header.join(" "), color));
    for (template, type_cell) in templates.iter().zip(&type_cells) {
        output.push_str(&format!(
            "{} {} {} {}\n",
            pad(template.id.inner(), ID_WIDTH),
            pad(&truncate_text(&template.name, name_width), name_width),
            pad(type_cell, type_width),
            pad(scope_label(template), team_width),
        ));
    }
    let noun = if templates.len() == 1 {
        "template"
    } else {
        "templates"
    };
    output.push_str(&format!("\n{} {noun} found.\n", templates.len()));
    output
}
