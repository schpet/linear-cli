use super::{LegacyRequest, ResponseError, graphql_message, is_not_found, parse_response};
use crate::graphql::operations::agent_session::GetAgentSessionDetails;
use crate::graphql::operations::issue_update::UpdateIssue;
use serde_json::Value;

const SUCCESS_BODY: &str = r#"{"data":{"issueUpdate":{"success":true,"issue":{"id":"i1","identifier":"ENG-1","url":"https://linear.app/x/issue/ENG-1","title":"T"}}}}"#;

#[test]
fn data_only_yields_typed_data() {
    let data: UpdateIssue = parse_response(SUCCESS_BODY.as_bytes()).expect("data");
    assert!(data.issue_update.success);
    let issue = data.issue_update.issue.expect("issue");
    assert_eq!(issue.identifier, "ENG-1");
}

#[test]
fn errors_only_classify_as_graphql_without_partial_data() {
    let body = r#"{"errors":[{"message":"Entity not found: Issue","path":["issueUpdate"],"locations":[{"line":2,"column":3}],"extensions":{"type":"invalid input","userPresentableMessage":"Could not find referenced Issue.","code":"INVALID_INPUT"}}]}"#;
    let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err("errors");
    match &error {
        ResponseError::GraphQl {
            errors,
            partial_data,
        } => {
            assert!(!partial_data);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].message, "Entity not found: Issue");
            let extensions = errors[0].extensions.as_ref().expect("extensions");
            assert_eq!(
                extensions["userPresentableMessage"],
                Value::from("Could not find referenced Issue.")
            );
            assert_eq!(extensions["code"], Value::from("INVALID_INPUT"));
            assert!(errors[0].path.is_some());
            assert!(errors[0].locations.is_some());
            assert_eq!(
                graphql_message(errors).as_deref(),
                Some("Could not find referenced Issue.")
            );
            assert!(is_not_found(errors));
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
    assert_eq!(error.to_string(), "Could not find referenced Issue.");
    let app: crate::error::Error = error.into();
    assert_eq!(app.to_string(), "Could not find referenced Issue.");
}

