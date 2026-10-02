//! Markdown serialization through the vendored mdast serializer.
use crate::error::Error;
use markdown::mdast::Node;

pub fn serialize(node: &Node) -> Result<String, Error> {
    mdast_util_to_markdown::to_markdown_with_options(
        node,
        &mdast_util_to_markdown::Options {
            bullet: '-',
            ..mdast_util_to_markdown::Options::default()
        },
    )
    .map_err(|error| Error::new(format!("Could not rewrite document Markdown: {error}")))
}
