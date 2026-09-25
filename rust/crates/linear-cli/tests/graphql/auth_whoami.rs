use cynic::QueryBuilder;
use linear_cli::graphql::envelope::GraphQlRequest;
use linear_cli::graphql::operations::auth_whoami::AuthStatus;
use serde_json::{Value, json};

#[test]
fn auth_status_has_exact_name_selection_and_no_variables() {
    let request = GraphQlRequest::without_variables(AuthStatus::build(()));
    let envelope = serde_json::to_value(&request).expect("envelope serializes");
    let object = envelope.as_object().expect("object");
    assert_eq!(object.get("operationName"), Some(&json!("AuthStatus")));
    assert!(!object.contains_key("variables"));
    let query = object.get("query").and_then(Value::as_str).expect("query");
    let fields = [
        "viewer",
        "id",
        "name",
        "displayName",
        "email",
        "admin",
        "guest",
        "organization",
        "name",
        "urlKey",
        "logoUrl",
    ];
    let mut rest = query;
    for field in fields {
        let (_, tail) = rest.split_once(field).expect("field appears in order");
        rest = tail;
    }
    assert!(!query.contains("__typename"));
}

#[test]
fn auth_status_decodes_nullable_logo_without_defaults() {
    let data = serde_json::from_value::<AuthStatus>(json!({
        "viewer": {
            "id": "user-1", "name": "Alice", "displayName": "Ali", "email": "alice@example.invalid",
            "admin": false, "guest": false,
            "organization": {"name": "Example", "urlKey": "acme", "logoUrl": null}
        }
    }))
    .expect("schema-valid response");
    assert_eq!(data.viewer.organization.logo_url, None);
    assert_eq!(data.viewer.display_name, "Ali");
    let error = serde_json::from_value::<AuthStatus>(json!({"viewer": {"name": "Alice"}}))
        .expect_err("missing non-null fields reject");
    assert!(error.to_string().contains("missing field"));
}
