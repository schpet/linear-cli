//! `issue view`: the issue as Markdown with comment threads, or JSON, or
//! opened in Linear.
use crate::client::LinearClient;
use crate::commands::comments;
use crate::graphql::pagination::{self, Page, PageInfo};
use crate::{
    cli::issue::IssueView,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue_read::*,
    platform::{
        markdown_assets,
        markdown_terminal::{self, RenderOptions},
    },
};
use chrono::{DateTime, Utc};

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
pub fn run(ctx: &Ctx, args: &IssueView) -> Result<()> {
    view(ctx, args).context("Failed to view issue")
}

fn view(ctx: &Ctx, args: &IssueView) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    if args.web || args.app {
        return ctx.open_in_linear(&format!("issue/{identifier}"), args.app);
    }
    let client = ctx.client()?;
    let fetched = ctx.spin(!args.json, fetch(client, identifier, !args.no_comments))?;
    if args.json {
        return ctx.print(fetched.json());
    }
    let options = ctx.options();
    let download = !args.no_download && options.download_images();
    let attachments = download && options.auto_download_attachments();
    let mut issue = fetched.into_issue();
    if download {
        let image_root = ctx.cache_dir("images");
        ctx.block_on(download_images(
            client,
            &mut issue,
            image_root.as_deref(),
            |line| ctx.eprint(line),
        ))?;
    }
    let downloadable = issue
        .attachments
        .nodes
        .iter()
        .any(|attachment| hosted_by_linear(&attachment.url));
    let paths = if attachments && downloadable {
        let attachment_root = match options.attachment_dir() {
            Some(dir) => PathBuf::from(dir),
            None => {
                let root = ctx.cache_dir("attachments");
                markdown_assets::private_cache(root.as_deref())?.to_owned()
            }
        };
        ctx.block_on(download_attachments(
            client,
            &issue,
            &attachment_root,
            |bytes| ctx.eprint(bytes),
        ))?
    } else {
        HashMap::new()
    };
    let now = Utc::now();
    if !ctx.stdout_tty() {
        let markdown = markdown(&issue, &paths, args.show_resolved_threads, now)?;
        return ctx.print(format!("{markdown}\n"));
    }
    let render = ctx.render_options();
    let rendered = terminal(
        &issue,
        &paths,
        args.show_resolved_threads,
        now,
        &render,
        true,
    )?;
    ctx.page(&rendered, !args.no_pager)
}

pub type Issue = GetIssueDetailsWithCommentsIssue;
pub type Comment = GetIssueDetailsWithCommentsIssueCommentsNodes;
pub enum Fetched {
    Without(GetIssueDetailsIssue),
    With(Issue),
}
impl Fetched {
    pub fn json(&self) -> Vec<u8> {
        match self {
            Self::Without(i) => crate::commands::json::render(i),
            Self::With(i) => crate::commands::json::render(i),
        }
    }
    pub fn into_issue(self) -> Issue {
        match self {
            Self::With(i) => i,
            Self::Without(i) => Issue {
                identifier: i.identifier,
                title: i.title,
                description: i.description,
                url: i.url,
                branch_name: i.branch_name,
                state: i.state,
                assignee: i.assignee,
                priority: i.priority,
                project: i.project,
                project_milestone: i.project_milestone,
                cycle: i.cycle,
                team: i.team,
                labels: i.labels,
                parent: i.parent,
                children: i.children,
                comments: GetIssueDetailsWithCommentsIssueComments {
                    nodes: vec![],
                    page_info: PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                },
                attachments: i.attachments,
                documents: i.documents,
            },
        }
    }
}
/// The issue with every label, sub-issue, attachment, document and, with
/// `comments`, comment. Collections longer than the page that came with the
/// issue are fetched page by page.
pub async fn fetch(client: &LinearClient, id: String, comments: bool) -> Result<Fetched, Error> {
    let missing = || Error::not_found("Issue", &id);
    if comments {
        let data: GetIssueDetailsWithComments = client
            .query(GetIssueDetailsWithCommentsVariables { id: id.clone() })
            .await
            .map_err(|failure| failure.or_not_found("Issue", &id))?;
        let mut issue = data.issue.ok_or_else(missing)?;
        let lists = Lists {
            labels: &mut issue.labels,
            children: &mut issue.children,
            attachments: &mut issue.attachments,
            documents: &mut issue.documents,
        };
        lists.finish(client, &id).await?;
        let comments = &mut issue.comments;
        let id = id.as_str();
        comments.nodes = rest(
            &mut comments.nodes,
            &comments.page_info,
            |after, first| async move {
                let data: GetIssueCommentsPage = client.query(page(id, after, first)).await?;
                let comments = data.issue.comments;
                Ok(Page {
                    nodes: comments.nodes,
                    page_info: comments.page_info,
                })
            },
        )
        .await?;
        Ok(Fetched::With(issue))
    } else {
        let data: GetIssueDetails = client
            .query(GetIssueDetailsVariables { id: id.clone() })
            .await
            .map_err(|failure| failure.or_not_found("Issue", &id))?;
        let mut issue = data.issue.ok_or_else(missing)?;
        let lists = Lists {
            labels: &mut issue.labels,
            children: &mut issue.children,
            attachments: &mut issue.attachments,
            documents: &mut issue.documents,
        };
        lists.finish(client, &id).await?;
        Ok(Fetched::Without(issue))
    }
}

