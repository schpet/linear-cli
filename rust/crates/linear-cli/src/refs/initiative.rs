//! Strict initiative references: URL slug, UUID, plain slug, then exact name.
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind, is_linear_uuid};
use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::operations::initiative_reference::{
    NameVariables, ResolveInitiativeByName, ResolveInitiativeBySlug, UrlSlugVariables,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitiativeReference {
    Id(String),
    NameOrSlug(String),
    UrlSlug(String),
}
pub fn prepare_initiative_lookup(
    input: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<InitiativeReference, Error> {
    match expect_url_kind(
        input,
        LinearUrlKind::Initiative,
        "an initiative URL, UUID, slug ID, or exact name",
        scope,
    )? {
        Some(LinearUrlRef::Initiative { slug_id, .. }) => Ok(InitiativeReference::UrlSlug(slug_id)),
        Some(_) => Err(Error::new(
            "initiative URL kind check returned a different kind",
        )),
        None if is_linear_uuid(input) => Ok(InitiativeReference::Id(input.to_owned())),
        None => Ok(InitiativeReference::NameOrSlug(input.to_owned())),
    }
}
pub async fn resolve_initiative_with_transport(
    reference: &InitiativeReference,
    original: &str,
    client: &LinearClient,
) -> Result<String, Error> {
    resolve_initiative_with(
        reference,
        original,
        |variables| async move { Ok(client.query(variables).await?) },
        |variables| async move { Ok(client.query(variables).await?) },
    )
    .await
}
pub async fn resolve_initiative_with<S, SF, N, NF>(
    reference: &InitiativeReference,
    original: &str,
    mut slug_fetch: S,
    mut name_fetch: N,
) -> Result<String, Error>
where
    S: FnMut(UrlSlugVariables) -> SF,
    SF: std::future::Future<Output = Result<ResolveInitiativeBySlug, Error>>,
    N: FnMut(NameVariables) -> NF,
    NF: std::future::Future<Output = Result<ResolveInitiativeByName, Error>>,
{
    let slug = match reference {
        InitiativeReference::Id(id) => return Ok(id.clone()),
        InitiativeReference::UrlSlug(slug) | InitiativeReference::NameOrSlug(slug) => slug,
    };
    let data = slug_fetch(UrlSlugVariables {
        slug_id: slug.clone(),
        include_archived: Some(false),
    })
    .await?;
    if let Some(id) = data
        .initiatives
        .nodes
        .into_iter()
        .next()
        .map(|initiative| initiative.id.into_inner())
        .filter(|id| matches!(reference, InitiativeReference::UrlSlug(_)) || !id.is_empty())
    {
        return Ok(id);
    }
    if matches!(reference, InitiativeReference::UrlSlug(_)) {
        return Err(Error::not_found("Initiative", original).with_hint("The initiative in that URL may have been deleted, or be in a workspace this key cannot see."));
    }
    let data = name_fetch(NameVariables { name: slug.clone() }).await?;
    let matches = data.initiatives.nodes;
    if matches.len() > 1 {
        let listing = matches
            .iter()
            .map(|item| format!("  {} — {} ({})", item.name, item.slug_id, item.id.inner()))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Error::new(format!(
            "Initiative \"{original}\" is ambiguous; it matches multiple initiatives:\n{listing}"
        ))
        .with_hint("Pass the initiative's slug ID or UUID instead."));
    }
    matches
        .into_iter()
        .next()
        .map(|item| item.id.into_inner())
        .ok_or_else(|| {
            Error::not_found("Initiative", original)
                .with_hint("Pass an initiative UUID, slug ID, or exact initiative name.")
        })
}

#[cfg(test)]
mod tests;
