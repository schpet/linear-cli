//! `document view`: a document as Markdown, raw content or JSON (with every
//! comment), or opened in the browser.
use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;

use crate::cli::document::DocumentView;
use crate::commands::json;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::documents::*;
use crate::graphql::pagination::{self, Page};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::markdown_assets;

pub fn run(ctx: &Ctx, args: &DocumentView) -> Result<()> {
    view(ctx, args).context("Failed to view document")
}

fn view(ctx: &Ctx, args: &DocumentView) -> Result<()> {
    let original = &args.id;
    let id = super::reference(ctx, original)?;
    let client = ctx.client()?;
    if args.web {
        let document = ctx.spin(true, body(client, &id, original))?;
        return ctx.open_url(&document.url, false);
    }
    if args.json {
        let document = ctx.spin(false, with_comments(client, &id, original))?;
        return ctx.print(json::render(&document));
    }
    let document = ctx.spin(!args.raw, body(client, &id, original))?;
    let download = !args.no_download
        && ctx
            .options()
            .download_images()
            .is_none_or(|value| *value.value());
    let content = match document
        .content
        .as_deref()
        .filter(|content| !content.is_empty())
    {
        Some(content) if download => Some(local_images(ctx, client, content)?),
        content => content.map(str::to_owned),
    };
    if args.raw {
        return match content {
            Some(content) => ctx.print(format!("{content}\n")),
            None => Ok(()),
        };
    }
    let markdown = markdown(&document, content.as_deref(), Utc::now(), &chrono::Local);
    ctx.show_markdown(&markdown, false)
}

/// The content with its uploaded files downloaded and pointed at the local
/// copies.
fn local_images(ctx: &Ctx, client: &GraphQlTransport, content: &str) -> Result<String> {
    let root = &ctx.config().image_cache_root;
    let paths = ctx.block_on(markdown_assets::download(
        client,
        root,
        &[content],
        |line| ctx.eprint(line),
    ))?;
    Ok(markdown_assets::rewrite(content, &paths))
}

async fn body(client: &GraphQlTransport, id: &str, original: &str) -> Result<DocumentBody> {
    let request = GraphQlRequest::with_variables(GetDocument::build(GetDocumentVariables {
        id: id.to_owned(),
    }));
    let data: GetDocument = client
        .execute(&request)
        .await
        .map_err(|failure| super::not_found(failure, original))?;
    data.document
        .ok_or_else(|| Error::not_found("Document", original))
}

/// The document with every page of its comments.
async fn with_comments(
    client: &GraphQlTransport,
    id: &str,
    original: &str,
) -> Result<DocumentWithComments> {
    pagination::collect_within(
        None,
        |after, _first| {
            let request = GraphQlRequest::with_variables(GetDocumentWithComments::build(
                GetDocumentCommentsVariables {
                    id: id.to_owned(),
                    comments_after: after,
                },
            ));
            async move {
                let data: GetDocumentWithComments = client
                    .execute(&request)
                    .await
                    .map_err(|failure| super::not_found(failure, original))?;
                data.document
                    .ok_or_else(|| Error::not_found("Document", original))
            }
        },
        |document| Page {
            nodes: std::mem::take(&mut document.comments.nodes),
            page_info: document.comments.page_info.clone(),
        },
        |document, page| {
            document.comments.nodes = page.nodes;
            document.comments.page_info = page.page_info;
        },
    )
    .await
}

fn markdown<Tz: TimeZone>(
    document: &DocumentBody,
    content: Option<&str>,
    now: DateTime<Utc>,
    zone: &Tz,
) -> String {
    let mut lines = vec![
        format!("# {}", document.title),
        String::new(),
        format!("**Slug:** {}", document.slug_id),
        format!("**URL:** {}", document.url),
    ];
    if let Some(creator) = &document.creator {
        lines.push(format!("**Creator:** {}", creator.name));
    }
    if let Some(project) = &document.project {
        lines.push(format!("**Project:** {}", project.name));
    }
    if let Some(issue) = &document.issue {
        lines.push(format!("**Issue:** {} - {}", issue.identifier, issue.title));
    }
    if let Some(initiative) = &document.initiative {
        lines.push(format!("**Initiative:** {}", initiative.name));
    }
    if let Some(team) = &document.team {
        lines.push(format!("**Team:** {} ({})", team.name, team.key));
    }
    if let Some(cycle) = &document.cycle {
        let name = cycle
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map_or(String::new(), |name| format!(" - {name}"));
        lines.push(format!(
            "**Cycle:** {} #{}{name}",
            cycle.team.key, cycle.number
        ));
    }
    if let Some(release) = &document.release {
        let version = release
            .version
            .as_deref()
            .filter(|version| !version.is_empty())
            .map_or(String::new(), |version| format!(" ({version})"));
        lines.push(format!("**Release:** {}{version}", release.name));
    }
    lines.push(format!(
        "**Created:** {}",
        crate::commands::relative_time::format_relative_time(&document.created_at.0, now, zone)
    ));
    lines.push(format!(
        "**Updated:** {}",
        crate::commands::relative_time::format_relative_time(&document.updated_at.0, now, zone)
    ));
    if let Some(content) = content.filter(|content| !content.is_empty()) {
        lines.extend([
            String::new(),
            "---".to_owned(),
            String::new(),
            content.to_owned(),
        ]);
    }
    lines.join("\n")
}
