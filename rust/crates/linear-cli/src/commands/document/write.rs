//! `document create` and `document update`: fields from flags, stdin, an
//! editor or prompts, then one mutation.
use cynic::{MutationBuilder, QueryBuilder};

use crate::cli::document::{DocumentCreate, DocumentUpdate};
use crate::commands::team_key::configured_team_key;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::document_write::*;
use crate::graphql::pagination::{self, Page};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::editor;
use crate::platform::prompt::{Choice, Prompter, Text};

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
    if args.interactive && !ctx.interactive() {
        return Err(Error::new("Interactive mode needs a terminal").with_hint(
            "Pass --title and one of --project, --issue, --initiative, --team, --cycle, or --release instead of --interactive.",
        ));
    }
    let fields = if ctx.interactive() && (args.interactive || no_flags) {
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
            (None, None) if ctx.interactive() => {
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
    let comments = pagination::collect(None, |after, _first| {
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
                page_info: document.comments.page_info,
            })
        }
    })
    .await?;
    let open_quote = comments.into_iter().find_map(|comment| {
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
    let editor = editor::configured_name(&ctx.config().child_env);
    prompt(
        ctx,
        &ctx.prompter()?,
        editor.as_deref(),
        default_team.as_deref(),
    )
}

/// Asks for the title, content, icon and attachment. The content can come
/// from the editor named `editor`.
fn prompt(
    ctx: &Ctx,
    prompter: &Prompter<'_>,
    editor: Option<&str>,
    default_team: Option<&str>,
) -> Result<Fields> {
    let mut fields = Fields {
        title: prompter.text(Text::new("Document title").required())?,
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
        Choice::new("Skip (no content)", Content::Skip),
        Choice::new("Enter inline", Content::Inline),
    ];
    if let Some(label) = editor {
        methods.push(Choice::new(format!("Open {label}"), Content::Editor));
    }
    methods.push(Choice::new("Read from file", Content::File));
    match prompter.select("How would you like to enter content?", methods)? {
        Content::Skip => {}
        Content::Inline => {
            fields.content =
                text_input::edited_body(&prompter.text(Text::new("Content (markdown)"))?);
        }
        Content::File => {
            let path = prompter.text(Text::new("File path").required())?;
            fields.content = Some(read_file(&path)?);
        }
        Content::Editor => {
            let label = editor.expect("the editor option is offered only with an editor");
            ctx.print(format!("Opening {label}...\n"))?;
            fields.content = optional_editor(ctx)?;
            if let Some(content) = &fields.content {
                ctx.print(format!(
                    "Content entered ({} characters)\n",
                    content.chars().count()
                ))?;
            }
        }
    }
    fields.icon =
        text_input::edited_body(&prompter.text(Text::new("Icon (emoji, leave blank for none)"))?);
    let targets = vec![
        Choice::new("Project", Kind::Project),
        Choice::new("Issue", Kind::Issue),
        Choice::new("Team", Kind::Team),
        Choice::new("Initiative", Kind::Initiative),
        Choice::new("Cycle", Kind::Cycle),
        Choice::new("Release", Kind::Release),
    ];
    let team = |message| {
        let text = Text::new(message).required();
        prompter.text(match default_team {
            Some(team) => text.with_default(team),
            None => text,
        })
    };
    let required = |message| prompter.text(Text::new(message).required());
    match prompter.select("Attach document to", targets)? {
        Kind::Project => fields.project = Some(required("Project (UUID, slug ID, or name)")?),
        Kind::Issue => fields.issue = Some(required("Issue identifier (e.g., TC-123)")?),
        Kind::Team => fields.team = Some(team("Team key (e.g., ENG)")?),
        Kind::Initiative => {
            fields.initiative = Some(required("Initiative (UUID, slug ID, or name)")?);
        }
        Kind::Cycle => {
            fields.team = Some(team("Team key for the cycle (e.g., ENG)")?);
            fields.cycle = Some(required(
                "Cycle (name, number, 'active', 'next', or 'previous')",
            )?);
        }
        Kind::Release => fields.release = Some(required("Release (UUID, name, or version)")?),
    }
    Ok(fields)
}

/// Where prompted content comes from.
enum Content {
    Skip,
    Inline,
    Editor,
    File,
}
