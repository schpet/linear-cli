//! `linear document`.
pub mod comment_list;
pub mod delete;
pub mod list;
pub mod target;
pub mod view;
pub mod write;

use crate::app::legacy::{
    block_on_network, delete_confirmation, document_fetch_with_spinner, finish_comment_add,
    relation_transport, spinner, submit_comment,
};
use crate::cli;
use crate::cli::document::DocumentCommand;
use crate::commands::client;
use crate::commands::comment_add;
use crate::commands::document::comment_list as document_comment_list;
use crate::commands::document::list as document_list;
use crate::commands::document::target as document_target;
use crate::commands::document::view as document_view;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{WorkspaceScope, resolve_document_reference};

pub fn run(ctx: &Ctx, command: &DocumentCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        DocumentCommand::List(action) => dispatch_document_list(context, action, workspace),
        DocumentCommand::View(action) => dispatch_document_view(context, action, workspace),
        DocumentCommand::Create(action) => dispatch_document_create(context, action, workspace),
        DocumentCommand::Update(action) => dispatch_document_update(context, action, workspace),
        DocumentCommand::Delete(action) => dispatch_document_delete(context, action, workspace),
        DocumentCommand::Comment(action) => match &action.command {
            cli::document::DocumentCommentCommand::Add(action) => {
                dispatch_document_comment_add(context, action, workspace)
            }
            cli::document::DocumentCommentCommand::List(action) => {
                dispatch_document_comment_list(context, action, workspace)
            }
        },
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

/// Order: local document URL reduction, body flags, then the content
/// record lookup for every reference (UUIDs included), the omitted-body
/// prompt, and a second client before parent validation.
fn dispatch_document_comment_add(
    context: &Ctx,
    action: &cli::document::DocumentCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let result = (|| {
        let document = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            resolve_document_reference(
                &action.document,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?
        };
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let document_content_id = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            block_on_network(comment_add::document_content_id(&transport, &document))?
        };
        let body = match body {
            Some(body) => body,
            None => crate::commands::comment_add::prompt(context)?,
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Document {
                document_content_id,
            },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| comment_add::output("document", &document, &comment))
    })();
    finish_comment_add(context, result)
}

