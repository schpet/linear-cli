//! Shared project reference resolution for UUID, exact name, slug and URL.

use cynic::QueryBuilder;

use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_view::{
    GetProjectIdByName, GetProjectIdBySlugId, ProjectReferenceVariables, ProjectSlugVariables,
};
use crate::graphql::transport::GraphQlTransport;

use super::is_linear_uuid;
use super::url::{LinearUrlKind, LinearUrlRef};
use super::workspace::{WorkspaceScope, expect_url_kind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectReference {
    Id(String),
    NameOrSlug(String),
    Slug(String),
}

pub fn prepare_project_lookup(
    input: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<ProjectReference, Error> {
    match expect_url_kind(
        input,
        LinearUrlKind::Project,
        "a project URL, UUID, slug ID, or exact name",
        scope,
    )? {
        Some(LinearUrlRef::Project { slug_id, .. }) => Ok(ProjectReference::Slug(slug_id)),
        Some(_) => Err(Error::new(
            "project URL kind check returned a different kind",
        )),
        None if is_linear_uuid(input) => Ok(ProjectReference::Id(input.to_owned())),
        None => Ok(ProjectReference::NameOrSlug(input.to_owned())),
    }
}

pub async fn resolve_project_with_transport(
    reference: &ProjectReference,
    original: &str,
    transport: &GraphQlTransport,
) -> Result<String, Error> {
    match reference {
        ProjectReference::Id(id) => Ok(id.clone()),
        ProjectReference::Slug(slug) => find_slug(slug, transport)
            .await?
            .ok_or_else(|| not_found(original)),
        ProjectReference::NameOrSlug(name) => {
            let query = GraphQlRequest::with_variables(GetProjectIdByName::build(
                ProjectReferenceVariables { name: name.clone() },
            ));
            let data: GetProjectIdByName = transport.execute(&query).await.map_err(Error::from)?;
            let matches = data.projects.nodes;
            if matches.len() > 1 {
                return Err(Error::new(format!(
                    "Project \"{name}\" is ambiguous; it matches {} projects:\n{}",
                    matches.len(),
                    matches
                        .iter()
                        .map(|item| format!("  {}", item.id.inner()))
                        .collect::<Vec<_>>()
                        .join("\n")
                ))
                .with_hint(
                    "Pass the project's UUID or slug ID instead. `linear project list` shows both.",
                ));
            }
            if let Some(id) = matches
                .into_iter()
                .next()
                .map(|project| project.id.into_inner())
                .filter(|id| !id.is_empty())
            {
                return Ok(id);
            }
            find_slug(name, transport)
                .await?
                .ok_or_else(|| not_found(original))
        }
    }
}

async fn find_slug(slug: &str, transport: &GraphQlTransport) -> Result<Option<String>, Error> {
    let query = GraphQlRequest::with_variables(GetProjectIdBySlugId::build(ProjectSlugVariables {
        slug_id: slug.to_owned(),
    }));
    let data: GetProjectIdBySlugId = transport.execute(&query).await.map_err(Error::from)?;
    Ok(data
        .projects
        .nodes
        .into_iter()
        .next()
        .map(|project| project.id.into_inner())
        .filter(|id| !id.is_empty()))
}

fn not_found(original: &str) -> Error {
    Error::not_found("Project", original).with_hint(
        "Pass a project UUID, slug ID (from `linear project list`), or exact project name.",
    )
}
