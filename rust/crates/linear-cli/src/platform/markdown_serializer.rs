//! Source-qualified wrapper for the maintained minimal mdast serializer fork.
//! Original baseline and exact25-row helper qualification are retained.
use crate::error::{AppError, AppErrorKind};
use markdown::mdast::Node;

pub fn serialize(node: &Node) -> Result<String, AppError> {
    mdast_util_to_markdown::to_markdown_with_options(
        node,
        &mdast_util_to_markdown::Options {
            bullet: '-',
            ..mdast_util_to_markdown::Options::default()
        },
    )
    .map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            format!("Could not rewrite document Markdown: {error}"),
        )
    })
}
