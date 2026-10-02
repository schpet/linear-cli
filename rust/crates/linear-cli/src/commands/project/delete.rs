//! Delete exactly one resolved project; preserve the original argument fallback.
use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_delete::{DeleteProject, DeleteProjectVariables};
use crate::graphql::transport::GraphQlTransport;
use cynic::MutationBuilder;

pub const CONTEXT: &str = "Failed to delete project";

pub fn request(id: &str) -> GraphQlRequest<DeleteProjectVariables> {
    GraphQlRequest::with_variables(DeleteProject::build(DeleteProjectVariables {
        id: id.to_owned(),
    }))
}

pub async fn submit(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
) -> Result<Vec<u8>, Error> {
    let result: DeleteProject = transport.execute(&request(id)).await?;
    if !result.project_delete.success {
        return Err(Error::new(CONTEXT));
    }
    let name = result
        .project_delete
        .entity
        .as_ref()
        .map_or(original, |entity| entity.name.as_str());
    Ok(format!("✓ Deleted project: {name}\n").into_bytes())
}
