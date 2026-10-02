//! `linear auth`.
pub mod default;
pub mod list;
pub mod login;
pub mod logout;
pub mod migrate;
pub mod token;
pub mod whoami;

use crate::app::legacy::block_on_network;
use crate::cli;
use crate::cli::auth::AuthCommand;
use crate::commands::auth::default as auth_default;
use crate::commands::auth::list as auth_list;
use crate::commands::auth::token as auth_token;
use crate::commands::auth::whoami as auth_whoami;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, command: &AuthCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        AuthCommand::Login(action) => dispatch_auth_login(context, action),
        AuthCommand::Logout(action) => dispatch_auth_logout(context, action),
        AuthCommand::List(action) => dispatch_auth_list(context, action, workspace),
        AuthCommand::Default(action) => dispatch_auth_default(context, action),
        AuthCommand::Token(_) => dispatch_auth_token(context, workspace),
        AuthCommand::Whoami(action) => dispatch_auth_whoami(context, action, workspace),
        AuthCommand::Migrate(_) => dispatch_auth_migrate(context),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

fn dispatch_auth_token(context: &Ctx, workspace: Option<&str>) -> Result<()> {
    let output = auth_token::run(&context.config().options, context.credentials()?, workspace)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_auth_default(context: &Ctx, action: &cli::auth::AuthDefault) -> Result<()> {
    use crate::auth::write::RealCredentialFileWriter;
    use crate::commands::auth::default::DefaultAction;
    use crate::platform::prompt::{PlainSelect, PromptOutcome, PromptSession};
    use std::io::IsTerminal;

    let result = (|| {
        let prepared =
            auth_default::prepare(context.credentials()?, action.workspace_name.as_deref())?;
        let selected = match prepared {
            DefaultAction::Select(options) => {
                // stdin_stdio otherwise falls back to the line-script protocol.
                if !std::io::stdin().is_terminal() {
                    return Err(auth_default::non_tty_error());
                }
                let outcome = {
                    let mut session = PromptSession::stdin_stdio(context.stdout())?;
                    let prompted = session.select(&PlainSelect {
                        message: "Select default workspace",
                        options: &options,
                        default_index: 0,
                        default_hint: None,
                    });
                    session.finish_result(prompted)?
                };
                match outcome {
                    PromptOutcome::Submitted(workspace) => {
                        auth_default::prepare(context.credentials()?, Some(&workspace))?
                    }
                    PromptOutcome::Interrupted => {
                        return Err(Error::cancelled());
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while selecting a default workspace",
                        ));
                    }
                }
            }
            other => other,
        };
        let output = match selected {
            DefaultAction::Output(bytes) => bytes,
            DefaultAction::Save(workspace) => {
                let startup = crate::app::legacy::Loaded::new(context)?;
                auth_default::save(
                    startup.credentials,
                    &workspace,
                    startup.credentials_path.as_deref(),
                    &RealCredentialFileWriter,
                )?
            }
            DefaultAction::Select(_) => {
                return Err(Error::new(
                    "submitted workspace did not resolve to a default action",
                ));
            }
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(auth_default::CONTEXT)
}

fn dispatch_auth_list(
    context: &Ctx,
    _action: &cli::auth::AuthList,
    _workspace: Option<&str>,
) -> Result<()> {
    let config = context.config();
    let rows = auth_list::classify(context.credentials()?);
    context.report_credential_warnings()?;
    let output = if rows.is_empty() {
        auth_list::EMPTY_OUTPUT.as_bytes().to_vec()
    } else {
        let prepared = auth_list::prepare_transports(
            rows,
            config.options.endpoint().value(),
            &config.transport_env,
        )
        .context(auth_list::CONTEXT)?;
        let listed = block_on_network(auth_list::fetch(prepared)).context(auth_list::CONTEXT)?;
        auth_list::render(&listed, context.stdout_tty() && context.color())
    };
    context.print(&output)?;
    Ok(())
}

fn dispatch_auth_whoami(
    context: &Ctx,
    _action: &cli::auth::AuthWhoami,
    workspace: Option<&str>,
) -> Result<()> {
    let config = context.config();
    let credentials = context.credentials()?;

    let transport = auth_whoami::prepare_transport(
        &config.options,
        credentials,
        workspace,
        &config.transport_env,
    )?;
    let output = block_on_network(async move { auth_whoami::run(&transport).await })?;
    context.print(&output)?;
    Ok(())
}

fn auth_prompt_eof(phase: &str) -> Error {
    Error::new(format!("unexpected EOF while prompting for {phase}"))
}

fn auth_prompt_output<R: std::io::Read, W: std::io::Write>(
    session: &mut Option<crate::platform::prompt::PromptSession<R, W>>,
    output: &mut Option<W>,
    bytes: &[u8],
) -> Result<(), Error> {
    match session {
        Some(current) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|error| Error::new("auth output must be UTF-8").with_source(error))?;
            for line in text.split_terminator('\n') {
                current.print_line(line)?;
            }
            Ok(())
        }
        None => {
            let writer = output
                .as_mut()
                .ok_or_else(|| Error::new("auth output missing"))?;
            writer
                .write_all(bytes)
                .and_then(|()| writer.flush())
                .map_err(crate::platform::output::write_error)
        }
    }
}

