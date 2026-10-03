//! `issue relation add`: relate two issues.
use crate::cli::issue::{IssueRelationAdd, RelationType};
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue::{CreateIssueRelation, CreateVariables};

pub fn run(ctx: &Ctx, args: &IssueRelationAdd) -> Result<()> {
    add(ctx, args).context("Failed to create relation")
}

fn add(ctx: &Ctx, args: &IssueRelationAdd) -> Result<()> {
    let (a, b) = super::pair(ctx, &args.issue_id, &args.related_issue_id)?;
    let client = ctx.client()?;
    ctx.spin(true, create(client, args.relation_type, &a, &b))?;
    ctx.print(outcome::done(
        "Created",
        "relation",
        &super::label(args.relation_type, &a, &b),
        None,
    ))
}

async fn create(client: &LinearClient, kind: RelationType, a: &str, b: &str) -> Result<(), Error> {
    let input = super::lookup_pair(client, kind, a, b).await?;
    let data: CreateIssueRelation = client.mutate(CreateVariables { input }).await?;
    if !data.issue_relation_create.success {
        return Err(Error::new("Linear did not create the relation"));
    }
    Ok(())
}
