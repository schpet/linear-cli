//! `issue relation`: add, delete and list relations between issues.
use crate::cli::issue::RelationType;
use crate::commands::issue_id;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_relations::{
    ApiRelationType, CreateIssueRelation, CreateVariables, DeleteIssueRelation, DeleteVariables,
    FindIssueRelation, IssueVariables, ListIssueRelations, ListedIssue, RelationInput,
};
use crate::graphql::transport::GraphQlTransport;
use cynic::{MutationBuilder, QueryBuilder};

pub const LIST_CONTEXT: &str = "Failed to list relations";
pub const ADD_CONTEXT: &str = "Failed to create relation";
pub const DELETE_CONTEXT: &str = "Failed to delete relation";

pub fn list_request(identifier: &str) -> GraphQlRequest<IssueVariables> {
    GraphQlRequest::with_variables(ListIssueRelations::build(IssueVariables {
        issue_id: identifier.to_owned(),
    }))
}
pub fn find_request(id: &str) -> GraphQlRequest<IssueVariables> {
    GraphQlRequest::with_variables(FindIssueRelation::build(IssueVariables {
        issue_id: id.to_owned(),
    }))
}
pub fn create_request(input: RelationInput) -> GraphQlRequest<CreateVariables> {
    GraphQlRequest::with_variables(CreateIssueRelation::build(CreateVariables { input }))
}
pub fn delete_request(id: &str) -> GraphQlRequest<DeleteVariables> {
    GraphQlRequest::with_variables(DeleteIssueRelation::build(DeleteVariables {
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
pub async fn list(transport: &GraphQlTransport, identifier: &str) -> Result<Vec<u8>, AppError> {
    let data: ListIssueRelations = transport
        .execute(&list_request(identifier))
        .await
        .map_err(|failure| issue_id::lookup_error(failure, identifier))?;
    Ok(list_output(&data.issue))
}

async fn lookup_pair(
    transport: &GraphQlTransport,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<RelationInput, AppError> {
    let a_id = issue_id::fetch(transport, a).await?;
    // Even equal identifiers must be looked up twice, sequentially.
    let b_id = issue_id::fetch(transport, b).await?;
    Ok(directional_input(kind, a_id, b_id))
}
pub async fn add(
    transport: &GraphQlTransport,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<Vec<u8>, AppError> {
    let input = lookup_pair(transport, kind, a, b).await?;
    let data: CreateIssueRelation = transport.execute(&create_request(input)).await?;
    if !data.issue_relation_create.success {
        return Err(AppError::new(AppErrorKind::GraphQl, ADD_CONTEXT));
    }
    Ok(format!("✓ Created relation: {a} {} {b}\n", kind.spelling()).into_bytes())
}
pub async fn delete(
    transport: &GraphQlTransport,
    kind: RelationType,
    a: &str,
    b: &str,
) -> Result<Vec<u8>, AppError> {
    let input = lookup_pair(transport, kind, a, b).await?;
    let data: FindIssueRelation = transport.execute(&find_request(&input.issue_id)).await?;
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
            AppError::not_found(
                "Relation",
                &format!("{} between {a} and {b}", kind.spelling()),
            )
        })?;
    let deleted: DeleteIssueRelation = transport
        .execute(&delete_request(relation.id.inner()))
        .await?;
    if !deleted.issue_relation_delete.success {
        return Err(AppError::new(AppErrorKind::GraphQl, DELETE_CONTEXT));
    }
    Ok(format!("✓ Deleted relation: {a} {} {b}\n", kind.spelling()).into_bytes())
}
