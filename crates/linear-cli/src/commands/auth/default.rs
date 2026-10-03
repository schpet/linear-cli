//! `auth default`: choose the workspace used when none is named.
use crate::cli::auth::AuthDefault;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::platform::prompt::Choice;

pub fn run(ctx: &Ctx, args: &AuthDefault) -> Result<()> {
    set_default(ctx, args).context("Failed to set default workspace")
}

fn set_default(ctx: &Ctx, args: &AuthDefault) -> Result<()> {
    let mut credentials = super::credentials(ctx)?;
    let workspaces = credentials.workspaces();
    match workspaces {
        [] => {
            return Err(Error::auth("No workspaces configured")
                .with_hint("Run `linear auth login` to add a workspace."));
        }
        [only] => return ctx.print(format!("Only one workspace configured: {only}\n")),
        _ => {}
    }
    let target = match super::named_workspace(ctx, args.workspace_name.as_deref())? {
        Some(target) => {
            if !credentials.has_workspace(target) {
                return Err(Error::not_found("Workspace", target)
                    .with_hint(format!("Available workspaces: {}", workspaces.join(", "))));
            }
            target.to_owned()
        }
        None => pick(ctx, workspaces, credentials.default())?,
    };
    if credentials.default() == Some(target.as_str()) {
        return ctx.print(format!("\"{target}\" is already the default workspace\n"));
    }
    credentials.set_default(&target)?;
    ctx.print(format!("Default workspace set to: {target}\n"))
}

fn pick(ctx: &Ctx, workspaces: &[String], current: Option<&str>) -> Result<String> {
    if !ctx.interactive() {
        return Err(Error::new("No workspace given")
            .with_hint("Name it: `linear auth default <workspace>`."));
    }
    let choices = workspaces
        .iter()
        .map(|name| {
            let label = if current == Some(name.as_str()) {
                format!("{name} (current)")
            } else {
                name.clone()
            };
            Choice::new(label, name.clone())
        })
        .collect();
    ctx.prompter()?.select("Select default workspace", choices)
}
