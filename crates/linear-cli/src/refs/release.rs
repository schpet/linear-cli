//! Releases, referenced by UUID, exact name or exact version.
use std::collections::HashSet;

use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::release::{ResolveReleases, ResolveReleasesVariables};
use crate::graphql::pagination::{self, Page};

/// The ID of the release `input` names. A release that matches by both name
/// and version, or shows up on two pages, counts once.
pub async fn resolve(client: &LinearClient, input: &str) -> Result<String> {
    super::reject_linear_url(input, "a release name, version, or UUID")?;
    if super::is_linear_uuid(input) {
        return Ok(input.to_owned());
    }
    let nodes = pagination::collect(None, |after, first| async move {
        let data: ResolveReleases = client
            .query(ResolveReleasesVariables {
                input: input.to_owned(),
                first,
                after,
            })
            .await?;
        Ok(Page {
            nodes: data.releases.nodes,
            page_info: data.releases.page_info,
        })
    })
    .await?;
    let mut seen = HashSet::new();
    let mut matches: Vec<_> = nodes
        .into_iter()
        .filter(|node| seen.insert(node.id.inner().to_owned()))
        .collect();
    if matches.len() > 1 {
        return Err(super::ambiguous(
            "Release",
            input,
            matches.iter().map(|node| match &node.version {
                Some(version) => format!("{} ({version}) — {}", node.name, node.id.inner()),
                None => format!("{} — {}", node.name, node.id.inner()),
            }),
        )
        .with_hint("Pass the release UUID instead."));
    }
    matches
        .pop()
        .map(|node| node.id.into_inner())
        .ok_or_else(|| {
            Error::not_found("Release", input)
                .with_hint("Pass a release UUID, exact release name, or exact version.")
        })
}
