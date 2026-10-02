//! Release lookup by name or version, across pages.
use linear_cli::{
    commands::release_lookup,
    graphql::{envelope::parse_response, operations::releases::*},
};
use serde_json::{Value, json};
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
