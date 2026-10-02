//! `project delete`: move one project to the trash after confirmation.
use cynic::MutationBuilder;

use crate::cli::project::ProjectDelete;
use crate::commands::confirm;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_delete::{DeleteProject, DeleteProjectVariables};
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete project")
}

fn delete(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    let original = &args.project_id;
    let reference = prepare_project_lookup(original, &ctx.scope()?)?;
    let question = format!("Are you sure you want to delete project {original}?");
    if !confirm::deletion(ctx, args.force, &question)? {
        return Ok(());
    }
    let client = ctx.client()?;
    let result: DeleteProject = ctx.spin(true, async {
        let id = resolve_project_with_transport(&reference, original, client).await?;
        let request =
            GraphQlRequest::with_variables(DeleteProject::build(DeleteProjectVariables { id }));
        Ok::<_, Error>(client.execute(&request).await?)
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
