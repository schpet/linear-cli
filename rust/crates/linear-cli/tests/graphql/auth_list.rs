use cynic::QueryBuilder;
use linear_cli::graphql::envelope::GraphQlRequest;
use linear_cli::graphql::operations::auth_list::AuthListViewer;
use serde_json::json;

#[test]
fn auth_list_viewer_envelope_is_exact_and_has_no_variables() {
    let request = GraphQlRequest::without_variables(AuthListViewer::build(()));
    let envelope = serde_json::to_value(&request).expect("envelope serializes");
    assert_eq!(
        envelope,
        json!({
            "query": "query AuthListViewer {\n  viewer {\n    name\n    email\n    organization {\n      name\n      urlKey\n    }\n  }\n}\n",
            "operationName": "AuthListViewer",
        })
    );
}

#[test]
fn auth_list_viewer_decodes_selection_and_rejects_null_viewer() {
    let data = serde_json::from_value::<AuthListViewer>(json!({
        "viewer": {
            "name": "Alice", "email": "alice@example.invalid",
            "organization": {"name": "Acme", "urlKey": "acme"}
        }
    }))
    .expect("schema-valid response");
    assert_eq!(data.viewer.organization.url_key, "acme");
    let error = serde_json::from_value::<AuthListViewer>(json!({"viewer": null}))
        .expect_err("non-null viewer");
    assert!(error.to_string().contains("invalid type: null"));
}
