//! One milestone mutation, with no retry or local UUID validation.
use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_delete::{
    DeleteProjectMilestone, DeleteProjectMilestoneVariables,
};
use crate::graphql::transport::GraphQlTransport;
use cynic::MutationBuilder;

pub const CONTEXT: &str = "Failed to delete milestone";

pub fn request(id: &str) -> GraphQlRequest<DeleteProjectMilestoneVariables> {
    GraphQlRequest::with_variables(DeleteProjectMilestone::build(
        DeleteProjectMilestoneVariables { id: id.to_owned() },
    ))
}

pub async fn submit(transport: &GraphQlTransport, id: &str) -> Result<Vec<u8>, Error> {
    let result: DeleteProjectMilestone = transport.execute(&request(id)).await?;
    if !result.project_milestone_delete.success {
        return Err(Error::new(CONTEXT));
    }
    Ok(format!("✓ Deleted milestone {id}\n").into_bytes())
}
