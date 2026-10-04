//! Projects, referenced by UUID, exact name, slug ID or project URL.

use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::project::{
    GetProjectIdByName, GetProjectIdBySlugId, ProjectReferenceVariables, ProjectSlugVariables,
};

use super::is_linear_uuid;
use super::url::{LinearUrlKind, LinearUrlRef};
use super::workspace::{WorkspaceScope, expect_url_kind};

/// A project argument, checked locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectReference {
    input: String,
    target: Target,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Target {
    Id(String),
    /// An exact name, else a slug ID.
    NameOrSlug,
    /// The slug ID from a project URL.
    Slug(String),
}

impl ProjectReference {
    pub fn parse(input: &str, scope: &WorkspaceScope<'_>) -> Result<Self> {
        let target = match expect_url_kind(
            input,
            LinearUrlKind::Project,
            "a project URL, UUID, slug ID, or exact name",
            scope,
            |url| match url {
                LinearUrlRef::Project { slug_id, .. } => Some(slug_id),
                _ => None,
            },
        )? {
            Some(slug_id) => Target::Slug(slug_id),
            None if is_linear_uuid(input) => Target::Id(input.to_owned()),
            None => Target::NameOrSlug,
        };
        Ok(Self {
            input: input.to_owned(),
            target,
        })
    }

    /// A project already known by its UUID.
    pub fn from_id(id: String) -> Self {
        Self {
            input: id.clone(),
            target: Target::Id(id),
        }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    /// The UUID, when the argument was one.
    pub fn id(&self) -> Option<&str> {
        match &self.target {
            Target::Id(id) => Some(id),
            Target::NameOrSlug | Target::Slug(_) => None,
        }
    }
}

/// The ID of the project `reference` names: a UUID as given, else an exact
/// name (refusing an ambiguous one), else a slug ID; `None` when nothing
/// matches.
pub async fn find(client: &LinearClient, reference: &ProjectReference) -> Result<Option<String>> {
    let slug = match &reference.target {
        Target::Id(id) => return Ok(Some(id.clone())),
        Target::Slug(slug) => slug,
        Target::NameOrSlug => {
            let data: GetProjectIdByName = client
                .query(ProjectReferenceVariables {
                    name: reference.input.clone(),
                })
                .await?;
            let mut matches = data.projects.nodes;
            if matches.len() > 1 {
                return Err(super::ambiguous(
                    "Project",
                    &reference.input,
                    matches.iter().map(|project| project.id.inner().to_owned()),
                )
                .with_hint("Pass one of these UUIDs instead."));
            }
            if let Some(project) = matches.pop() {
                return Ok(Some(project.id.into_inner()));
            }
            &reference.input
        }
    };
    let data: GetProjectIdBySlugId = client
        .query(ProjectSlugVariables {
            slug_id: slug.clone(),
        })
        .await?;
    Ok(data
        .projects
        .nodes
        .into_iter()
        .next()
        .map(|project| project.id.into_inner()))
}

/// The ID of the project `reference` names.
pub async fn resolve(client: &LinearClient, reference: &ProjectReference) -> Result<String> {
    find(client, reference).await?.ok_or_else(|| {
        Error::not_found("Project", &reference.input).with_hint(
            "Pass a project UUID, slug ID (from `linear project list`), or exact project name.",
        )
    })
}
