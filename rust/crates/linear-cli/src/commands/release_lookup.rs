//! Complete release name/version lookup, insertion-ordered UUID deduplication.
use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::{envelope::GraphQlRequest, operations::releases::*};
use cynic::QueryBuilder;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
};
pub fn request(input: &str, after: Option<String>) -> GraphQlRequest<ResolveReleasesVariables> {
    GraphQlRequest::with_variables(ResolveReleases::build(ResolveReleasesVariables {
        input: input.to_owned(),
        after,
    }))
}
pub async fn resolve(client: &LinearClient, input: &str) -> Result<String, Error> {
    resolve_with(input, |query| async move {
        client.execute(&query).await.map_err(Error::from)
    })
    .await
}
pub async fn resolve_with<F, Fut>(input: &str, mut fetch: F) -> Result<String, Error>
where
    F: FnMut(GraphQlRequest<ResolveReleasesVariables>) -> Fut,
    Fut: Future<Output = Result<ResolveReleases, Error>>,
{
    crate::refs::reject_linear_url(input, "a release name, version, or UUID")?;
    if crate::refs::is_linear_uuid(input) {
        return Ok(input.to_owned());
    }
    let mut ordered: Vec<ReleaseNode> = Vec::new();
    let mut indexes = HashMap::new();
    let mut cursors = HashSet::new();
    let mut after = None;
    loop {
        let data = fetch(request(input, after.clone())).await?;
        for node in data.releases.nodes {
            if let Some(index) = indexes.get(node.id.inner()).copied() {
                *ordered
                    .get_mut(index)
                    .ok_or_else(|| Error::new("Release deduplication index missing"))? = node;
            } else {
                indexes.insert(node.id.inner().to_owned(), ordered.len());
                ordered.push(node);
            }
        }
        let info = data.releases.page_info;
        if !info.has_next_page {
            break;
        }
        let cursor = info.end_cursor.ok_or_else(|| {
            Error::new("Linear reported more releases but returned no pagination cursor")
        })?;
        if !cursors.insert(cursor.clone()) {
            return Err(Error::new("Linear repeated a release pagination cursor"));
        }
        after = Some(cursor);
    }
    if ordered.is_empty() {
        return Err(Error::not_found("Release", input)
            .with_hint("Pass a release UUID, exact release name, or exact version."));
    }
    if ordered.len() > 1 {
        let listing = ordered
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
    Ok(ordered.remove(0).id.into_inner())
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
        let result = resolve_with(input, |request| {
            cursors.push(request.variables.expect("variables").after);
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
