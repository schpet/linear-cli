//! Inputs and lookups shared by `project create` and `project update`.
use cynic::QueryBuilder;
use futures_util::future::try_join_all;

use crate::cli::project::{ProjectFields, Status};
use crate::commands::issue::template_scope::{self, TemplateScope};
use crate::commands::project::collections::ResolvedRef;
use crate::commands::text_input;
use crate::error::{Error, Result};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_write::{
    GetInitiativeByIdForUpdate, GetProjectLabelIdByName, GetProjectStatuses, InitiativeIdVariables,
    NameVariables, ProjectStatus,
};
use crate::graphql::operations::projects::ProjectStatusType;
use crate::graphql::operations::templates::GetTemplates;
use crate::graphql::transport::GraphQlTransport;
use crate::refs::{
    self, InitiativeReference, PreparedTeamLookup, ResolvedTeam, WorkspaceScope,
    prepare_initiative_lookup, prepare_team_lookup, reject_linear_url,
    resolve_initiative_with_transport, resolve_team_with_transport,
};

/// Linear's limit on a project description.
const DESCRIPTION_LIMIT: usize = 255;

/// The description from `--description` or `--description-file`.
pub fn description(fields: &ProjectFields) -> Result<Option<String>> {
    let description = match &fields.description_file {
        Some(path) => Some(read(path, "description")?),
        None => fields.description.clone(),
    };
    if let Some(description) = &description {
        // Linear's server counts the limit in UTF-16 code units.
        let length = description.encode_utf16().count();
        if length > DESCRIPTION_LIMIT {
            return Err(Error::new(format!(
                "Project description is {length} characters, over Linear's limit of {DESCRIPTION_LIMIT}"
            ))
            .with_hint("Shorten the description, or put the long text in --content (the project overview)."));
        }
    }
    Ok(description)
}

/// The overview Markdown from `--content` or `--content-file`.
pub fn content(fields: &ProjectFields) -> Result<Option<String>> {
    match &fields.content_file {
        Some(path) => read(path, "content").map(Some),
        None => Ok(fields.content.clone()),
    }
}

fn read(path: &str, what: &str) -> Result<String> {
    text_input::read_file(path).map_err(|error| {
        Error::new(format!("Failed to read {what} file {path}: {error}")).with_source(error)
    })
}

/// Rejects a Linear URL where only a name or ID is accepted.
pub fn plain_references<'a>(
    values: impl IntoIterator<Item = &'a String>,
    what: &str,
) -> Result<()> {
    values
        .into_iter()
        .try_for_each(|value| reject_linear_url(value, what))
}

/// The workspace's project statuses.
pub async fn statuses(client: &GraphQlTransport) -> Result<Vec<ProjectStatus>> {
    let request = GraphQlRequest::without_variables(GetProjectStatuses::build(()));
    let data: GetProjectStatuses = client.execute(&request).await?;
    Ok(data.project_statuses.nodes)
}

/// The ID of the workspace's first status of kind `status`.
pub async fn status_id(client: &GraphQlTransport, status: Status) -> Result<String> {
    let kind = match status {
        Status::Planned => ProjectStatusType::Planned,
        Status::Started => ProjectStatusType::Started,
        Status::Paused => ProjectStatusType::Paused,
        Status::Completed => ProjectStatusType::Completed,
        Status::Canceled => ProjectStatusType::Canceled,
        Status::Backlog => ProjectStatusType::Backlog,
    };
    statuses(client)
        .await?
        .into_iter()
        .find(|candidate| candidate.status_type == kind)
        .map(|found| found.id.into_inner())
        .ok_or_else(|| Error::not_found("Project status", kind.as_str()))
}

