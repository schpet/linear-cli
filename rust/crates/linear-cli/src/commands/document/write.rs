//! `document create` and `document update`: fields from flags, stdin, an
//! editor or prompts, then one mutation.
use std::io::{Read, Write};

use cynic::{MutationBuilder, QueryBuilder};

use crate::cli::document::{DocumentCreate, DocumentUpdate};
use crate::commands::team_key::configured_team_key;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::document_write::*;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::editor;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::platform::prompt_text::TextOptions;

use super::target::{self, Kind, PreparedTarget, TargetOptions};

pub fn create(ctx: &Ctx, args: &DocumentCreate) -> Result<()> {
    create_document(ctx, args).context("Failed to create document")
}

pub fn update(ctx: &Ctx, args: &DocumentUpdate) -> Result<()> {
    update_document(ctx, args).context("Failed to update document")
}

fn create_document(ctx: &Ctx, args: &DocumentCreate) -> Result<()> {
    let flags = TargetOptions {
        project: args.project.as_deref(),
        issue: args.issue.as_deref(),
        initiative: args.initiative.as_deref(),
        team: args.team.as_deref(),
        cycle: args.cycle.as_deref(),
        release: args.release.as_deref(),
    };
    let no_flags = args.title.is_none()
        && args.content.is_none()
        && args.content_file.is_none()
        && args.icon.is_none()
        && !flags.any();
    let fields = if ctx.stdout_tty() && (args.interactive || no_flags) {
        prompted(ctx)?
    } else {
        let title = args.title.clone().ok_or_else(|| {
            Error::new("Title is required")
                .with_hint("Use --title or run with -i for interactive mode.")
        })?;
        if !flags.any() {
            return Err(
                Error::new("A document attachment target is required").with_hint(
                    "Pass one of --project, --issue, --initiative, --team, --cycle, or --release.",
                ),
            );
        }
        let content = match (&args.content, &args.content_file) {
            (Some(content), _) => Some(content.clone()),
            (None, Some(path)) => Some(read_file(path)?),
            (None, None) if !ctx.stdin_tty() => text_input::read_stdin(std::io::stdin().lock())?,
            (None, None) if ctx.stdout_tty() => {
                ctx.print("Opening editor for document content...\n")?;
                let content = optional_editor(ctx)?;
                if content.is_none() {
                    ctx.print("No content entered. Creating document without content.\n")?;
                }
                content
            }
            (None, None) => None,
        };
        Fields {
            title,
            content,
            icon: args.icon.clone(),
            project: args.project.clone(),
            issue: args.issue.clone(),
            initiative: args.initiative.clone(),
            team: args.team.clone(),
            cycle: args.cycle.clone(),
            release: args.release.clone(),
        }
    };
    let target = target::prepare(ctx, fields.target())?;
    let client = ctx.client()?;
    let mut input = DocumentUpdateInput {
        content: fields.content,
        icon: fields.icon,
        ..Default::default()
    };
    let title = fields.title;
    let created = ctx.spin(true, async {
        attach(client, &mut input, target.as_ref()).await?;
        let request =
            GraphQlRequest::with_variables(CreateDocument::build(CreateDocumentVariables {
                input: DocumentCreateInput {
                    title,
                    content: input.content,
                    icon: input.icon,
                    project_id: input.project_id,
                    issue_id: input.issue_id,
                    initiative_id: input.initiative_id,
                    team_id: input.team_id,
                    cycle_id: input.cycle_id,
                    release_id: input.release_id,
                },
            }));
        let data: CreateDocument = client.execute(&request).await?;
        if !data.document_create.success {
            return Err(Error::new("Linear did not create the document"));
        }
        Ok(data.document_create.document)
    })?;
    ctx.print(format!(
        "✓ Created document: {}\n{}\n",
        created.title, created.url
    ))
}

