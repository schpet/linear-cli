//! `linear initiative`.
pub mod bulk;
pub mod comment_list;
pub mod create;
pub mod list;
pub mod projects;
pub mod unarchive;
pub mod update;
pub mod view;

use crate::app::legacy::{
    block_on_network, document_fetch_with_spinner, finish_comment_add, relation_transport, spinner,
    submit_comment,
};
use crate::cli;
use crate::cli::initiative::InitiativeCommand;
use crate::commands::client;
use crate::commands::comment_add;
use crate::commands::initiative::bulk as initiative_bulk;
use crate::commands::initiative::comment_list as initiative_comment_list;
use crate::commands::initiative::create as initiative_create;
use crate::commands::initiative::list as initiative_list;
use crate::commands::initiative::projects as initiative_projects;
use crate::commands::initiative::unarchive as initiative_unarchive;
use crate::commands::initiative::view as initiative_view;
use crate::commands::table;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{
    InitiativeReference, WorkspaceScope, prepare_initiative_lookup,
    resolve_initiative_with_transport,
};

pub fn run(ctx: &Ctx, command: &InitiativeCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        InitiativeCommand::List(action) => dispatch_initiative_list(context, action, workspace),
        InitiativeCommand::View(action) => dispatch_initiative_view(context, action, workspace),
        InitiativeCommand::Create(action) => dispatch_initiative_create(context, action, workspace),
        InitiativeCommand::Archive(action) => dispatch_initiative_bulk(
            context,
            InitiativeAction {
                target: action.initiative_id.as_deref(),
                force: action.force,
                bulk: crate::commands::bulk::BulkInput {
                    argv: action.bulk.as_deref(),
                    file: action.bulk_file.as_deref().map(std::path::Path::new),
                    stdin: action.bulk_stdin,
                },
            },
            initiative_bulk::Mode::Archive,
            workspace,
        ),
        InitiativeCommand::Update(action) => dispatch_initiative_update(context, action, workspace),
        InitiativeCommand::Unarchive(action) => {
            dispatch_initiative_unarchive(context, action, workspace)
        }
        InitiativeCommand::Delete(action) => dispatch_initiative_bulk(
            context,
            InitiativeAction {
                target: action.initiative_id.as_deref(),
                force: action.force,
                bulk: crate::commands::bulk::BulkInput {
                    argv: action.bulk.as_deref(),
                    file: action.bulk_file.as_deref().map(std::path::Path::new),
                    stdin: action.bulk_stdin,
                },
            },
            initiative_bulk::Mode::Delete,
            workspace,
        ),
        InitiativeCommand::AddProject(action) => dispatch_initiative_projects(
            context,
            &action.initiative,
            &action.project,
            action.sort_order,
            true,
            initiative_projects::Mode::Add,
            workspace,
        ),
        InitiativeCommand::RemoveProject(action) => dispatch_initiative_projects(
            context,
            &action.initiative,
            &action.project,
            None,
            action.force,
            initiative_projects::Mode::Remove,
            workspace,
        ),
        InitiativeCommand::Comment(action) => match &action.command {
            cli::initiative::InitiativeCommentCommand::Add(action) => {
                dispatch_initiative_comment_add(context, action, workspace)
            }
            cli::initiative::InitiativeCommentCommand::List(action) => {
                dispatch_initiative_comment_list(context, action, workspace)
            }
        },
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

fn dispatch_initiative_create(
    context: &Ctx,
    action: &cli::initiative::InitiativeCreate,
    workspace: Option<&str>,
) -> Result<()> {
    let mut options = initiative_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        status: action.status.clone(),
        owner: action.owner.clone(),
        target_date: action.target_date.clone(),
        color: action.color.clone(),
        icon: action.icon.clone(),
        interactive: action.interactive,
    };
    let config = context.config();
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_create::CREATE_CONTEXT)?;
    if initiative_create::should_prompt(&options, context.stdout_tty()) {
        context.print(b"\nCreate a new initiative\n\n")?;
        let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
        let prompted = initiative_create::prompt(&mut options, &mut session);
        let result = match prompted {
            Ok(outcome) => {
                session.close()?;
                outcome
            }
            Err(error) => {
                return Err(match session.close() {
                    Ok(()) => error,
                    Err(mut cleanup) => {
                        cleanup.push_message(&format!("; prompt also failed: {}", error));
                        cleanup
                    }
                });
            }
        };
        match result {
            initiative_create::PromptResult::Complete => {}
            initiative_create::PromptResult::Interrupted => {
                return Err(Error::cancelled());
            }
            initiative_create::PromptResult::EndOfInput => {
                return Err(Error::new("unexpected EOF while prompting for initiative"));
            }
        }
    }
    let status = initiative_create::validate(&options)?;
    let owner_id = block_on_network(initiative_create::resolve_owner(
        &transport,
        options.owner.as_deref(),
    ))
    .context(initiative_create::CREATE_CONTEXT)?;
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_create::submit_create(
        &transport, options, status, owner_id,
    ));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let result = result.context(initiative_create::CREATE_CONTEXT)?;
    context.print(&result)?;
    Ok(())
}

