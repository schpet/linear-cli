use std::collections::VecDeque;
use std::future::ready;

use serde_json::{Value, json};

use super::*;
use crate::auth::ApiKeyInput;
use crate::error::ErrorKind;
use crate::graphql::envelope::{GraphQlRequest, parse_response};
use crate::graphql::operations::team::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables,
};
use crate::refs::WorkspaceScope;
use crate::refs::test_support::{absent_scope, argument, case, expected_error};

fn scope_for<'a>(spec: &'a Value, key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    WorkspaceScope {
        cli_workspace: spec["cli_workspace"].as_str(),
        sourced_workspace: spec["workspace_env"].as_str(),
        default_workspace: spec["default_workspace"].as_str(),
        api_key: key.clone(),
    }
}

fn prepared_for<'a>(spec: &Value, key: &'a ApiKeyInput<'a>) -> PreparedTeamLookup {
    prepare_team_lookup(argument(spec), &scope_for(spec, key))
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
}

fn resolve_response(spec: &Value) -> ResolveTeam {
    let step = &spec["steps"][0];
    let mut data = step["data"].clone();
    if step["variables"]["isUuid"] == true {
        data["teamById"] = data["teams"].clone();
    }
    serde_json::from_value(data).unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
}

fn expected_variables(spec: &Value) -> Value {
    spec["steps"][0]["variables"].clone()
}

fn request_variables<V: serde::Serialize>(variables: &V) -> Value {
    serde_json::to_value(variables).unwrap_or_else(|error| panic!("typed variables: {error}"))
}

fn assert_error(spec: &Value, error: &Error, kind: ErrorKind) {
    let (message, suggestion) = expected_error(spec);
    assert_eq!(error.kind(), kind, "{}", spec["id"]);
    assert_eq!(error.message(), message, "{}", spec["id"]);
    assert_eq!(error.hint(), suggestion, "{}", spec["id"]);
    assert_eq!(error.to_string(), error.message(), "{}", spec["id"]);
}

fn unexpected_all(_: GetAllTeamsVariables) -> std::future::Ready<Result<GetAllTeams, Error>> {
    ready(Err(Error::new("GetAllTeams was not expected")))
}

#[tokio::test]
async fn hits_keep_request_variables_and_winning_keys() {
    for name in [
        "name",
        "key-before-name",
        "nonuuid-name-over-first",
        "direct-uuid",
        "uuid-key-precedence",
        "url-uuid",
        "url-segment-name",
        "nonascii-name",
    ] {
        let spec = case(name);
        let absent = ApiKeyInput::Absent;
        let prepared = prepared_for(&spec, &absent);
        let response = resolve_response(&spec);
        let expected = expected_variables(&spec);
        let found = resolve_team(
            &prepared,
            |variables| {
                assert_eq!(request_variables(&variables), expected, "{}", spec["id"]);
                ready(Ok(response))
            },
            unexpected_all,
        )
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]));
        let steps = spec["steps"]
            .as_array()
            .unwrap_or_else(|| panic!("{} steps", spec["id"]));
        let team_key = steps
            .iter()
            .find(|step| step["name"] == "empty-members")
            .and_then(|step| step["variables"]["teamKey"].as_str())
            .unwrap_or_else(|| panic!("{} teamKey", spec["id"]));
        assert_eq!(found.key, team_key, "{}", spec["id"]);
    }
}

#[tokio::test]
async fn blank_and_ambiguous_errors_are_exact() {
    for name in ["blank-space-before-key", "blank-nel"] {
        let spec = case(name);
        let absent = ApiKeyInput::Absent;
        let error = prepare_team_lookup(argument(&spec), &scope_for(&spec, &absent))
            .err()
            .unwrap_or_else(|| panic!("{} should be blank", spec["id"]));
        assert_error(&spec, &error, ErrorKind::Other);
    }

    for name in ["ambiguous-name", "url-ambiguity-key"] {
        let spec = case(name);
        let absent = ApiKeyInput::Absent;
        let prepared = prepared_for(&spec, &absent);
        let expected = expected_variables(&spec);
        let response = resolve_response(&spec);
        let error = find_team(&prepared, |variables| {
            assert_eq!(request_variables(&variables), expected);
            ready(Ok(response))
        })
        .await
        .err()
        .unwrap_or_else(|| panic!("{} should be ambiguous", spec["id"]));
        assert_error(&spec, &error, ErrorKind::Other);
    }
}

