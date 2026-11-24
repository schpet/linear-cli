//! `issue view`: the issue as Markdown with comment threads, or JSON, or
//! opened in Linear.
use crate::client::LinearClient;
use crate::commands::comments;
use crate::graphql::pagination::{self, Page, PageInfo};
use crate::graphql::scalars::TimelessDate;
use crate::platform::kitty::InlineImages;
use crate::refs::IssueReference;
use crate::templates::markdown::{push_markdown_label, push_markdown_link};
use crate::templates::{DisplayText, TemplateConfig, ViewPart, ViewSection, ViewSectionKind};
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
use futures_util::{StreamExt, TryStreamExt, stream};
use minijinja::Value;
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use super::lifecycle::Lifecycle;

pub fn run(ctx: &Ctx, args: &IssueView) -> Result<()> {
    view(ctx, args).context("Failed to view issue")
}

fn view(ctx: &Ctx, args: &IssueView) -> Result<()> {
    let several_inputs = args.issue_ids.len() > 1;
    let mut identifiers = Vec::new();
    for input in &args.issue_ids {
        let named = super::reference::require_all(ctx, input).map_err(|error| {
            if several_inputs {
                error.context(format!("Failed to resolve issue {input}"))
            } else {
                error
            }
        })?;
        identifiers.extend(named);
    }
    // A reference can name several issues, so the count is what was read.
    let multiple = identifiers.len() > 1;
    if args.web || args.app {
        let workspace = ctx.workspace_url_key()?;
        for identifier in identifiers {
            ctx.open_url(
                &format!("https://linear.app/{workspace}/issue/{identifier}"),
                args.app,
            )?;
        }
        return Ok(());
    }
    let client = ctx.client()?;
    let fetched: Vec<Fetched> = ctx.spin(!args.json, async {
        stream::iter(identifiers)
            .map(|identifier| async move {
                fetch(client, identifier.clone(), !args.no_comments)
                    .await
                    .map_err(|error| {
                        if multiple {
                            error.context(format!("Failed to fetch issue {identifier}"))
                        } else {
                            error
                        }
                    })
            })
            .buffered(5)
            .try_collect()
            .await
    })?;
    if args.json {
        return ctx.print(if multiple {
            crate::commands::json::render(&fetched)
        } else {
            crate::commands::json::render(
                fetched.first().expect("view resolves at least one issue"),
            )
        });
    }
    let now = Utc::now();
    // One registry for every issue, so image IDs and limits span the view.
    let images = ctx.stdout_tty().then(|| ctx.inline_images()).flatten();
    let views = fetched
        .into_iter()
        .map(|issue| render(ctx, args, client, issue, now, images.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    if !ctx.stdout_tty() {
        return ctx.print(format!("{}\n", views.join("\n\n---\n\n")));
    }
    let separator = format!("\n{}\n", ctx.render_markdown("---"));
    ctx.page_with_images(
        &views.join(&separator),
        images.as_deref().as_slice(),
        !args.no_pager,
    )
}

fn render(
    ctx: &Ctx,
    args: &IssueView,
    client: &LinearClient,
    fetched: Fetched,
    now: DateTime<Utc>,
    images: Option<&Arc<InlineImages>>,
) -> Result<String> {
    let options = ctx.options();
    let download = !args.no_download && options.download_images();
    let auto_download_attachments = download && options.auto_download_attachments();
    let mut issue = fetched.into_issue();
    let downloaded = if download {
        let image_root = ctx.cache_dir("images");
        ctx.block_on(download_images(
            client,
            &mut issue,
            image_root.as_deref(),
            |line| ctx.eprint(line),
        ))?
    } else {
        HashSet::new()
    };
    let downloadable = issue
        .attachments
        .nodes
        .iter()
        .any(|attachment| hosted_by_linear(&attachment.url));
    let paths = if auto_download_attachments && downloadable {
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
    let issue_ids = IssueIds {
        templates: ctx.templates(),
        styled: ctx.color(),
    };
    let issue_reference = IssueReference::new(&issue.identifier, Some(&issue.url));
    let threads = threads(
        &issue.comments.nodes,
        args.show_resolved_threads,
        |comment| {
            issue_ids
                .templates
                .format_comment_id(&issue_reference, comment, issue_ids.styled)
        },
    )?;
    let context = View {
        terminal: ctx.stdout_tty(),
        body: body_context(&issue, now, &issue_ids)?,
        sections: Sections {
            hierarchy: ViewSection::value(
                hierarchy(&issue, &issue_ids)?,
                ViewSectionKind::Hierarchy,
            ),
            attachments: ViewSection::value(
                attachments(&issue, &paths),
                ViewSectionKind::Attachments,
            ),
            documents: ViewSection::value(documents(&issue), ViewSectionKind::Documents),
            comments: ViewSection::value(
                markdown_comments(&threads, now),
                ViewSectionKind::Comments,
            ),
        },
    };
    let parts = issue_ids
        .templates
        .render_view(&context, issue_ids.styled)?;
    if !ctx.stdout_tty() {
        let mut markdown = String::new();
        for part in &parts {
            part.write_plain(&mut markdown);
        }
        return Ok(markdown);
    }
    let mut render = ctx.render_options();
    let mut bodies: Vec<&str> = issue.description.iter().map(String::as_str).collect();
    bodies.extend(
        issue
            .comments
            .nodes
            .iter()
            .map(|comment| comment.body.as_str()),
    );
    ctx.load_inline_images(images.map(Arc::as_ref), &bodies, &downloaded)?;
    render.images = images.cloned();
    Ok(terminal(parts, &threads, now, &render))
}

pub type Issue = GetIssueDetailsWithCommentsIssue;
pub type Comment = GetIssueDetailsWithCommentsIssueCommentsNodes;
#[derive(Serialize)]
#[serde(untagged)]
pub enum Fetched {
    Without(GetIssueDetailsIssue),
    With(Issue),
}
impl Fetched {
    pub fn into_issue(self) -> Issue {
        match self {
            Self::With(i) => i,
            Self::Without(i) => Issue {
                trashed: i.trashed,
                archived_at: i.archived_at,
                identifier: i.identifier,
                title: i.title,
                description: i.description,
                url: i.url,
                branch_name: i.branch_name,
                state: i.state,
                assignee: i.assignee,
                delegate: i.delegate,
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
                creator: i.creator,
                created_at: i.created_at,
                updated_at: i.updated_at,
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
/// Downloads the files the description and comments reference and points
/// them at the local copies, which it returns.
async fn download_images(
    client: &LinearClient,
    issue: &mut Issue,
    root: Option<&Path>,
    report: impl FnMut(String) -> Result<(), Error>,
) -> Result<HashSet<String>, Error> {
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
    Ok(paths.into_values().collect())
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
struct IssueIds<'a> {
    templates: &'a TemplateConfig,
    styled: bool,
}

impl IssueIds<'_> {
    fn markdown(&self, identifier: &str, url: &str) -> Result<String> {
        let display = self
            .templates
            .format_issue_id(&IssueReference::new(identifier, Some(url)), self.styled)?;
        let mut markdown = String::new();
        for span in display.spans {
            if let Some(url) = span.url {
                push_markdown_link(&mut markdown, &span.text, &url);
            } else {
                push_markdown_label(&mut markdown, &span.text);
            }
        }
        Ok(markdown)
    }
}

fn hierarchy(issue: &Issue, issue_ids: &IssueIds<'_>) -> Result<String> {
    let mut out = String::new();
    if let Some(parent) = &issue.parent {
        out.push_str(&format!(
            "\n\n## Parent\n\n- **{}{}**: {} _[{}]_\n",
            issue_ids.markdown(&parent.identifier, &parent.url)?,
            Lifecycle::of(parent.trashed, parent.archived_at).marker(),
            parent.title,
            parent.state.name
        ));
    }
    if !issue.children.nodes.is_empty() {
        out.push_str("\n\n## Sub-issues\n\n");
        for child in &issue.children.nodes {
            out.push_str(&format!(
                "- **{}{}**: {} _[{}]_\n",
                issue_ids.markdown(&child.identifier, &child.url)?,
                Lifecycle::of(child.trashed, child.archived_at).marker(),
                child.title,
                child.state.name
            ));
        }
    }
    Ok(out)
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
/// A user's display name, or their name when that is empty; `None` when both are.
fn user_name(user: &GetIssueDetailsIssueAssignee) -> Option<&str> {
    [user.display_name.as_str(), user.name.as_str()]
        .into_iter()
        .find(|name| !name.is_empty())
}

#[derive(Serialize)]
struct Body<'a> {
    issue: Value,
    display: BodyDisplay<'a>,
}

#[derive(Serialize)]
struct BodyDisplay<'a> {
    identifier: String,
    lifecycle: LifecycleDisplay,
    priority: String,
    cycle: Option<String>,
    creator: Option<&'a str>,
    created: String,
    updated: String,
}

#[derive(Serialize)]
struct LifecycleDisplay {
    kind: LifecycleKind,
    date: Option<TimelessDate>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LifecycleKind {
    Active,
    Archived,
    Deleted,
}

impl From<Lifecycle> for LifecycleDisplay {
    fn from(lifecycle: Lifecycle) -> Self {
        let (kind, timestamp) = match lifecycle {
            Lifecycle::Active => (LifecycleKind::Active, None),
            Lifecycle::Archived(at) => (LifecycleKind::Archived, Some(at)),
            Lifecycle::Deleted(at) => (LifecycleKind::Deleted, at),
        };
        Self {
            kind,
            date: timestamp.map(|at| TimelessDate::from(at.0.date_naive())),
        }
    }
}

fn body_context<'a>(
    issue: &'a Issue,
    now: DateTime<Utc>,
    issue_ids: &IssueIds<'_>,
) -> Result<Body<'a>> {
    let lifecycle = Lifecycle::of(issue.trashed, issue.archived_at);
    let ago = |at: &crate::graphql::scalars::DateTime| {
        crate::commands::relative_time::ago(at.0, now, &chrono::Local)
    };
    Ok(Body {
        issue: super::template_data(
            issue,
            issue.project.as_ref().map(|project| project.url.as_str()),
        ),
        display: BodyDisplay {
            identifier: issue_ids.markdown(&issue.identifier, &issue.url)?,
            lifecycle: lifecycle.into(),
            priority: super::list_view::priority(issue.priority),
            cycle: issue.cycle.as_ref().map(|cycle| cycle_label(issue, cycle)),
            creator: issue.creator.as_ref().and_then(user_name),
            created: ago(&issue.created_at),
            updated: ago(&issue.updated_at),
        },
    })
}

fn cycle_label(issue: &Issue, cycle: &GetIssuesForStateIssuesNodesCycle) -> String {
    let mut label = format!("#{}", cycle.number);
    if let Some(name) = &cycle.name {
        label.push(' ');
        label.push_str(name);
    }
    let active = issue.team.active_cycle.as_ref().map(|cycle| cycle.number);
    let (short, _) = super::list_view::cycle_short(Some(cycle), active);
    if short.starts_with('#') {
        label
    } else {
        format!("{label} ({short})")
    }
}
struct Threads<'a> {
    visible: Vec<Thread<'a>>,
    hidden: usize,
}

struct Thread<'a> {
    root: DisplayComment<'a>,
    replies: Vec<DisplayComment<'a>>,
}

struct DisplayComment<'a> {
    comment: &'a Comment,
    label: DisplayText,
}

impl DisplayComment<'_> {
    fn suffix(&self, styled: bool) -> Vec<(String, usize)> {
        let plain = self.label.plain();
        let mut parts = Vec::new();
        if !plain.trim().is_empty() {
            let width = plain.width();
            let text = if styled { self.label.terminal() } else { plain };
            parts.push((text, width));
        }
        if self.comment.parent.is_none() && self.comment.resolved_at.is_some() {
            parts.push((comments::RESOLVED.to_owned(), comments::RESOLVED.width()));
        }
        parts
    }
}

