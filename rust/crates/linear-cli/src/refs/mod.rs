//! References to Linear entities: Linear URLs, the local workspace check, and
//! one module per entity that parses an argument (before any request) and
//! resolves it to an ID.
pub mod cycle;
pub mod document;
pub mod initiative;
mod issue;
pub mod project;
pub mod release;
pub mod team;
mod url;
pub mod user;
mod uuid;
pub mod workflow_states;
mod workspace;

pub use issue::{IssueReference, find_issue_identifier, prepare_issue_reference};
pub use url::{LinearUrlKind, LinearUrlRef};
pub use uuid::is_linear_uuid;
pub use workspace::{WorkspaceScope, expect_url_kind, reject_comment_url, reject_linear_url};

use crate::error::Error;

/// `input` names more than one `entity` (like "Team"); `candidates` describe
/// each match, one per line. Callers add a hint naming an unambiguous form.
pub fn ambiguous(entity: &str, input: &str, candidates: impl IntoIterator<Item = String>) -> Error {
    let listing: Vec<String> = candidates
        .into_iter()
        .map(|candidate| format!("  {candidate}"))
        .collect();
    Error::new(format!(
        "{entity} \"{input}\" is ambiguous; it matches:\n{}",
        listing.join("\n")
    ))
}

#[cfg(test)]
mod test_support;
