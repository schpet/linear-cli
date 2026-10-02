//! `document create` and `document update`: fields from flags, stdin, an editor or prompts.
use crate::{
    commands::{
        document_target::{Kind, TargetOptions},
        text_input,
    },
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
    graphql::{
        envelope::GraphQlRequest, operations::document_write::*, transport::GraphQlTransport,
    },
    platform::{
        editor::{self, EditorOutcome},
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
};
use cynic::{MutationBuilder, QueryBuilder};
use std::{
    io::{Read, Write},
    path::Path,
};

#[derive(Debug, Default)]
pub struct Fields {
    pub title: Option<String>,
    pub content: Option<String>,
    pub icon: Option<String>,
    pub project: Option<String>,
    pub issue: Option<String>,
    pub initiative: Option<String>,
    pub team: Option<String>,
    pub cycle: Option<String>,
    pub release: Option<String>,
}
impl Fields {
    pub fn target(&self) -> TargetOptions<'_> {
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
/// The editor's file name after the last `/`, independent of host path rules.
pub fn editor_label(name: &std::ffi::OsStr) -> Option<String> {
    name.to_string_lossy()
        .rsplit('/')
        .next()
        .map(str::to_owned)
        .filter(|label| !label.is_empty())
}

pub fn input(title: Option<String>, icon: Option<String>) -> DocumentUpdateInput {
    DocumentUpdateInput {
        title,
        icon,
        ..Default::default()
    }
}
pub fn attach(input: &mut DocumentUpdateInput, kind: Kind, id: String) {
    match kind {
        Kind::Project => input.project_id = Some(id),
        Kind::Issue => input.issue_id = Some(id),
        Kind::Initiative => input.initiative_id = Some(id),
        Kind::Team => input.team_id = Some(id),
        Kind::Cycle => input.cycle_id = Some(id),
        Kind::Release => input.release_id = Some(id),
    }
}
pub fn has_fields(input: &DocumentUpdateInput) -> bool {
    input.title.is_some()
        || input.content.is_some()
        || input.icon.is_some()
        || input.project_id.is_some()
        || input.issue_id.is_some()
        || input.initiative_id.is_some()
        || input.team_id.is_some()
        || input.cycle_id.is_some()
        || input.release_id.is_some()
}
pub fn create_request(
    title: String,
    input: DocumentUpdateInput,
) -> GraphQlRequest<CreateDocumentVariables> {
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
    }))
}
pub fn update_request(
    id: &str,
    input: DocumentUpdateInput,
) -> GraphQlRequest<UpdateDocumentVariables> {
    GraphQlRequest::with_variables(UpdateDocument::build(UpdateDocumentVariables {
        id: id.to_owned(),
        input,
    }))
}
pub async fn create(
    transport: &GraphQlTransport,
    title: String,
    input: DocumentUpdateInput,
) -> Result<Vec<u8>, AppError> {
    let data: CreateDocument = transport
        .execute(&create_request(title, input))
        .await
        .map_err(AppError::from)?;
    if !data.document_create.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Document creation failed",
        ));
    }
    let document = data.document_create.document;
    Ok(format!("✓ Created document: {}\n{}\n", document.title, document.url).into_bytes())
}
pub async fn update(
    transport: &GraphQlTransport,
    id: &str,
    input: DocumentUpdateInput,
) -> Result<Vec<u8>, AppError> {
    let data: UpdateDocument = transport
        .execute(&update_request(id, input))
        .await
        .map_err(AppError::from)?;
    if !data.document_update.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Document update failed",
        ));
    }
    let document = data.document_update.document;
    Ok(format!("✓ Updated document: {}\n{}\n", document.title, document.url).into_bytes())
}
pub async fn for_edit(transport: &GraphQlTransport, id: &str) -> Result<DocumentForEdit, AppError> {
    let request =
        GraphQlRequest::with_variables(GetDocumentForEdit::build(DocumentEditVariables {
            id: id.to_owned(),
        }));
    let data: GetDocumentForEdit = transport.execute(&request).await.map_err(AppError::from)?;
    data.document
        .ok_or_else(|| AppError::not_found("Document", id))
}
pub async fn guard(transport: &GraphQlTransport, id: &str) -> Result<(), AppError> {
    let mut after = None;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let request = GraphQlRequest::with_variables(DocumentInlineCommentGuard::build(
            DocumentGuardVariables {
                id: id.to_owned(),
                after: after.clone(),
            },
        ));
        let data: DocumentInlineCommentGuard =
            transport.execute(&request).await.map_err(AppError::from)?;
        let document = data
            .document
            .ok_or_else(|| AppError::not_found("Document", id))?;
        for comment in document.comments.nodes {
            if let Some(quoted) = comment.quoted_text
                && comment.resolved_at.is_none()
                && comment.archived_at.is_none()
            {
                return Err(AppError::new(AppErrorKind::Validation,"Refusing to update document content because this document has inline comments.").with_suggestion(format!("Updating Markdown content can detach or hide Linear document comments. First review comment {} quoting \"{quoted}\", then rerun with --force if you accept that risk.",comment.id.into_inner())));
            }
        }
        if !document.comments.page_info.has_next_page {
            return Ok(());
        }
        let cursor = document.comments.page_info.end_cursor.ok_or_else(|| {
            AppError::new(
                AppErrorKind::GraphQl,
                "Document comments page has no end cursor",
            )
        })?;
        if !seen.insert(cursor.clone()) {
            return Err(AppError::new(
                AppErrorKind::GraphQl,
                "Document comments pagination cursor did not advance",
            ));
        }
        after = Some(cursor);
    }
}
pub fn file(path: &str, interactive: bool) -> Result<String, AppError> {
    match text_input::read_file(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(AppError::not_found("File", path))
        }
        Err(error) => Err(AppError::new(
            AppErrorKind::IoProcess,
            format!(
                "Failed to read {}file: {error}",
                if interactive { "" } else { "content " }
            ),
        )
        .with_source(error)),
    }
}
pub fn optional_editor(
    env: &ChildEnvOverlay,
    root: &Path,
    stderr: &mut dyn Write,
) -> Result<Option<String>, AppError> {
    match editor::open(env, None, root)? {
        EditorOutcome::Content(content) => Ok(content),
        EditorOutcome::Missing => {
            writeln!(stderr,"No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>").map_err(io_error)?;
            Ok(None)
        }
        EditorOutcome::Failed(error) => {
            writeln!(stderr, "{}", error.message).map_err(io_error)?;
            Ok(None)
        }
    }
}
pub fn required_editor(
    env: &ChildEnvOverlay,
    root: &Path,
    seed: &str,
) -> Result<Option<String>, AppError> {
    match editor::open(env, Some(seed), root)? {
        EditorOutcome::Content(content) => Ok(content),
        EditorOutcome::Missing => Err(AppError::new(AppErrorKind::Validation, "No editor found")
            .with_suggestion(editor::NO_EDITOR)),
        EditorOutcome::Failed(error) => Err(error),
    }
}
fn io_error(error: std::io::Error) -> AppError {
    AppError::new(AppErrorKind::IoProcess, "Failed to write editor diagnostic").with_source(error)
}
pub struct PromptSettings<'a> {
    pub env: &'a ChildEnvOverlay,
    pub temp_root: &'a Path,
    pub default_team: Option<&'a str>,
}
pub fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    stderr: &mut dyn Write,
    settings: PromptSettings<'_>,
) -> Result<PromptOutcome<Fields>, AppError> {
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
        title: Some(answer!(
            session.text_with_options("Document title", options(true, None))
        )),
        ..Default::default()
    };
    // Discovery for the menu is intentionally separate from discovery on open.
    let editor = editor::discover(settings.env);
    let editor_label = editor.as_deref().and_then(editor_label);
    let mut methods = vec![
        choice("Skip (no content)", "skip"),
        choice("Enter inline", "inline"),
    ];
    if let Some(label) = &editor_label {
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
        "skip" => (),
        "inline" => {
            fields.content = text_input::edited_body(&answer!(
                session.text_with_options("Content (markdown)", options(false, Some("")))
            ))
        }
        "file" => {
            fields.content = Some(file(
                &answer!(session.text_with_options("File path", options(false, None))),
                true,
            )?)
        }
        "editor" => {
            let label = editor_label.ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "editor menu selection has no label",
                )
            })?;
            session.print_line(&format!("Opening {label}..."))?;
            session.suspend()?;
            let content = optional_editor(settings.env, settings.temp_root, stderr);
            session.resume()?;
            fields.content = content?;
            if let Some(content) = &fields.content {
                session.print_line(&format!(
                    "Content entered ({} characters)",
                    content.chars().count()
                ))?;
            }
        }
        _ => {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                "unrecognized content method",
            ));
        }
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
            ))
        }
        "issue" => {
            fields.issue = Some(answer!(
                session.text_with_options("Issue identifier (e.g., TC-123)", options(false, None))
            ))
        }
        "team" => {
            fields.team = Some(answer!(session.text_with_options(
                "Team key (e.g., ENG)",
                options(false, settings.default_team)
            )))
        }
        "initiative" => {
            fields.initiative = Some(answer!(
                session
                    .text_with_options("Initiative (UUID, slug ID, or name)", options(false, None))
            ))
        }
        "cycle" => {
            fields.team = Some(answer!(session.text_with_options(
                "Team key for the cycle (e.g., ENG)",
                options(false, settings.default_team)
            )));
            fields.cycle = Some(answer!(session.text_with_options(
                "Cycle (name, number, 'active', 'next', or 'previous')",
                options(false, None)
            )));
        }
        "release" => {
            fields.release = Some(answer!(
                session.text_with_options("Release (UUID, name, or version)", options(false, None))
            ))
        }
        _ => {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                "unrecognized attachment menu value",
            ));
        }
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
