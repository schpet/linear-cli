//! `project delete`: move one project to the trash after confirmation.
use crate::cli::project::ProjectDelete;
use crate::commands::confirm;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::project::{DeleteProject, DeleteProjectVariables};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete project")
}

fn delete(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    let original = &args.project_id;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    let question = format!("Are you sure you want to delete project {original}?");
    if !confirm::deletion(ctx, args.force, &question)? {
        return Ok(());
    }
    let client = ctx.client()?;
    let result: DeleteProject = ctx.spin(true, async {
        let id = refs::project::resolve(client, &reference).await?;
        Ok::<_, Error>(client.mutate(DeleteProjectVariables { id }).await?)
    })?;
    let payload = result.project_delete;
    if !payload.success {
        return Err(Error::new("Linear did not delete the project"));
    }
    let name = payload
        .entity
        .as_ref()
        .map_or(original.as_str(), |entity| entity.name.as_str());
    ctx.print(format!("✓ Deleted project: {name}\n"))
}
