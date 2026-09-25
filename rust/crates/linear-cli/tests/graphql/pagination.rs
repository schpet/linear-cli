//! Cursor pagination strictness without any network: pages come from closures.

use std::collections::VecDeque;
use std::fmt;

use linear_cli::graphql::pagination::{
    EmptyCursorPolicy, Page, PageInfo, Paginated, PaginationError, paginate, paginate_with_policy,
};

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
async fn walk_with_policy(
    policy: EmptyCursorPolicy,
    script: Vec<Result<Page<String>, FetchFailed>>,
) -> (
    Result<Paginated<String>, PaginationError<FetchFailed>>,
    Vec<Option<String>>,
) {
    let mut queue: VecDeque<_> = script.into();
    let mut asked = Vec::new();
    let result = paginate_with_policy(policy, |after| {
        asked.push(after);
        let next = queue
            .pop_front()
            .unwrap_or(Err(FetchFailed("script exhausted")));
        async move { next }
    })
    .await;
    (result, asked)
}

async fn walk(
    script: Vec<Result<Page<String>, FetchFailed>>,
) -> (
    Result<Paginated<String>, PaginationError<FetchFailed>>,
    Vec<Option<String>>,
) {
    walk_with_policy(EmptyCursorPolicy::Reject, script).await
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
async fn default_paginate_wrapper_rejects_empty_before_another_fetch() {
    let mut asked = Vec::new();
    let result: Result<Paginated<String>, PaginationError<FetchFailed>> = paginate(|after| {
        asked.push(after);
        async { Ok(page(&["a"], true, Some(""))) }
    })
    .await;
    assert!(matches!(
        result,
        Err(PaginationError::MissingCursor { page: 1 })
    ));
    assert_eq!(asked, [None]);
}

#[tokio::test(flavor = "current_thread")]
async fn allow_empty_sends_the_empty_cursor_and_retains_final_page_info() {
    let (result, asked) = walk_with_policy(
        EmptyCursorPolicy::Allow,
        vec![
            Ok(page(&["a"], true, Some(""))),
            Ok(page(&["b"], false, Some("final"))),
        ],
    )
    .await;
    let completed = result.expect("empty cursor advances once");
    assert_eq!(completed.nodes, ["a", "b"]);
    assert_eq!(completed.page_info.end_cursor.as_deref(), Some("final"));
    assert_eq!(asked, [None, Some(String::new())]);
}

#[tokio::test(flavor = "current_thread")]
async fn allow_empty_accepts_the_first_empty_cursor_on_a_later_page() {
    let (result, asked) = walk_with_policy(
        EmptyCursorPolicy::Allow,
        vec![
            Ok(page(&["a"], true, Some("A"))),
            Ok(page(&["b"], true, Some(""))),
            Ok(page(&["c"], false, None)),
        ],
    )
    .await;
    assert_eq!(result.expect("later empty cursor").nodes, ["a", "b", "c"]);
    assert_eq!(asked, [None, Some("A".to_owned()), Some(String::new())]);
}

#[tokio::test(flavor = "current_thread")]
async fn allow_empty_rejects_immediate_repeat_and_seen_cycles() {
    for (script, page_number, asked) in [
        (
            vec![
                Ok(page(&["a"], true, Some(""))),
                Ok(page(&["b"], true, Some(""))),
            ],
            2,
            vec![None, Some(String::new())],
        ),
        (
            vec![
                Ok(page(&["a"], true, Some(""))),
                Ok(page(&["b"], true, Some("A"))),
                Ok(page(&["c"], true, Some(""))),
            ],
            3,
            vec![None, Some(String::new()), Some("A".to_owned())],
        ),
    ] {
        let (result, actual_asked) = walk_with_policy(EmptyCursorPolicy::Allow, script).await;
        match result {
            Err(PaginationError::RepeatedCursor { page, cursor }) => {
                assert_eq!(page, page_number);
                assert!(cursor.is_empty());
            }
            other => panic!("expected repeated empty cursor, got {other:?}"),
        }
        assert_eq!(actual_asked, asked);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn allow_empty_still_rejects_null_and_nonempty_cycles() {
    let (missing, asked) =
        walk_with_policy(EmptyCursorPolicy::Allow, vec![Ok(page(&["a"], true, None))]).await;
    assert!(matches!(
        missing,
        Err(PaginationError::MissingCursor { page: 1 })
    ));
    assert_eq!(asked, [None]);

    let (cycle, asked) = walk_with_policy(
        EmptyCursorPolicy::Allow,
        vec![
            Ok(page(&["a"], true, Some("A"))),
            Ok(page(&["b"], true, Some("B"))),
            Ok(page(&["c"], true, Some("A"))),
        ],
    )
    .await;
    assert!(matches!(
        cycle,
        Err(PaginationError::RepeatedCursor { page: 3, cursor }) if cursor == "A"
    ));
    assert_eq!(asked, [None, Some("A".to_owned()), Some("B".to_owned())]);
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
