//! `issue relation`: relations between issues.
mod add;
mod delete;
mod list;

use crate::cli::issue::{IssueRelationCommand, RelationType};
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::issue::{ApiRelationType, RelationInput};

pub fn run(ctx: &Ctx, command: &IssueRelationCommand) -> Result<()> {
    match command {
        IssueRelationCommand::Add(args) => add::run(ctx, args),
        IssueRelationCommand::Delete(args) => delete::run(ctx, args),
        IssueRelationCommand::List(args) => list::run(ctx, args),
    }
}

/// Both identifiers, resolved before any request.
fn pair(ctx: &Ctx, a: &str, b: &str) -> Result<(String, String)> {
    let resolve = |input: &str| {
        super::resolve(ctx, Some(input))?
            .ok_or_else(|| Error::new(format!("Could not resolve issue identifier: {input}")))
    };
    Ok((resolve(a)?, resolve(b)?))
}

/// The relation between `a` and `b` as Linear stores it. `blocked-by` is
/// stored as `b blocks a`; messages keep the user's order.
async fn lookup_pair(
    client: &LinearClient,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<RelationInput, Error> {
    let a_id = super::id::fetch(client, a).await?;
    let b_id = super::id::fetch(client, b).await?;
    let (issue_id, related_issue_id, relation_type) = match kind {
        RelationType::Blocks => (a_id, b_id, ApiRelationType::Blocks),
        RelationType::BlockedBy => (b_id, a_id, ApiRelationType::Blocks),
        RelationType::Related => (a_id, b_id, ApiRelationType::Related),
        RelationType::Duplicate => (a_id, b_id, ApiRelationType::Duplicate),
    };
    Ok(RelationInput {
        issue_id,
        related_issue_id,
        relation_type,
    })
}

/// How success lines name a relation: `ENG-1 blocks ENG-2`.
fn label(kind: RelationType, a: &str, b: &str) -> String {
    format!("{a} {} {b}", kind.spelling())
}
