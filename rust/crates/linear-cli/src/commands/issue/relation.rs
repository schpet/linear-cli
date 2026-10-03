//! `issue relation`: add, delete and list relations between issues.
use crate::cli::issue::{IssueRelationAdd, IssueRelationDelete, IssueRelationList, RelationType};
use crate::client::LinearClient;
use crate::commands::issue::id;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::LegacyRequest;
use crate::graphql::operations::issue_relations::{
    ApiRelationType, CreateIssueRelation, CreateVariables, DeleteIssueRelation, DeleteVariables,
    FindIssueRelation, IssueVariables, ListIssueRelations, ListedIssue, RelationInput,
};
use cynic::{MutationBuilder, QueryBuilder};

pub fn list(ctx: &Ctx, args: &IssueRelationList) -> Result<()> {
    print_list(ctx, args).context("Failed to list relations")
}

pub fn add(ctx: &Ctx, args: &IssueRelationAdd) -> Result<()> {
    print_add(ctx, args).context("Failed to create relation")
}

pub fn delete(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    print_delete(ctx, args).context("Failed to delete relation")
}

fn print_list(ctx: &Ctx, args: &IssueRelationList) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    ctx.print(ctx.spin(true, fetch_list(client, &identifier))?)
}

fn print_add(ctx: &Ctx, args: &IssueRelationAdd) -> Result<()> {
    let (a, b) = pair(ctx, &args.issue_id, &args.related_issue_id)?;
    let client = ctx.client()?;
    ctx.print(ctx.spin(true, create(client, args.relation_type, &a, &b))?)
}

fn print_delete(ctx: &Ctx, args: &IssueRelationDelete) -> Result<()> {
    let (a, b) = pair(ctx, &args.issue_id, &args.related_issue_id)?;
    let client = ctx.client()?;
    ctx.print(ctx.spin(true, remove(client, args.relation_type, &a, &b))?)
}

/// Both identifiers, resolved before any request.
fn pair(ctx: &Ctx, a: &str, b: &str) -> Result<(String, String)> {
    let resolve = |input: &str| {
        super::resolve(ctx, Some(input))?
            .ok_or_else(|| Error::new(format!("Could not resolve issue identifier: {input}")))
    };
    Ok((resolve(a)?, resolve(b)?))
}

pub fn list_request(identifier: &str) -> LegacyRequest<IssueVariables> {
    LegacyRequest::with_variables(ListIssueRelations::build(IssueVariables {
        issue_id: identifier.to_owned(),
    }))
}
pub fn find_request(id: &str) -> LegacyRequest<IssueVariables> {
    LegacyRequest::with_variables(FindIssueRelation::build(IssueVariables {
        issue_id: id.to_owned(),
    }))
}
pub fn create_request(input: RelationInput) -> LegacyRequest<CreateVariables> {
    LegacyRequest::with_variables(CreateIssueRelation::build(CreateVariables { input }))
}
pub fn delete_request(id: &str) -> LegacyRequest<DeleteVariables> {
    LegacyRequest::with_variables(DeleteIssueRelation::build(DeleteVariables {
        id: id.to_owned(),
    }))
}

/// The reversal affects API endpoints only. CLI diagnostics keep A then B.
pub fn directional_input(kind: RelationType, a: String, b: String) -> RelationInput {
    let (issue_id, related_issue_id, relation_type) = match kind {
        RelationType::Blocks => (a, b, ApiRelationType::Blocks),
        RelationType::BlockedBy => (b, a, ApiRelationType::Blocks),
        RelationType::Related => (a, b, ApiRelationType::Related),
        RelationType::Duplicate => (a, b, ApiRelationType::Duplicate),
    };
    RelationInput {
        issue_id,
        related_issue_id,
        relation_type,
    }
}

pub fn list_output(issue: &ListedIssue) -> Vec<u8> {
    let mut text = format!("Relations for {}: {}\n\n", issue.identifier, issue.title);
    if issue.relations.nodes.is_empty() && issue.inverse_relations.nodes.is_empty() {
        text.push_str("  No relations\n");
    }
    if !issue.relations.nodes.is_empty() {
        text.push_str("Outgoing:\n");
        for relation in &issue.relations.nodes {
            text.push_str(&format!(
                "  {} {} {}: {}\n",
                issue.identifier,
                relation.relation_type,
                relation.related_issue.identifier,
                relation.related_issue.title
            ));
        }
    }
    if !issue.inverse_relations.nodes.is_empty() {
        if !issue.relations.nodes.is_empty() {
            text.push('\n');
        }
        text.push_str("Incoming:\n");
        for relation in &issue.inverse_relations.nodes {
            let kind = if relation.relation_type == "blocks" {
                "blocked-by"
            } else {
                &relation.relation_type
            };
            text.push_str(&format!(
                "  {} {} {}: {}\n",
                issue.identifier, kind, relation.issue.identifier, relation.issue.title
            ));
        }
    }
    text.into_bytes()
}
async fn fetch_list(client: &LinearClient, identifier: &str) -> Result<Vec<u8>> {
    let data: ListIssueRelations = client
        .execute_legacy(&list_request(identifier))
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    Ok(list_output(&data.issue))
}

async fn lookup_pair(
    client: &LinearClient,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<RelationInput, Error> {
    let a_id = id::fetch(client, a).await?;
    // Even equal identifiers must be looked up twice, sequentially.
    let b_id = id::fetch(client, b).await?;
    Ok(directional_input(kind, a_id, b_id))
}
async fn create(
    client: &LinearClient,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<Vec<u8>, Error> {
    let input = lookup_pair(client, kind, a, b).await?;
    let data: CreateIssueRelation = client.execute_legacy(&create_request(input)).await?;
    if !data.issue_relation_create.success {
        return Err(Error::new("Linear did not create the relation"));
    }
    Ok(format!("✓ Created relation: {a} {} {b}\n", kind.spelling()).into_bytes())
}
async fn remove(
    client: &LinearClient,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<Vec<u8>, Error> {
    let input = lookup_pair(client, kind, a, b).await?;
    let data: FindIssueRelation = client
        .execute_legacy(&find_request(&input.issue_id))
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
        .ok_or_else(|| {
            Error::not_found(
                "Relation",
                &format!("{} between {a} and {b}", kind.spelling()),
            )
        })?;
    let deleted: DeleteIssueRelation = client
        .execute_legacy(&delete_request(relation.id.inner()))
        .await?;
    if !deleted.issue_relation_delete.success {
        return Err(Error::new("Linear did not delete the relation"));
    }
    Ok(format!("✓ Deleted relation: {a} {} {b}\n", kind.spelling()).into_bytes())
}
