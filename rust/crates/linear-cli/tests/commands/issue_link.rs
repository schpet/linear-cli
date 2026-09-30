use linear_cli::cli;
use linear_cli::commands::issue_link;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::issue_link::AttachmentLinkURL;
use serde_json::{Value, json};

#[test]
fn url_input_is_case_sensitive_prefix_only_and_untouched() {
    for url in ["http://", "https://example.com/a?q=1#x", "http:// odd path"] {
        assert_eq!(
            issue_link::inputs("ENG-1", Some(url)).unwrap(),
            (Some("ENG-1"), url)
        );
        assert_eq!(issue_link::inputs(url, None).unwrap(), (None, url));
    }
    for bad in ["HTTPS://example.com", "ftp://x", " ENG-1", "", "https:/"] {
        let single = issue_link::inputs(bad, None).unwrap_err();
        assert_eq!(single.message, format!("Expected a URL but got '{bad}'"));
        let double = issue_link::inputs("ENG-1", Some(bad)).unwrap_err();
        assert_eq!(double.message, format!("Invalid URL: '{bad}'"));
        assert_eq!(
            double.suggestion.as_deref(),
            Some("Provide a URL starting with http:// or https://.")
        );
    }
}
#[test]
fn link_request_preserves_title_omission_and_exact_selections() {
    for title in [None, Some("Custom")] {
        let actual =
            serde_json::to_value(issue_link::request("uuid", "https://example.com", title))
                .unwrap();
        let mut variables = json!({"issueId":"uuid","url":"https://example.com"});
        if let Some(title) = title {
            variables["title"] = json!(title);
        }
        assert_eq!(actual["variables"], variables);
        assert_eq!(actual["operationName"], "AttachmentLinkURL");
        let compact = |s: &str| {
            s.chars()
                .filter(|c| !c.is_whitespace() && *c != ',')
                .collect::<String>()
        };
        assert_eq!(
            compact(actual["query"].as_str().unwrap()),
            compact(
                "mutation AttachmentLinkURL($issueId:String!,$url:String!,$title:String){attachmentLinkURL(issueId:$issueId,url:$url,title:$title){success attachment{id title url}}}"
            )
        );
    }
}
#[test]
fn link_payload_and_selected_attachment_fields_are_strict() {
    for payload in [
        json!(null),
        json!({}),
        json!({"success":true,"attachment":null}),
        json!({"success":false,"attachment":null}),
        json!({"success":true,"attachment":{"id":"x","url":"u"}}),
        json!({"success":true,"attachment":{"id":"x","title":9,"url":"u"}}),
    ] {
        assert!(
            parse_response::<AttachmentLinkURL>(
                json!({"data":{"attachmentLinkURL":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
    let good:AttachmentLinkURL=parse_response(br#"{"data":{"attachmentLinkURL":{"success":false,"attachment":{"id":"x","title":"","url":""}}}}"#).unwrap();
    assert!(!good.attachment_link_url.success);
}
#[test]
fn native_link_cli_rejects_empty_title_missing_url_and_json() {
    for args in [
        vec!["issue", "link"],
        vec!["issue", "link", "https://example.com", "--title", ""],
        vec!["issue", "link", "ENG-1", "https://example.com", "extra"],
        vec!["issue", "link", "https://example.com", "--json"],
    ] {
        assert!(
            cli::parse(
                &args
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }
}

#[tokio::test]
async fn link_uses_returned_title_and_reports_false_success_after_lookup() {
    // The shared public transport script asserts no retry/extra connections.
    // URL behavior is exercised through the same production submit entry point.
    for success in [true, false] {
        let (transport, server) = super::issue_relations::sequence(vec![
            json!({"data":{"issue":{"id":"uuid"}}}),
            json!({"data":{"attachmentLinkURL":{"success":success,"attachment":{"id":"a","title":"API title","url":"https://example.com"}}}}),
        ]);
        let result = issue_link::submit(
            &transport,
            "ENG-1",
            "https://example.com",
            Some("Input title"),
        )
        .await;
        if success {
            assert_eq!(result.unwrap(), "✓ Linked to ENG-1: API title\n".as_bytes());
        } else {
            assert_eq!(result.unwrap_err().message, "Failed to link URL to issue");
        }
        let requests: Vec<Value> = server.join().unwrap();
        assert_eq!(requests[0]["variables"], json!({"id":"ENG-1"}));
        assert_eq!(
            requests[1]["variables"],
            json!({"issueId":"uuid","url":"https://example.com","title":"Input title"})
        );
    }
}

#[tokio::test]
async fn link_empty_lookup_id_never_reaches_attachment_mutation() {
    let (transport, server) =
        super::issue_relations::sequence(vec![json!({"data":{"issue":{"id":""}}})]);
    let error = issue_link::submit(&transport, "ENG-1", "https://example.com", None)
        .await
        .unwrap_err();
    assert_eq!(error.message, "Issue not found: ENG-1");
    assert_eq!(server.join().unwrap().len(), 1);
}
