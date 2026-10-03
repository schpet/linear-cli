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
    )? {
        Some(LinearUrlRef::Document { slug_id, .. }) => Ok(slug_id),
        Some(other) => unreachable!("expect_url_kind returned a {:?} URL", other.kind()),
        None => Ok(input.to_owned()),
    }
}
