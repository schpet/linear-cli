//! Documents, referenced by UUID, slug ID or document URL. The API accepts
//! either ID form directly, so a document needs no lookup request.
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind};
use crate::error::Result;

/// The UUID or slug ID that `input` names.
pub fn parse(input: &str, scope: &WorkspaceScope<'_>) -> Result<String> {
    match expect_url_kind(
        input,
        LinearUrlKind::Document,
        "a document URL, UUID, or slug ID",
        scope,
        |url| match url {
            LinearUrlRef::Document { slug_id, .. } => Some(slug_id),
            _ => None,
        },
    )? {
        Some(slug_id) => Ok(slug_id),
        None => Ok(input.to_owned()),
    }
}
