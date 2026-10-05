//! Inputs and lookups shared by `project create` and `project update`.

use crate::graphql::operations::common::NameVariables;
use futures_util::future::try_join_all;

use crate::cli::project::{ProjectFields, Status};
use crate::cli::values::TextSource;
use crate::client::LinearClient;
use crate::commands::project::collections::ResolvedRef;
use crate::commands::template::scope::{self, TemplateScope};
use crate::commands::text_input;
use crate::error::{Error, Result};
use crate::graphql::operations::project::ProjectStatusType;
use crate::graphql::operations::project::{
    GetInitiativeByIdForUpdate, GetProjectLabelIdByName, GetProjectStatuses, InitiativeIdVariables,
    StatusOption,
};
use crate::graphql::operations::template::GetTemplates;
use crate::refs::{
    self, WorkspaceScope,
    initiative::{Archived, InitiativeReference},
    reject_linear_url,
    team::ResolvedTeam,
    team::TeamReference,
};

/// Linear's limit on a project description.
const DESCRIPTION_LIMIT: usize = 255;

/// The description from `--description` or `--description-file`.
pub fn description(fields: &ProjectFields) -> Result<Option<String>> {
    let description = match &fields.description_file {
        Some(path) => read(path, "description")?,
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
        Some(path) => read(path, "content"),
        None => Ok(fields.content.clone()),
    }
}

fn read(source: &TextSource, what: &str) -> Result<Option<String>> {
    text_input::read_source(source).map_err(|error| {
        Error::new(format!("Failed to read {what} file {source}: {error}")).with_source(error)
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
pub async fn statuses(client: &LinearClient) -> Result<Vec<StatusOption>> {
    let data: GetProjectStatuses = client.query(()).await?;
    Ok(data.project_statuses.nodes)
}

/// The ID of the workspace's first status of kind `status`.
pub async fn status_id(client: &LinearClient, status: Status) -> Result<String> {
    let kind = ProjectStatusType::from(status);
    statuses(client)
        .await?
        .into_iter()
        .find(|candidate| candidate.status_type == kind)
        .map(|found| found.id.into_inner())
        .ok_or_else(|| Error::not_found("Project status", kind.as_str()))
}

/// The project labels named by `values`, without duplicates.
pub async fn labels(client: &LinearClient, values: &[String]) -> Result<Vec<ResolvedRef>> {
    let mut labels: Vec<ResolvedRef> = Vec::new();
    for value in values {
        let data: GetProjectLabelIdByName = client
            .query(NameVariables {
                name: value.clone(),
            })
            .await?;
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

pub fn prepare_teams(values: &[String], scope: &WorkspaceScope<'_>) -> Result<Vec<TeamReference>> {
    values
        .iter()
        .map(|value| TeamReference::parse(value, scope))
        .collect()
}

/// The teams named by `teams`, without duplicates.
pub async fn teams(client: &LinearClient, teams: &[TeamReference]) -> Result<Vec<ResolvedTeam>> {
    let resolved = try_join_all(teams.iter().map(|team| refs::team::resolve(client, team))).await?;
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
) -> Result<Vec<InitiativeReference>> {
    values
        .iter()
        .map(|value| InitiativeReference::parse(value, scope))
        .collect()
}

/// The initiatives named by `initiatives`, without duplicates. An initiative
/// given by UUID is looked up for its name.
pub async fn initiatives(
    client: &LinearClient,
    initiatives: &[InitiativeReference],
) -> Result<Vec<ResolvedRef>> {
    let mut resolved: Vec<ResolvedRef> = Vec::new();
    for reference in initiatives {
        let initiative = match reference.id() {
            Some(id) => {
                let data: GetInitiativeByIdForUpdate = client
                    .query(InitiativeIdVariables {
                        id: cynic::Id::new(id),
                    })
                    .await?;
                let found = data.initiatives.nodes.into_iter().next().ok_or_else(|| {
                    Error::not_found("Initiative", reference.input())
                        .with_hint("Pass an initiative UUID, slug ID, or exact initiative name.")
                })?;
                ResolvedRef {
                    id: found.id.into_inner(),
                    label: found.name,
                }
            }
            None => ResolvedRef {
                id: refs::initiative::resolve(client, reference, Archived::Exclude).await?,
                label: reference.input().to_owned(),
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
    client: &LinearClient,
    reference: &str,
    team_ids: &[String],
) -> Result<String> {
    let data: GetTemplates = client.query(()).await?;
    let template = if refs::is_linear_uuid(reference) {
        let template = data
            .templates
            .into_iter()
            .find(|template| template.id.inner() == reference)
            .ok_or_else(|| {
                Error::not_found("Template", reference)
                    .with_hint("Run `linear template list` to see every template.")
            })?;
        scope::assert_scope(&template, team_ids, TemplateScope::Project)?;
        template
    } else {
        scope::select(reference, data.templates, team_ids, TemplateScope::Project)?
    };
    Ok(template.id.into_inner())
}
