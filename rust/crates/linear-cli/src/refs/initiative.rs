//! Initiatives, referenced by UUID, slug ID, exact name or initiative URL.
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind, is_linear_uuid};
use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::initiative::{
    InitiativeNameVariables, ResolveInitiativeByName, ResolveInitiativeBySlug, UrlSlugVariables,
};

/// An initiative argument, checked locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitiativeReference {
    input: String,
    target: Target,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Target {
    Id(String),
    /// A slug ID, else an exact name.
    NameOrSlug,
    /// The slug ID from an initiative URL.
    UrlSlug(String),
}

/// Whether a slug or name may match an archived initiative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Archived {
    Exclude,
    Include,
}

impl InitiativeReference {
    pub fn parse(input: &str, scope: &WorkspaceScope<'_>) -> Result<Self> {
        let target = match expect_url_kind(
            input,
            LinearUrlKind::Initiative,
            "an initiative URL, UUID, slug ID, or exact name",
            scope,
        )? {
            Some(LinearUrlRef::Initiative { slug_id, .. }) => Target::UrlSlug(slug_id),
            Some(other) => unreachable!("expect_url_kind returned a {:?} URL", other.kind()),
            None if is_linear_uuid(input) => Target::Id(input.to_owned()),
            None => Target::NameOrSlug,
        };
        Ok(Self {
            input: input.to_owned(),
            target,
        })
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    /// The UUID, when the argument was one.
    pub fn id(&self) -> Option<&str> {
        match &self.target {
            Target::Id(id) => Some(id),
            Target::NameOrSlug | Target::UrlSlug(_) => None,
        }
    }
}

/// The ID of the initiative `reference` names: a UUID as given, else a slug
/// ID, else an exact (case-insensitive) name. A URL's slug never falls back
/// to a name.
pub async fn resolve(
    client: &LinearClient,
    reference: &InitiativeReference,
    archived: Archived,
) -> Result<String> {
    let include_archived = archived == Archived::Include;
    let slug = match &reference.target {
        Target::Id(id) => return Ok(id.clone()),
        Target::UrlSlug(slug) => slug,
        Target::NameOrSlug => &reference.input,
    };
    let data: ResolveInitiativeBySlug = client
        .query(UrlSlugVariables {
            slug_id: slug.clone(),
            include_archived,
        })
        .await?;
    if let Some(initiative) = data.initiatives.nodes.into_iter().next() {
        return Ok(initiative.id.into_inner());
    }
    if let Target::UrlSlug(_) = reference.target {
        return Err(
            Error::not_found("Initiative", &reference.input).with_hint(
                "The initiative in that URL may have been deleted, or be in a workspace this key cannot see.",
            ),
        );
    }
    let data: ResolveInitiativeByName = client
        .query(InitiativeNameVariables {
            name: reference.input.clone(),
            include_archived,
        })
        .await?;
    let mut matches = data.initiatives.nodes;
    if matches.len() > 1 {
        return Err(super::ambiguous(
            "Initiative",
            &reference.input,
            matches
                .iter()
                .map(|item| format!("{} — {} ({})", item.name, item.slug_id, item.id.inner())),
        )
        .with_hint("Pass the initiative's slug ID or UUID instead."));
    }
    matches
        .pop()
        .map(|item| item.id.into_inner())
        .ok_or_else(|| {
            Error::not_found("Initiative", &reference.input)
                .with_hint("Pass an initiative UUID, slug ID, or exact initiative name.")
        })
}
