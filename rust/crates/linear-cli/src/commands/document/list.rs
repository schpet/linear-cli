//! `document list`: one page of documents as a table or JSON.
use crate::client::LinearClient;
use std::time::SystemTime;

use cynic::QueryBuilder;

use crate::cli::Limit;
use crate::cli::document::DocumentList;
use crate::commands::{
    json,
    relative_time::format_relative_time,
    table::{Cell, Column, Table},
};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::{
    envelope::LegacyRequest,
    operations::documents::*,
    pagination::{self, Page, PageInfo},
};
use crate::platform::style;

use super::target::{self, TargetOptions};

pub fn run(ctx: &Ctx, args: &DocumentList) -> Result<()> {
    list(ctx, args).context("Failed to list documents")
}

fn list(ctx: &Ctx, args: &DocumentList) -> Result<()> {
    let target = target::prepare(
        ctx,
        TargetOptions {
            project: args.project.as_deref(),
            issue: args.issue.as_deref(),
            initiative: args.initiative.as_deref(),
            team: args.team.as_deref(),
            cycle: args.cycle.as_deref(),
            release: args.release.as_deref(),
        },
    )?;
    let client = ctx.client()?;
    let documents = ctx.spin(!args.json, async {
        let filter = match &target {
            Some(target) => {
                let (kind, id) = target::resolve(target, client).await?;
                Some(target::filter(kind, id))
            }
            None => None,
        };
        fetch(client, filter, args.limit).await
    })?;
    if args.json {
        ctx.print(json::render(&documents))
    } else if documents.is_empty() {
        ctx.print("No documents found.\n")
    } else {
        ctx.print(text(&documents, SystemTime::now()).render_for(ctx))
    }
}

async fn fetch(
    client: &LinearClient,
    filter: Option<DocumentFilter>,
    limit: Limit,
) -> Result<Vec<ListedDocument>> {
    pagination::collect(limit.max(), |after, first| {
        let request = LegacyRequest::with_variables(ListDocuments::build(ListDocumentsVariables {
            filter: filter.clone(),
            first: Some(first),
            after,
        }));
        async move {
            let data: ListDocuments = client.execute_legacy(&request).await?;
            Ok(data.documents.map_or_else(
                || Page {
                    nodes: Vec::new(),
                    page_info: PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                },
                |connection| Page {
                    nodes: connection.nodes,
                    page_info: connection.page_info,
                },
            ))
        }
    })
    .await
}

fn attachment(doc: &ListedDocument) -> String {
    if let Some(project) = &doc.project
        && !project.name.is_empty()
    {
        return format!("Project: {}", project.name);
    }
    if let Some(issue) = &doc.issue
        && !issue.identifier.is_empty()
    {
        return format!("Issue: {}", issue.identifier);
    }
    if let Some(initiative) = &doc.initiative
        && !initiative.name.is_empty()
    {
        return format!("Initiative: {}", initiative.name);
    }
    if let Some(team) = &doc.team {
        return format!("Team: {} ({})", team.name, team.key);
    }
    if let Some(cycle) = &doc.cycle {
        let name = cycle
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map_or(String::new(), |name| format!(" — {name}"));
        return format!("Cycle: {} #{}{name}", cycle.team.key, cycle.number);
    }
    if let Some(release) = &doc.release {
        let version = release
            .version
            .as_deref()
            .filter(|version| !version.is_empty())
            .map_or(String::new(), |version| format!(" ({version})"));
        return format!("Release: {}{version}", release.name);
    }
    "-".to_owned()
}
fn text(documents: &[ListedDocument], now: SystemTime) -> Table {
    let mut table = Table::new([
        Column::fixed("SLUG"),
        Column::flexible("TITLE"),
        Column::flexible("ATTACHMENT"),
        Column::fixed("UPDATED"),
    ]);
    for doc in documents {
        table.row([
            Cell::from(doc.slug_id.as_str()),
            Cell::from(doc.title.as_str()),
            Cell::from(attachment(doc)),
            Cell::styled(
                format_relative_time(&doc.updated_at.0, now.into(), &chrono::Local),
                style::gray,
            ),
        ]);
    }
    table
}
