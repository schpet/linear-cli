//! Forward cursor pagination for Linear connections.
//!
//! Requests continue with `after = pageInfo.endCursor` while `hasNextPage` and
//! the limit is not reached. A next page without a usable cursor (null or
//! empty), or a cursor Linear already sent, is an error rather than a silent
//! stop or an endless loop. Any failure discards the pages fetched so far.

use std::collections::HashSet;
use std::num::NonZeroU32;

use crate::error::{Error, Result};
use crate::graphql::schema;

/// The most nodes one request asks for.
pub const PAGE_SIZE: u32 = 100;

/// The forward-pagination fields of a connection.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

/// One fetched page: its nodes and pagination fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<N> {
    pub nodes: Vec<N>,
    pub page_info: PageInfo,
}

/// Where a walk over a connection stands: the cursor for the next request
/// and how many more nodes are wanted.
#[derive(Debug)]
pub struct Pages {
    after: Option<String>,
    seen: HashSet<String>,
    remaining: Option<u32>,
}

impl Pages {
    /// A walk that stops after `limit` nodes, or at the last page without one.
    pub fn new(limit: Option<NonZeroU32>) -> Self {
        Self {
            after: None,
            seen: HashSet::new(),
            remaining: limit.map(NonZeroU32::get),
        }
    }

    /// The `after` variable for the next request; `None` for the first page.
    pub fn after(&self) -> Option<String> {
        self.after.clone()
    }

    /// The `first` variable for the next request.
    pub fn first(&self) -> i32 {
        let size = self.remaining.map_or(PAGE_SIZE, |left| left.min(PAGE_SIZE));
        i32::try_from(size).expect("page sizes are at most PAGE_SIZE")
    }

    /// Records a page that returned `received` nodes. `Ok(true)` means the
    /// walk needs another page.
    pub fn advance(&mut self, received: usize, info: &PageInfo) -> Result<bool> {
        if let Some(left) = &mut self.remaining {
            let received = u32::try_from(received).unwrap_or(u32::MAX);
            *left = left.saturating_sub(received);
            if *left == 0 {
                return Ok(false);
            }
        }
        if !info.has_next_page {
            return Ok(false);
        }
        let cursor = info
            .end_cursor
            .as_deref()
            .filter(|cursor| !cursor.is_empty())
            .ok_or_else(|| {
                Error::new("Linear reported more results but sent no cursor to fetch them")
                    .with_hint("Retry the command.")
            })?;
        if !self.seen.insert(cursor.to_owned()) {
            return Err(Error::new("Linear sent the same pagination cursor twice")
                .with_hint("Retry the command."));
        }
        self.after = Some(cursor.to_owned());
        Ok(true)
    }
}

/// The nodes of every page up to `limit`. `fetch` gets the `after` and
/// `first` variables for each request.
pub async fn collect<N, F, Fut>(limit: Option<NonZeroU32>, mut fetch: F) -> Result<Vec<N>>
where
    F: FnMut(Option<String>, i32) -> Fut,
    Fut: Future<Output = Result<Page<N>>>,
{
    let mut pages = Pages::new(limit);
    let mut nodes = Vec::new();
    loop {
        let page = fetch(pages.after(), pages.first()).await?;
        let more = pages.advance(page.nodes.len(), &page.page_info)?;
        nodes.extend(page.nodes);
        if !more {
            break;
        }
    }
    if let Some(limit) = limit {
        nodes.truncate(usize::try_from(limit.get()).unwrap_or(usize::MAX));
    }
    Ok(nodes)
}

/// The nodes of `first`, a page that arrived inside a larger response, and of
/// every page after it.
pub async fn complete<N, F, Fut>(first: Page<N>, mut fetch: F) -> Result<Vec<N>>
where
    F: FnMut(Option<String>, i32) -> Fut,
    Fut: Future<Output = Result<Page<N>>>,
{
    let mut pages = Pages::new(None);
    let mut more = pages.advance(first.nodes.len(), &first.page_info)?;
    let mut nodes = first.nodes;
    while more {
        let page = fetch(pages.after(), pages.first()).await?;
        more = pages.advance(page.nodes.len(), &page.page_info)?;
        nodes.extend(page.nodes);
    }
    Ok(nodes)
}

/// [`collect`] for a connection inside a parent record, such as a document's
/// comments. `fetch` returns the parent holding one page; `take` moves that
/// page out of it and `put` stores every node, with the last page's info, in
/// the first page's parent, which is returned.
pub async fn collect_within<P, N, F, Fut>(
    limit: Option<NonZeroU32>,
    mut fetch: F,
    take: impl Fn(&mut P) -> Page<N>,
    put: impl FnOnce(&mut P, Page<N>),
) -> Result<P>
where
    F: FnMut(Option<String>, i32) -> Fut,
    Fut: Future<Output = Result<P>>,
{
    let mut pages = Pages::new(limit);
    let mut parent = None;
    let mut nodes = Vec::new();
    let page_info = loop {
        let mut record = fetch(pages.after(), pages.first()).await?;
        let page = take(&mut record);
        let more = pages.advance(page.nodes.len(), &page.page_info)?;
        nodes.extend(page.nodes);
        parent.get_or_insert(record);
        if !more {
            break page.page_info;
        }
    };
    if let Some(limit) = limit {
        nodes.truncate(usize::try_from(limit.get()).unwrap_or(usize::MAX));
    }
    let mut parent = parent.expect("the walk fetched at least one page");
    put(&mut parent, Page { nodes, page_info });
    Ok(parent)
}

#[cfg(test)]
mod tests;
