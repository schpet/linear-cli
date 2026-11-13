//! `auth migrate`: move plaintext keys from the credentials file to the
//! system keyring.
use crate::auth::CredentialFormat;
use crate::auth::keyring::Keyring;
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
    let keyring = ctx.credentials()?.keyring();
    if !keyring.available() {
        return Err(no_keyring(keyring));
    }
    let migrated = {
        let _spinner = ctx.spinner(true, "");
        credentials.migrate(keyring)?
    };
    let mut output = format!(
        "Migrated {} workspace(s) to system keyring:\n",
        migrated.len()
    );
    for name in migrated {
        output.push_str(&format!("  {name}\n"));
    }
    ctx.print(output)
}

fn no_keyring(keyring: &dyn Keyring) -> Error {
    let hint = match keyring.install_hint() {
        Some(install) => format!("{install} Or set LINEAR_API_KEY instead."),
        None => "Set LINEAR_API_KEY instead.".to_owned(),
    };
    Error::new("No system keyring found").with_hint(hint)
}
