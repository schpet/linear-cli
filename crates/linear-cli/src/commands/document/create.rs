//! `document create`: fields from flags, stdin, an editor or prompts, then
//! one mutation.
use std::path::Path;

use crate::cli::document::DocumentCreate;
use crate::commands::outcome;
use crate::commands::team_key::configured_team_key;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::document::*;
use crate::platform::editor;
use crate::platform::prompt::{Choice, Prompter, Text};

use super::common::{attach, read_file};
use super::target::{self, Kind, TargetOptions};

pub fn run(ctx: &Ctx, args: &DocumentCreate) -> Result<()> {
    create(ctx, args).context("Failed to create document")
}

fn create(ctx: &Ctx, args: &DocumentCreate) -> Result<()> {
    let optional = ctx.optional_prompts(args.interactive)?;
    let mut fields = Fields {
        title: args.title.clone(),
        content: match (&args.content, &args.content_file) {
            (Some(content), _) => Some(content.clone()),
            (None, Some(path)) => Some(read_file(path)?),
            (None, None) => None,
        },
        icon: args.icon.clone(),
        project: args.project.clone(),
        issue: args.issue.clone(),
        initiative: args.initiative.clone(),
        team: args.team.clone(),
        cycle: args.cycle.clone(),
        release: args.release.clone(),
    };
    let given_content = fields.content.is_some();
    if ctx.interactive() {
        let default_team = configured_team_key(ctx.options());
        prompt(
            ctx,
            &ctx.prompter()?,
            &mut fields,
            optional,
            default_team.as_deref(),
        )?;
    }
    let title = fields
        .title
        .clone()
        .ok_or_else(|| ctx.missing_value("Title is required", "--title"))?;
    if !fields.target().any() {
        return Err(
            Error::invalid("A document attachment target is required").with_hint(
                "Pass one of --project, --issue, --initiative, --team, --cycle, or --release.",
            ),
        );
    }
    if !given_content && !optional {
        fields.content = if !ctx.stdin_tty() {
            text_input::read_stdin(std::io::stdin().lock())?
        } else if ctx.interactive() {
            ctx.print("Opening editor for document content...\n")?;
            let content = optional_editor(ctx)?;
            if content.is_none() {
                ctx.print("No content entered. Creating document without content.\n")?;
            }
            content
        } else {
            None
        };
    }
    let target = target::prepare(ctx, fields.target())?;
    let client = ctx.client()?;
    let mut input = DocumentUpdateInput {
        content: fields.content,
        icon: fields.icon,
        ..Default::default()
    };
    let created = ctx.spin(true, async {
        attach(client, &mut input, target.as_ref()).await?;
        let data: CreateDocument = client
            .mutate(CreateDocumentVariables {
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
            })
            .await
            .map_err(|failure| failure.into_create_error("document"))?;
        if !data.document_create.success {
            return Err(Error::new("Linear did not create the document"));
        }
        Ok(data.document_create.document)
    })?;
    ctx.print(outcome::done(
        "Created",
        "document",
        &created.title,
        Some(&created.url),
    ))
}

/// Opens an empty editor; `None` when nothing was written.
fn optional_editor(ctx: &Ctx) -> Result<Option<String>> {
    Ok(text_input::edited_body(&ctx.edit_text("")?))
}

/// A new document's fields, from flags or prompts.
struct Fields {
    title: Option<String>,
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

/// Asks for the title and attachment when they are missing, and with
/// `optional` for the content and icon the flags left out. The content can
/// come from the configured editor.
fn prompt(
    ctx: &Ctx,
    prompter: &Prompter<'_>,
    fields: &mut Fields,
    optional: bool,
    default_team: Option<&str>,
) -> Result<()> {
    if fields.title.is_none() {
        fields.title = Some(prompter.text(Text::new("Document title").required())?);
    }
    if optional {
        if fields.content.is_none() {
            let editor = editor::configured_name(&ctx.config().child_env);
            fields.content = prompt_content(ctx, prompter, editor.as_deref())?;
        }
        if fields.icon.is_none() {
            fields.icon = text_input::edited_body(
                &prompter.text(Text::new("Icon (emoji, leave blank for none)"))?,
            );
        }
    }
    if fields.target().any() {
        return Ok(());
    }
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
    Ok(())
}

/// Asks where the content comes from, then for the content. The editor
/// named `editor` is offered when one is configured.
fn prompt_content(
    ctx: &Ctx,
    prompter: &Prompter<'_>,
    editor: Option<&str>,
) -> Result<Option<String>> {
    let mut methods = vec![
        Choice::new("Skip (no content)", Content::Skip),
        Choice::new("Enter inline", Content::Inline),
    ];
    if let Some(label) = editor {
        methods.push(Choice::new(format!("Open {label}"), Content::Editor));
    }
    methods.push(Choice::new("Read from file", Content::File));
    Ok(
        match prompter.select("How would you like to enter content?", methods)? {
            Content::Skip => None,
            Content::Inline => {
                text_input::edited_body(&prompter.text(Text::new("Content (markdown)"))?)
            }
            Content::File => {
                let path = prompter.text(Text::new("File path").required())?;
                Some(read_file(Path::new(&path))?)
            }
            Content::Editor => {
                let label = editor.expect("the editor option is offered only with an editor");
                ctx.print(format!("Opening {label}...\n"))?;
                let content = optional_editor(ctx)?;
                if let Some(content) = &content {
                    ctx.print(format!(
                        "Content entered ({} characters)\n",
                        content.chars().count()
                    ))?;
                }
                content
            }
        },
    )
}

/// Where prompted content comes from.
enum Content {
    Skip,
    Inline,
    Editor,
    File,
}