/// The collections both issue selections share.
struct Lists<'a> {
    labels: &'a mut IssueLabels,
    children: &'a mut GetIssueDetailsIssueChildren,
    attachments: &'a mut GetIssueDetailsIssueAttachments,
    documents: &'a mut GetIssueDetailsIssueDocuments,
}

impl Lists<'_> {
    /// Fetches the rest of every collection Linear reports more pages of.
    async fn finish(self, client: &LinearClient, id: &str) -> Result<()> {
        let Self {
            labels,
            children,
            attachments,
            documents,
        } = self;
        labels.nodes = rest(
            &mut labels.nodes,
            &labels.page_info,
            |after, first| async move {
                let data: GetIssueLabelsPage = client.query(page(id, after, first)).await?;
                let labels = data.issue.labels;
                Ok(Page {
                    nodes: labels.nodes,
                    page_info: labels.page_info,
                })
            },
        )
        .await?;
        children.nodes = rest(
            &mut children.nodes,
            &children.page_info,
            |after, first| async move {
                let data: GetIssueChildrenPage = client.query(page(id, after, first)).await?;
                let children = data.issue.children;
                Ok(Page {
                    nodes: children.nodes,
                    page_info: children.page_info,
                })
            },
        )
        .await?;
        attachments.nodes = rest(
            &mut attachments.nodes,
            &attachments.page_info,
            |after, first| async move {
                let data: GetIssueAttachmentsPage = client.query(page(id, after, first)).await?;
                let attachments = data.issue.attachments;
                Ok(Page {
                    nodes: attachments.nodes,
                    page_info: attachments.page_info,
                })
            },
        )
        .await?;
        documents.nodes = rest(
            &mut documents.nodes,
            &documents.page_info,
            |after, first| async move {
                let data: GetIssueDocumentsPage = client.query(page(id, after, first)).await?;
                let documents = data.issue.documents;
                Ok(Page {
                    nodes: documents.nodes,
                    page_info: documents.page_info,
                })
            },
        )
        .await?;
        Ok(())
    }
}

/// Every node of a collection whose first page is `nodes`.
async fn rest<N, F, Fut>(nodes: &mut Vec<N>, page_info: &PageInfo, fetch: F) -> Result<Vec<N>>
where
    F: FnMut(Option<String>, i32) -> Fut,
    Fut: Future<Output = Result<Page<N>>>,
{
    let first = Page {
        nodes: std::mem::take(nodes),
        page_info: page_info.clone(),
    };
    pagination::complete(first, fetch).await
}

fn page(id: &str, after: Option<String>, first: i32) -> IssuePageVariables {
    IssuePageVariables {
        id: id.to_owned(),
        first,
        after,
    }
}
pub async fn download_images(
    client: &LinearClient,
    issue: &mut Issue,
    root: Option<&Path>,
    report: impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut sources = vec![];
    if let Some(body) = issue.description.as_deref() {
        sources.push(body);
    }
    sources.extend(issue.comments.nodes.iter().map(|c| c.body.as_str()));
    let paths = markdown_assets::download(client, root, &sources, report).await?;
    if !paths.is_empty() {
        if let Some(body) = issue.description.as_mut() {
            *body = markdown_assets::rewrite(body, &paths);
        }
        for comment in &mut issue.comments.nodes {
            comment.body = markdown_assets::rewrite(&comment.body, &paths);
        }
    }
    Ok(())
}
/// Whether `url` is a file Linear hosts; only those are downloaded.
fn hosted_by_linear(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        matches!(
            url.host_str(),
            Some("uploads.linear.app" | "public.linear.app")
        )
    })
}