#[test]
fn errors_with_partial_data_are_still_errors() {
    let body = r#"{"data":{"issueUpdate":{"success":true,"issue":null}},"errors":[{"message":"Something failed"}]}"#;
    let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err("errors");
    match error {
        ResponseError::GraphQl {
            errors,
            partial_data,
        } => {
            assert!(partial_data);
            assert_eq!(
                graphql_message(&errors).as_deref(),
                Some("Something failed")
            );
            assert!(!is_not_found(&errors));
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
}

#[test]
fn message_falls_back_when_presentable_message_is_missing_or_empty() {
    let body =
        r#"{"errors":[{"message":"raw message","extensions":{"userPresentableMessage":""}}]}"#;
    let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err("errors");
    assert_eq!(error.to_string(), "raw message");
    let body = r#"{"errors":[{"message":"no extensions"}]}"#;
    let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err("errors");
    assert_eq!(error.to_string(), "no extensions");
}

#[test]
fn null_data_without_errors_is_missing_data() {
    for body in [r#"{"data":null}"#, "{}", r#"{"data":null,"errors":[]}"#] {
        let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err(body);
        assert!(
            matches!(error, ResponseError::MissingData),
            "{body}: {error:?}"
        );
    }
}

#[test]
fn empty_errors_array_with_data_is_data() {
    let body = r#"{"data":{"issueUpdate":{"success":true,"issue":null}},"errors":[]}"#;
    let data: UpdateIssue = parse_response(body.as_bytes()).expect("data");
    assert!(data.issue_update.success);
}

#[test]
fn errors_with_null_root_field_incompatible_with_the_type_are_graphql_errors() {
    // `issueUpdate` is non-null in `UpdateIssue`; decoding it first would have
    // reported a shape failure instead of the error the server actually sent.
    let body = r#"{"data":{"issueUpdate":null},"errors":[{"message":"Entity not found: Issue","path":["issueUpdate"],"extensions":{"userPresentableMessage":"Could not find referenced Issue.","code":"INVALID_INPUT"}}]}"#;
    let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err("errors");
    match &error {
        ResponseError::GraphQl {
            errors,
            partial_data,
        } => {
            assert!(partial_data);
            assert_eq!(errors.len(), 1);
            assert!(is_not_found(errors));
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
    assert_eq!(error.to_string(), "Could not find referenced Issue.");

    let body = r#"{"data":{"agentSession":null},"errors":[{"message":"Entity not found: AgentSession","path":["agentSession"]}]}"#;
    let error = parse_response::<GetAgentSessionDetails>(body.as_bytes()).expect_err("errors");
    match &error {
        ResponseError::GraphQl {
            errors,
            partial_data,
        } => {
            assert!(partial_data);
            assert_eq!(errors[0].message, "Entity not found: AgentSession");
            assert!(is_not_found(errors));
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
    let app: crate::error::Error = error.into();
    assert_eq!(app.to_string(), "Entity not found: AgentSession");
}

#[test]
fn errors_with_null_data_are_graphql_errors_without_partial_data() {
    let body = r#"{"data":null,"errors":[{"message":"Unauthorized"}]}"#;
    let error = parse_response::<GetAgentSessionDetails>(body.as_bytes()).expect_err("errors");
    match error {
        ResponseError::GraphQl {
            errors,
            partial_data,
        } => {
            assert!(!partial_data);
            assert_eq!(errors[0].message, "Unauthorized");
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
}

#[test]
fn malformed_json_syntax_classifies_as_malformed_json() {
    for body in [
        "<html>502</html>",
        "",
        r#"{"data":"#,
        r#"{"data":{"issueUpdate":{"success":true,}}}"#,
    ] {
        let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err(body);
        assert!(
            matches!(error, ResponseError::MalformedJson(_)),
            "{body}: {error:?}"
        );
        assert!(
            error
                .to_string()
                .starts_with("response body is not valid JSON: "),
            "{body}: {error}"
        );
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn well_formed_json_with_the_wrong_shape_is_unexpected_shape_not_malformed() {
    for (body, fragment) in [
        // Wrong scalar type inside the typed payload.
        (
            r#"{"data":{"issueUpdate":{"success":"yes"}}}"#,
            "expected a boolean",
        ),
        // A null non-null root field without any GraphQL error to explain it.
        (r#"{"data":{"issueUpdate":null}}"#, "null"),
        // Missing non-null root field.
        (r#"{"data":{}}"#, "missing field `issueUpdate`"),
        // Envelope-level shape failures: `errors` must be an array of objects
        // with a `message`, and the top level must be an object.
        (r#"{"errors":"boom"}"#, "expected a sequence"),
        (r#"{"errors":[{"code":1}]}"#, "missing field `message`"),
        ("[]", "expected struct ResponseEnvelope"),
        (r#""just a string""#, "expected struct ResponseEnvelope"),
    ] {
        let error = parse_response::<UpdateIssue>(body.as_bytes()).expect_err(body);
        assert!(
            matches!(error, ResponseError::UnexpectedShape(_)),
            "{body}: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.starts_with("response JSON did not match the expected operation shape: "),
            "{body}: {message}"
        );
        assert!(message.contains(fragment), "{body}: {message}");
        assert!(std::error::Error::source(&error).is_some());
        let app: crate::error::Error = error.into();
        assert_eq!(
            app.context("Failed to update issue").to_string(),
            format!("Failed to update issue: {message}")
        );
    }
}

#[test]
fn unknown_top_level_envelope_keys_are_tolerated() {
    let body = r#"{"data":{"issueUpdate":{"success":true,"issue":null}},"extensions":{"cost":1}}"#;
    let data: UpdateIssue = parse_response(body.as_bytes()).expect("data");
    assert!(data.issue_update.success);
}

#[test]
fn request_envelope_carries_query_variables_and_operation_name() {
    use cynic::QueryBuilder;

    use crate::graphql::operations::teams::{GetTeams, GetTeamsVariables};

    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: None,
    });
    let query = operation.query.clone();
    let request = LegacyRequest::with_variables(operation);
    assert_eq!(
        serde_json::to_value(&request).expect("request"),
        serde_json::json!({"query": query, "variables": {"first": 100}, "operationName": "GetTeams"})
    );
}

#[test]
fn request_envelope_without_variables_omits_the_variables_key() {
    let request = LegacyRequest::<()> {
        query: "query Viewer { viewer { id } }".to_owned(),
        variables: None,
        operation_name: Some("Viewer".to_owned()),
    };
    assert_eq!(
        serde_json::to_string(&request).expect("string"),
        r#"{"query":"query Viewer { viewer { id } }","operationName":"Viewer"}"#
    );
    let anonymous = LegacyRequest::<()> {
        query: "{ viewer { id } }".to_owned(),
        variables: None,
        operation_name: None,
    };
    assert_eq!(
        serde_json::to_string(&anonymous).expect("string"),
        r#"{"query":"{ viewer { id } }"}"#
    );
}

#[test]
fn raw_variables_keep_an_explicit_null() {
    // `linear api` forwards user variables untouched, including nulls.
    let request = LegacyRequest {
        query: "query ($after: String) { teams(after: $after) { nodes { id } } }".to_owned(),
        variables: Some(serde_json::json!({"first": 100, "after": null})),
        operation_name: None,
    };
    let text = serde_json::to_string(&request).expect("string");
    assert!(
        text.ends_with(r#""variables":{"first":100,"after":null}}"#),
        "{text}"
    );
}
