//! Public contracts; staged only, not compiled or claimed passing.
use linear_cli::{
    commands::{
        document_list,
        document_target::{self, Kind},
        document_view, release_lookup,
    },
    graphql::{
        envelope::parse_response,
        operations::{documents::*, releases::*},
    },
};
use serde_json::{Value, json};
use std::time::SystemTime;
fn frozen(id: &str) -> Value {
    let file = format!(
        "{}/../../parity/runner/c050-c051-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(file).expect("frozen source")).expect("case")
}
fn response(case: &Value, index: usize) -> Value {
    let response = &case["graphql"]["groups"][0]["steps"][index]["response"];
    if response["kind"] == "transport" {
        serde_json::from_str(response["body"]["utf8"].as_str().expect("raw fixture"))
            .expect("envelope")
    } else {
        json!({"data":response["data"]})
    }
}
#[test]
fn list_request_omits_absent_filter_and_does_not_request_a_document_cursor() {
    let request = document_list::request(None, 50);
    assert!(!request.query.contains("after:"));
    assert_eq!(
        serde_json::to_value(request.variables.unwrap()).unwrap(),
        json!({"first":50})
    );
    for (kind, key) in [
        (Kind::Project, "project"),
        (Kind::Issue, "issue"),
        (Kind::Initiative, "initiative"),
        (Kind::Team, "team"),
        (Kind::Cycle, "cycle"),
        (Kind::Release, "release"),
    ] {
        let request = document_list::request(
            Some(document_target::filter(kind, "target-id".to_owned())),
            2,
        );
        assert_eq!(
            serde_json::to_value(request.variables.unwrap()).unwrap(),
            json!({"filter":{key:{"id":{"eq":"target-id"}}},"first":2})
        );
    }
}
#[test]
fn list_json_preserves_every_attachment_shape_and_first_page_info() {
    let case = frozen("c050-all-attachments-json");
    let page: ListDocuments = parse_response(response(&case, 0).to_string().as_bytes()).unwrap();
    let page = page.documents.unwrap();
    assert!(page.page_info.has_next_page);
    assert_eq!(
        String::from_utf8(document_list::json(&page).unwrap()).unwrap(),
        case["expected"]["stdout"]["utf8"]
    );
    assert_eq!(
        page.nodes
            .iter()
            .map(document_list::attachment)
            .collect::<Vec<_>>(),
        [
            "Project: Project %s",
            "Issue: ENG-7",
            "Initiative: Initiative",
            "Team: Engineering (ENG)",
            "Cycle: ENG #7 — Sprint",
            "Release: Summer (2026.8)",
            "-"
        ]
    );
}
#[test]
fn list_human_truncates_long_titles_by_display_width() {
    let case = frozen("c050-alias-table-limit");
    let page: ListDocuments = parse_response(response(&case, 0).to_string().as_bytes()).unwrap();
    assert_eq!(
        document_list::text(&page.documents.unwrap(), 120, false, SystemTime::now()),
        "SLUG   TITLE                                                                          ATTACHMENT               UPDATED \nslug-0 A very long title 界 %s detail detail detail detail detail detail detail de... Cycle: ENG #7 — Sprint   just now\nslug-1 A very long title 界 %s detail detail detail detail detail detail detail de... Release: Summer (2026.8) just now\n"
    );
}
#[test]
fn list_empty_output_is_exact() {
    let case = frozen("c050-empty-text");
    let page: ListDocuments = parse_response(response(&case, 0).to_string().as_bytes()).unwrap();
    assert_eq!(
        document_list::text(&page.documents.unwrap(), 120, false, SystemTime::now()),
        case["expected"]["stdout"]["utf8"]
    );
}
#[tokio::test]
async fn json_comments_append_nodes_keep_first_metadata_and_accept_empty_cursor() {
    let case = frozen("c051-json-two-pages");
    let mut index = 0;
    let document = document_view::all_comments_with("d4b93e3b2695", |query| {
        let data = response(&case, index);
        let expected = if index == 0 {
            None
        } else {
            Some(String::new())
        };
        assert_eq!(query.variables.unwrap().comments_after, expected);
        index += 1;
        async move {
            Ok(parse_response::<GetDocumentWithComments>(data.to_string().as_bytes()).unwrap())
        }
    })
    .await
    .unwrap();
    assert_eq!(index, 2);
    assert_eq!(document.title, "Document %s 界");
    assert_eq!(document.comments.nodes.len(), 2);
    assert_eq!(
        String::from_utf8(
            document_view::DocumentResult::WithComments(document)
                .json()
                .unwrap()
        )
        .unwrap(),
        case["expected"]["stdout"]["utf8"]
    );
}
#[test]
fn non_json_query_excludes_comments_and_raw_preserves_input_bytes() {
    assert!(
        !document_view::body_request("slug")
            .query
            .contains("comments")
    );
    let query = document_view::comments_request("slug", None);
    assert!(query.query.contains("comments"));
    assert_eq!(
        serde_json::to_value(query.variables.unwrap()).unwrap(),
        json!({"id":"slug","commentsAfter":null})
    );
    for content in [None, Some(""), Some("\n %s 界\r\n")] {
        let expected = content
            .filter(|value| !value.is_empty())
            .map_or(Vec::new(), |value| format!("{value}\n").into_bytes());
        assert_eq!(document_view::raw(content), expected);
    }
}
#[tokio::test]
async fn missing_document_on_later_json_page_never_emits_partial_output() {
    let case = frozen("c051-null-later");
    let mut index = 0;
    let error = document_view::all_comments_with("d4b93e3b2695", |_| {
        let data = response(&case, index);
        index += 1;
        async move {
            Ok(parse_response::<GetDocumentWithComments>(data.to_string().as_bytes()).unwrap())
        }
    })
    .await
    .unwrap_err();
    assert_eq!(error.message(), "Document not found: d4b93e3b2695");
    assert_eq!(index, 2);
}
#[test]
fn malformed_required_title_is_honest_source_success_and_strict_rust_error() {
    for (id, is_list) in [
        ("c050-missing-required-title", true),
        ("c051-missing-required-title", false),
    ] {
        let case = frozen(id);
        assert_eq!(case["expected"]["exit"], json!({"code":0}));
        let bytes = response(&case, 0).to_string().into_bytes();
        if is_list {
            assert!(parse_response::<ListDocuments>(&bytes).is_err());
        } else {
            assert!(parse_response::<GetDocumentWithComments>(&bytes).is_err());
        }
    }
}
#[tokio::test]
async fn release_pages_deduplicate_uuid_before_ambiguity_and_preserve_query_variables() {
    let case = frozen("c050-release-pages-dedup");
    let mut index = 0;
    let id = release_lookup::resolve_with("2026.8", |request| {
        assert_eq!(
            request.variables.unwrap().after,
            if index == 0 {
                None
            } else {
                Some("release-next".to_owned())
            }
        );
        let data = response(&case, index);
        index += 1;
        async move { Ok(parse_response::<ResolveReleases>(data.to_string().as_bytes()).unwrap()) }
    })
    .await
    .unwrap();
    assert_eq!(id, "00000000-0000-4000-9000-000000000050");
    assert_eq!(index, 2);
}
#[tokio::test]
async fn release_ambiguity_and_url_precedence_are_public_refusals() {
    let case = frozen("c050-release-ambiguous");
    let mut index = 0;
    let error = release_lookup::resolve_with("2026.8", |_| {
        let data = response(&case, index);
        index += 1;
        async move { Ok(parse_response::<ResolveReleases>(data.to_string().as_bytes()).unwrap()) }
    })
    .await
    .unwrap_err();
    assert!(error.message().contains("matches multiple releases"));
    assert_eq!(error.hint(), Some("Pass the release UUID instead."));
    assert_eq!(index, 2);
    let mut called = false;
    let result =
        release_lookup::resolve_with("https://linear.app/acme/project/title-a1b2c3d4e5f6", |_| {
            called = true;
            async {
                Err::<ResolveReleases, _>(linear_cli::error::Error::new(
                    "invalid URL unexpectedly fetched",
                ))
            }
        })
        .await;
    assert!(result.is_err());
    assert!(!called);
}

#[tokio::test]
async fn invalid_comment_pagination_reports_missing_or_repeated_cursor_without_partial_result() {
    // Named strict protocol boundary: source cannot terminate a repeated cursor.
    // These are Rust-only controls, not invented source-success goldens.
    let case = frozen("c051-json-two-pages");
    for repeated in [false, true] {
        let mut first = response(&case, 0);
        let mut second = response(&case, 1);
        first["data"]["document"]["comments"]["pageInfo"]["endCursor"] = if repeated {
            json!("repeat")
        } else {
            Value::Null
        };
        second["data"]["document"]["comments"]["pageInfo"] =
            json!({"hasNextPage":true,"endCursor":"repeat"});
        let mut index = 0;
        let error = document_view::all_comments_with("slug", |_| {
            let data = if index == 0 {
                first.clone()
            } else {
                second.clone()
            };
            index += 1;
            async move {
                Ok(parse_response::<GetDocumentWithComments>(data.to_string().as_bytes()).unwrap())
            }
        })
        .await
        .unwrap_err();
        assert_eq!(index, if repeated { 2 } else { 1 });
        assert_eq!(
            error.message(),
            if repeated {
                "Linear repeated a document comment pagination cursor"
            } else {
                "Linear reported more document comments but returned no pagination cursor"
            }
        );
    }
}
