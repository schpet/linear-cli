//! `auth logout`: forget a workspace's credential.
use crate::auth::keyring::native_backend;
use crate::cli::auth::AuthLogout;
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
    let workspace = match (&args.workspace_name, credentials.workspaces()) {
        (Some(name), _) if !credentials.has_workspace(name) => {
            return Err(Error::not_found("Workspace", name));
        }
        (Some(name), _) => name.clone(),
        (None, [only]) => only.clone(),
        (None, workspaces) => {
            if !ctx.stdin_tty() {
                return Err(Error::new("No workspace given")
                    .with_hint("Name it: `linear auth logout <workspace>`."));
            }
            pick(ctx, workspaces, credentials.default())?
        }
    };
    let question = format!("Remove credentials for workspace \"{workspace}\"?");
    if !args.force && !ctx.confirm(&question, "--force")? {
        return ctx.print("Logout canceled\n");
    }
    let backend = native_backend(&ctx.config().child_env);
    ctx.spin(true, credentials.remove(&workspace, &backend))?;
    let mut output = format!("Removed credentials for workspace: {workspace}\n");
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
    ctx.prompter()?
        .select("Select workspace to remove", choices)
}