/// The project labels named by `values`, without duplicates.
pub async fn labels(client: &GraphQlTransport, values: &[String]) -> Result<Vec<ResolvedRef>> {
    let mut labels: Vec<ResolvedRef> = Vec::new();
    for value in values {
        let request =
            GraphQlRequest::with_variables(GetProjectLabelIdByName::build(NameVariables {
                name: value.clone(),
            }));
        let data: GetProjectLabelIdByName = client.execute(&request).await?;
        let id = data
            .project_labels
            .nodes
            .into_iter()
            .next()
            .map(|label| label.id.into_inner())
            .ok_or_else(|| Error::not_found("Project label", value))?;
        if !labels.iter().any(|label| label.id == id) {
            labels.push(ResolvedRef {
                id,
                label: value.clone(),
            });
        }
    }
    Ok(labels)
}

pub fn prepare_teams(
    values: &[String],
    scope: &WorkspaceScope<'_>,
) -> Result<Vec<PreparedTeamLookup>> {
    values
        .iter()
        .map(|value| prepare_team_lookup(value, scope))
        .collect()
}

/// The teams named by `teams`, without duplicates.
pub async fn teams(
    client: &GraphQlTransport,
    teams: &[PreparedTeamLookup],
) -> Result<Vec<ResolvedTeam>> {
    let resolved = try_join_all(
        teams
            .iter()
            .map(|team| resolve_team_with_transport(team, client)),
    )
    .await?;
    let mut unique: Vec<ResolvedTeam> = Vec::new();
    for team in resolved {
        if !unique.iter().any(|kept| kept.id == team.id) {
            unique.push(team);
        }
    }
    Ok(unique)
}

pub fn prepare_initiatives(
    values: &[String],
    scope: &WorkspaceScope<'_>,
) -> Result<Vec<(String, InitiativeReference)>> {
    values
        .iter()
        .map(|value| Ok((value.clone(), prepare_initiative_lookup(value, scope)?)))
        .collect()
}

/// The initiatives named by `initiatives`, without duplicates. An initiative
/// given by UUID is looked up for its name.
pub async fn initiatives(
    client: &GraphQlTransport,
    initiatives: &[(String, InitiativeReference)],
) -> Result<Vec<ResolvedRef>> {
    let mut resolved: Vec<ResolvedRef> = Vec::new();
    for (original, reference) in initiatives {
        let initiative = match reference {
            InitiativeReference::Id(id) => {
                let request = GraphQlRequest::with_variables(GetInitiativeByIdForUpdate::build(
                    InitiativeIdVariables {
                        id: cynic::Id::new(id),
                    },
                ));
                let data: GetInitiativeByIdForUpdate = client.execute(&request).await?;
                let found = data.initiatives.nodes.into_iter().next().ok_or_else(|| {
                    Error::not_found("Initiative", original)
                        .with_hint("Pass an initiative UUID, slug ID, or exact initiative name.")
                })?;
                ResolvedRef {
                    id: found.id.into_inner(),
                    label: found.name,
                }
            }
            InitiativeReference::NameOrSlug(_) | InitiativeReference::UrlSlug(_) => ResolvedRef {
                id: resolve_initiative_with_transport(reference, original, client).await?,
                label: original.clone(),
            },
        };
        if !resolved.iter().any(|kept| kept.id == initiative.id) {
            resolved.push(initiative);
        }
    }
    Ok(resolved)
}

/// The ID of the project template named by `reference` (a name or UUID) that
/// may be applied to a project of `team_ids`.
pub async fn template(
    client: &GraphQlTransport,
    reference: &str,
    team_ids: &[String],
) -> Result<String> {
    let request = GraphQlRequest::without_variables(GetTemplates::build(()));
    let data: GetTemplates = client.execute(&request).await?;
    let template = if refs::is_linear_uuid(reference) {
        let template = data
            .templates
            .into_iter()
            .find(|template| template.id.inner() == reference)
            .ok_or_else(|| {
                Error::not_found("Template", reference)
                    .with_hint("Run `linear template list` to see every template.")
            })?;
        template_scope::assert_scope(&template, team_ids, TemplateScope::Project)?;
        template
    } else {
        template_scope::select(reference, data.templates, team_ids, TemplateScope::Project)?
    };
    Ok(template.id.into_inner())
}