fn threads(
    comments: &[Comment],
    show_resolved: bool,
    mut format_label: impl FnMut(&Comment) -> Result<DisplayText>,
) -> Result<Threads<'_>> {
    let mut roots = comments
        .iter()
        .filter(|c| c.parent.is_none())
        .collect::<Vec<_>>();
    roots.sort_by_key(|comment| comment.created_at);
    let by_id = comments
        .iter()
        .map(|c| (c.id.inner(), c))
        .collect::<HashMap<_, _>>();
    let mut replies: HashMap<&str, Vec<&Comment>> = HashMap::new();
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
        replies.entry(id).or_default().push(comment);
    }
    for children in replies.values_mut() {
        children.sort_by_key(|comment| comment.created_at);
    }
    let before = roots.len();
    if !show_resolved {
        roots.retain(|c| c.resolved_at.is_none());
    }
    let hidden = before - roots.len();
    let mut visible = Vec::with_capacity(roots.len());
    for comment in roots {
        let root = DisplayComment {
            comment,
            label: format_label(comment)?,
        };
        let mut children = Vec::new();
        for &reply in replies.get(comment.id.inner()).into_iter().flatten() {
            children.push(DisplayComment {
                comment: reply,
                label: format_label(reply)?,
            });
        }
        visible.push(Thread {
            root,
            replies: children,
        });
    }
    Ok(Threads { visible, hidden })
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
fn summary(hidden: usize) -> String {
    format!(
        "Resolved {} hidden: {hidden}. Use --show-resolved-threads to show them.",
        if hidden == 1 { "thread" } else { "threads" }
    )
}
/// A comment's body as Markdown, after the description passage it is
/// anchored to, if any.
fn body_markdown(c: &Comment) -> String {
    comments::quoted_body(c.quoted_text.as_deref(), &c.body)
}

