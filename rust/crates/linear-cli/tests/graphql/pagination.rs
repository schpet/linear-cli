//! Cursor pagination without any network: pages come from closures.

use std::collections::VecDeque;
use std::num::NonZeroU32;

use linear_cli::error::Error;
use linear_cli::graphql::pagination::{Page, PageInfo, Pages, collect, collect_within};

fn page(nodes: &[&str], has_next_page: bool, end_cursor: Option<&str>) -> Page<String> {
    Page {
        nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
        page_info: PageInfo {
            has_next_page,
            end_cursor: end_cursor.map(str::to_owned),
        },
    }
}

/// Serves scripted pages and records every `(after, first)` it was asked for.
async fn walk(
    limit: Option<u32>,
    script: Vec<Result<Page<String>, Error>>,
) -> (Result<Vec<String>, Error>, Vec<(Option<String>, i32)>) {
    let mut queue: VecDeque<_> = script.into();
    let mut asked = Vec::new();
    let limit = limit.map(|limit| NonZeroU32::new(limit).expect("positive limit"));
    let result = collect(limit, |after, first| {
        asked.push((after, first));
        let next = queue
            .pop_front()
            .unwrap_or_else(|| Err(Error::new("script exhausted")));
        async move { next }
    })
    .await;
    (result, asked)
}

#[tokio::test(flavor = "current_thread")]
async fn first_page_omits_the_cursor_and_later_pages_carry_it() {
    let (result, asked) = walk(
        None,
        vec![
            Ok(page(&["a", "b"], true, Some("cursor-a"))),
            Ok(page(&["c"], true, Some("cursor-b"))),
            Ok(page(&[], false, Some("cursor-c"))),
        ],
    )
    .await;
    assert_eq!(result.expect("three pages"), ["a", "b", "c"]);
    assert_eq!(
        asked,
        [
            (None, 100),
            (Some("cursor-a".to_owned()), 100),
            (Some("cursor-b".to_owned()), 100),
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_limit_shrinks_the_page_size_and_stops_early() {
    let (result, asked) = walk(
        Some(3),
        vec![
            Ok(page(&["a", "b"], true, Some("cursor-a"))),
            Ok(page(&["c", "d"], true, Some("cursor-b"))),
        ],
    )
    .await;
    assert_eq!(result.expect("limited"), ["a", "b", "c"]);
    assert_eq!(asked, [(None, 3), (Some("cursor-a".to_owned()), 1)]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_next_page_without_a_usable_cursor_is_an_error() {
    for cursor in [None, Some("")] {
        let (result, asked) = walk(None, vec![Ok(page(&["a"], true, cursor))]).await;
        let error = result.expect_err("no cursor");
        assert!(error.message().contains("no cursor"), "{error}");
        assert_eq!(asked.len(), 1, "no second fetch is attempted");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_seen_before_is_an_error() {
    let (result, asked) = walk(
        None,
        vec![
            Ok(page(&[], true, Some("A"))),
            Ok(page(&[], true, Some("B"))),
            Ok(page(&[], true, Some("A"))),
        ],
    )
    .await;
    let error = result.expect_err("repeated cursor");
    assert!(
        error.message().contains("same pagination cursor"),
        "{error}"
    );
    assert_eq!(asked.len(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_page_discards_earlier_pages() {
    let (result, asked) = walk(
        None,
        vec![Ok(page(&["a"], true, Some("A"))), Err(Error::new("boom"))],
    )
    .await;
    assert_eq!(result.expect_err("second page fails").message(), "boom");
    assert_eq!(asked.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn nested_connections_keep_the_first_parent_and_every_node() {
    struct Parent {
        name: &'static str,
        children: Page<String>,
    }
    let mut script = VecDeque::from([
        Parent {
            name: "first",
            children: page(&["a"], true, Some("A")),
        },
        Parent {
            name: "second",
            children: page(&["b"], false, None),
        },
    ]);
    let parent = collect_within(
        None,
        |_after, _first| {
            let next = script.pop_front().expect("scripted page");
            async move { Ok(next) }
        },
        |parent: &mut Parent| Page {
            nodes: std::mem::take(&mut parent.children.nodes),
            page_info: parent.children.page_info.clone(),
        },
        |parent, page| parent.children = page,
    )
    .await
    .expect("two pages");
    assert_eq!(parent.name, "first");
    assert_eq!(parent.children.nodes, ["a", "b"]);
    assert!(!parent.children.page_info.has_next_page);
}

#[test]
fn pages_track_the_cursor_and_remaining_limit() {
    let mut pages = Pages::new(NonZeroU32::new(150));
    assert_eq!((pages.after(), pages.first()), (None, 100));
    let more = pages
        .advance(100, &page(&[], true, Some("A")).page_info)
        .expect("valid page");
    assert!(more);
    assert_eq!((pages.after(), pages.first()), (Some("A".to_owned()), 50));
    let more = pages
        .advance(50, &page(&[], true, Some("B")).page_info)
        .expect("valid page");
    assert!(!more, "the limit is reached");
}
