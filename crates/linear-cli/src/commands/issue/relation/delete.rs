//! `issue relation delete`: remove a relation between two issues.
use crate::cli::issue::{IssueRelationDelete, RelationType};
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue::{
    DeleteIssueRelation, DeleteVariables, FindIssueRelation, RelationsVariables,
};

pub fn run(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete relation")
}

fn delete(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    let (a, b) = super::pair(ctx, &args.issue_id, &args.related_issue_id)?;
    let client = ctx.client()?;
    ctx.spin(true, remove(client, args.relation_type, &a, &b))?;
    ctx.print(outcome::done(
        "Deleted",
        "relation",
        &super::label(args.relation_type, &a, &b),
        None,
    ))
}

async fn remove(client: &LinearClient, kind: RelationType, a: &str, b: &str) -> Result<(), Error> {
    let input = super::lookup_pair(client, kind, a, b).await?;
    let data: FindIssueRelation = client
        .query(RelationsVariables {
            issue_id: input.issue_id.clone(),
        })
        .await?;
    let relation = data
        .issue
        .relations
        .nodes
        .iter()
        .find(|relation| {
            relation.relation_type == input.relation_type.spelling()
                && relation.related_issue.id.inner() == input.related_issue_id
        })
        .ok_or_else(|| Error::not_found("Relation", &super::label(kind, a, b)))?;
    let deleted: DeleteIssueRelation = client
        .mutate(DeleteVariables {
            id: relation.id.inner().to_owned(),
        })
        .await?;
    if !deleted.issue_relation_delete.success {
        return Err(Error::new("Linear did not delete the relation"));
    }
    Ok(())
}
