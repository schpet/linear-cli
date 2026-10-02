//! `auth migrate`: move plaintext keys from the credentials file to the
//! system keyring.
use crate::auth::CredentialFormat;
use crate::auth::keyring::native_backend;
use crate::auth::mutation::KeyringBackend;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx) -> Result<()> {
    migrate(ctx).context("Failed to migrate credentials")
}

fn migrate(ctx: &Ctx) -> Result<()> {
    let mut credentials = super::credentials(ctx)?;
    if credentials.format() != CredentialFormat::Inline {
        return ctx.print("Credentials are already using the system keyring.\n");
    }
    let backend = native_backend(&ctx.config().child_env);
    let migrated = ctx.spin(true, async {
        if !backend.available().await {
            return Err(no_keyring());
        }
        credentials.migrate(&backend).await
    })?;
    let mut output = format!(
        "Migrated {} workspace(s) to system keyring:\n",
        migrated.len()
    );
    for name in migrated {
        output.push_str(&format!("  {name}\n"));
    }
    ctx.print(output)
}

pub(super) fn no_keyring() -> Error {
    Error::new("No system keyring found").with_hint(
        "Install libsecret (e.g. `apt install libsecret-tools` or `pacman -S libsecret`), or set LINEAR_API_KEY instead.",
    )
}
