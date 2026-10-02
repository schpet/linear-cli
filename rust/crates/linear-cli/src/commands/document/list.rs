//! `document list`: one page of documents as a table or JSON.
use std::time::SystemTime;

use cynic::QueryBuilder;

use crate::cli::document::DocumentList;
use crate::commands::{
    display::{display_width, fit, pad},
    relative_time::format_relative_time,
    table::{self, underlined_header},
};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::{
    envelope::GraphQlRequest,
    operations::{documents::*, teams::PageInfo},
    transport::GraphQlTransport,
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
    let first = i32::try_from(args.limit.get()).map_err(|error| {
        Error::new(format!("--limit must be at most {}", i32::MAX)).with_source(error)
    })?;
    let client = ctx.client()?;
    let documents = ctx.spin(!args.json, async {
        let filter = match &target {
            Some(target) => {
                let (kind, id) = target::resolve(target, client).await?;
                Some(target::filter(kind, id))
            }
            None => None,
        };
        fetch(client, filter, first).await
    })?;
    if args.json {
        let mut output =
            serde_json::to_vec_pretty(&documents).expect("document JSON always serializes");
        output.push(b'\n');
        ctx.print(output)
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(text(&documents, columns, ctx.color(), SystemTime::now()))
    }
}

async fn fetch(
    client: &GraphQlTransport,
    filter: Option<DocumentFilter>,
    first: i32,
) -> Result<DocumentConnection> {
    let request = GraphQlRequest::with_variables(ListDocuments::build(ListDocumentsVariables {
        filter,
        first: Some(first),
    }));
    let data: ListDocuments = client.execute(&request).await?;
    Ok(data.documents.unwrap_or(DocumentConnection {
        nodes: Vec::new(),
        page_info: PageInfo {
            has_next_page: false,
            end_cursor: None,
        },
    }))
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
fn text(documents: &DocumentConnection, columns: usize, color: bool, now: SystemTime) -> String {
    if documents.nodes.is_empty() {
        return "No documents found.\n".to_owned();
    }
    let labels: Vec<_> = documents.nodes.iter().map(attachment).collect();
    let ages: Vec<_> = documents
        .nodes
        .iter()
        .map(|doc| format_relative_time(&doc.updated_at.0, now.into(), &chrono::Local))
        .collect();
    let slug_width = documents
        .nodes
        .iter()
        .map(|doc| display_width(&doc.slug_id))
        .max()
        .unwrap_or(0)
        .max(4);
    let attachment_width = labels
        .iter()
        .map(|label| display_width(label))
        .max()
        .unwrap_or(0)
        .max(10);
    let updated_width = ages
        .iter()
        .map(|age| display_width(age))
        .max()
        .unwrap_or(0)
        .max(7);
    let available = columns
        .saturating_sub(slug_width + attachment_width + updated_width + 4)
        .max(10);
    let title_width = documents
        .nodes
        .iter()
        .map(|doc| display_width(&doc.title))
        .max()
        .unwrap_or(0)
        .min(available);
    let mut out = underlined_header(
        &[
            pad("SLUG", slug_width),
            pad("TITLE", title_width),
            pad("ATTACHMENT", attachment_width),
            pad("UPDATED", updated_width),
        ],
        color,
    );
    for ((doc, label), age) in documents.nodes.iter().zip(labels).zip(ages) {
        out.push_str(&format!(
            "{} {} {} {}\n",
            pad(&doc.slug_id, slug_width),
            fit(&doc.title, title_width),
            pad(&label, attachment_width),
            style::gray(&pad(&age, updated_width), color)
        ));
    }
    out
}
