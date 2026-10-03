//! Helpers shared by the document commands.
use crate::client::{LinearClient, RequestError};
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::document::DocumentUpdateInput;

use super::target::{self, Kind, PreparedTarget};

/// The document's slug ID or UUID from a document argument, which may be a URL.
pub fn reference(ctx: &Ctx, input: &str) -> Result<String> {
    crate::refs::document::parse(input, &ctx.scope()?)
}

/// `document(id:)` is non-null, so Linear reports a missing document as a
/// GraphQL error.
pub fn not_found(failure: RequestError, original: &str) -> Error {
    failure.or_not_found("Document", original)
}

/// Looks up the target and sets its ID in the field for its kind.
pub async fn attach(
    client: &LinearClient,
    input: &mut DocumentUpdateInput,
    target: Option<&PreparedTarget>,
) -> Result<()> {
    let Some(target) = target else {
        return Ok(());
    };
    let (kind, id) = target::resolve(target, client).await?;
    let field = match kind {
        Kind::Project => &mut input.project_id,
        Kind::Issue => &mut input.issue_id,
        Kind::Initiative => &mut input.initiative_id,
        Kind::Team => &mut input.team_id,
        Kind::Cycle => &mut input.cycle_id,
        Kind::Release => &mut input.release_id,
    };
    *field = Some(id);
    Ok(())
}

pub fn read_file(path: &str) -> Result<String> {
    text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::not_found("File", path)
        } else {
            Error::new(format!("Failed to read {path}: {error}")).with_source(error)
        }
    })
}
