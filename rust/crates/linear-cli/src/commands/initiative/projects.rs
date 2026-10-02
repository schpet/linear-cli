//! `initiative add-project` / `remove-project`: resolve both sides, then link or unlink.
use crate::error::{Error, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_projects::*;
use crate::graphql::operations::initiative_view::{ResolveInitiativeBySlug, UrlSlugVariables};
use crate::graphql::operations::project_view::{GetProjectIdBySlugId, ProjectSlugVariables};
use crate::graphql::transport::{GraphQlTransport, TransportFailure, classify_typed};
use crate::refs::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind, is_linear_uuid};
use cynic::{MutationBuilder, QueryBuilder};
pub const ADD_CONTEXT: &str = "Failed to add project to initiative";
pub const REMOVE_CONTEXT: &str = "Failed to remove project from initiative";
#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Add,
    Remove,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: String,
    pub name: String,
}
pub async fn resolve_initiative(
    transport: &GraphQlTransport,
    original: &str,
    scope: &WorkspaceScope<'_>,
    mode: Mode,
) -> Result<Entity, Error> {
    let url = expect_url_kind(
        original,
        LinearUrlKind::Initiative,
        "an initiative URL, UUID, slug ID, or exact name",
        scope,
    )?;
    let mut token = original.to_owned();
    if let Some(url) = url {
        let LinearUrlRef::Initiative { slug_id, .. } = url else {
            return Err(Error::new("initiative URL kind mismatch"));
        };
        let request =
            GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(UrlSlugVariables {
                slug_id,
                include_archived: Some(false),
            }));
        let data: ResolveInitiativeBySlug =
            transport.execute(&request).await.map_err(Error::from)?;
        token = data
            .initiatives
            .nodes
            .into_iter()
            .next()
            .ok_or_else(|| Error::not_found("Initiative", original))?
            .id
            .into_inner();
    }
    if is_linear_uuid(&token) {
        let variables = IdVariables { id: token.clone() };
        let node = match mode {
            Mode::Add => {
                let request =
                    GraphQlRequest::with_variables(GetInitiativeNameById::build(variables));
                transport
                    .execute::<GetInitiativeNameById, _>(&request)
                    .await
                    .ok()
                    .and_then(|data| data.initiative)
            }
            Mode::Remove => {
                let request = GraphQlRequest::with_variables(
                    GetInitiativeNameByIdForRemove::build(variables),
                );
                transport
                    .execute::<GetInitiativeNameByIdForRemove, _>(&request)
                    .await
                    .ok()
                    .and_then(|data| data.initiative)
            }
        };
        return Ok(node.map_or_else(
            || Entity {
                id: token.clone(),
                name: token,
            },
            |node| Entity {
                id: node.id.into_inner(),
                name: node.name,
            },
        ));
    }
    let variables = SlugVariables {
        slug_id: token.clone(),
    };
    let node = match mode {
        Mode::Add => {
            let request =
                GraphQlRequest::with_variables(GetInitiativeBySlugForAddProject::build(variables));
            transport
                .execute::<GetInitiativeBySlugForAddProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.initiatives.nodes.into_iter().next())
        }
        Mode::Remove => {
            let request = GraphQlRequest::with_variables(
                GetInitiativeBySlugForRemoveProject::build(variables),
            );
            transport
                .execute::<GetInitiativeBySlugForRemoveProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.initiatives.nodes.into_iter().next())
        }
    };
    if let Some(node) = node {
        return Ok(Entity {
            id: node.id.into_inner(),
            name: node.name,
        });
    }
    let variables = NameVariables { name: token };
    let node = match mode {
        Mode::Add => {
            let request =
                GraphQlRequest::with_variables(GetInitiativeByNameForAddProject::build(variables));
            transport
                .execute::<GetInitiativeByNameForAddProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.initiatives.nodes.into_iter().next())
        }
        Mode::Remove => {
            let request = GraphQlRequest::with_variables(
                GetInitiativeByNameForRemoveProject::build(variables),
            );
            transport
                .execute::<GetInitiativeByNameForRemoveProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.initiatives.nodes.into_iter().next())
        }
    };
    node.map(|node| Entity {
        id: node.id.into_inner(),
        name: node.name,
    })
    .ok_or_else(|| Error::not_found("Initiative", original))
}
pub async fn resolve_project(
    transport: &GraphQlTransport,
    original: &str,
    scope: &WorkspaceScope<'_>,
    mode: Mode,
) -> Result<Entity, Error> {
    let url = expect_url_kind(
        original,
        LinearUrlKind::Project,
        "a project URL, UUID, slug ID, or exact name",
        scope,
    )?;
    let mut token = original.to_owned();
    if let Some(url) = url {
        let LinearUrlRef::Project { slug_id, .. } = url else {
            return Err(Error::new("project URL kind mismatch"));
        };
        let request =
            GraphQlRequest::with_variables(GetProjectIdBySlugId::build(ProjectSlugVariables {
                slug_id,
            }));
        let data: GetProjectIdBySlugId = transport.execute(&request).await.map_err(Error::from)?;
        token = data
            .projects
            .nodes
            .into_iter()
            .next()
            .ok_or_else(|| Error::not_found("Project", original))?
            .id
            .into_inner();
    }
    if is_linear_uuid(&token) {
        let variables = IdVariables { id: token.clone() };
        let node = match mode {
            Mode::Add => {
                let request = GraphQlRequest::with_variables(GetProjectNameById::build(variables));
                transport
                    .execute::<GetProjectNameById, _>(&request)
                    .await
                    .ok()
                    .and_then(|data| data.project)
            }
            Mode::Remove => {
                let request =
                    GraphQlRequest::with_variables(GetProjectNameByIdForRemove::build(variables));
                transport
                    .execute::<GetProjectNameByIdForRemove, _>(&request)
                    .await
                    .ok()
                    .and_then(|data| data.project)
            }
        };
        return Ok(node.map_or_else(
            || Entity {
                id: token.clone(),
                name: token,
            },
            |node| Entity {
                id: node.id.into_inner(),
                name: node.name,
            },
        ));
    }
    let variables = SlugVariables {
        slug_id: token.clone(),
    };
    let node = match mode {
        Mode::Add => {
            let request =
                GraphQlRequest::with_variables(GetProjectBySlugForAddProject::build(variables));
            transport
                .execute::<GetProjectBySlugForAddProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.projects.nodes.into_iter().next())
        }
        Mode::Remove => {
            let request =
                GraphQlRequest::with_variables(GetProjectBySlugForRemoveProject::build(variables));
            transport
                .execute::<GetProjectBySlugForRemoveProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.projects.nodes.into_iter().next())
        }
    };
    if let Some(node) = node {
        return Ok(Entity {
            id: node.id.into_inner(),
            name: node.name,
        });
    }
    let variables = NameVariables { name: token };
    let node = match mode {
        Mode::Add => {
            let request =
                GraphQlRequest::with_variables(GetProjectByNameForAddProject::build(variables));
            transport
                .execute::<GetProjectByNameForAddProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.projects.nodes.into_iter().next())
        }
        Mode::Remove => {
            let request =
                GraphQlRequest::with_variables(GetProjectByNameForRemoveProject::build(variables));
            transport
                .execute::<GetProjectByNameForRemoveProject, _>(&request)
                .await
                .ok()
                .and_then(|data| data.projects.nodes.into_iter().next())
        }
    };
    node.map(|node| Entity {
        id: node.id.into_inner(),
        name: node.name,
    })
    .ok_or_else(|| Error::not_found("Project", original))
}
fn duplicate_text(text: &str) -> bool {
    text.contains("already exists") || text.contains("duplicate")
}
/// graphql-request embeds the full response and request in its thrown string.
/// Inspect this local equivalent before typed classification discards extensions,
/// partial data and body. Never render this metadata in the user diagnostic.
fn duplicate_exchange<V: serde::Serialize>(
    body: &[u8],
    request: &GraphQlRequest<V>,
) -> Result<bool, Error> {
    let response_text = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(value) => value.to_string(),
        Err(_) => String::from_utf8_lossy(body).into_owned(),
    };
    let request_text =
        serde_json::to_string(request).map_err(|error| Error::new(error.to_string()))?;
    Ok(duplicate_text(&response_text) || duplicate_text(&request_text))
}
pub async fn add(
    transport: &GraphQlTransport,
    initiative: &Entity,
    project: &Entity,
    sort_order: Option<f64>,
) -> Result<Vec<u8>, Error> {
    let request = GraphQlRequest::with_variables(AddProjectToInitiative::build(AddVariables {
        input: InitiativeToProjectCreateInput {
            initiative_id: initiative.id.clone(),
            project_id: project.id.clone(),
            sort_order,
        },
    }));
    let response = transport
        .send_request(&request)
        .await
        .map_err(Error::from)
        .context(ADD_CONTEXT)?;
    let duplicate = duplicate_exchange(&response.body, &request)?;
    let result: Result<AddProjectToInitiative, _> = classify_typed(response);
    match result {
        Err(TransportFailure::GraphQl { .. } | TransportFailure::Http { .. }) if duplicate => {
            Ok(format!(
                "Project \"{}\" is already linked to initiative \"{}\"\n",
                project.name, initiative.name
            )
            .into_bytes())
        }
        Err(error) => Err(Error::from(error).context(ADD_CONTEXT)),
        Ok(data) if !data.initiative_to_project_create.success => {
            Err(Error::new(ADD_CONTEXT).context(ADD_CONTEXT))
        }
        Ok(_) => Ok(format!(
            "✓ Added \"{}\" to initiative \"{}\"\n",
            project.name, initiative.name
        )
        .into_bytes()),
    }
}
pub async fn find_link(
    transport: &GraphQlTransport,
    initiative: &Entity,
    project: &Entity,
) -> Result<Option<String>, Error> {
    let request = GraphQlRequest::with_variables(GetInitiativeToProjects::build(LinksVariables {
        first: Some(250),
    }));
    let data: GetInitiativeToProjects = transport
        .execute(&request)
        .await
        .map_err(Error::from)
        .context("Failed to find project link")?;
    Ok(data
        .initiative_to_projects
        .nodes
        .into_iter()
        .find(|link| {
            link.initiative
                .as_ref()
                .is_some_and(|node| node.id.inner() == initiative.id)
                && link
                    .project
                    .as_ref()
                    .is_some_and(|node| node.id.inner() == project.id)
        })
        .map(|link| link.id.into_inner())
        .filter(|id| !id.is_empty()))
}
pub async fn remove(
    transport: &GraphQlTransport,
    link_id: &str,
    initiative: &Entity,
    project: &Entity,
) -> Result<Vec<u8>, Error> {
    let request = GraphQlRequest::with_variables(RemoveProjectFromInitiative::build(IdVariables {
        id: link_id.to_owned(),
    }));
    let data: RemoveProjectFromInitiative = transport
        .execute(&request)
        .await
        .map_err(Error::from)
        .context(REMOVE_CONTEXT)?;
    if !data.initiative_to_project_delete.success {
        return Err(Error::new(REMOVE_CONTEXT).context(REMOVE_CONTEXT));
    }
    Ok(format!(
        "✓ Removed \"{}\" from initiative \"{}\"\n",
        project.name, initiative.name
    )
    .into_bytes())
}