fn dispatch_initiative_projects(
    context: &Ctx,
    initiative_arg: &str,
    project_arg: &str,
    sort_order: Option<f64>,
    force: bool,
    mode: initiative_projects::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    let scope = WorkspaceScope::from_selection(&inputs, credentials);
    let initiative = block_on_network(initiative_projects::resolve_initiative(
        &transport,
        initiative_arg,
        &scope,
        mode,
    ))?;
    let project = block_on_network(initiative_projects::resolve_project(
        &transport,
        project_arg,
        &scope,
        mode,
    ))?;
    let link = match mode {
        initiative_projects::Mode::Add => None,
        initiative_projects::Mode::Remove => {
            let link = block_on_network(initiative_projects::find_link(
                &transport,
                &initiative,
                &project,
            ))?;
            if link.is_none() {
                context.print(
                    format!(
                        "Project \"{}\" is not linked to initiative \"{}\"\n",
                        project.name, initiative.name
                    )
                    .as_bytes(),
                )?;
                return Ok(());
            }
            if !force {
                if !context.stdin_tty() {
                    return Err(Error::new(
                        "Interactive confirmation required. Use --force to skip.",
                    ));
                }
                let outcome = {
                    let mut session = PromptSession::confirmation_stdio(context.stdout())?;
                    let result = session.confirm(
                        &format!(
                            "Remove \"{}\" from initiative \"{}\"?",
                            project.name, initiative.name
                        ),
                        true,
                    );
                    session.finish_result(result)?
                };
                match outcome {
                    PromptOutcome::Submitted(true) => {}
                    PromptOutcome::Submitted(false) => {
                        context.print(b"Removal cancelled.\n")?;
                        return Ok(());
                    }
                    PromptOutcome::Interrupted => {
                        return Err(Error::cancelled());
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            link
        }
    };
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = match mode {
        initiative_projects::Mode::Add => block_on_network(initiative_projects::add(
            &transport,
            &initiative,
            &project,
            sort_order,
        )),
        initiative_projects::Mode::Remove => {
            let link_id = link.ok_or_else(|| Error::new("confirmed removal requires a link ID"))?;
            block_on_network(initiative_projects::remove(
                &transport,
                &link_id,
                &initiative,
                &project,
            ))
        }
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    context.print(&result?)?;
    Ok(())
}

fn dispatch_initiative_unarchive(
    context: &Ctx,
    action: &cli::initiative::InitiativeUnarchive,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let original = &action.initiative_id;
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let reference = initiative_view::prepare_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let id = block_on_network(initiative_unarchive::resolve_reference(
        &transport, &reference, original,
    ))?;
    let detail = block_on_network(initiative_unarchive::fetch_details(
        &transport, &id, original,
    ))?;
    if let Some(output) = initiative_unarchive::active_output(&detail) {
        context.print(&output)?;
        return Ok(());
    }
    if !action.force {
        if !context.stdin_tty() {
            return Err(Error::new(
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let outcome = {
            let mut session = PromptSession::confirmation_stdio(context.stdout())?;
            let result = session.confirm(
                &format!("Are you sure you want to unarchive \"{}\"?", detail.name),
                true,
            );
            session.finish_result(result)?
        };
        match outcome {
            PromptOutcome::Submitted(true) => {}
            PromptOutcome::Submitted(false) => {
                context.print(b"Unarchive cancelled.\n")?;
                return Ok(());
            }
            PromptOutcome::Interrupted => {
                return Err(Error::cancelled());
            }
            PromptOutcome::EndOfInput => {
                return Err(Error::new(
                    "unexpected EOF while prompting for confirmation",
                ));
            }
        }
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_unarchive::submit(&transport, &id));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_view(
    context: &Ctx,
    action: &cli::initiative::InitiativeView,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.initiative_id;
    let app = action.app;
    let web = action.web;
    let json = action.json;
    let config = context.config();
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let scope = WorkspaceScope::from_selection(&inputs, credentials);
    let reference = initiative_view::prepare_reference(original, &scope)
        .context(initiative_view::RESOLVE_CONTEXT)?;
    let id = block_on_network(initiative_view::resolve_reference(
        &transport, &reference, original,
    ))?;
    let show_spinner = !(app || web) && spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_view::fetch_details(&transport, id, original));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let detail = result?;
    if app || web {
        if detail.url.is_empty() {
            return Err(
                Error::not_found("Initiative", original).context(initiative_view::FETCH_CONTEXT)
            );
        }
        context.print(initiative_view::opening(&detail, app))?;
        crate::platform::opener::open(&detail.url, app).context(initiative_view::OPEN_CONTEXT)?;
        return Ok(());
    }
    let output = if json {
        initiative_view::render_json(&detail)
    } else {
        let columns = std::num::NonZeroU16::new(
            u16::try_from(table::stdout_columns(context.stdout_tty())).unwrap_or(80),
        )
        .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
        initiative_view::render_text(&detail, context.stdout_tty(), columns, context.color())
    }
    .context(initiative_view::FETCH_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_list(
    context: &Ctx,
    action: &cli::initiative::InitiativeList,
    workspace: Option<&str>,
) -> Result<()> {
    let options = initiative_list::Options {
        status: action.status.clone(),
        all_statuses: action.all_statuses,
        owner: action.owner.clone(),
        web: action.web,
        app: action.app,
        json: action.json,
        archived: action.archived,
    };
    let cli_workspace = workspace;
    if options.web || options.app {
        let config = context.config();
        let workspace = match config
            .options
            .workspace()
            .map(|value| value.value().clone())
            .filter(|value| !value.is_empty())
        {
            Some(workspace) => workspace,
            None => {
                let credentials = context
                    .credentials()
                    .context(initiative_list::OPEN_CONTEXT)?;
                let inputs = client::selection_inputs(&config.options, cli_workspace);
                let transport = client::prepare_transport_with_inputs(
                    &config.options,
                    credentials,
                    &inputs,
                    &config.transport_env,
                )
                .context(initiative_list::OPEN_CONTEXT)?;
                block_on_network(initiative_list::viewer_workspace(&transport))
                    .context(initiative_list::OPEN_CONTEXT)?
            }
        };
        let (url, opening) = initiative_list::opening(&workspace, options.app);
        context.print(&opening)?;
        initiative_list::open(&url, options.app)?;
        return Ok(());
    }

    let show_spinner = spinner::enabled(options.json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let status =
            initiative_list::status_filter(options.status.as_deref(), options.all_statuses)?;
        initiative_list::validate_owner(options.owner.as_deref())?;
        let config = context.config();
        let credentials = context.credentials()?;
        let transport = client::prepare_transport(
            &config.options,
            credentials,
            cli_workspace,
            &config.transport_env,
        )?;
        block_on_network(initiative_list::run(
            &transport,
            status.as_deref(),
            options.owner.as_deref(),
            options.archived,
            options.json,
            table::stdout_columns(context.stdout_tty()),
            context.stdout_tty() && context.color(),
        ))
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(initiative_list::FETCH_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

/// Order: body flags, initiative reference (a UUID needs no client),
/// the omitted-body prompt, then client construction before parent validation.
fn dispatch_initiative_comment_add(
    context: &Ctx,
    action: &cli::initiative::InitiativeCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.initiative;
    let result = (|| {
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let initiative_id = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let reference = prepare_initiative_lookup(
                original,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?;
            match &reference {
                InitiativeReference::Id(id) => id.clone(),
                InitiativeReference::NameOrSlug(_) | InitiativeReference::UrlSlug(_) => {
                    let transport = client::prepare_transport_with_inputs(
                        &config.options,
                        credentials,
                        &inputs,
                        &config.transport_env,
                    )?;
                    block_on_network(resolve_initiative_with_transport(
                        &reference, original, &transport,
                    ))?
                }
            }
        };
        let body = match body {
            Some(body) => body,
            None => crate::commands::comment_add::prompt(context)?,
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Initiative { initiative_id },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| comment_add::output("initiative", original, &comment))
    })();
    finish_comment_add(context, result)
}

fn dispatch_initiative_comment_list(
    context: &Ctx,
    action: &cli::initiative::InitiativeCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = &action.initiative;
    let config = context.config();
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace);
    let reference = prepare_initiative_lookup(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(initiative_comment_list::CONTEXT)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_comment_list::CONTEXT)?;
    let color = context.color();
    let output = block_on_network(async {
        let id = resolve_initiative_with_transport(&reference, original, &transport)
            .await
            .context(initiative_comment_list::CONTEXT)?;
        initiative_comment_list::run(&transport, original, &id, json, color).await
    })?;
    context.print(&output)?;
    Ok(())
}

struct InitiativeAction<'a> {
    target: Option<&'a str>,
    force: bool,
    bulk: crate::commands::bulk::BulkInput<'a>,
}

fn initiative_prompt_confirm(
    context: &Ctx,
    message: &str,
    default: bool,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, Error> {
    use crate::platform::prompt::PromptSession;
    if !context.stdin_tty() {
        return Err(Error::new(
            "Interactive confirmation required. Use --force to skip.",
        ));
    }
    let mut session = PromptSession::confirmation_stdio(context.stdout())?;
    let result = session.confirm(message, default);
    session.finish_result(result)
}

fn initiative_prompt_stop<T>(
    outcome: crate::platform::prompt::PromptOutcome<T>,
) -> Result<T, Error> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new(
            "unexpected EOF while prompting for confirmation",
        )),
    }
}

fn dispatch_initiative_bulk(
    context: &Ctx,
    action: InitiativeAction<'_>,
    mode: initiative_bulk::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    // The client is built before reading or validating the collected IDs.
    let transport = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?
    };
    if action.bulk.requested() {
        let ids = crate::commands::bulk::collect_ids(&action.bulk, &mut std::io::stdin().lock())?;
        if ids.is_empty() {
            return Err(Error::new(format!(
                "No initiative IDs provided for bulk {}.",
                mode.verb()
            )));
        }
        context
            .print(format!("Found {} initiative(s) to {}.\n", ids.len(), mode.verb()).as_bytes())?;
        if mode == initiative_bulk::Mode::Delete {
            context.print("\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes())?;
        }
        if !action.force {
            let message = match mode {
                initiative_bulk::Mode::Archive => format!("Archive {} initiative(s)?", ids.len()),
                initiative_bulk::Mode::Delete => {
                    format!("Permanently delete {} initiative(s)?", ids.len())
                }
            };
            let outcome = initiative_prompt_confirm(context, &message, false)?;
            if matches!(outcome, PromptOutcome::Interrupted) {
                return Err(Error::cancelled());
            }
            if !initiative_prompt_stop(outcome)? {
                context.print(mode.bulk_cancelled())?;
                return Ok(());
            }
        }
        let targets = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            ids.into_iter()
                .map(|id| initiative_bulk::Target::prepare(id, &scope))
                .collect()
        };
        let progress_enabled = spinner::enabled(false, context.stdout_tty(), true);
        let results = block_on_network(initiative_bulk::execute(
            &transport,
            targets,
            mode,
            |progress| {
                if progress_enabled {
                    context.print(progress.render())?;
                }
                Ok(())
            },
        ))?;
        if progress_enabled {
            context.print(crate::commands::bulk::PROGRESS_CLEAR)?;
        }
        let (output, failed) = initiative_bulk::summary(&results, mode);
        context.print(&output)?;
        return if failed {
            Err(Error::reported())
        } else {
            Ok(())
        };
    }
    let original = action
        .target
        .filter(|target| !target.is_empty())
        .ok_or_else(|| {
            Error::new("Initiative ID required. Use --bulk for multiple initiatives.")
        })?;
    let target = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        initiative_bulk::Target::prepare(
            original.to_owned(),
            &WorkspaceScope::from_selection(&inputs, credentials),
        )
    };
    let id = block_on_network(initiative_bulk::resolve(
        &transport,
        &target.reference?,
        mode,
    ))?
    .ok_or_else(|| Error::not_found("Initiative", original))?;
    let detail = block_on_network(initiative_bulk::fetch_single(&transport, &id, mode))?
        .ok_or_else(|| Error::not_found("Initiative", original))?;
    if detail.already_archived() {
        context
            .print(format!("Initiative \"{}\" is already archived.\n", detail.name()).as_bytes())?;
        return Ok(());
    }
    if let Some(warning) = detail.linked_warning() {
        context.print(&warning)?;
    }
    if !action.force {
        // The terminal check comes before the permanent-deletion warning.
        if !context.stdin_tty() {
            return Err(Error::new(
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let (message, default) = match mode {
            initiative_bulk::Mode::Archive => {
                (format!("Archive initiative \"{}\"?", detail.name()), true)
            }
            initiative_bulk::Mode::Delete => {
                context
                    .print("\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes())?;
                (
                    format!(
                        "Are you sure you want to permanently delete \"{}\"?",
                        detail.name()
                    ),
                    false,
                )
            }
        };
        let outcome = initiative_prompt_confirm(context, &message, default)?;
        if matches!(outcome, PromptOutcome::Interrupted) {
            return Err(Error::cancelled());
        }
        if !initiative_prompt_stop(outcome)? {
            context.print(mode.single_cancelled())?;
            return Ok(());
        }
        if mode == initiative_bulk::Mode::Delete {
            // Keep the raw answer; the prompt owns terminal handling and rendering.
            let raw = std::cell::RefCell::new(String::new());
            let outcome = {
                let mut session = PromptSession::confirmation_stdio(context.stdout())?;
                let result = session.text(
                    "Type the initiative name to confirm deletion:",
                    0,
                    |answer| {
                        *raw.borrow_mut() = answer.to_owned();
                        Ok(())
                    },
                );
                session.finish_result(result)?
            };
            if matches!(outcome, PromptOutcome::Interrupted) {
                return Err(Error::cancelled());
            }
            initiative_prompt_stop(outcome)?;
            if raw.into_inner().trim() != detail.name() {
                context.print(b"Name does not match. Delete cancelled.\n")?;
                return Ok(());
            }
        }
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_bulk::submit_single(
        &transport,
        &id,
        detail.name(),
        mode,
    ));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    context.print(&result?)?;
    Ok(())
}

fn dispatch_initiative_update(
    context: &Ctx,
    action: &cli::initiative::InitiativeUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::initiative::update as command;
    use crate::platform::prompt::{PromptOutcome, PromptSession, escaped_display};
    let transport = relation_transport(context, workspace)?;
    let reference = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        initiative_view::prepare_reference(&action.initiative_id, &scope)?
    };
    let id = block_on_network(command::resolve(
        &transport,
        &reference,
        &action.initiative_id,
    ))?;
    let current = block_on_network(command::details(&transport, &id, &action.initiative_id))?;
    let mut fields = command::Fields {
        name: action.name.clone(),
        description: action.description.clone(),
        status: action.status.clone(),
        owner: action.owner.clone(),
        target_date: action.target_date.clone(),
        color: action.color.clone(),
        icon: action.icon.clone(),
    };
    if fields.should_prompt(action.interactive, context.stdout_tty()) {
        context.print(
            format!(
                "\nUpdating initiative: {}\n\n",
                escaped_display(&current.name)
            )
            .as_bytes(),
        )?;
        let mut session = PromptSession::stdio_cr_or_lf(context.stdout())?;
        let prompted = command::prompt(&mut session, &current);
        fields = match session.finish_result(prompted)? {
            PromptOutcome::Submitted(fields) => fields,
            PromptOutcome::Interrupted => return Err(Error::cancelled()),
            PromptOutcome::EndOfInput => {
                return Err(Error::new("unexpected EOF while updating initiative"));
            }
        };
    }
    let owner_id = block_on_network(command::owner(&transport, fields.owner.as_deref()))?;
    if fields.empty() {
        context.print(b"No changes specified\n")?;
        return Ok(());
    }
    let output = document_fetch_with_spinner(
        context,
        false,
        command::submit(&transport, &id, fields.input(owner_id)),
    )?;
    context.print(&output)?;
    Ok(())
}
