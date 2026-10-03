//! `initiative add-project` / `remove-project`: link or unlink a project.
use crate::cli::initiative::{InitiativeAddProject, InitiativeRemoveProject};
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::initiative::{
    AddProjectToInitiative, AddVariables, GetInitiativeProjectLinks,
    InitiativeToProjectCreateInput, LinksVariables, RemoveProjectFromInitiative,
};
use crate::graphql::pagination::{self, Page};
use crate::refs::{
    self,
    initiative::{Archived, InitiativeReference},
    project::ProjectReference,
};

pub fn add(ctx: &Ctx, args: &InitiativeAddProject) -> Result<()> {
    add_project(ctx, args).context("Failed to add project to initiative")
}

pub fn remove(ctx: &Ctx, args: &InitiativeRemoveProject) -> Result<()> {
    remove_project(ctx, args).context("Failed to remove project from initiative")
}

fn add_project(ctx: &Ctx, args: &InitiativeAddProject) -> Result<()> {
    let pair = Pair::prepare(ctx, &args.initiative, &args.project)?;
    let client = ctx.client()?;
    let link = ctx.spin(true, pair.link(client))?;
    if link.id.is_some() {
        return ctx.print(format!(
            "Project \"{}\" is already linked to initiative \"{}\"\n",
            link.project, link.initiative
        ));
    }
    let result: AddProjectToInitiative = ctx.spin(
        true,
        client.mutate(AddVariables {
            input: InitiativeToProjectCreateInput {
                initiative_id: link.initiative_id.clone(),
                project_id: link.project_id.clone(),
                sort_order: args.sort_order.clone(),
            },
        }),
    )?;
    if !result.initiative_to_project_create.success {
        return Err(Error::new("Linear did not link the project"));
    }
    ctx.print(outcome::done(
        "Added",
        "project",
        &format!("{} to initiative {}", link.project, link.initiative),
        None,
    ))
}

fn remove_project(ctx: &Ctx, args: &InitiativeRemoveProject) -> Result<()> {
    if !args.force {
        ctx.require_tty("--force")?;
    }
    let pair = Pair::prepare(ctx, &args.initiative, &args.project)?;
    let client = ctx.client()?;
    let link = ctx.spin(true, pair.link(client))?;
    let Some(link_id) = link.id else {
        return Err(Error::new(format!(
            "Project \"{}\" is not linked to initiative \"{}\"",
            link.project, link.initiative
        )));
    };
    let question = format!(
        "Remove \"{}\" from initiative \"{}\"?",
        link.project, link.initiative
    );
    if !args.force && !ctx.confirm(&question, "--force")? {
        return outcome::canceled(ctx);
    }
    let result: RemoveProjectFromInitiative =
        ctx.spin(true, client.mutate(IdVariables { id: link_id }))?;
    if !result.initiative_to_project_delete.success {
        return Err(Error::new("Linear did not unlink the project"));
    }
    ctx.print(outcome::done(
        "Removed",
        "project",
        &format!("{} from initiative {}", link.project, link.initiative),
        None,
    ))
}

/// The initiative and project arguments, parsed without a request.
struct Pair<'a> {
    initiative: (&'a str, InitiativeReference),
    project: (&'a str, ProjectReference),
}

/// Both sides by ID and name, and the link between them if there is one.
struct Link {
    initiative_id: String,
    initiative: String,
    project_id: String,
    project: String,
    id: Option<String>,
}

impl<'a> Pair<'a> {
    fn prepare(ctx: &Ctx, initiative: &'a str, project: &'a str) -> Result<Self> {
        let scope = ctx.scope()?;
        Ok(Self {
            initiative: (initiative, InitiativeReference::parse(initiative, &scope)?),
            project: (project, ProjectReference::parse(project, &scope)?),
        })
    }

    async fn link(&self, client: &LinearClient) -> Result<Link> {
        let initiative_id =
            refs::initiative::resolve(client, &self.initiative.1, Archived::Exclude).await?;
        let project_id = refs::project::resolve(client, &self.project.1).await?;
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
