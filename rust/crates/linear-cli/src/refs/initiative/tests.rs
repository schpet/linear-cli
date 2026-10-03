use std::cell::RefCell;
use std::future::ready;

use serde_json::json;

use super::*;
use crate::auth::ApiKeyInput;
use crate::error::Error;
use crate::graphql::envelope::parse_response;
use crate::refs::resolve_document_reference;
use crate::refs::test_support::absent_scope;

const ID: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";
#[test]
fn comment_references_validate_urls_locally_and_reduce_only_document_urls() {
    let key = ApiKeyInput::Absent;
    let mut local = absent_scope(&key);
    assert_eq!(
        prepare_initiative_lookup(ID, &local).expect("id"),
        InitiativeReference::Id(ID.into())
    );
    assert_eq!(
        prepare_initiative_lookup("Growth", &local).expect("name"),
        InitiativeReference::NameOrSlug("Growth".into())
    );
    assert_eq!(
        prepare_initiative_lookup(
            "https://linear.app/acme/initiative/Growth-ABC123DEF456",
            &local
        )
        .expect("url"),
        InitiativeReference::UrlSlug("abc123def456".into())
    );
    assert_eq!(
        resolve_document_reference(
            "https://linear.app/acme/document/Notes-ABC123DEF456",
            &local
        )
        .expect("url"),
        "abc123def456"
    );
    assert_eq!(
        resolve_document_reference("plain slug", &local).expect("plain"),
        "plain slug"
    );
    assert!(
        prepare_initiative_lookup(
            "https://linear.app/acme/document/Notes-abc123def456",
            &local
        )
        .expect_err("kind")
        .message()
        .contains("not an initiative URL")
    );
    assert!(
        resolve_document_reference("https://linear.app/acme/project/X-abc123def456", &local)
            .expect_err("kind")
            .message()
            .contains("not a document URL")
    );
    local.sourced_workspace = Some("other");
    assert!(
        prepare_initiative_lookup(
            "https://linear.app/acme/document/Notes-abc123def456",
            &local
        )
        .expect_err("workspace first")
        .message()
        .starts_with("That URL is for")
    );
}
#[tokio::test]
async fn strict_initiative_uuid_and_url_hits_never_fall_through_to_names() {
    let no_slug = |_| ready(Err(Error::not_found("unexpected", "slug")));
    let no_name = |_| ready(Err(Error::not_found("unexpected", "name")));
    assert_eq!(
        resolve_initiative_with(&InitiativeReference::Id(ID.into()), ID, no_slug, no_name)
            .await
            .expect("id"),
        ID
    );
    let sent = RefCell::new(Vec::new());
    let found = resolve_initiative_with(
        &InitiativeReference::UrlSlug("abc123def456".into()),
        "url",
        |query| {
            sent.borrow_mut()
                .push(serde_json::to_value(query.variables).expect("variables"));
            ready(
                parse_response(
                    json!({"data":{"initiatives":{"nodes":[{"id":ID}]}}})
                        .to_string()
                        .as_bytes(),
                )
                .map_err(Error::from),
            )
        },
        no_name,
    )
    .await
    .expect("url hit");
    assert_eq!(found, ID);
    assert_eq!(
        *sent.borrow(),
        vec![json!({"slugId":"abc123def456","includeArchived":false})]
    );
}
#[tokio::test]
async fn strict_initiative_slug_miss_then_exact_name_preserves_full_ambiguity() {
    let result = resolve_initiative_with(&InitiativeReference::NameOrSlug("Growth".into()), "Growth", |_| ready(parse_response(json!({"data":{"initiatives":{"nodes":[]}}}).to_string().as_bytes()).map_err(Error::from)), |query| {
        assert!(query.query.contains("eqIgnoreCase: $name"));
        assert_eq!(serde_json::to_value(query.variables).expect("variables"), json!({"name":"Growth"}));
        ready(parse_response(json!({"data":{"initiatives":{"nodes":[{"id":ID,"name":"Growth","slugId":"abc123def456"},{"id":"other","name":"growth","slugId":"other-slug"}]}}}).to_string().as_bytes()).map_err(Error::from))
    }).await.expect_err("ambiguous");
    assert_eq!(
        result.message(),
        format!(
            "Initiative \"Growth\" is ambiguous; it matches multiple initiatives:\n  Growth — abc123def456 ({ID})\n  growth — other-slug (other)"
        )
    );
    assert_eq!(
        result.hint(),
        Some("Pass the initiative's slug ID or UUID instead.")
    );
}
#[tokio::test]
async fn strict_initiative_missing_url_never_attempts_name_and_slug_errors_propagate() {
    let no_name = |_| ready(Err(Error::not_found("unexpected", "name")));
    let error = resolve_initiative_with(
        &InitiativeReference::UrlSlug("abc123def456".into()),
        "the URL",
        |_| {
            ready(
                parse_response(
                    json!({"data":{"initiatives":{"nodes":[]}}})
                        .to_string()
                        .as_bytes(),
                )
                .map_err(Error::from),
            )
        },
        no_name,
    )
    .await
    .expect_err("missing url");
    assert_eq!(error.message(), "Initiative not found: the URL");
    assert_eq!(
        error.hint(),
        Some(
            "The initiative in that URL may have been deleted, or be in a workspace this key cannot see."
        )
    );
    let error = resolve_initiative_with(
        &InitiativeReference::NameOrSlug("Growth".into()),
        "Growth",
        |_| ready(Err(Error::not_found("API", "unavailable"))),
        no_name,
    )
    .await
    .expect_err("slug API error");
    assert_eq!(error.message(), "API not found: unavailable");
}
