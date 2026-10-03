//! `issue link`: attach a URL to an issue (the issue may be inferred from the branch).
use crate::cli::issue::IssueLink;
use crate::client::LinearClient;
use crate::commands::issue::id;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::issue::{AttachmentLinkURL, LinkVariables, LinkedAttachment};

pub fn run(ctx: &Ctx, args: &IssueLink) -> Result<()> {
    link(ctx, args).context("Failed to link URL")
}

fn link(ctx: &Ctx, args: &IssueLink) -> Result<()> {
    let (issue, url) = inputs(&args.url_or_issue_id, args.url.as_deref())?;
    let identifier = super::require(ctx, issue)?;
    let client = ctx.client()?;
    let attachment = ctx.block_on(submit(client, &identifier, url, args.title.as_deref()))?;
    ctx.print(outcome::done(
        "Linked",
        "issue",
        &format!("{identifier} to {}", attachment.title),
        Some(&attachment.url),
    ))
}

const URL_SUGGESTION: &str = "Provide a URL starting with http:// or https://.";
fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

pub fn inputs<'a>(
    first: &'a str,
    second: Option<&'a str>,
) -> Result<(Option<&'a str>, &'a str), Error> {
    let (issue, url) = match second {
        Some(url) => (Some(first), url),
        None if looks_like_url(first) => (None, first),
        None => {
            return Err(
                Error::new(format!("Expected a URL but got '{first}'")).with_hint(URL_SUGGESTION)
            );
        }
    };
    if !looks_like_url(url) {
        return Err(Error::new(format!("Invalid URL: '{url}'")).with_hint(URL_SUGGESTION));
    }
    Ok((issue, url))
}
async fn submit(
    client: &LinearClient,
    identifier: &str,
    url: &str,
    title: Option<&str>,
) -> Result<LinkedAttachment, Error> {
    let issue_id = id::fetch(client, identifier).await?;
    let result: AttachmentLinkURL = client
        .mutate(LinkVariables {
            issue_id,
            url: url.to_owned(),
            title: title.map(str::to_owned),
        })
        .await?;
    if !result.attachment_link_url.success {
        return Err(Error::new("Linear did not link the URL to the issue"));
    }
    Ok(result.attachment_link_url.attachment)
}
