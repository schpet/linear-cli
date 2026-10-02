//! Complete release name/version lookup, insertion-ordered UUID deduplication.
use crate::error::Error;
use crate::graphql::{
    envelope::GraphQlRequest, operations::releases::*, transport::GraphQlTransport,
};
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
pub async fn resolve(transport: &GraphQlTransport, input: &str) -> Result<String, Error> {
    resolve_with(input, |query| async move {
        transport.execute(&query).await.map_err(Error::from)
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