fn update_document(ctx: &Ctx, args: &DocumentUpdate) -> Result<()> {
    let id = super::reference(ctx, &args.document_id)?;
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
    let metadata = args.title.is_some() || args.icon.is_some() || target.is_some();
    let content = match (&args.content, &args.content_file) {
        (Some(content), _) => Some(content.clone()),
        (None, Some(path)) => Some(read_file(path)?),
        // Piped stdin is the new content when nothing else is being changed.
        (None, None) if !args.edit && !metadata && !ctx.stdin_tty() => {
            text_input::read_stdin(std::io::stdin().lock())?
        }
        (None, None) => None,
    };
    let edit = args.edit && content.is_none();
    if content.is_none() && !edit && !metadata {
        return Err(Error::new("No update fields provided").with_hint(
            "Use --title, --content, --content-file, --icon, --edit, or re-point the attachment with --project, --issue, --initiative, --team, --cycle, or --release.",
        ));
    }
    let client = ctx.client()?;
    let mut input = DocumentUpdateInput {
        title: args.title.clone(),
        icon: args.icon.clone(),
        content,
        ..Default::default()
    };
    if target.is_some() {
        ctx.spin(true, attach(client, &mut input, target.as_ref()))?;
    }
    if edit {
        let document = ctx.spin(true, for_edit(client, &id))?;
        let seed = document.content.unwrap_or_default();
        ctx.print(format!("Opening {} in editor...\n", document.title))?;
        let edited = ctx.edit_text(&seed)?;
        if edited == seed {
            return ctx.print("No changes detected, update cancelled.\n");
        }
        let Some(content) = text_input::edited_body(&edited) else {
            return ctx.print("No changes made, update cancelled.\n");
        };
        input.content = Some(content);
    }
    let updated = ctx.spin(true, async {
        if input.content.is_some() && !args.force {
            refuse_inline_comments(client, &id).await?;
        }
        let request =
            GraphQlRequest::with_variables(UpdateDocument::build(UpdateDocumentVariables {
                id: id.clone(),
                input,
            }));
        let data: UpdateDocument = client.execute(&request).await?;
        if !data.document_update.success {
            return Err(Error::new("Linear did not update the document"));
        }
        Ok(data.document_update.document)
    })?;
    ctx.print(format!(
        "✓ Updated document: {}\n{}\n",
        updated.title, updated.url
    ))
}

/// Looks up the target and sets its ID in the field for its kind.
async fn attach(
    client: &GraphQlTransport,
    input: &mut DocumentUpdateInput,
    target: Option<&PreparedTarget>,
) -> Result<()> {
    let Some(target) = target else {
        return Ok(());
    };
    let (kind, id) = target::resolve(target, client).await?;
    let field = match kind {
        Kind::Project => &mut input.project_id,
        Kind::Issue => &mut input.issue_id,
        Kind::Initiative => &mut input.initiative_id,
        Kind::Team => &mut input.team_id,
        Kind::Cycle => &mut input.cycle_id,
        Kind::Release => &mut input.release_id,
    };
    *field = Some(id);
    Ok(())
}

async fn for_edit(client: &GraphQlTransport, id: &str) -> Result<DocumentForEdit> {
    let request =
        GraphQlRequest::with_variables(GetDocumentForEdit::build(DocumentEditVariables {
            id: id.to_owned(),
        }));
    let data: GetDocumentForEdit = client
        .execute(&request)
        .await
        .map_err(|failure| super::not_found(failure, id))?;
    data.document
        .ok_or_else(|| Error::not_found("Document", id))
}

/// Replacing the Markdown can detach or hide inline comments, so content
/// updates stop while any open comment quotes the document.
async fn refuse_inline_comments(client: &GraphQlTransport, id: &str) -> Result<()> {
    let comments = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(DocumentInlineCommentGuard::build(
            DocumentGuardVariables {
                id: id.to_owned(),
                after,
            },
        ));
        async move {
            let data: DocumentInlineCommentGuard = client
                .execute(&request)
                .await
                .map_err(|failure| super::not_found(failure, id))?;
            let document = data
                .document
                .ok_or_else(|| Error::not_found("Document", id))?;
            Ok::<_, Error>(Page {
                nodes: document.comments.nodes,
                page_info: document.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            Error::new("Linear reported more document comments but returned no usable cursor")
                .with_hint("Retry the command.")
        }
    })?;
    let open_quote = comments.nodes.into_iter().find_map(|comment| {
        let open = comment.resolved_at.is_none() && comment.archived_at.is_none();
        comment
            .quoted_text
            .filter(|_| open)
            .map(|quoted| (comment.id, quoted))
    });
    match open_quote {
        None => Ok(()),
        Some((comment, quoted)) => Err(Error::new(
            "Refusing to update document content because this document has inline comments.",
        )
        .with_hint(format!(
            "Updating Markdown content can detach or hide Linear document comments. First review comment {} quoting \"{quoted}\", then rerun with --force if you accept that risk.",
            comment.inner()
        ))),
    }
}

fn read_file(path: &str) -> Result<String> {
    text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::not_found("File", path)
        } else {
            Error::new(format!("Failed to read {path}: {error}")).with_source(error)
        }
    })
}

/// Opens an empty editor. An editor failure is reported on stderr and
/// leaves the document without content, so the rest of the flow continues.
fn optional_editor(ctx: &Ctx) -> Result<Option<String>> {
    match ctx.edit_text("") {
        Ok(text) => Ok(text_input::edited_body(&text)),
        Err(error) => {
            ctx.eprint(format!("{error}\n"))?;
            Ok(None)
        }
    }
}

/// A new document's fields, from flags or prompts.
struct Fields {
    title: String,
    content: Option<String>,
    icon: Option<String>,
    project: Option<String>,
    issue: Option<String>,
    initiative: Option<String>,
    team: Option<String>,
    cycle: Option<String>,
    release: Option<String>,
}

