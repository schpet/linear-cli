//! Faithful mdast extraction/mutation; serializer selection remains a byte gate.
use crate::{error::Error, platform::markdown_assets::Asset};
use markdown::{ParseOptions, mdast::Node};
use std::collections::HashMap;
pub fn parse(content: &str) -> Result<Node, Error> {
    markdown::to_mdast(content, &ParseOptions::gfm())
        .map_err(|error| Error::new(format!("Could not parse document Markdown: {error}")))
}
fn host(url: &str) -> bool {
    reqwest::Url::parse(url).ok().is_some_and(|url| {
        matches!(
            url.host_str(),
            Some("uploads.linear.app" | "public.linear.app")
        )
    })
}
fn gather(node: &Node, images: &mut Vec<Asset>, links: &mut Vec<Asset>) {
    match node {
        Node::Image(image) if !image.url.is_empty() => images.push(Asset {
            url: image.url.clone(),
            alt: (!image.alt.is_empty()).then(|| image.alt.clone()),
        }),
        Node::Link(link) if host(&link.url) => links.push(Asset {
            url: link.url.clone(),
            alt: match link.children.first() {
                Some(Node::Text(text)) => Some(text.value.clone()),
                _ => None,
            },
        }),
        // Reference/definition nodes deliberately do not participate.
        _ => {}
    }
    if let Some(children) = node.children() {
        for child in children {
            gather(child, images, links);
        }
    }
}
pub fn extract(content: &str) -> Result<(Vec<Asset>, Vec<Asset>), Error> {
    let node = parse(content)?;
    let mut images = Vec::new();
    let mut links = Vec::new();
    gather(&node, &mut images, &mut links);
    Ok((images, links))
}
fn replace(node: &mut Node, paths: &HashMap<String, String>) {
    match node {
        Node::Image(image) => {
            if let Some(path) = paths.get(&image.url) {
                image.url = path.clone();
            }
        }
        Node::Link(link) => {
            if let Some(path) = paths.get(&link.url) {
                link.url = path.clone();
            }
        }
        _ => {}
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            replace(child, paths);
        }
    }
}
/// The serializer is passed in, keeping it independent of AST extraction and
/// URL replacement.
pub fn rewrite_with<F>(
    content: &str,
    paths: &HashMap<String, String>,
    serialize: F,
) -> Result<String, Error>
where
    F: FnOnce(&Node) -> Result<String, Error>,
{
    let mut node = parse(content)?;
    replace(&mut node, paths);
    serialize(&node)
}
