//! `issue relation list`: an issue's outgoing and incoming relations.
use crate::cli::issue::IssueRelationList;
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::issue::{ListIssueRelations, ListedIssue, RelationsVariables};

pub fn run(ctx: &Ctx, args: &IssueRelationList) -> Result<()> {
    list(ctx, args).context("Failed to list relations")
}

fn list(ctx: &Ctx, args: &IssueRelationList) -> Result<()> {
    let identifier = super::super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let issue = ctx.spin(true, fetch(client, &identifier))?;
    ctx.print(render(&issue))
}

async fn fetch(client: &LinearClient, identifier: &str) -> Result<ListedIssue> {
    let data: ListIssueRelations = client
        .query(RelationsVariables {
            issue_id: identifier.to_owned(),
        })
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    Ok(data.issue)
}

fn render(issue: &ListedIssue) -> String {
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
    text
}