async fn assert_miss(name: &str) {
    let spec = case(name);
    let absent = ApiKeyInput::Absent;
    let prepared = prepared_for(&spec, &absent);
    let resolve_response = resolve_response(&spec);
    let mut pages = VecDeque::new();
    for step in spec["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{} steps", spec["id"]))
        .iter()
        .skip(1)
    {
        pages.push_back((
            step["variables"].clone(),
            serde_json::from_value::<GetAllTeams>(step["data"].clone())
                .unwrap_or_else(|error| panic!("{}: {error}", spec["id"])),
        ));
    }
    let expected_page_count = pages.len();
    let mut seen_pages = 0;
    let expected_resolve = expected_variables(&spec);
    let error = resolve_team(
        &prepared,
        |variables| {
            assert_eq!(request_variables(&variables), expected_resolve);
            ready(Ok(resolve_response))
        },
        |variables| {
            let (expected_vars, response) = pages
                .pop_front()
                .unwrap_or_else(|| panic!("{} unexpected page", spec["id"]));
            assert_eq!(
                request_variables(&variables),
                expected_vars,
                "{} page",
                spec["id"]
            );
            seen_pages += 1;
            ready(Ok(response))
        },
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("{} should miss", spec["id"]));
    assert_eq!(seen_pages, expected_page_count, "{} page count", spec["id"]);
    assert!(pages.is_empty());
    assert_error(&spec, &error, ErrorKind::NotFound);
}

#[tokio::test]
async fn misses_fetch_every_page_before_failing() {
    for name in [
        "miss-empty",
        "miss-two-pages",
        "url-miss-keeps-input",
        "untrimmed-text",
    ] {
        assert_miss(name).await;
    }
}

#[tokio::test]
async fn graphql_failures_pass_through_without_context() {
    let spec = case("resolve-error");
    let absent = ApiKeyInput::Absent;
    let prepared = prepared_for(&spec, &absent);
    let (message, _) = expected_error(&spec);
    let error = resolve_team(
        &prepared,
        |_| ready(Err(Error::new(message))),
        unexpected_all,
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("ResolveTeam failure"));
    assert_error(&spec, &error, ErrorKind::Other);

    let spec = case("all-teams-error");
    let prepared = prepared_for(&spec, &absent);
    let response = resolve_response(&spec);
    let (message, _) = expected_error(&spec);
    let error = resolve_team(
        &prepared,
        |_| ready(Ok(response)),
        |_| ready(Err(Error::new(message))),
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("GetAllTeams failure"));
    assert_error(&spec, &error, ErrorKind::Other);
}

#[test]
fn requests_name_the_operation_and_omit_an_absent_cursor() {
    use cynic::QueryBuilder;
    let resolve = GraphQlRequest::new(ResolveTeam::build(ResolveTeamVariables {
        reference: "eng".to_owned(),
        id: None,
        is_uuid: false,
    }))
    .expect("variables serialize");
    assert_eq!(resolve.operation_name.as_deref(), Some("ResolveTeam"));
    assert_eq!(
        resolve.variables,
        Some(json!({"reference":"eng","id":null,"isUuid":false}))
    );

    for (after, expected) in [
        (None, json!({"first":100})),
        (Some("next".to_owned()), json!({"first":100,"after":"next"})),
    ] {
        let request = GraphQlRequest::new(GetAllTeams::build(GetAllTeamsVariables {
            first: Some(100),
            after,
        }))
        .expect("variables serialize");
        assert_eq!(request.operation_name.as_deref(), Some("GetAllTeams"));
        assert_eq!(request.variables, Some(expected));
    }
}

#[tokio::test]
async fn disjoint_alias_prioritizes_key_then_id_then_name() {
    let absent = ApiKeyInput::Absent;
    let scope = absent_scope(&absent);
    let uuid = "01234567-89ab-4cde-8f01-23456789abcd";
    let prepared = prepare_team_lookup(uuid, &scope).unwrap_or_else(|error| panic!("{error}"));
    let by_key = json!({"id":"key","key":uuid,"name":"Other"});
    let by_name = json!({"id":"name","key":"NME","name":uuid});
    let by_id = json!({"id":"id","key":"ID","name":"ID Team"});
    for (nodes, expected_key) in [(vec![by_name.clone(), by_key], uuid), (vec![by_name], "ID")] {
        let response: ResolveTeam = serde_json::from_value(json!({
            "teams":{"nodes":nodes},
            "teamById":{"nodes":[by_id.clone()]}
        }))
        .unwrap_or_else(|error| panic!("disjoint alias: {error}"));
        let team = find_team(&prepared, |_| ready(Ok(response)))
            .await
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("disjoint alias should match"));
        assert_eq!(team.key, expected_key);
    }
}

