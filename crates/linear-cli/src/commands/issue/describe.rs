//! `issue describe` and `issue jj start` share the commit-description template.
use crate::{
    cli::issue::IssueDescribe,
    commands::issue::details,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    platform::terminal_text::{multiline, single_line},
    templates::{CommitContext, IssueData, Slot, TemplateConfig},
};

pub fn run(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    describe(ctx, args).context("Failed to get issue description")
}

fn describe(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    let identifier = super::require(ctx, &args.issue_id)?;
    let client = ctx.client()?;
    let detail = ctx.spin(true, details::fetch(client, identifier.clone()))?;
    let mut issue = IssueData {
        identifier,
        title: detail.title,
        url: detail.url,
        description: detail.description.unwrap_or_default(),
    };
    if ctx.stdout_tty() {
        issue.title = single_line(&issue.title).into_owned();
        issue.url = single_line(&issue.url).into_owned();
        issue.description = multiline(&issue.description).into_owned();
    }
    ctx.print(render(ctx.templates(), issue, args.references)?)
}

/// Render completely before printing or preparing a jj change. Commands keep
/// their own output boundary rules: describe preserves the final newline while
/// jj normalizes trailing newlines before passing the text to `jj describe`.
pub fn render(templates: &TemplateConfig, issue: IssueData, references: bool) -> Result<String> {
    let context = CommitContext {
        issue,
        linkage: if references { "References" } else { "Fixes" }.to_owned(),
        references,
    };
    let text = templates.render(Slot::CommitDescription, &context)?;
    if text.trim().is_empty() {
        return Err(Error::new(
            "templates.commit_description produced an empty commit description",
        )
        .with_hint("Configure templates.commit_description to render a nonblank description."));
    }
    Ok(text)
}
