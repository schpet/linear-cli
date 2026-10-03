//! Helpers shared by the initiative commands.
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::Result;
use crate::graphql::operations::initiative::{GetInitiativeProjectLinks, LinksVariables};
use crate::graphql::pagination::{self, Page};
use crate::refs::{
    self,
    initiative::{Archived, InitiativeReference},
    project::ProjectReference,
};

/// Parses an initiative argument (URL, UUID, slug ID or name) without a request.
pub fn reference(ctx: &Ctx, input: &str) -> Result<InitiativeReference> {
    InitiativeReference::parse(input, &ctx.scope()?)
}

/// Rejects a Linear URL where an owner is expected, before any request.
pub fn check_owner(owner: Option<&str>) -> Result<()> {
    match owner {
        Some(owner) => refs::reject_linear_url(owner, "an email, username, display name, or @me"),
        None => Ok(()),
    }
}

/// The initiative and project arguments, parsed without a request.
pub struct Pair {
    initiative: InitiativeReference,
    project: ProjectReference,
}

/// Both sides by ID and name, and the link between them if there is one.
pub struct Link {
    pub initiative_id: String,
    pub initiative: String,
    pub project_id: String,
    pub project: String,
    pub id: Option<String>,
}

impl Pair {
    pub fn parse(ctx: &Ctx, initiative: &str, project: &str) -> Result<Self> {
        let scope = ctx.scope()?;
        Ok(Self {
            initiative: InitiativeReference::parse(initiative, &scope)?,
            project: ProjectReference::parse(project, &scope)?,
        })
    }

    pub async fn link(&self, client: &LinearClient) -> Result<Link> {
        let initiative_id =
            refs::initiative::resolve(client, &self.initiative, Archived::Exclude).await?;
        let project_id = refs::project::resolve(client, &self.project).await?;
        let data = pagination::collect_within(
            None,
            |after, _first| {
                let variables = LinksVariables {
                    initiative_id: initiative_id.clone(),
                    project_id: project_id.clone(),
                    after,
                };
                async move {
                    Ok(client
                        .query::<GetInitiativeProjectLinks, _>(variables)
                        .await?)
                }
            },
            |data| Page {
                nodes: std::mem::take(&mut data.project.initiative_to_projects.nodes),
                page_info: data.project.initiative_to_projects.page_info.clone(),
            },
            |data, page| data.project.initiative_to_projects.nodes = page.nodes,
        )
        .await?;
        let id = data
            .project
            .initiative_to_projects
            .nodes
            .into_iter()
            .find(|link| link.initiative.id.inner() == initiative_id)
            .map(|link| link.id.into_inner());
        let (initiative, project) = (data.initiative.name, data.project.name);
        Ok(Link {
            initiative_id,
            initiative,
            project_id,
            project,
            id,
        })
    }
}
