//! `auth token`: print the API key the other commands would use.
use crate::auth::{self, CredentialSelection};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx) -> Result<()> {
    token(ctx).context("Failed to get API token")
}

fn token(ctx: &Ctx) -> Result<()> {
    let inputs = ctx.selection();
    let selection = auth::resolve(&inputs, ctx.credentials()?);
    ctx.report_credential_warnings()?;
    match selection {
        CredentialSelection::Selected { secret, .. } => ctx.print(format!("{}\n", secret.expose())),
        // Building the client reports why no key was selected.
        CredentialSelection::NoKey
        | CredentialSelection::EnvWorkspaceConflict
        | CredentialSelection::MissingExplicitWorkspace { .. } => match ctx.client() {
            Err(error) => Err(error),
            Ok(_) => unreachable!("the client cannot be built without a selected key"),
        },
    }
}