impl Fields {
    fn target(&self) -> TargetOptions<'_> {
        TargetOptions {
            project: self.project.as_deref(),
            issue: self.issue.as_deref(),
            initiative: self.initiative.as_deref(),
            team: self.team.as_deref(),
            cycle: self.cycle.as_deref(),
            release: self.release.as_deref(),
        }
    }
}

fn prompted(ctx: &Ctx) -> Result<Fields> {
    let default_team = configured_team_key(ctx.options());
    let editor = editor::configured(&ctx.config().child_env)
        .as_deref()
        .and_then(editor_label);
    let mut session = ctx.prompts()?;
    let result = prompt(
        &mut session,
        editor.as_deref(),
        default_team.as_deref(),
        || optional_editor(ctx),
    );
    match session.finish_result(result)? {
        PromptOutcome::Submitted(fields) => Ok(fields),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("Unexpected end of input at a prompt")),
    }
}

/// The editor's file name after the last `/`, for the content menu.
pub fn editor_label(name: &std::ffi::OsStr) -> Option<String> {
    name.to_string_lossy()
        .rsplit('/')
        .next()
        .map(str::to_owned)
        .filter(|label| !label.is_empty())
}

/// Asks for the title, content, icon and attachment. `edit` opens the
/// editor named `editor` while the prompts are suspended.
fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    editor: Option<&str>,
    default_team: Option<&str>,
    mut edit: impl FnMut() -> Result<Option<String>>,
) -> Result<PromptOutcome<Fields>> {
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let options = |required, default| TextOptions { required, default };
    let mut fields = Fields {
        title: answer!(session.text_with_options("Document title", options(true, None))),
        content: None,
        icon: None,
        project: None,
        issue: None,
        initiative: None,
        team: None,
        cycle: None,
        release: None,
    };
    let mut methods = vec![
        choice("Skip (no content)", "skip"),
        choice("Enter inline", "inline"),
    ];
    if let Some(label) = editor {
        methods.push(choice(&format!("Open {label}"), "editor"));
    }
    methods.push(choice("Read from file", "file"));
    let method = answer!(session.select(&PlainSelect {
        message: "How would you like to enter content?",
        options: &methods,
        default_index: 0,
        default_hint: None
    }));
    match method.as_str() {
        "skip" => {}
        "inline" => {
            fields.content = text_input::edited_body(&answer!(
                session.text_with_options("Content (markdown)", options(false, Some("")))
            ));
        }
        "file" => {
            let path = answer!(session.text_with_options("File path", options(false, None)));
            fields.content = Some(read_file(&path)?);
        }
        "editor" => {
            let label = editor.expect("the editor option is offered only with an editor");
            session.print_line(&format!("Opening {label}..."))?;
            session.suspend()?;
            let content = edit();
            session.resume()?;
            fields.content = content?;
            if let Some(content) = &fields.content {
                session.print_line(&format!(
                    "Content entered ({} characters)",
                    content.chars().count()
                ))?;
            }
        }
        other => unreachable!("not a content menu value: {other}"),
    }
    fields.icon = text_input::edited_body(&answer!(session.text_with_options(
        "Icon (emoji, leave blank for none)",
        options(false, Some(""))
    )));
    let targets = [
        choice("Project", "project"),
        choice("Issue", "issue"),
        choice("Team", "team"),
        choice("Initiative", "initiative"),
        choice("Cycle", "cycle"),
        choice("Release", "release"),
    ];
    let target = answer!(session.select(&PlainSelect {
        message: "Attach document to",
        options: &targets,
        default_index: 0,
        default_hint: None
    }));
    match target.as_str() {
        "project" => {
            fields.project = Some(answer!(
                session.text_with_options("Project (UUID, slug ID, or name)", options(false, None))
            ));
        }
        "issue" => {
            fields.issue = Some(answer!(
                session.text_with_options("Issue identifier (e.g., TC-123)", options(false, None))
            ));
        }
        "team" => {
            fields.team = Some(answer!(
                session.text_with_options("Team key (e.g., ENG)", options(false, default_team))
            ));
        }
        "initiative" => {
            fields.initiative = Some(answer!(
                session
                    .text_with_options("Initiative (UUID, slug ID, or name)", options(false, None))
            ));
        }
        "cycle" => {
            fields.team = Some(answer!(session.text_with_options(
                "Team key for the cycle (e.g., ENG)",
                options(false, default_team)
            )));
            fields.cycle = Some(answer!(session.text_with_options(
                "Cycle (name, number, 'active', 'next', or 'previous')",
                options(false, None)
            )));
        }
        "release" => {
            fields.release = Some(answer!(
                session.text_with_options("Release (UUID, name, or version)", options(false, None))
            ));
        }
        other => unreachable!("not an attachment menu value: {other}"),
    }
    Ok(PromptOutcome::Submitted(fields))
}

fn choice(label: &str, value: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    }
}
