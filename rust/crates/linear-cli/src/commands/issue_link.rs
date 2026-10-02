//! `issue link`: attach a URL to an issue (the issue may be inferred from the branch).
use crate::commands::issue_id;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_link::{AttachmentLinkURL, Variables};
use crate::graphql::transport::GraphQlTransport;
use cynic::MutationBuilder;

pub const CONTEXT: &str = "Failed to link URL";
const URL_SUGGESTION: &str = "Provide a URL starting with http:// or https://.";
fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

pub fn inputs<'a>(
    first: &'a str,
    second: Option<&'a str>,
) -> Result<(Option<&'a str>, &'a str), AppError> {
    let (issue, url) = match second {
        Some(url) => (Some(first), url),
        None if looks_like_url(first) => (None, first),
        None => {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("Expected a URL but got '{first}'"),
            )
            .with_suggestion(URL_SUGGESTION));
        }
    };
    if !looks_like_url(url) {
        return Err(
            AppError::new(AppErrorKind::Validation, format!("Invalid URL: '{url}'"))
                .with_suggestion(URL_SUGGESTION),
        );
    }
    Ok((issue, url))
}
pub fn request(id: &str, url: &str, title: Option<&str>) -> GraphQlRequest<Variables> {
    GraphQlRequest::with_variables(AttachmentLinkURL::build(Variables {
        issue_id: id.to_owned(),
        url: url.to_owned(),
        title: title.map(str::to_owned),
    }))
}
pub async fn submit(
    transport: &GraphQlTransport,
    identifier: &str,
    url: &str,
    title: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let id = issue_id::fetch(transport, identifier).await?;
    let result: AttachmentLinkURL = transport.execute(&request(&id, url, title)).await?;
    if !result.attachment_link_url.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Failed to link URL to issue",
        ));
    }
    Ok(format!(
        "✓ Linked to {identifier}: {}\n",
        result.attachment_link_url.attachment.title
    )
    .into_bytes())
}