#[derive(Serialize)]
struct View<'a> {
    #[serde(flatten)]
    body: Body<'a>,
    terminal: bool,
    sections: Sections,
}

#[derive(Serialize)]
struct Sections {
    hierarchy: Value,
    attachments: Value,
    documents: Value,
    comments: Value,
}

fn markdown_comments(threads: &Threads<'_>, now: DateTime<Utc>) -> String {
    let mut out = String::new();
    if !threads.visible.is_empty() {
        out.push_str("\n\n## Comments\n\n");
        for thread in &threads.visible {
            write_markdown_comment(&mut out, &thread.root, "commented", "", now);
            for reply in &thread.replies {
                write_markdown_comment(&mut out, reply, "replied", "  ", now);
            }
        }
    }
    if threads.hidden > 0 {
        out.push_str(&format!("\n\n{}", summary(threads.hidden)));
    }
    out
}

fn write_markdown_comment(
    out: &mut String,
    displayed: &DisplayComment<'_>,
    verb: &str,
    indent: &str,
    now: DateTime<Utc>,
) {
    let comment = displayed.comment;
    out.push_str(&format!(
        "{indent}- **@{}** {verb} *{}*",
        author(comment),
        date(comment, now),
    ));
    for (text, _) in displayed.suffix(false) {
        out.push(' ');
        out.push_str(&text);
    }
    out.push_str(&format!(
        "\n\n{indent}  {}\n\n",
        body_markdown(comment).replace('\n', &format!("\n{indent}  ")),
    ));
}

