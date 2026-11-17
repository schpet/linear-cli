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
    let data: Result<CreateIssueRelation, _> = client.mutate(CreateVariables { input }).await;
    let data = match data {
        Ok(data) => data,
        Err(failure) => {
            let uncertain = failure.outcome_unknown();
            let mut error = super::super::lifecycle::write_error(client, &[a, b], failure).await;
            if uncertain {
                error.push_message("; relation may already exist");
            }
            return Err(error);
        }
    };
    if !data.issue_relation_create.success {
        return Err(Error::new("Linear did not create the relation"));
    }
    Ok(())
}
