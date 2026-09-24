//! Cursor pagination strictness without any network: pages come from closures.

use std::collections::VecDeque;
use std::fmt;

use linear_cli::graphql::pagination::{Page, PageInfo, Paginated, PaginationError, paginate};

#[derive(Debug, PartialEq, Eq)]
struct FetchFailed(&'static str);

impl fmt::Display for FetchFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fetch failed: {}", self.0)
    }
}

impl std::error::Error for FetchFailed {}

fn page(nodes: &[&str], has_next_page: bool, end_cursor: Option<&str>) -> Page<String> {
    Page {
        nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
        page_info: PageInfo {
            has_next_page,
            end_cursor: end_cursor.map(str::to_owned),
        },
    }
}

/// Serves scripted pages and records every cursor it was asked for.
async fn walk(
    script: Vec<Result<Page<String>, FetchFailed>>,
) -> (
    Result<Paginated<String>, PaginationError<FetchFailed>>,
    Vec<Option<String>>,
) {
    let mut queue: VecDeque<_> = script.into();
    let mut asked = Vec::new();
    let result = paginate(|after| {
        asked.push(after);
        let next = queue
            .pop_front()
            .unwrap_or(Err(FetchFailed("script exhausted")));
        async move { next }
    })
    .await;
    (result, asked)
}

#[tokio::test(flavor = "current_thread")]
async fn first_page_omits_cursor_and_later_pages_carry_it() {
    let (result, asked) = walk(vec![
        Ok(page(&["a", "b"], true, Some("cursor-a"))),
        Ok(page(&["c"], true, Some("cursor-b"))),
        Ok(page(&[], false, Some("cursor-c"))),
    ])
    .await;
    let paginated = result.expect("three pages");
    assert_eq!(paginated.nodes, vec!["a", "b", "c"]);
    assert_eq!(
        paginated.page_info,
        PageInfo {
            has_next_page: false,
            end_cursor: Some("cursor-c".to_owned()),
        }
    );
    assert_eq!(
        asked,
        vec![
            None,
            Some("cursor-a".to_owned()),
            Some("cursor-b".to_owned())
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn single_page_without_next_keeps_its_page_info() {
    let (result, asked) = walk(vec![Ok(page(&["only"], false, None))]).await;
    let paginated = result.expect("one page");
    assert_eq!(paginated.nodes, vec!["only"]);
    assert_eq!(paginated.page_info.end_cursor, None);
    assert_eq!(asked, vec![None]);
}

#[tokio::test(flavor = "current_thread")]
async fn has_next_page_without_cursor_is_an_error() {
    for cursor in [None, Some("")] {
        let (result, asked) = walk(vec![Ok(page(&["a"], true, cursor))]).await;
        match result {
            Err(PaginationError::MissingCursor { page }) => assert_eq!(page, 1),
            other => panic!("expected MissingCursor, got {other:?}"),
        }
        assert_eq!(asked.len(), 1, "no second fetch is attempted");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unchanged_cursor_is_rejected() {
    let (result, asked) = walk(vec![
        Ok(page(&["a"], true, Some("cursor-a"))),
        Ok(page(&["b"], true, Some("cursor-a"))),
    ])
    .await;
    match result {
        Err(PaginationError::RepeatedCursor { page, cursor }) => {
            assert_eq!(page, 2);
            assert_eq!(cursor, "cursor-a");
        }
        other => panic!("expected RepeatedCursor, got {other:?}"),
    }
    assert_eq!(asked.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn previously_seen_cursor_is_rejected() {
    let (result, asked) = walk(vec![
        Ok(page(&["a"], true, Some("cursor-a"))),
        Ok(page(&["b"], true, Some("cursor-b"))),
        Ok(page(&["c"], true, Some("cursor-a"))),
    ])
    .await;
    match result {
        Err(PaginationError::RepeatedCursor { page, cursor }) => {
            assert_eq!(page, 3);
            assert_eq!(cursor, "cursor-a");
        }
        other => panic!("expected RepeatedCursor, got {other:?}"),
    }
    assert_eq!(asked.len(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn later_page_failure_discards_earlier_pages() {
    let (result, asked) = walk(vec![
        Ok(page(&["a"], true, Some("cursor-a"))),
        Err(FetchFailed("boom")),
    ])
    .await;
    match &result {
        Err(PaginationError::Fetch { page, source }) => {
            assert_eq!(*page, 2);
            assert_eq!(*source, FetchFailed("boom"));
        }
        other => panic!("expected Fetch, got {other:?}"),
    }
    let error = result.expect_err("fetch failure");
    assert_eq!(error.to_string(), "page 2 failed: fetch failed: boom");
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(asked.len(), 2);
}

#[test]
fn error_messages_name_the_page() {
    let missing: PaginationError<FetchFailed> = PaginationError::MissingCursor { page: 4 };
    assert_eq!(
        missing.to_string(),
        "page 4 reported more results but returned no pagination cursor"
    );
    let repeated: PaginationError<FetchFailed> = PaginationError::RepeatedCursor {
        page: 2,
        cursor: "x".to_owned(),
    };
    assert_eq!(
        repeated.to_string(),
        "page 2 returned a pagination cursor that was already used (x)"
    );
}
