//! `issue relation delete`: remove a relation between two issues after
//! confirmation.
use crate::cli::issue::{IssueRelationDelete, RelationType};
use crate::client::LinearClient;
use crate::commands::{confirm, outcome};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue::{
    DeleteIssueRelation, DeleteVariables, FindIssueRelation, RelationsPageVariables,
};
use crate::graphql::pagination::{self, Page};

pub fn run(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete relation")
}

fn delete(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    let (a, b) = super::pair(ctx, &args.issue_id, &args.related_issue_id)?;
    let label = super::label(args.relation_type, &a, &b);
    let client = ctx.client()?;
    let id = ctx.spin(true, find(client, args.relation_type, &a, &b))?;
    let question = format!("Delete relation {label}?");
    if !confirm::proceed(ctx, args.confirm.yes, &question)? {
        return Ok(());
    }
    let deleted: DeleteIssueRelation = ctx.spin(true, client.mutate(DeleteVariables { id }))?;
    if !deleted.issue_relation_delete.success {
        return Err(Error::new("Linear did not delete the relation"));
    }
    ctx.print(outcome::done("Deleted", "relation", &label, None))
}

/// The ID of the relation of `kind` from `a` to `b`.
async fn find(client: &LinearClient, kind: RelationType, a: &str, b: &str) -> Result<String> {
    let input = super::lookup_pair(client, kind, a, b).await?;
    let relations = pagination::collect(None, |after, first| {
        let variables = RelationsPageVariables {
            issue_id: input.issue_id.clone(),
            first,
            after,
        };
        async move {
            let data: FindIssueRelation = client.query(variables).await?;
            let relations = data.issue.relations;
            Ok(Page {
                nodes: relations.nodes,
                page_info: relations.page_info,
            })
        }
    })
    .await?;
    relations
        .iter()
        .find(|relation| {
            relation.relation_type == input.relation_type.spelling()
                && relation.related_issue.id.inner() == input.related_issue_id
        })
        .map(|relation| relation.id.inner().to_owned())
        .ok_or_else(|| Error::not_found("Relation", &super::label(kind, a, b)))
}
