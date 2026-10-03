//! `linear user`: workspace members.
pub mod list;

use crate::cli::user::UserCommand;
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::user::{
    GetViewerId, LookupUser, LookupUserNode, LookupUserVariables,
};

pub fn run(ctx: &Ctx, command: &UserCommand) -> Result<()> {
    match command {
        UserCommand::List(args) => list::run(ctx, args),
    }
}

/// The ID of the user `input` names: `@me` (or `self`), an email, a display
/// name, or part of a name. `noun` names the role in the not-found error,
/// like "Owner".
pub async fn resolve(client: &LinearClient, input: &str, noun: &str) -> Result<String> {
    if input == "@me" || input == "self" {
        let data: GetViewerId = client.query(()).await?;
        return Ok(data.viewer.id.into_inner());
    }
    let data: LookupUser = client
        .query(LookupUserVariables {
            input: input.to_owned(),
        })
        .await?;
    select(&data.users.nodes, input)
        .map(|user| user.id.inner().to_owned())
        .ok_or_else(|| Error::not_found(noun, input))
}

/// An exact email match, then an exact display name, then the first user
/// Linear matched by name.
fn select<'a>(users: &'a [LookupUserNode], input: &str) -> Option<&'a LookupUserNode> {
    let wanted = input.to_lowercase();
    users
        .iter()
        .find(|user| user.email.to_lowercase() == wanted)
        .or_else(|| {
            users
                .iter()
                .find(|user| user.display_name.to_lowercase() == wanted)
        })
        .or_else(|| users.first())
}
