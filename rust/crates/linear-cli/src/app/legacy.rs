//! Helpers used only by command code that has not moved to `Ctx` yet. Delete
//! each one when its last caller is migrated.
use std::future::Future;
use std::time::Duration;

use crate::commands::{client, comment_add};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

/// Runs one future on a fresh runtime. Migrated commands use `Ctx::block_on`.
pub fn block_on_network<T, F>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::new("could not start network runtime").with_source(error))?;
    let result = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}

/// The old stdout spinner, switched off: migrated commands use `Ctx::spin`.
pub mod spinner {
    use std::time::Duration;

    pub const CLEAR: &[u8] = b"";
    pub const TICK_INTERVAL: Duration = Duration::from_millis(75);

    pub fn enabled(_json: bool, _stdout_tty: bool, _color: bool) -> bool {
        false
    }

    pub fn frame(_index: usize) -> String {
        String::new()
    }
}

/// Loaded configuration and credentials for auth commands not yet on `Ctx`.
pub struct Loaded<'a> {
    pub config: &'a crate::config::StartupConfig,
    pub credentials: &'a crate::auth::CredentialStore,
    pub credentials_path: Option<std::path::PathBuf>,
}

impl<'a> Loaded<'a> {
    pub fn new(ctx: &'a crate::ctx::Ctx) -> Result<Self> {
        Ok(Self {
            config: ctx.config(),
            credentials: ctx.credentials()?,
            credentials_path: ctx.credentials_path().map(std::path::Path::to_path_buf),
        })
    }
}

/// A fresh API client for the `--workspace`. Migrated commands use `Ctx::client`.
pub fn relation_transport(
    ctx: &crate::ctx::Ctx,
    workspace: Option<&str>,
) -> Result<crate::graphql::transport::GraphQlTransport> {
    crate::commands::client::prepare_transport(
        ctx.options(),
        ctx.credentials()?,
        workspace,
        &ctx.config().transport_env,
    )
}

/// `createComment` constructs its client before building the input.
pub fn submit_comment(
    context: &Ctx,
    workspace: Option<&str>,
    target: comment_add::CommentTarget,
    body: String,
    parent: Option<&str>,
) -> Result<crate::graphql::operations::comment_create::CreatedComment, Error> {
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    let input = comment_add::build_input(target, body, parent, None)?;
    block_on_network(comment_add::create(&transport, input))
}

pub fn finish_comment_add(context: &Ctx, result: Result<Vec<u8>>) -> Result<()> {
    context.print(result.context(comment_add::CONTEXT)?)
}

pub fn document_fetch_with_spinner<T>(
    context: &Ctx,
    json: bool,
    pending: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let enabled = spinner::enabled(json, context.stdout_tty(), true);
    if !enabled {
        return block_on_network(pending);
    }
    context.print(spinner::frame(0).as_bytes())?;
    let result = block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1;
        loop {
            tokio::select! {
                biased;
                result = &mut pending => break result,
                _ = ticks.tick() => {
                    context.print(spinner::frame(frame).as_bytes())?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.print(spinner::CLEAR)?;
    result
}

pub fn delete_confirmation(
    context: &Ctx,
    message: &str,
    flag: &str,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, Error> {
    if !context.stdin_tty() {
        return Err(Error::new("Interactive confirmation required")
            .with_hint(format!("Use --{flag} to skip.")));
    }
    let mut session = crate::platform::prompt::PromptSession::confirmation_stdio(context.stdout())?;
    let outcome = session.confirm(message, false);
    session.finish_result(outcome)
}
