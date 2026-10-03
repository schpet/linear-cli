//! Resolves a release by UUID, exact name or exact version. A release that
//! matches by both name and version, or shows up on two pages, counts once.
use std::collections::HashSet;
use std::future::Future;

use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::operations::release::{ReleaseNode, ResolveReleases, ResolveReleasesVariables};
use crate::graphql::pagination::{self, Page};

pub async fn resolve(client: &LinearClient, input: &str) -> Result<String, Error> {
    resolve_with(input, |variables| async move {
        Ok(client.query(variables).await?)
    })
    .await
}

pub async fn resolve_with<F, Fut>(input: &str, mut fetch: F) -> Result<String, Error>
where
    F: FnMut(ResolveReleasesVariables) -> Fut,
    Fut: Future<Output = Result<ResolveReleases, Error>>,
{
    crate::refs::reject_linear_url(input, "a release name, version, or UUID")?;
    if crate::refs::is_linear_uuid(input) {
        return Ok(input.to_owned());
    }
    let nodes = pagination::collect(None, |after, first| {
        let response = fetch(ResolveReleasesVariables {
            input: input.to_owned(),
            first,
            after,
        });
        async move {
            let releases = response.await?.releases;
            Ok(Page {
                nodes: releases.nodes,
                page_info: releases.page_info,
            })
        }
    })
    .await?;
    let mut seen = HashSet::new();
    let mut matches: Vec<ReleaseNode> = nodes
        .into_iter()
        .filter(|node| seen.insert(node.id.inner().to_owned()))
        .collect();
    if matches.len() > 1 {
        let listing = matches
            .iter()
            .map(|node| {
                format!(
                    "  {}{} — {}",
                    node.name,
                    node.version
                        .as_ref()
                        .map_or(String::new(), |version| format!(" ({version})")),
                    node.id.inner()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Error::new(format!(
            "Release \"{input}\" is ambiguous; it matches multiple releases:\n{listing}"
        ))
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use serde_json::json;

    use super::{ResolveReleases, resolve_with};
    use crate::error::Error;
    use crate::graphql::envelope::parse_response;

    const SUMMER: &str = "00000000-0000-4000-9000-000000000050";

    fn page(id: &str, name: &str, next: Option<&str>) -> ResolveReleases {
        let body = json!({"data": {"releases": {
            "nodes": [{"id": id, "name": name, "version": "2026.8"}],
            "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next},
        }}});
        parse_response(body.to_string().as_bytes()).expect("release page")
    }

    async fn resolve(
        input: &str,
        pages: Vec<ResolveReleases>,
    ) -> (Result<String, Error>, Vec<Option<String>>) {
        let mut pages = VecDeque::from(pages);
        let mut cursors = Vec::new();
        let result = resolve_with(input, |variables| {
            assert_eq!(variables.first, 100);
            cursors.push(variables.after);
            let page = pages.pop_front().expect("another page");
            async move { Ok(page) }
        })
        .await;
        (result, cursors)
    }

    #[tokio::test]
    async fn the_same_release_on_two_pages_is_one_match() {
        let (result, cursors) = resolve(
            "2026.8",
            vec![
                page(SUMMER, "Summer", Some("release-next")),
                page(SUMMER, "Summer", None),
            ],
        )
        .await;
        assert_eq!(result.expect("one release"), SUMMER);
        assert_eq!(cursors, [None, Some("release-next".to_owned())]);
    }

    #[tokio::test]
    async fn distinct_matches_are_ambiguous() {
        let (result, _) = resolve(
            "2026.8",
            vec![
                page(SUMMER, "Summer", Some("release-next")),
                page("release-two", "Other", None),
            ],
        )
        .await;
        let error = result.expect_err("ambiguous");
        assert!(
            error.message().contains("matches multiple releases"),
            "{error}"
        );
        assert_eq!(error.hint(), Some("Pass the release UUID instead."));
    }

    #[tokio::test]
    async fn uuids_and_linear_urls_need_no_lookup() {
        let (result, cursors) = resolve(SUMMER, vec![]).await;
        assert_eq!(result.expect("uuid"), SUMMER);
        assert!(cursors.is_empty());
        let (result, cursors) =
            resolve("https://linear.app/acme/project/title-a1b2c3d4e5f6", vec![]).await;
        result.expect_err("not a release URL");
        assert!(cursors.is_empty());
    }
}
