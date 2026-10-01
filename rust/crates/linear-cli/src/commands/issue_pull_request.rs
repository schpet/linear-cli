//! Complete PR template/body/argv policy; gh owns its browser/editor/interactive flow.
use crate::{
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
    platform::gh_script::GhRunner,
    text::{js_space, js_trim},
};
use std::path::Path;
pub const CONTEXT: &str = "Failed to create pull request";
pub const TEMPLATE_SUGGESTION: &str = "Pass a readable file to --template, fix the pr_template config option, or use --no-template to skip the template.";
fn unusable(reason: impl Into<String>) -> AppError {
    AppError::new(
        AppErrorKind::Validation,
        format!("Cannot read pull request template: {}", reason.into()),
    )
    .with_suggestion(TEMPLATE_SUGGESTION)
}
pub fn read_template(path: &Path) -> Result<String, AppError> {
    let display = path.to_string_lossy();
    if js_trim(&display).is_empty() {
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
    let bytes = std::fs::read(path).map_err(|error| {
        unusable(format!("\"{display}\" could not be read: {error}")).with_source(error)
    })?;
    let contents = String::from_utf8_lossy(&bytes).into_owned();
    if contents.contains('\0') {
        return Err(unusable(format!("\"{display}\" is not a text file")));
    }
    Ok(contents)
}
pub fn body(template: Option<&str>, issue_url: &str) -> String {
    let template = template.unwrap_or("").trim_end_matches(js_space);
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
) -> Result<(), AppError> {
    if runner.create(args, cwd, env)? {
        Ok(())
    } else {
        Err(AppError::new(
            AppErrorKind::IoProcess,
            "Failed to create pull request",
        ))
    }
}
