//! `project delete`: move one project to the trash after confirmation.
use crate::cli::project::ProjectDelete;
use crate::commands::confirm;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::project::{DeleteProject, DeleteProjectVariables, GetProjectName};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete project")
}

fn delete(ctx: &Ctx, args: &ProjectDelete) -> Result<()> {
    let original = &args.project_id;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let project = ctx.spin(true, async {
        let id = refs::project::resolve(client, &reference).await?;
        let data: GetProjectName = client
            .query(DeleteProjectVariables { id: id.clone() })
            .await
            .map_err(|failure| failure.or_not_found("Project", original))?;
        Ok::<_, Error>(data.project)
    })?;
    let question = format!(
        "Are you sure you want to delete project \"{}\"?",
        project.name
    );
    if !confirm::proceed(ctx, args.confirm.yes, &question)? {
        return Ok(());
    }
    let result: DeleteProject = ctx.spin(
        true,
        client.mutate(DeleteProjectVariables {
            id: project.id.inner().to_owned(),
        }),
    )?;
    let payload = result.project_delete;
    if !payload.success {
        return Err(Error::new("Linear did not delete the project"));
    }
    ctx.print(outcome::done("Deleted", "project", &project.name, None))
}
