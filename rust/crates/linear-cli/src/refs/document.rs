//! Local document URL reduction, before credential selection.
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind};
use crate::error::{AppError, AppErrorKind};
pub fn resolve_document_reference(
    input: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<String, AppError> {
    match expect_url_kind(
        input,
        LinearUrlKind::Document,
        "a document URL, UUID, or slug ID",
        scope,
    )? {
        Some(LinearUrlRef::Document { slug_id, .. }) => Ok(slug_id),
        Some(_) => Err(AppError::new(
            AppErrorKind::Invariant,
            "document URL kind check returned a different kind",
        )),
        None => Ok(input.to_owned()),
    }
}
