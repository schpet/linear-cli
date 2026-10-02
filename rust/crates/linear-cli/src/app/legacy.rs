//! Helpers used only by command code that has not moved to `Ctx` yet. Delete
//! each one when its last caller is migrated.
use std::future::Future;
use std::time::Duration;

use crate::error::{Error, Result};

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
