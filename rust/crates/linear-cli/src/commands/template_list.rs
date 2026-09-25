//! `template list`: one typed template request, the source filters and
//! display order, and the two output formats.
//!
//! [`prepare`] and [`run_with`] each attach [`CONTEXT`] once to their own
//! failures. Dispatch adds context only to failures outside those helpers,
//! such as network-runtime setup. Output is returned as bytes for the caller
//! to write with the console-like stdout policy.

use std::future::Future;

use cynic::QueryBuilder;
use serde::Serialize;
use serde_json::value::RawValue;

use crate::auth::CredentialStore;
use crate::commands::client;
use crate::commands::display::{display_width, pad, truncate_text};
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_resolver::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables,
};
use crate::graphql::operations::templates::{
    GetTemplates, InheritedTemplate, Template, TemplateCreator, TemplateTeam,
};
use crate::graphql::scalars::{DateTime, Json};
use crate::graphql::transport::GraphQlTransport;
use crate::json_number::finite_js_number;
use crate::platform::collation;
use crate::refs::{PreparedTeamLookup, WorkspaceScope, prepare_team_lookup, resolve_team};

pub const CONTEXT: &str = "Failed to list templates";

const ID_WIDTH: usize = 36;
const MIN_COLUMN_WIDTH: usize = 4;
const MIN_TRUNCATED_NAME_WIDTH: usize = 20;
const MAX_TEAM_WIDTH: usize = 15;
const SPACE_WIDTH: usize = 3;

/// The types `--type` accepts. Listing without a filter keeps any other type
/// Linear returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateType {
    Issue,
    Project,
    Document,
}

impl TemplateType {
    pub const ALL: [Self; 3] = [Self::Issue, Self::Project, Self::Document];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Project => "project",
            Self::Document => "document",
        }
    }

    /// Convert a value the route's `template-type` enum already accepted.
    pub fn from_route_value(value: &str) -> Result<Self, AppError> {
        Self::ALL
            .into_iter()
            .find(|template_type| template_type.as_str() == value)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    format!("template list received an unexpected --type value {value:?}"),
                )
            })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub template_type: Option<TemplateType>,
    pub json: bool,
}

pub struct Prepared {
    pub team: Option<PreparedTeamLookup>,
    pub transport: GraphQlTransport,
}

/// Local team-reference checks, then credential selection and client setup.
/// A bad explicit team reference fails before credentials are selected, and
/// its URL workspace check reads the same selection inputs as the transport.
pub fn prepare(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
    team: Option<&str>,
) -> Result<Prepared, AppError> {
    prepare_uncontextualized(options, credentials, cli_workspace, transport_env, team)
        .map_err(|error| error.with_context(CONTEXT))
}

fn prepare_uncontextualized(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
    team: Option<&str>,
) -> Result<Prepared, AppError> {
    let inputs = client::selection_inputs(options, cli_workspace)?;
    let team = team
        .map(|reference| {
            prepare_team_lookup(
                reference,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )
        })
        .transpose()?;
    let transport =
        client::prepare_transport_with_inputs(options, credentials, &inputs, transport_env)?;
    Ok(Prepared { team, transport })
}

pub fn request() -> GraphQlRequest<()> {
    GraphQlRequest::without_variables(GetTemplates::build(()))
}

/// Resolve the team (if any) before the single template request; a team
/// lookup failure never sends `GetTemplates`.
pub async fn run_with<RF, RFut, AF, AFut, TF, TFut>(
    team: Option<&PreparedTeamLookup>,
    options: Options,
    columns: usize,
    color: bool,
    resolve_fetch: RF,
    all_teams_fetch: AF,
    templates_fetch: TF,
) -> Result<Vec<u8>, AppError>
where
    RF: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> RFut,
    RFut: Future<Output = Result<ResolveTeam, AppError>>,
    AF: FnMut(GraphQlRequest<GetAllTeamsVariables>) -> AFut,
    AFut: Future<Output = Result<GetAllTeams, AppError>>,
    TF: FnOnce(GraphQlRequest<()>) -> TFut,
    TFut: Future<Output = Result<GetTemplates, AppError>>,
{
    run_uncontextualized(
        team,
        options,
        columns,
        color,
        resolve_fetch,
        all_teams_fetch,
        templates_fetch,
    )
    .await
    .map_err(|error| error.with_context(CONTEXT))
}