fn dispatch_document_comment_list(
    context: &Ctx,
    action: &cli::document::DocumentCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = &action.document;
    let config = context.config();
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace);
    let reference = resolve_document_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(document_comment_list::CONTEXT)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(document_comment_list::CONTEXT)?;
    let color = context.color();
    let output = block_on_network(async {
        document_comment_list::run(&transport, &reference, &reference, json, color).await
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_document_list(
    context: &Ctx,
    action: &cli::document::DocumentList,
    workspace: Option<&str>,
) -> Result<()> {
    let result: Result<()> = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let team = configured_team_key(&config.options);
        let target = document_target::prepare(
            action,
            &WorkspaceScope::from_selection(&inputs, credentials),
            team.as_deref(),
        )?;
        let first = i32::try_from(action.limit.get()).map_err(|error| {
            Error::new("Document limit exceeds GraphQL's signed integer range").with_source(error)
        })?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let documents = document_fetch_with_spinner(context, action.json, async {
            let filter = match target {
                Some(target) => {
                    let (kind, id) = document_target::resolve(&target, &transport).await?;
                    Some(document_target::filter(kind, id))
                }
                None => None,
            };
            document_list::fetch(&transport, filter, first).await
        })?;
        let output = if action.json {
            document_list::json(&documents)?
        } else {
            let columns = crate::platform::pager::stdout_size()
                .map(|size| usize::from(size.columns))
                .unwrap_or(120);
            document_list::text(
                &documents,
                columns,
                context.stdout_tty() && context.color(),
                std::time::SystemTime::now(),
            )
            .into_bytes()
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(document_list::CONTEXT)
}

fn dispatch_document_view(
    context: &Ctx,
    action: &cli::document::DocumentView,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::{markdown_assets, markdown_ast, markdown_serializer, markdown_terminal};
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let id = resolve_document_reference(
            &action.id,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let cache_root = config.image_cache_root.clone();
        let download = !action.no_download
            && config
                .options
                .download_images()
                .is_none_or(|value| *value.value());
        let hyperlink = config
            .options
            .hyperlink_format()
            .map(|value| value.value().clone());
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let document = document_fetch_with_spinner(
            context,
            action.raw || action.json,
            document_view::fetch(&transport, &action.id, &id, action.json),
        )?;
        if action.web {
            context.print(format!("Opening {} in web browser\n", document.url()).as_bytes())?;
            crate::platform::opener::open(document.url(), false)?;
            return Ok(());
        }
        if action.json {
            context.print(&document.json()?)?;
            return Ok(());
        }
        let document = match document {
            document_view::DocumentResult::Body(document) => document,
            document_view::DocumentResult::WithComments(_) => {
                return Err(Error::new(
                    "Non-JSON document unexpectedly included comments",
                ));
            }
        };
        let mut content = document.content.clone();
        if download && let Some(original) = content.as_deref().filter(|value| !value.is_empty()) {
            let paths = block_on_network(markdown_assets::download_with(
                original,
                &cache_root,
                |url| {
                    let transport = &transport;
                    async move { transport.download_markdown_image(&url).await }
                },
                |bytes| context.eprint(bytes),
            ))?
            .paths;
            if !paths.is_empty() {
                content = Some(markdown_ast::rewrite_with(
                    original,
                    &paths,
                    markdown_serializer::serialize,
                )?);
            }
        }
        let output = if action.raw || !context.stdout_tty() {
            document_view::raw(content.as_deref())
        } else {
            let markdown = document_view::markdown(
                &document,
                content.as_deref(),
                chrono::Utc::now(),
                &chrono::Local,
            );
            let columns = crate::platform::pager::stdout_size()
                .and_then(|size| std::num::NonZeroU16::new(size.columns))
                .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
            let options = markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                hyperlink.as_deref(),
                markdown_terminal::HostSource::System,
            );
            format!("{}\n", markdown_terminal::render(&markdown, &options)?).into_bytes()
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(document_view::CONTEXT)
}

fn dispatch_document_delete(
    context: &Ctx,
    action: &cli::document::DocumentDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{commands::document::delete as command, platform::prompt::PromptOutcome};
    let result = (|| {
        let transport = relation_transport(context, workspace)?;
        let input = crate::commands::bulk::BulkInput {
            argv: action.bulk.as_deref(),
            file: action.bulk_file.as_deref().map(std::path::Path::new),
            stdin: action.bulk_stdin,
        };
        if input.requested() {
            let ids = crate::commands::bulk::collect_ids(&input, &mut std::io::stdin().lock())?;
            if ids.is_empty() {
                return Err(Error::new("No document IDs provided for bulk delete"));
            }
            context.print(format!("Found {} document(s) to delete.\n", ids.len()).as_bytes())?;
            if !action.yes {
                match delete_confirmation(
                    context,
                    &format!("Delete {} document(s)?", ids.len()),
                    "yes",
                )? {
                    PromptOutcome::Submitted(true) => {}
                    PromptOutcome::Submitted(false) => {
                        context.print(b"Bulk delete cancelled.\n")?;
                        return Ok(());
                    }
                    PromptOutcome::Interrupted => return Err(Error::cancelled()),
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            let targets = {
                let inputs = client::selection_inputs(&context.config().options, workspace);
                let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
                ids.into_iter()
                    .map(|id| command::Target::prepare(id, &scope))
                    .collect()
            };
            let show = spinner::enabled(false, context.stdout_tty(), true);
            let results = block_on_network(command::execute(&transport, targets, |progress| {
                if show {
                    context.print(progress.render())?
                }
                Ok(())
            }));
            if show {
                context.print(crate::commands::bulk::PROGRESS_CLEAR)?
            }
            let (output, failed) = command::summary(&results?);
            context.print(&output)?;
            return if failed {
                Err(Error::reported())
            } else {
                Ok(())
            };
        }
        let original = action
            .document_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                Error::new("Document ID required").with_hint("Use --bulk for multiple documents.")
            })?;
        let id = {
            let inputs = client::selection_inputs(&context.config().options, workspace);
            resolve_document_reference(
                original,
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )?
        };
        let document = block_on_network(command::single_details(&transport, original, &id))?;
        if !action.yes {
            match delete_confirmation(
                context,
                &format!("Are you sure you want to delete \"{}\"?", document.title),
                "yes",
            )? {
                PromptOutcome::Submitted(true) => {}
                PromptOutcome::Submitted(false) => {
                    context.print(b"Delete cancelled.\n")?;
                    return Ok(());
                }
                PromptOutcome::Interrupted => return Err(Error::cancelled()),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new(
                        "unexpected EOF while prompting for confirmation",
                    ));
                }
            }
        }
        let output = block_on_network(command::submit_single(&transport, &document))?;
        context.print(&output)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}

fn document_write_target(
    context: &Ctx,
    target: crate::commands::document::target::TargetOptions<'_>,
    workspace: Option<&str>,
) -> Result<
    (
        crate::graphql::transport::GraphQlTransport,
        Option<crate::commands::document::target::PreparedTarget>,
    ),
    Error,
> {
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let team = configured_team_key(&config.options);
    let prepared = crate::commands::document::target::prepare_options(
        target,
        &WorkspaceScope::from_selection(&inputs, credentials),
        team.as_deref(),
    )?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    Ok((transport, prepared))
}

fn document_prompt_exit(
    outcome: crate::platform::prompt::PromptOutcome<crate::commands::document::write::Fields>,
) -> Result<crate::commands::document::write::Fields> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(fields) => Ok(fields),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("unexpected EOF while prompting for document")),
    }
}

fn dispatch_document_create(
    context: &Ctx,
    action: &cli::document::DocumentCreate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{
        document::target::TargetOptions, document::write as command, text_input,
    };
    let result = (|| {
        let target = TargetOptions {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        };
        let interactive = context.stdout_tty()
            && (action.interactive
                || (action.title.is_none()
                    && action.content.is_none()
                    && action.content_file.is_none()
                    && action.icon.is_none()
                    && !target.any()));
        let root = std::env::temp_dir();
        let fields = if interactive {
            if target.any() {
                return Err(Error::new("Attachment target flags cannot be combined with interactive mode").with_hint("Drop the target flags to choose the attachment interactively, or drop -i/--interactive to use the flags."));
            }
            let config = context.config();
            let env = config.child_env.clone();
            let default_team = configured_team_key(&config.options);
            crate::platform::prompt_text::TextOptions {
                required: false,
                default: default_team.as_deref(),
            }
            .preflight()
            .map_err(Error::new)?;
            let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
            let prompted = command::prompt(
                &mut session,
                &mut std::io::stderr(),
                command::PromptSettings {
                    env: &env,
                    temp_root: &root,
                    default_team: default_team.as_deref(),
                },
            );
            let outcome = session.finish_result(prompted)?;
            document_prompt_exit(outcome)?
        } else {
            let title = action.title.clone().ok_or_else(|| {
                Error::new("Title is required")
                    .with_hint("Use --title or run with -i for interactive mode.")
            })?;
            target.cardinality(true)?;
            let content = if let Some(content) = &action.content {
                Some(content.clone())
            } else if let Some(path) = &action.content_file {
                Some(command::file(path, false)?)
            } else if !context.stdin_tty() {
                text_input::read_stdin(std::io::stdin().lock())?
            } else if context.stdout_tty() {
                context.print(b"Opening editor for document content...\n")?;
                let env = context.config().child_env.clone();
                let content = command::optional_editor(&env, &mut std::io::stderr())?;
                if content.is_none() {
                    context.print(b"No content entered. Creating document without content.\n")?;
                }
                content
            } else {
                None
            };
            command::Fields {
                title: Some(title),
                content,
                icon: action.icon.clone(),
                project: action.project.clone(),
                issue: action.issue.clone(),
                initiative: action.initiative.clone(),
                team: action.team.clone(),
                cycle: action.cycle.clone(),
                release: action.release.clone(),
            }
        };
        let (transport, target) = document_write_target(context, fields.target(), workspace)?;
        let mut input = command::input(None, fields.icon);
        input.content = fields.content;
        let output = block_on_network(async {
            if let Some(target) = target {
                let (kind, id) =
                    crate::commands::document::target::resolve(&target, &transport).await?;
                command::attach(&mut input, kind, id);
            }
            let title = fields
                .title
                .filter(|title| !title.is_empty())
                .ok_or_else(|| Error::new("Title is required"))?;
            command::create(&transport, title, input).await
        })?;
        context.print(&output)?;
        Ok(())
    })();
    result.context("Failed to create document")
}

fn dispatch_document_update(
    context: &Ctx,
    action: &cli::document::DocumentUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{
        document::target::TargetOptions, document::write as command, text_input,
    };
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let id = resolve_document_reference(
            &action.document_id,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let target = TargetOptions {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        };
        target.cardinality(false)?;
        let (transport, target) = document_write_target(context, target, workspace)?;
        let mut input = command::input(action.title.clone(), action.icon.clone());
        if let Some(target) = target {
            let (kind, id) = block_on_network(crate::commands::document::target::resolve(
                &target, &transport,
            ))?;
            command::attach(&mut input, kind, id);
        }
        input.content = if let Some(content) = &action.content {
            Some(content.clone())
        } else if let Some(path) = &action.content_file {
            Some(command::file(path, false)?)
        } else if action.edit {
            let document = block_on_network(command::for_edit(&transport, &id))?;
            let seed = document.content.unwrap_or_default();
            context.print(format!("Opening {} in editor...\n", document.title).as_bytes())?;
            let edited = context.edit_text(&seed)?;
            if edited == seed {
                context.print(b"No changes detected, update cancelled.\n")?;
                return Ok(());
            }
            let Some(content) = text_input::edited_body(&edited) else {
                context.print(b"No changes made, update cancelled.\n")?;
                return Ok(());
            };
            Some(content)
        } else if !context.stdin_tty() && !command::has_fields(&input) {
            text_input::read_stdin(std::io::stdin().lock())?
        } else {
            None
        };
        if !command::has_fields(&input) {
            return Err(Error::new("No update fields provided").with_hint("Use --title, --content, --content-file, --icon, --edit, or re-point the attachment with --project, --issue, --initiative, --team, --cycle, or --release."));
        }
        let output = block_on_network(async {
            if input.content.is_some() && !action.force {
                command::guard(&transport, &id).await?;
            }
            command::update(&transport, &id, input).await
        })?;
        context.print(&output)?;
        Ok(())
    })();
    result.context("Failed to update document")
}
