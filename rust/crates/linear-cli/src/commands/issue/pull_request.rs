//! `issue pull-request`: open a GitHub pull request for an issue with `gh`,
//! which owns the browser, editor and interactive flow.
use crate::{
    cli::issue::IssuePullRequest,
    commands::text_input,
    config::{ChildEnvOverlay, PrTemplateCli},
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    platform::gh_script::{GhRunner, NativeGhRunner},
};
use std::path::Path;

pub fn run(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    create_pull_request(ctx, args).context("Failed to create pull request")
}

fn create_pull_request(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    let selected = if args.no_template {
        PrTemplateCli::Disabled
    } else if let Some(path) = args.template.as_deref() {
        PrTemplateCli::Path(path)
    } else {
        PrTemplateCli::Unset
    };
    let template = ctx
        .options()
        .pr_template(selected)?
        .map(|path| read_template(path.path()))
        .transpose()?;
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let details = ctx.spin(true, super::describe::fetch(client, &identifier))?;
    let argv = self::args(
        &identifier,
        &details.title,
        &details.url,
        template.as_deref(),
        Options {
            title: args.title.as_deref(),
            base: args.base.as_deref(),
            head: args.head.as_deref(),
            draft: args.draft,
            web: args.web,
        },
    );
    ctx.flush()?;
    create(
        &mut NativeGhRunner,
        &argv,
        ctx.cwd(),
        &ctx.config().child_env,
    )
}
pub const TEMPLATE_SUGGESTION: &str = "Pass a readable file to --template, fix the pr_template config option, or use --no-template to skip the template.";
fn unusable(reason: impl Into<String>) -> Error {
    Error::new(format!(
        "Cannot read pull request template: {}",
        reason.into()
    ))
    .with_hint(TEMPLATE_SUGGESTION)
}
pub fn read_template(path: &Path) -> Result<String, Error> {
    let display = path.to_string_lossy();
    if display.trim().is_empty() {
        return Err(unusable("the path is empty"));
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            unusable(format!("\"{display}\" does not exist"))
        } else {
            unusable(format!("\"{display}\" could not be read: {error}")).with_source(error)
        }
    })?;
    if metadata.is_dir() {
        return Err(unusable(format!(
            "\"{display}\" is a directory, not a file"
        )));
    }
    if !metadata.is_file() {
        return Err(unusable(format!("\"{display}\" is not a regular file")));
    }
    let contents = text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            unusable(format!("\"{display}\" is not valid UTF-8 text")).with_source(error)
        } else {
            unusable(format!("\"{display}\" could not be read: {error}")).with_source(error)
        }
    })?;
    if contents.contains('\0') {
        return Err(unusable(format!("\"{display}\" is not a text file")));
    }
    Ok(contents)
}
pub fn body(template: Option<&str>, issue_url: &str) -> String {
    let template = template.unwrap_or("").trim_end();
    if template.is_empty() {
        issue_url.to_owned()
    } else {
        format!("{template}\n\n{issue_url}")
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    pub title: Option<&'a str>,
    pub base: Option<&'a str>,
    pub head: Option<&'a str>,
    pub draft: bool,
    pub web: bool,
}
pub fn args(
    identifier: &str,
    issue_title: &str,
    issue_url: &str,
    template: Option<&str>,
    options: Options<'_>,
) -> Vec<String> {
    let mut args = vec![
        "pr".to_owned(),
        "create".to_owned(),
        "--title".to_owned(),
        format!("{identifier} {}", options.title.unwrap_or(issue_title)),
        "--body".to_owned(),
        body(template, issue_url),
    ];
    for (flag, value) in [("--base", options.base), ("--head", options.head)] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            args.push(flag.to_owned());
            args.push(value.to_owned());
        }
    }
    if options.draft {
        args.push("--draft".to_owned());
    }
    if options.web {
        args.push("--web".to_owned());
    }
    args
}
pub fn create(
    runner: &mut impl GhRunner,
    args: &[String],
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<(), Error> {
    if runner.create(args, cwd, env)? {
        Ok(())
    } else {
        Err(Error::new("Failed to create pull request"))
    }
}

#[cfg(test)]
mod tests;