fn terminal(
    parts: Vec<ViewPart>,
    threads: &Threads<'_>,
    now: DateTime<Utc>,
    options: &RenderOptions,
) -> String {
    let mut out = String::new();
    let mut rendered_comments = None;
    for part in parts {
        match part {
            ViewPart::Markdown(markdown) => append_terminal_part(
                &mut out,
                &markdown_terminal::render_with_inlines(
                    &markdown.source,
                    &markdown.inlines,
                    options,
                ),
            ),
            ViewPart::Section(ViewSectionKind::Comments, _) => {
                let rendered = rendered_comments
                    .get_or_insert_with(|| terminal_comments(threads, now, options));
                if threads.visible.is_empty() && !out.is_empty() {
                    out.push_str("\n\n");
                } else {
                    separate_terminal_part(&mut out);
                }
                out.push_str(rendered);
            }
            ViewPart::Section(_, markdown) => {
                append_terminal_part(&mut out, &markdown_terminal::render(&markdown, options))
            }
        }
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn append_terminal_part(out: &mut String, rendered: &str) {
    if rendered.is_empty() {
        return;
    }
    separate_terminal_part(out);
    out.push_str(rendered);
}

fn separate_terminal_part(out: &mut String) {
    let end = out.trim_end_matches('\n').len();
    out.truncate(end);
    if end > 0 {
        out.push_str("\n\n");
    }
}

fn terminal_comments(threads: &Threads<'_>, now: DateTime<Utc>, options: &RenderOptions) -> String {
    let mut out = String::new();
    let header = |displayed: &DisplayComment<'_>, verb: &str, indent: &str| {
        let comment = displayed.comment;
        let action = format!("{verb} {}", date(comment, now));
        comments::terminal_header(
            author(comment),
            &action,
            displayed.suffix(options.styled),
            indent,
            options,
        )
    };
    if !threads.visible.is_empty() {
        out.push_str(&markdown_terminal::render("## Comments", options));
        out.push('\n');
        for (index, Thread { root, replies }) in threads.visible.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&header(root, "commented", ""));
            out.push('\n');
            out.push_str(&markdown_terminal::render(
                &body_markdown(root.comment),
                options,
            ));
            for reply in replies {
                out.push('\n');
                out.push_str(&header(reply, "replied", "  "));
                out.push('\n');
                out.push_str(&comments::render_reply(
                    &body_markdown(reply.comment),
                    options,
                ));
            }
        }
    }
    if threads.hidden > 0 {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&summary(threads.hidden));
    }
    out
}

#[cfg(test)]
mod tests;