async fn run_uncontextualized<RF, RFut, AF, AFut, TF, TFut>(
    team: Option<&PreparedTeamLookup>,
    options: Options,
    columns: usize,
    color: bool,
    resolve_fetch: RF,
    all_teams_fetch: AF,
    templates_fetch: TF,
) -> Result<Vec<u8>, AppError>
where
    RF: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> RFut,
    RFut: Future<Output = Result<ResolveTeam, AppError>>,
    AF: FnMut(GraphQlRequest<GetAllTeamsVariables>) -> AFut,
    AFut: Future<Output = Result<GetAllTeams, AppError>>,
    TF: FnOnce(GraphQlRequest<()>) -> TFut,
    TFut: Future<Output = Result<GetTemplates, AppError>>,
{
    let team_id = match team {
        Some(prepared) => Some(
            resolve_team(prepared, resolve_fetch, all_teams_fetch)
                .await?
                .id,
        ),
        None => None,
    };
    let response = templates_fetch(request()).await?;
    let templates = select(
        response.templates,
        options.template_type,
        team_id.as_deref(),
    )?;
    if options.json {
        render_json(&templates)
    } else {
        Ok(render_text(&templates, columns, color).into_bytes())
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    team: Option<&PreparedTeamLookup>,
    options: Options,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(
        team,
        options,
        columns,
        color,
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
    )
    .await
}

/// Filter by type, then keep workspace templates and the resolved team's,
/// then stable-sort by type, lowercased name, workspace first, and team key.
fn select(
    templates: Vec<Template>,
    template_type: Option<TemplateType>,
    team_id: Option<&str>,
) -> Result<Vec<Template>, AppError> {
    let mut selected: Vec<Template> = templates
        .into_iter()
        .filter(|template| {
            template_type.is_none_or(|wanted| template.template_type == wanted.as_str())
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
    let collator = collation::root()?;
    selected.sort_by(|left, right| {
        collator
            .compare(&left.template_type, &right.template_type)
            .then_with(|| collator.compare(&left.name.to_lowercase(), &right.name.to_lowercase()))
            .then_with(|| left.team.is_some().cmp(&right.team.is_some()))
            .then_with(|| collator.compare(team_key(left), team_key(right)))
    });
    Ok(selected)
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

/// The source's GraphQL field names, nesting and nulls; `sortOrder` uses the
/// JavaScript number spelling.
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
    sort_order: Box<RawValue>,
    created_at: &'a DateTime,
    updated_at: &'a DateTime,
    team: Option<&'a TemplateTeam>,
    inherited_from: Option<&'a InheritedTemplate>,
    creator: Option<&'a TemplateCreator>,
    template_data: &'a Json,
}

fn render_json(templates: &[Template]) -> Result<Vec<u8>, AppError> {
    let projected = templates
        .iter()
        .map(|template| {
            Ok(JsonTemplate {
                id: template.id.inner(),
                name: &template.name,
                description: template.description.as_deref(),
                template_type: &template.template_type,
                icon: template.icon.as_deref(),
                color: template.color.as_deref(),
                has_form_fields: template.has_form_fields,
                last_applied_at: template.last_applied_at.as_ref(),
                sort_order: finite_js_number(template.sort_order)?,
                created_at: &template.created_at,
                updated_at: &template.updated_at,
                team: template.team.as_ref(),
                inherited_from: template.inherited_from.as_ref(),
                creator: template.creator.as_ref(),
                template_data: &template.template_data,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut output = serde_json::to_vec_pretty(&projected).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, "could not serialize templates").with_source(error)
    })?;
    output.push(b'\n');
    Ok(output)
}

/// The `ID NAME TYPE TEAM` table. NAME is truncated only when the widest name
/// does not fit, and then to no fewer than 20 columns; IDs, types and team
/// keys are padded but never truncated. Trailing padding is preserved.
pub fn render_text(templates: &[Template], columns: usize, color: bool) -> String {
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
    let mut output = if color {
        let underlined: Vec<String> = header.iter().map(|cell| format!("\x1b[4m{cell}")).collect();
        format!("{}\x1b[0m\n", underlined.join("\x1b[24m "))
    } else {
        format!("{}\n", header.join(" "))
    };
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
