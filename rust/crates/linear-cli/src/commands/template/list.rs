//! `template list`: one template request, filtered by type and team, as a
//! table or JSON.
//!
//! [`prepare`] and [`run_with`] each attach [`CONTEXT`] once to their own
//! failures. Dispatch adds context only to failures outside those helpers,
//! such as network-runtime setup. Output is returned as bytes for the caller
//! to write with the console-like stdout policy.

use std::future::Future;

use cynic::QueryBuilder;

use crate::auth::CredentialStore;
use crate::commands::client;
use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::template::json as template_json;
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{Error, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_resolver::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables,
};
use crate::graphql::operations::templates::{GetTemplates, Template};
use crate::graphql::transport::GraphQlTransport;
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Project => "project",
            Self::Document => "document",
        }
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
) -> Result<Prepared, Error> {
    prepare_uncontextualized(options, credentials, cli_workspace, transport_env, team)
        .context(CONTEXT)
}

fn prepare_uncontextualized(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
    team: Option<&str>,
) -> Result<Prepared, Error> {
    let inputs = client::selection_inputs(options, cli_workspace);
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
) -> Result<Vec<u8>, Error>
where
    RF: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> RFut,
    RFut: Future<Output = Result<ResolveTeam, Error>>,
    AF: FnMut(GraphQlRequest<GetAllTeamsVariables>) -> AFut,
    AFut: Future<Output = Result<GetAllTeams, Error>>,
    TF: FnOnce(GraphQlRequest<()>) -> TFut,
    TFut: Future<Output = Result<GetTemplates, Error>>,
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
    .context(CONTEXT)
}

async fn run_uncontextualized<RF, RFut, AF, AFut, TF, TFut>(
    team: Option<&PreparedTeamLookup>,
    options: Options,
    columns: usize,
    color: bool,
    resolve_fetch: RF,
    all_teams_fetch: AF,
    templates_fetch: TF,
) -> Result<Vec<u8>, Error>
where
    RF: FnOnce(GraphQlRequest<ResolveTeamVariables>) -> RFut,
    RFut: Future<Output = Result<ResolveTeam, Error>>,
    AF: FnMut(GraphQlRequest<GetAllTeamsVariables>) -> AFut,
    AFut: Future<Output = Result<GetAllTeams, Error>>,
    TF: FnOnce(GraphQlRequest<()>) -> TFut,
    TFut: Future<Output = Result<GetTemplates, Error>>,
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
        template_json::render_list(&templates)
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
) -> Result<Vec<u8>, Error> {
    run_with(
        team,
        options,
        columns,
        color,
        |request| async move { transport.execute(&request).await.map_err(Error::from) },
        |request| async move { transport.execute(&request).await.map_err(Error::from) },
        |request| async move { transport.execute(&request).await.map_err(Error::from) },
    )
    .await
}

/// Filter by type, then keep workspace templates and the resolved team's,
/// then stable-sort by type, lowercased name, workspace first, and team key.
fn select(
    templates: Vec<Template>,
    template_type: Option<TemplateType>,
    team_id: Option<&str>,
) -> Result<Vec<Template>, Error> {
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
    selected.sort_by(|left, right| {
        collation::compare(&left.template_type, &right.template_type)
            .then_with(|| collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase()))
            .then_with(|| left.team.is_some().cmp(&right.team.is_some()))
            .then_with(|| collation::compare(team_key(left), team_key(right)))
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