#[tokio::test]
async fn unusable_cursor_fails_without_partial_result() {
    let absent = ApiKeyInput::Absent;
    let scope = absent_scope(&absent);
    let prepared = prepare_team_lookup("Unknown", &scope).unwrap_or_else(|error| panic!("{error}"));
    for (cursor, expected_pages, message) in [
        (
            None,
            1,
            "Linear reported more results but sent no cursor to fetch them",
        ),
        (
            Some(String::new()),
            1,
            "Linear reported more results but sent no cursor to fetch them",
        ),
        (
            Some("repeat".to_owned()),
            2,
            "Linear sent the same pagination cursor twice",
        ),
    ] {
        let empty_resolve: ResolveTeam = serde_json::from_value(json!({"teams":{"nodes":[]}}))
            .unwrap_or_else(|error| panic!("empty ResolveTeam: {error}"));
        let mut pages = 0;
        let error = resolve_team(
            &prepared,
            |_| ready(Ok(empty_resolve)),
            |variables| {
                pages += 1;
                let expected_after = if pages == 1 {
                    json!({"first":100})
                } else {
                    json!({"first":100,"after":cursor})
                };
                assert_eq!(request_variables(&variables), expected_after);
                let response: GetAllTeams = serde_json::from_value(json!({
                    "teams": {
                        "nodes": [{"id":"partial","key":"PART","name":"Partial"}],
                        "pageInfo": {"hasNextPage":true,"endCursor":cursor}
                    }
                }))
                .unwrap_or_else(|error| panic!("repeat page: {error}"));
                ready(Ok(response))
            },
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("unusable cursor should fail"));
        assert_eq!(pages, expected_pages);
        assert_eq!(error.message(), message);
        assert_eq!(error.hint(), Some("Retry the command."));
    }
}

#[tokio::test]
async fn later_page_failure_passes_through_without_partial_result() {
    let absent = ApiKeyInput::Absent;
    let scope = absent_scope(&absent);
    let prepared = prepare_team_lookup("Unknown", &scope).unwrap_or_else(|error| panic!("{error}"));
    let empty_resolve: ResolveTeam = serde_json::from_value(json!({"teams":{"nodes":[]}}))
        .unwrap_or_else(|error| panic!("empty ResolveTeam: {error}"));
    let page: GetAllTeams = serde_json::from_value(json!({
        "teams": {
            "nodes": [{"id":"partial","key":"PART","name":"Partial"}],
            "pageInfo": {"hasNextPage":true,"endCursor":"next"}
        }
    }))
    .unwrap_or_else(|error| panic!("first page: {error}"));
    let mut calls = 0;
    let error = resolve_team(
        &prepared,
        |_| ready(Ok(empty_resolve)),
        |variables| {
            calls += 1;
            if calls == 1 {
                assert_eq!(request_variables(&variables), json!({"first":100}));
                ready(Ok(page.clone()))
            } else {
                assert_eq!(
                    request_variables(&variables),
                    json!({"first":100,"after":"next"})
                );
                ready(Err(Error::new("later page failed")))
            }
        },
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("later page should fail"));
    assert_eq!(calls, 2);
    assert_eq!(error.message(), "later page failed");
    assert_eq!(error.to_string(), error.message());
}

#[tokio::test]
async fn malformed_team_decode_fails_strictly_without_context() {
    // A team with a null key fails the typed decode rather than being skipped.
    let body =
        br#"{"data":{"teams":{"nodes":[{"id":"team-eng","key":null,"name":"Engineering"}]}}}"#;
    let failure = parse_response::<ResolveTeam>(body)
        .err()
        .unwrap_or_else(|| panic!("Cynic should reject null key"));
    let app = Error::from(failure);
    assert_eq!(app.to_string(), app.message());

    for (label, body) in [
        (
            "missing id",
            br#"{"data":{"teams":{"nodes":[{"key":"ENG","name":"Engineering"}]}}}"#
                .as_slice(),
        ),
        (
            "malformed alias after valid key",
            br#"{"data":{"teams":{"nodes":[{"id":"team-eng","key":"ENG","name":"Engineering"}]},"teamById":{"nodes":[{"id":"other","key":null,"name":"Other"}]}}}"#
                .as_slice(),
        ),
    ] {
        assert!(parse_response::<ResolveTeam>(body).is_err(), "{label}");
    }
    let malformed_page = br#"{"data":{"teams":{"nodes":[{"id":"team-eng","key":null,"name":"Engineering"}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#;
    assert!(parse_response::<GetAllTeams>(malformed_page).is_err());

    let absent = ApiKeyInput::Absent;
    let scope = absent_scope(&absent);
    let prepared = prepare_team_lookup("ENG", &scope).unwrap_or_else(|error| panic!("{error}"));
    let original_message = app.message().to_owned();
    let error = find_team(&prepared, |_| ready(Err(app)))
        .await
        .err()
        .unwrap_or_else(|| panic!("decode failure should pass through"));
    assert_eq!(error.message(), original_message);
    assert_eq!(error.to_string(), error.message());
}