pub async fn download_attachments<E>(
    client: &LinearClient,
    issue: &Issue,
    root: &Path,
    mut emit: E,
) -> Result<HashMap<String, String>, Error>
where
    E: FnMut(&[u8]) -> Result<(), Error>,
{
    let mut paths = HashMap::new();
    let directory = root.join(&issue.identifier);
    for attachment in &issue.attachments.nodes {
        let result: Result<Option<String>, Error> = async {
            if !hosted_by_linear(&attachment.url) {
                return Ok(None);
            }
            let path = markdown_assets::cache_path(
                &directory,
                &attachment.url,
                &attachment.title,
                "attachment",
            );
            markdown_assets::fetch_cached(&path, || {
                client.download_issue_attachment(&attachment.url)
            })
            .await
            .map(Some)
        }
        .await;
        match result {
            Ok(Some(path)) => {
                paths.insert(attachment.url.clone(), path);
            }
            Ok(None) => {}
            Err(error) => emit(
                format!(
                    "Failed to download attachment \"{}\": {}\n",
                    attachment.title,
                    error.message()
                )
                .as_bytes(),
            )?,
        }
    }
    Ok(paths)
}
fn hierarchy(issue: &Issue) -> String {
    let mut out = String::new();
    if let Some(parent) = &issue.parent {
        out.push_str(&format!(
            "\n\n## Parent\n\n- **{}**: {} _[{}]_\n",
            parent.identifier, parent.title, parent.state.name
        ));
    }
    if !issue.children.nodes.is_empty() {
        out.push_str("\n\n## Sub-issues\n\n");
        for child in &issue.children.nodes {
            out.push_str(&format!(
                "- **{}**: {} _[{}]_\n",
                child.identifier, child.title, child.state.name
            ));
        }
    }
    out
}
fn attachments(issue: &Issue, paths: &HashMap<String, String>) -> String {
    let mut out = String::new();
    if !issue.attachments.nodes.is_empty() {
        out.push_str("\n\n## Attachments\n\n");
        for a in &issue.attachments.nodes {
            out.push_str(&format!(
                "- **{}**: {}{}\n",
                a.title,
                paths.get(&a.url).unwrap_or(&a.url),
                a.source_type
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" _[{s}]_"))
                    .unwrap_or_default()
            ));
            if let Some(subtitle) = a.subtitle.as_deref().filter(|s| !s.is_empty()) {
                out.push_str(&format!("  _{subtitle}_\n"));
            }
        }
    }
    out
}
fn documents(issue: &Issue) -> String {
    let mut out = String::new();
    if !issue.documents.nodes.is_empty() {
        out.push_str("\n\n## Documents\n\n");
        for d in &issue.documents.nodes {
            out.push_str(&format!("- **{}**: {}\n", d.title, d.url));
        }
    }
    out
}
fn body(issue: &Issue) -> Result<String, Error> {
    let mut parts = vec![
        format!("**State:** {}", issue.state.name),
        format!(
            "**Priority:** {}",
            super::list_view::priority(issue.priority)
        ),
        format!(
            "**Assignee:** {}",
            issue
                .assignee
                .as_ref()
                .map(|a| format!("@{}", a.display_name))
                .unwrap_or_else(|| "Unassigned".to_owned())
        ),
    ];
    if let Some(project) = &issue.project {
        parts.push(format!("**Project:** {}", project.name));
    }
    if let Some(m) = &issue.project_milestone {
        parts.push(format!("**Milestone:** {}", m.name));
    }
    if let Some(c) = &issue.cycle {
        let (short, _) = super::list_view::cycle_short(
            Some(c),
            issue.team.active_cycle.as_ref().map(|c| c.number),
        );
        let label = format!(
            "#{}{}",
            c.number,
            c.name.as_ref().map(|n| format!(" {n}")).unwrap_or_default()
        );
        parts.push(format!(
            "**Cycle:** {label}{}",
            if short.starts_with('#') {
                String::new()
            } else {
                format!(" ({short})")
            }
        ));
    }
    Ok(format!(
        "# {}: {}\n\n{}{}",
        issue.identifier,
        issue.title,
        parts.join(" | "),
        issue
            .description
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("\n\n{s}"))
            .unwrap_or_default()
    ))
}
#[derive(Debug)]
pub struct Threads<'a> {
    pub roots: Vec<&'a Comment>,
    pub replies: HashMap<String, Vec<&'a Comment>>,
    pub hidden: usize,
}
pub fn threads(comments: &[Comment], show_resolved: bool) -> Result<Threads<'_>, Error> {
    let mut roots = comments
        .iter()
        .filter(|c| c.parent.is_none())
        .collect::<Vec<_>>();
    roots.sort_by_key(|comment| comment.created_at);
    let by_id = comments
        .iter()
        .map(|c| (c.id.inner(), c))
        .collect::<HashMap<_, _>>();
    let mut replies: HashMap<String, Vec<&Comment>> = HashMap::new();
    for comment in comments.iter().filter(|c| c.parent.is_some()) {
        let mut id = comment.id.inner();
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(id) {
                return Err(Error::new("Issue comment parent graph contains a cycle"));
            }
            match by_id.get(id).and_then(|c| c.parent.as_ref()) {
                Some(parent) => id = parent.id.inner(),
                None => break,
            }
        }
        replies.entry(id.to_owned()).or_default().push(comment);
    }
    for children in replies.values_mut() {
        children.sort_by_key(|comment| comment.created_at);
    }
    let before = roots.len();
    if !show_resolved {
        roots.retain(|c| c.resolved_at.is_none());
    }
    let hidden = before - roots.len();
    Ok(Threads {
        roots,
        replies,
        hidden,
    })
}
fn author(c: &Comment) -> &str {
    c.user
        .as_ref()
        .map(|u| u.display_name.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            c.user
                .as_ref()
                .map(|u| u.name.as_str())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            c.external_user
                .as_ref()
                .map(|u| u.display_name.as_str())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            c.external_user
                .as_ref()
                .map(|u| u.name.as_str())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("Unknown")
}
fn date(c: &Comment, now: DateTime<Utc>) -> String {
    crate::commands::relative_time::ago(c.created_at.0, now, &chrono::Local)
}
/// A thread's ID in brackets (linked to it when `links`), then
/// `[resolved]` for a resolved thread, each with its display width.
fn suffix(c: &Comment, links: bool) -> Vec<(String, usize)> {
    let mut parts = vec![comments::id_part(c.id.inner(), &c.url, links)];
    if c.resolved_at.is_some() {
        parts.push(("[resolved]".to_owned(), "[resolved]".len()));
    }
    parts
}

fn summary(hidden: usize) -> String {
    format!(
        "Resolved {} hidden: {hidden}. Use --show-resolved-threads to show them.",
        if hidden == 1 { "thread" } else { "threads" }
    )
}
pub fn markdown(
    issue: &Issue,
    paths: &HashMap<String, String>,
    show_resolved: bool,
    now: DateTime<Utc>,
) -> Result<String, Error> {
    let mut out = body(issue)?;
    out.push_str(&hierarchy(issue));
    out.push_str(&attachments(issue, paths));
    out.push_str(&documents(issue));
    let threads = threads(&issue.comments.nodes, show_resolved)?;
    if !threads.roots.is_empty() {
        out.push_str("\n\n## Comments\n\n");
        for root in threads.roots {
            let suffix: Vec<String> = suffix(root, false)
                .into_iter()
                .map(|(text, _)| text)
                .collect();
            out.push_str(&format!(
                "- **@{}** commented *{}* {}\n\n  {}\n\n",
                author(root),
                date(root, now),
                suffix.join(" "),
                root.body.replace('\n', "\n  ")
            ));
            for reply in threads.replies.get(root.id.inner()).into_iter().flatten() {
                out.push_str(&format!(
                    "  - **@{}** replied *{}*\n\n    {}\n\n",
                    author(reply),
                    date(reply, now),
                    reply.body.replace('\n', "\n    ")
                ));
            }
        }
    }
    if threads.hidden > 0 {
        out.push_str(&format!("\n\n{}", summary(threads.hidden)));
    }
    Ok(out)
}
pub fn terminal(
    issue: &Issue,
    paths: &HashMap<String, String>,
    show_resolved: bool,
    now: DateTime<Utc>,
    options: &RenderOptions,
    links: bool,
) -> Result<String, Error> {
    let mut out = markdown_terminal::render(&body(issue)?, options);
    for section in [
        hierarchy(issue),
        attachments(issue, paths),
        documents(issue),
    ] {
        if !section.is_empty() {
            out.push('\n');
            out.push_str(&markdown_terminal::render(&section, options));
        }
    }
    let threads = threads(&issue.comments.nodes, show_resolved)?;
    let header = |c: &Comment, verb: &str, suffix: Vec<(String, usize)>, indent: &str| {
        let action = format!("{verb} {}", date(c, now));
        comments::terminal_header(author(c), &action, suffix, indent, options)
    };
    if !threads.roots.is_empty() {
        out.push('\n');
        out.push_str(&markdown_terminal::render("## Comments", options));
        out.push('\n');
        for (index, root) in threads.roots.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&header(root, "commented", suffix(root, links), ""));
            out.push('\n');
            out.push_str(&markdown_terminal::render(&root.body, options));
            for reply in threads.replies.get(root.id.inner()).into_iter().flatten() {
                out.push('\n');
                out.push_str(&header(reply, "replied", Vec::new(), "  "));
                out.push('\n');
                out.push_str(&comments::render_reply(&reply.body, options));
            }
        }
    }
    if threads.hidden > 0 {
        out.push_str(&format!("\n\n{}", summary(threads.hidden)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
