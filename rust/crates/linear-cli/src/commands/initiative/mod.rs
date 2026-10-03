//! `linear initiative`: initiatives, their projects and comments.
mod archive;
mod comment_add;
mod comment_list;
mod create;
pub mod list;
mod projects;
mod unarchive;
mod update;
pub mod view;

use crate::cli::initiative::{InitiativeCommand, InitiativeCommentCommand};
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::initiative::ResolveInitiativeByNameIncludingArchived;
use crate::graphql::operations::initiative::ResolveInitiativeBySlug;
use crate::graphql::operations::initiative::{NameVariables, UrlSlugVariables};
use crate::refs::{self, InitiativeReference};

pub fn run(ctx: &Ctx, command: &InitiativeCommand) -> Result<()> {
    match command {
        InitiativeCommand::List(args) => list::run(ctx, args),
        InitiativeCommand::View(args) => view::run(ctx, args),
        InitiativeCommand::Create(args) => create::run(ctx, args),
        InitiativeCommand::Archive(args) => archive::archive(ctx, args),
        InitiativeCommand::Update(args) => update::run(ctx, args),
        InitiativeCommand::Unarchive(args) => unarchive::run(ctx, args),
        InitiativeCommand::Delete(args) => archive::delete(ctx, args),
        InitiativeCommand::AddProject(args) => projects::add(ctx, args),
        InitiativeCommand::RemoveProject(args) => projects::remove(ctx, args),
        InitiativeCommand::Comment(args) => match &args.command {
            InitiativeCommentCommand::Add(args) => comment_add::run(ctx, args),
            InitiativeCommentCommand::List(args) => comment_list::run(ctx, args),
        },
    }
}

/// Parses an initiative argument (URL, UUID, slug ID or name) without a request.
fn reference(ctx: &Ctx, input: &str) -> Result<InitiativeReference> {
    refs::prepare_initiative_lookup(input, &ctx.scope()?)
}

/// Which initiatives a slug or name may match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Archived {
    Exclude,
    Include,
}

/// The initiative's UUID: a slug ID first, then an exact (case-insensitive) name.
async fn resolve(
    client: &LinearClient,
    reference: &InitiativeReference,
    original: &str,
    archived: Archived,
) -> Result<String> {
    let slug = match (reference, archived) {
        (InitiativeReference::Id(id), _) => return Ok(id.clone()),
        (_, Archived::Exclude) => {
            return refs::resolve_initiative_with_transport(reference, original, client).await;
        }
        (InitiativeReference::UrlSlug(slug) | InitiativeReference::NameOrSlug(slug), _) => slug,
    };
    let data: ResolveInitiativeBySlug = client
        .query(UrlSlugVariables {
            slug_id: slug.clone(),
            include_archived: Some(true),
        })
        .await?;
    if let Some(node) = data.initiatives.nodes.into_iter().next() {
        return Ok(node.id.into_inner());
    }
    if let InitiativeReference::UrlSlug(_) = reference {
        return Err(Error::not_found("Initiative", original));
    }
    let data: ResolveInitiativeByNameIncludingArchived =
        client.query(NameVariables { name: slug.clone() }).await?;
    let mut matches = data.initiatives.nodes;
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
        .pop()
        .map(|item| item.id.into_inner())
        .ok_or_else(|| {
            Error::not_found("Initiative", original)
                .with_hint("Pass an initiative UUID, slug ID, or exact initiative name.")
        })
}

/// Rejects a Linear URL where an owner is expected, before any request.
fn check_owner(owner: Option<&str>) -> Result<()> {
    match owner {
        Some(owner) => refs::reject_linear_url(owner, "an email, username, display name, or @me"),
        None => Ok(()),
    }
}
