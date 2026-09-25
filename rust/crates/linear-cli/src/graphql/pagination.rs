//! Forward cursor pagination for built-in connections.
//!
//! The oracle loops `after = pageInfo.endCursor` while `hasNextPage` and
//! fails when `hasNextPage` is true without a cursor. The default rejects an
//! empty cursor too; opt-in `Allow` sends it like any concrete cursor. Both
//! policies add one reviewed strictness delta recorded in
//! `rust/compatibility.md`: a cursor equal to the one just sent, or to any
//! cursor seen earlier in the walk, aborts instead of looping. No page count
//! limit is imposed. A failure on any page discards every page: partial
//! results never become a completed result.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::graphql::operations::teams;

/// The forward-pagination fields of one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

impl From<teams::PageInfo> for PageInfo {
    fn from(info: teams::PageInfo) -> Self {
        Self {
            has_next_page: info.has_next_page,
            end_cursor: info.end_cursor,
        }
    }
}

/// One fetched page: its typed nodes and pagination fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<N> {
    pub nodes: Vec<N>,
    pub page_info: PageInfo,
}

/// Every node from every page plus the last page's `pageInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paginated<N> {
    pub nodes: Vec<N>,
    pub page_info: PageInfo,
}

/// Whether a connection may send an empty string as its next-page cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyCursorPolicy {
    /// Match strict built-in connections such as `team list`.
    Reject,
    /// Treat `""` as a concrete cursor, while still rejecting repeats/cycles.
    Allow,
}

/// Why a walk stopped without a complete result. `page` counts from 1.
#[derive(Debug)]
pub enum PaginationError<E> {
    /// Fetching this page failed; earlier pages are discarded.
    Fetch { page: usize, source: E },
    /// `hasNextPage` was true but `endCursor` was null, or empty under Reject.
    MissingCursor { page: usize },
    /// `endCursor` repeated the cursor just requested or one seen earlier.
    RepeatedCursor { page: usize, cursor: String },
}

impl<E: fmt::Display> fmt::Display for PaginationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch { page, source } => write!(f, "page {page} failed: {source}"),
            Self::MissingCursor { page } => write!(
                f,
                "page {page} reported more results but returned no pagination cursor"
            ),
            Self::RepeatedCursor { page, cursor } => write!(
                f,
                "page {page} returned a pagination cursor that was already used ({cursor})"
            ),
        }
    }
}

impl<E: Error + 'static> Error for PaginationError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Fetch { source, .. } => Some(source),
            Self::MissingCursor { .. } | Self::RepeatedCursor { .. } => None,
        }
    }
}

/// Walks every page with the strict policy. `fetch` receives `None` for the
/// first page (built-ins omit `after`) and `Some(cursor)` afterwards.
pub async fn paginate<N, E, F, Fut>(fetch: F) -> Result<Paginated<N>, PaginationError<E>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = Result<Page<N>, E>>,
{
    paginate_with_policy(EmptyCursorPolicy::Reject, fetch).await
}

/// Walks every page under an explicit empty-cursor policy. Under Allow, an
/// empty string is sent as `Some("")`; null still fails, and all seen cursors
/// (including `""`) are rejected if they recur.
pub async fn paginate_with_policy<N, E, F, Fut>(
    policy: EmptyCursorPolicy,
    mut fetch: F,
) -> Result<Paginated<N>, PaginationError<E>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = Result<Page<N>, E>>,
{
    let mut nodes = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut cursor: Option<String> = None;
    let mut page = 1;
    loop {
        let fetched = fetch(cursor.clone())
            .await
            .map_err(|source| PaginationError::Fetch { page, source })?;
        nodes.extend(fetched.nodes);
        let info = fetched.page_info;
        if !info.has_next_page {
            return Ok(Paginated {
                nodes,
                page_info: info,
            });
        }
        let next = match info.end_cursor.as_deref() {
            Some(next) if policy == EmptyCursorPolicy::Allow || !next.is_empty() => next.to_owned(),
            Some(_) | None => return Err(PaginationError::MissingCursor { page }),
        };
        if cursor.as_deref() == Some(next.as_str()) || !seen.insert(next.clone()) {
            return Err(PaginationError::RepeatedCursor { page, cursor: next });
        }
        cursor = Some(next);
        page += 1;
    }
}
