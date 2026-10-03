//! `issue view`: the issue as Markdown with comment threads, or JSON, or
//! opened in Linear.
use crate::{
    cli::issue::IssueView,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{envelope::GraphQlRequest, operations::issue_read::*, transport::GraphQlTransport},
    platform::{
        markdown_assets,
        markdown_terminal::{self, RenderOptions},
    },
};
use chrono::{DateTime, Utc};
use cynic::QueryBuilder;
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
        return ctx.print(format!("{}\n", fetched.json()?));
    }
    let options = ctx.options();
    let download = !args.no_download && options.download_images().is_none_or(|v| *v.value());
    let attachments = download
        && options
            .auto_download_attachments()
            .is_none_or(|v| *v.value());
    let image_root = &ctx.config().image_cache_root;
    let mut issue = fetched.into_issue();
    if download {
        ctx.block_on(download_images(client, &mut issue, image_root, |line| {
            ctx.eprint(line)
        }))?;
    }
    let paths = if attachments {
        let attachment_root = options
            .attachment_dir()
            .map(|v| v.value().clone())
            .filter(|dir| !dir.is_empty())
            .map_or_else(
                || {
                    image_root
                        .parent()
                        .unwrap_or(Path::new("/tmp"))
                        .join("linear-cli-attachments")
                },
                PathBuf::from,
            );
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
    let columns = crate::platform::pager::stdout_size()
        .and_then(|size| std::num::NonZeroU16::new(size.columns))
        .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
    let file_link = options.hyperlink_format().map(|v| v.value().as_str());
    let render = RenderOptions::for_terminal(columns, ctx.color(), file_link);
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
    pub fn json(&self) -> Result<String, Error> {
        match self {
            Self::Without(i) => serde_json::to_string_pretty(i),
            Self::With(i) => serde_json::to_string_pretty(i),
        }
        .map_err(|e| Error::new("could not serialize issue JSON").with_source(e))
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
                comments: GetIssueDetailsWithCommentsIssueComments { nodes: vec![] },
                attachments: i.attachments,
                documents: i.documents,
            },
        }
    }
}
pub async fn fetch(
    transport: &GraphQlTransport,
    id: String,
    comments: bool,
) -> Result<Fetched, Error> {
    let missing = || Error::not_found("Issue", &id);
    if comments {
        let data: GetIssueDetailsWithComments = super::read::exchange(
            transport,
            &GraphQlRequest::with_variables(GetIssueDetailsWithComments::build(
                GetIssueDetailsWithCommentsVariables { id: id.clone() },
            )),
        )
        .await?;
        Ok(Fetched::With(data.issue.ok_or_else(missing)?))
    } else {
        let data: GetIssueDetails = super::read::exchange(
            transport,
            &GraphQlRequest::with_variables(GetIssueDetails::build(GetIssueDetailsVariables {
                id: id.clone(),
            })),
        )
        .await?;
        Ok(Fetched::Without(data.issue.ok_or_else(missing)?))
    }
}
pub async fn download_images(
    transport: &GraphQlTransport,
    issue: &mut Issue,
    root: &Path,
    report: impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut sources = vec![];
    if let Some(body) = issue.description.as_deref() {
        sources.push(body);
    }
    sources.extend(issue.comments.nodes.iter().map(|c| c.body.as_str()));
    let paths = markdown_assets::download(transport, root, &sources, report).await?;
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
pub async fn download_attachments<E>(
    transport: &GraphQlTransport,
    issue: &Issue,
    root: &Path,
    mut emit: E,
) -> Result<HashMap<String, String>, Error>
where
    E: FnMut(&[u8]) -> Result<(), Error>,
{
    let mut paths = HashMap::new();
    if issue.attachments.nodes.is_empty() {
        return Ok(paths);
    }
    let directory = root.join(&issue.identifier);
    std::fs::create_dir_all(&directory).map_err(io_error)?;
    for attachment in &issue.attachments.nodes {
        let result: Result<Option<String>, Error> = async {
            let Ok(url) = reqwest::Url::parse(&attachment.url) else {
                return Ok(None);
            };
            if !matches!(
                url.host_str(),
                Some("uploads.linear.app" | "public.linear.app")
            ) {
                return Ok(None);
            }
            let path = directory.join(markdown_assets::sanitized_filename(
                &attachment.title,
                "attachment",
            ));
            if !path.exists() {
                let bytes = transport.download_issue_attachment(&attachment.url).await?;
                std::fs::write(&path, bytes).map_err(io_error)?;
            }
            path.into_os_string()
                .into_string()
                .map(Some)
                .map_err(|_| Error::new("Attachment path is not valid UTF-8"))
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
fn io_error(error: std::io::Error) -> Error {
    Error::new(error.to_string()).with_source(error)
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
        format!("**Priority:** {}", super::read::priority(issue.priority)),
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
        let (short, _) =
            super::read::cycle_short(Some(c), issue.team.active_cycle.as_ref().map(|c| c.number));
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
fn chronological(a: &Comment, b: &Comment) -> std::cmp::Ordering {
    match (
        DateTime::parse_from_rfc3339(&a.created_at.0),
        DateTime::parse_from_rfc3339(&b.created_at.0),
    ) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => std::cmp::Ordering::Equal,
    }
}
pub fn threads(comments: &[Comment], show_resolved: bool) -> Result<Threads<'_>, Error> {
    let mut roots = comments
        .iter()
        .filter(|c| c.parent.is_none())
        .collect::<Vec<_>>();
    roots.sort_by(|a, b| chronological(a, b));
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
        children.sort_by(|a, b| chronological(a, b));
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
    crate::commands::relative_time::format_relative_time(&c.created_at.0, now, &chrono::Local)
}
fn suffix(c: &Comment, links: bool) -> String {
    let text = format!("[thread: {}]", c.id.inner());
    format!(
        "{}{}",
        if links {
            markdown_terminal::hyperlink(&text, &c.url)
        } else {
            text
        },
        if c.resolved_at.is_some() {
            " [resolved]"
        } else {
            ""
        }
    )
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
            out.push_str(&format!(
                "- **@{}** - *{}* {}\n\n  {}\n\n",
                author(root),
                date(root, now),
                suffix(root, false),
                root.body.replace('\n', "\n  ")
            ));
            for reply in threads.replies.get(root.id.inner()).into_iter().flatten() {
                out.push_str(&format!(
                    "  - **@{}** - *{}*\n\n    {}\n\n",
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
    let header = |c: &Comment, suffix: &str, indent: &str| {
        let author = format!("@{}", author(c));
        let date = format!("commented {}", date(c, now));
        format!(
            "{indent}{} {}{}",
            if options.styled {
                format!("\x1b[4m\x1b[1m{author}\x1b[22m\x1b[24m")
            } else {
                author
            },
            if options.styled {
                format!("\x1b[4m{date}\x1b[24m")
            } else {
                date
            },
            if suffix.is_empty() {
                String::new()
            } else {
                format!(" {suffix}")
            }
        )
    };
    if !threads.roots.is_empty() {
        out.push('\n');
        out.push_str(&markdown_terminal::render("## Comments", options));
        out.push('\n');
        for (index, root) in threads.roots.iter().enumerate() {
            if index > 0 {
                out.push_str("\n\n");
            }
            out.push_str(&header(root, &suffix(root, links), ""));
            out.push('\n');
            out.push_str(&markdown_terminal::render(&root.body, options));
            let replies = threads.replies.get(root.id.inner());
            if replies.is_some_and(|v| !v.is_empty()) {
                out.push('\n');
            }
            for reply in replies.into_iter().flatten() {
                out.push('\n');
                out.push_str(&header(reply, "", "  "));
                out.push('\n');
                let width = options.columns.get().saturating_sub(2);
                let reply_options = RenderOptions {
                    columns: std::num::NonZeroU16::new(width).unwrap_or(std::num::NonZeroU16::MIN),
                    ..options.clone()
                };
                out.push_str(
                    &markdown_terminal::render(&reply.body, &reply_options)
                        .split('\n')
                        .map(|line| format!("  {line}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
        }
    }
    if threads.hidden > 0 {
        out.push_str(&format!("\n\n{}", summary(threads.hidden)));
    }
    Ok(out)
}
