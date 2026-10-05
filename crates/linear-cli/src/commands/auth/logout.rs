//! `auth logout`: forget a workspace's credential.
use crate::cli::auth::AuthLogout;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::platform::prompt::Choice;

pub fn run(ctx: &Ctx, args: &AuthLogout) -> Result<()> {
    logout(ctx, args).context("Failed to logout")
}

fn logout(ctx: &Ctx, args: &AuthLogout) -> Result<()> {
    let mut credentials = super::credentials(ctx)?;
    if credentials.workspaces().is_empty() {
        return Err(Error::auth("No workspaces configured"));
    }
    let named = super::named_workspace(ctx, args.workspace_name.as_deref())?;
    let workspace = match (named, credentials.workspaces()) {
        (Some(name), _) if !credentials.has_workspace(name) => {
            return Err(Error::not_found("Workspace", name));
        }
        (Some(name), _) => name.to_owned(),
        (None, [only]) => only.clone(),
        (None, workspaces) => {
            if !ctx.interactive() {
                return Err(Error::invalid("No workspace given")
                    .with_hint("Name it: `linear auth logout <workspace>`."));
            }
            pick(ctx, workspaces, credentials.default())?
        }
    };
    let question = format!("Remove credentials for workspace \"{workspace}\"?");
    if !args.confirm.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    {
        let _spinner = ctx.spinner(true, "");
        credentials.remove(&workspace, ctx.credentials()?.keyring())?;
    }
    let mut output = outcome::done("Removed", "credentials for workspace", &workspace, None);
    if let Some(default) = credentials.default() {
        output.push_str(&format!("  Default workspace is now: {default}\n"));
    }
    ctx.print(output)
}

fn pick(ctx: &Ctx, workspaces: &[String], default: Option<&str>) -> Result<String> {
    let choices = workspaces
        .iter()
        .map(|name| {
            let label = if default == Some(name.as_str()) {
                format!("{name} (default)")
            } else {
                name.clone()
            };
            Choice::new(label, name.clone())
        })
        .collect();
    ctx.prompter()?.select("Workspace to remove:", choices)
}