fn dispatch_auth_login(context: &Ctx, action: &cli::auth::AuthLogin) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth::login as command,
        platform::prompt::{PromptOutcome, PromptSession},
    };
    let no_color = !context.color();
    let loaded = crate::app::legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let config = &loaded.config;
    let path = loaded.credentials_path.as_deref();
    let backend = NativeMutationBackend {
        overlay: config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let mut session = None;
    let mut output = Some(context.stdout());
    let result = (|| {
        let key = match command::supplied_key(action.key.as_deref()) {
            Some(key) => key,
            None => {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("auth output already owned"))?,
                )?);
                let current = session
                    .as_mut()
                    .ok_or_else(|| Error::new("secret session absent"))?;
                let key = match current.secret(command::SECRET_MESSAGE, command::SECRET_HINT)? {
                    PromptOutcome::Submitted(key) => key,
                    PromptOutcome::Interrupted => return Err(Error::cancelled()),
                    PromptOutcome::EndOfInput => return Err(auth_prompt_eof("API key")),
                };
                current.suspend()?;
                key
            }
        };
        let key = command::clean_key(key)?;
        let transport = command::prepare_transport(&config.options, &config.transport_env, &key)?;
        let viewer = block_on_network(async {
            command::authenticate(&transport)
                .await
                .map_err(|failure| failure.login())
        })?;
        let bytes = block_on_network(async {
            command::add_authenticated(
                &mut state,
                viewer,
                key,
                command::LoginSaveOptions {
                    plaintext: action.plaintext,
                    no_color,
                },
                path,
                &backend,
                &writer,
            )
            .await
            .map_err(|failure| failure.login())
        })?;
        auth_prompt_output(&mut session, &mut output, &bytes)?;
        let offer =
            block_on_network(async { Ok(command::offer_migration(&state, &backend).await) })?;
        if offer {
            auth_prompt_output(
                &mut session,
                &mut output,
                &command::migration_notice(no_color),
            )?;
            match session.as_mut() {
                Some(current) => current.resume()?,
                None => {
                    session = Some(PromptSession::stdin_stdio_cr_or_lf(
                        output
                            .take()
                            .ok_or_else(|| Error::new("migration output absent"))?,
                    )?);
                }
            }
            let current = session
                .as_mut()
                .ok_or_else(|| Error::new("migration session absent"))?;
            let migrate = match current.confirm(command::MIGRATE_MESSAGE, true)? {
                PromptOutcome::Submitted(answer) => answer,
                PromptOutcome::Interrupted => return Err(Error::cancelled()),
                PromptOutcome::EndOfInput => return Err(auth_prompt_eof("credential migration")),
            };
            current.suspend()?;
            if migrate {
                let bytes = block_on_network(async {
                    command::migrate(&mut state, path, &backend, &writer)
                        .await
                        .map_err(|failure| failure.login())
                })?;
                auth_prompt_output(&mut session, &mut output, &bytes)?;
            }
        }
        auth_prompt_output(
            &mut session,
            &mut output,
            &command::environment_warning(&config.options, no_color)?,
        )?;
        Ok(())
    })();
    let cleanup = session.as_mut().map_or(Ok(()), PromptSession::close);
    cleanup.and(result).context(command::CONTEXT)
}

fn dispatch_auth_logout(context: &Ctx, action: &cli::auth::AuthLogout) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth::logout as command,
        platform::prompt::{PlainSelect, PromptOutcome, PromptSession},
    };
    let loaded = crate::app::legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let backend = NativeMutationBackend {
        overlay: loaded.config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let path = loaded.credentials_path.as_deref();
    let mut session = None;
    let mut output = Some(context.stdout());
    let result = (|| {
        let target = command::prepare(&state, action.workspace_name.as_deref())?;
        let name = match target {
            command::LogoutTarget::Selected(name) => name,
            command::LogoutTarget::Select(options) => {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("logout output absent"))?,
                )?);
                let current = session
                    .as_mut()
                    .ok_or_else(|| Error::new("logout session absent"))?;
                match current.select(&PlainSelect {
                    message: command::SELECT_MESSAGE,
                    options: &options,
                    default_index: 0,
                    default_hint: None,
                })? {
                    PromptOutcome::Submitted(name) => name,
                    PromptOutcome::Interrupted => return Err(Error::cancelled()),
                    PromptOutcome::EndOfInput => return Err(auth_prompt_eof("workspace")),
                }
            }
        };
        if !action.force {
            if session.is_none() {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("logout confirmation output absent"))?,
                )?)
            }
            let current = session
                .as_mut()
                .ok_or_else(|| Error::new("logout confirmation absent"))?;
            match current.confirm(&command::confirm_message(&name), false)? {
                PromptOutcome::Submitted(true) => {}
                PromptOutcome::Submitted(false) => {
                    auth_prompt_output(&mut session, &mut output, b"Cancelled\n")?;
                    return Ok(());
                }
                PromptOutcome::Interrupted => return Err(Error::cancelled()),
                PromptOutcome::EndOfInput => return Err(auth_prompt_eof("logout confirmation")),
            }
        }
        if let Some(current) = session.as_mut() {
            current.suspend()?
        }
        let bytes = block_on_network(async {
            command::remove(&mut state, &name, path, &backend, &writer)
                .await
                .map_err(|failure| failure.outer())
        })?;
        auth_prompt_output(&mut session, &mut output, &bytes)?;
        Ok(())
    })();
    let cleanup = session.as_mut().map_or(Ok(()), PromptSession::close);
    cleanup.and(result).context(command::CONTEXT)
}

fn dispatch_auth_migrate(context: &Ctx) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth::migrate as command,
    };
    let loaded = crate::app::legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let backend = NativeMutationBackend {
        overlay: loaded.config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let bytes = block_on_network(async {
        command::run(
            &mut state,
            loaded.credentials_path.as_deref(),
            &backend,
            &writer,
        )
        .await
        .map_err(|failure| failure.outer())
    })
    .context(command::CONTEXT)?;
    context.print(&bytes)?;
    Ok(())
}
