use std::collections::VecDeque;
use std::future::ready;

use linear_cli::auth::ApiKeyInput;
use linear_cli::error::{AppError, AppErrorKind};
use linear_cli::graphql::edit::Edit;
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::team_resolver::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables,
};
use linear_cli::refs::{WorkspaceScope, find_team, prepare_team_lookup, resolve_team};
use serde_json::{Value, json};

use super::{argument, case, expected_error};

const MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases.sha256"
));

fn pinned_case(raw: &str, hash: &str) -> Value {
    let spec = case(raw);
    let id = spec["id"].as_str().unwrap_or_else(|| panic!("E0 id"));
    let line = format!("{hash}  rust/parity/runner/f06-teamref-frozen-cases/{id}.json");
    assert!(MANIFEST.lines().any(|entry| entry == line), "{id} pin");
    spec
}

fn scope_for<'a>(spec: &'a Value, key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    let argv = spec["argv"].as_array().unwrap_or_else(|| panic!("E0 argv"));
    let cli = argv
        .windows(2)
        .find(|pair| pair[0].as_str() == Some("--workspace"))
        .and_then(|pair| pair[1].as_str());
    WorkspaceScope {
        cli_workspace: cli,
        sourced_workspace: spec["env"]["LINEAR_WORKSPACE"].as_str(),
        default_workspace: (spec["configFixture"].as_str() == Some("default-acme"))
            .then_some("acme"),
        api_key: key,
    }
}

fn prepared_for<'a>(
    spec: &Value,
    key: &'a ApiKeyInput<'a>,
) -> linear_cli::refs::PreparedTeamLookup {
    prepare_team_lookup(argument(spec), &scope_for(spec, key))
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
}

fn resolve_response(spec: &Value) -> ResolveTeam {
    let step = &spec["graphql"]["groups"][0]["steps"][0];
    let mut data = step["response"]["data"].clone();
    if step["operation"]["variables"]["isUuid"] == true {
        data["teamById"] = data["teams"].clone();
    }
    serde_json::from_value(data).unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
}

fn expected_variables(spec: &Value) -> Value {
    spec["graphql"]["groups"][0]["steps"][0]["operation"]["variables"].clone()
}

fn request_variables<V: serde::Serialize>(request: &GraphQlRequest<V>) -> Value {
    serde_json::to_value(request)
        .unwrap_or_else(|error| panic!("typed request: {error}"))["variables"]
        .clone()
}

fn assert_error(spec: &Value, error: &AppError, kind: AppErrorKind) {
    let (message, suggestion) = expected_error(spec);
    assert_eq!(error.kind, kind, "{}", spec["id"]);
    assert_eq!(error.message, message, "{}", spec["id"]);
    assert_eq!(error.suggestion.as_deref(), suggestion, "{}", spec["id"]);
    assert_eq!(error.context, None, "{}", spec["id"]);
}

fn unexpected_all(
    _: GraphQlRequest<GetAllTeamsVariables>,
) -> std::future::Ready<Result<GetAllTeams, AppError>> {
    ready(Err(AppError::new(
        AppErrorKind::Invariant,
        "GetAllTeams was not expected",
    )))
}

#[tokio::test]
async fn e0_hits_keep_request_variables_and_winning_keys() {
    // Full pins: name 7f57a8e5ae28e9ce370deba531c2b70c1df99de45f6bd03b4a6e46cb044c9ea8,
    // key-before-name 277b55b9b80147871109805ae27e7c519fd1c68b88564c7a4b12de4f487a21d2,
    // nonuuid-name-over-first 46d17e7b700f419089551f6688aa4e5262958350674d614ea23223bd0ecd3fb7,
    // direct-uuid 638b403675c03d9562bb09855fb02f9a42a824c2278e1b315c5395b792a3cba7,
    // uuid-key-precedence 34b1b857b064b26ad0ed2d7aec1f6814d50fc6c565c70b1a4f08ad1ad02b3f64,
    // url-uuid c07c2bdfd0159a3f419327a09bbcaf71faee7faa952e9147498b9d947a2842bd,
    // url-segment-name a4a7ad04d7e0011325cc02633cfc043dedeb559ed295e370981fb0acbc8e4af3,
    // nonascii-name bc2f8d6e4c1cd812f5f88fbab75f10e73f1494c8ac665785fc0bc96c0753b03c.
    for (raw, hash) in [
        (
            e0_case!("f06e0-name"),
            "7f57a8e5ae28e9ce370deba531c2b70c1df99de45f6bd03b4a6e46cb044c9ea8",
        ),
        (
            e0_case!("f06e0-key-before-name"),
            "277b55b9b80147871109805ae27e7c519fd1c68b88564c7a4b12de4f487a21d2",
        ),
        (
            e0_case!("f06e0-nonuuid-name-over-first"),
            "46d17e7b700f419089551f6688aa4e5262958350674d614ea23223bd0ecd3fb7",
        ),
        (
            e0_case!("f06e0-direct-uuid"),
            "638b403675c03d9562bb09855fb02f9a42a824c2278e1b315c5395b792a3cba7",
        ),
        (
            e0_case!("f06e0-uuid-key-precedence"),
            "34b1b857b064b26ad0ed2d7aec1f6814d50fc6c565c70b1a4f08ad1ad02b3f64",
        ),
        (
            e0_case!("f06e0-url-uuid"),
            "c07c2bdfd0159a3f419327a09bbcaf71faee7faa952e9147498b9d947a2842bd",
        ),
        (
            e0_case!("f06e0-url-segment-name"),
            "a4a7ad04d7e0011325cc02633cfc043dedeb559ed295e370981fb0acbc8e4af3",
        ),
        (
            e0_case!("f06e0-nonascii-name"),
            "bc2f8d6e4c1cd812f5f88fbab75f10e73f1494c8ac665785fc0bc96c0753b03c",
        ),
    ] {
        let spec = pinned_case(raw, hash);
        let absent = ApiKeyInput::Absent;
        let prepared = prepared_for(&spec, &absent);
        let response = resolve_response(&spec);
        let expected = expected_variables(&spec);
        let found = resolve_team(
            &prepared,
            |request| {
                assert_eq!(request.operation_name.as_deref(), Some("ResolveTeam"));
                assert_eq!(request_variables(&request), expected, "{}", spec["id"]);
                ready(Ok(response))
            },
            unexpected_all,
        )
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]));
        let steps = spec["graphql"]["groups"][0]["steps"]
            .as_array()
            .unwrap_or_else(|| panic!("E0 steps"));
        let team_key = steps
            .iter()
            .find(|step| step["id"] == "empty-members")
            .and_then(|step| step["operation"]["variables"]["teamKey"].as_str())
            .unwrap_or_else(|| panic!("{} E0 teamKey", spec["id"]));
        assert_eq!(found.key, team_key, "{}", spec["id"]);
    }
}

#[tokio::test]
async fn e0_blank_and_ambiguous_errors_are_exact() {
    // blank-FEFF 15e8963687341507ee292a3184b771cd57692149d3173803a1f9a40140c806dc,
    // blank-space 3597349cbf9a8800da190f607824f302027d823f59d52872e4f1ecbfb7f4517c.
    for (raw, hash) in [
        (
            e0_case!("f06e0-blank-feff-before-key"),
            "15e8963687341507ee292a3184b771cd57692149d3173803a1f9a40140c806dc",
        ),
        (
            e0_case!("f06e0-blank-space-before-key"),
            "3597349cbf9a8800da190f607824f302027d823f59d52872e4f1ecbfb7f4517c",
        ),
    ] {
        let spec = pinned_case(raw, hash);
        let absent = ApiKeyInput::Absent;
        let error = prepare_team_lookup(argument(&spec), &scope_for(&spec, &absent))
            .err()
            .unwrap_or_else(|| panic!("{} should be blank", spec["id"]));
        assert_error(&spec, &error, AppErrorKind::Validation);
    }

    // ambiguity 0051c1c40d681c0a83c62f1aae195c0530a9bcbf93177b95a44f66cfd713baea,
    // URL ambiguity 532e642da580033f234bb94a87a5a9f83d29b2c2872d7a4ba97f8010a5b5441c.
    for (raw, hash) in [
        (
            e0_case!("f06e0-ambiguous-name"),
            "0051c1c40d681c0a83c62f1aae195c0530a9bcbf93177b95a44f66cfd713baea",
        ),
        (
            e0_case!("f06e0-url-ambiguity-key"),
            "532e642da580033f234bb94a87a5a9f83d29b2c2872d7a4ba97f8010a5b5441c",
        ),
    ] {
        let spec = pinned_case(raw, hash);
        let absent = ApiKeyInput::Absent;
        let prepared = prepared_for(&spec, &absent);
        let expected = expected_variables(&spec);
        let response = resolve_response(&spec);
        let error = find_team(&prepared, |request| {
            assert_eq!(request_variables(&request), expected);
            ready(Ok(response))
        })
        .await
        .err()
        .unwrap_or_else(|| panic!("{} should be ambiguous", spec["id"]));
        assert_error(&spec, &error, AppErrorKind::Validation);
    }
}

async fn assert_e0_miss(raw: &str, hash: &str) {
    let spec = pinned_case(raw, hash);
    let absent = ApiKeyInput::Absent;
    let prepared = prepared_for(&spec, &absent);
    let resolve_response = resolve_response(&spec);
    let mut pages = VecDeque::new();
    for step in spec["graphql"]["groups"][0]["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("E0 steps"))
        .iter()
        .skip(1)
    {
        pages.push_back((
            step["operation"]["variables"].clone(),
            serde_json::from_value::<GetAllTeams>(step["response"]["data"].clone())
                .unwrap_or_else(|error| panic!("{}: {error}", spec["id"])),
        ));
    }
    let expected_page_count = pages.len();
    let mut seen_pages = 0;
    let expected_resolve = expected_variables(&spec);
    let error = resolve_team(
        &prepared,
        |request| {
            assert_eq!(request_variables(&request), expected_resolve);
            ready(Ok(resolve_response))
        },
        |request| {
            let (expected_vars, response) = pages
                .pop_front()
                .unwrap_or_else(|| panic!("{} unexpected page", spec["id"]));
            assert_eq!(request.operation_name.as_deref(), Some("GetAllTeams"));
            assert_eq!(
                request_variables(&request),
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
    assert_error(&spec, &error, AppErrorKind::NotFound);
}

#[tokio::test]
async fn e0_misses_fetch_every_page_and_keep_original_errors() {
    // empty 62881fed7499d2b91f9e878e701167ca3c73b3c8b7a912eee43ffe6b46f35bff,
    // two pages 86b3a176a885973a0660a1312588487941d08cf3f0a69e2408b6aa29d1455c32,
    // explicit null 7516ac7322c69e1d030c34c7c67a6c142c3bf2bd594d331efb57170062ddcb0a,
    // original URL c126a5d46ca6d0f0255a2b327de9be542acc4007fa586147b3d06b10918017ae,
    // untrimmed 629ec76174d75be4378d79dca34c537f14f7b6e2733e08b0a8f239b48bfd0248,
    // NEL 5c905770c84da5ad816c2730a19ebe7e0250b75473b31d8db11147d110c21539.
    for (raw, hash) in [
        (
            e0_case!("f06e0-miss-empty"),
            "62881fed7499d2b91f9e878e701167ca3c73b3c8b7a912eee43ffe6b46f35bff",
        ),
        (
            e0_case!("f06e0-miss-two-pages"),
            "86b3a176a885973a0660a1312588487941d08cf3f0a69e2408b6aa29d1455c32",
        ),
        (
            e0_case!("f06e0-miss-null-cursor"),
            "7516ac7322c69e1d030c34c7c67a6c142c3bf2bd594d331efb57170062ddcb0a",
        ),
        (
            e0_case!("f06e0-url-miss-original"),
            "c126a5d46ca6d0f0255a2b327de9be542acc4007fa586147b3d06b10918017ae",
        ),
        (
            e0_case!("f06e0-untrimmed-text"),
            "629ec76174d75be4378d79dca34c537f14f7b6e2733e08b0a8f239b48bfd0248",
        ),
        (
            e0_case!("f06e0-nel-nonblank"),
            "5c905770c84da5ad816c2730a19ebe7e0250b75473b31d8db11147d110c21539",
        ),
    ] {
        assert_e0_miss(raw, hash).await;
    }
}

#[tokio::test]
async fn e0_graphql_failures_pass_through_without_context() {
    // ResolveTeam 85cb22d88bab5e7263f0a6423f5a495c64b6f5e000c1b739829a88c326940dc9,
    // GetAllTeams c7398fac85f9c848403c510b8362ee962181c6daab5de2cd7928e8c8df141961.
    let spec = pinned_case(
        e0_case!("f06e0-resolve-error"),
        "85cb22d88bab5e7263f0a6423f5a495c64b6f5e000c1b739829a88c326940dc9",
    );
    let absent = ApiKeyInput::Absent;
    let prepared = prepared_for(&spec, &absent);
    let (message, _) = expected_error(&spec);
    let error = resolve_team(
        &prepared,
        |_| ready(Err(AppError::new(AppErrorKind::GraphQl, message))),
        unexpected_all,
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("ResolveTeam failure"));
    assert_error(&spec, &error, AppErrorKind::GraphQl);

    let spec = pinned_case(
        e0_case!("f06e0-all-teams-error"),
        "c7398fac85f9c848403c510b8362ee962181c6daab5de2cd7928e8c8df141961",
    );
    let prepared = prepared_for(&spec, &absent);
    let response = resolve_response(&spec);
    let (message, _) = expected_error(&spec);
    let error = resolve_team(
        &prepared,
        |_| ready(Ok(response)),
        |_| ready(Err(AppError::new(AppErrorKind::GraphQl, message))),
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("GetAllTeams failure"));
    assert_error(&spec, &error, AppErrorKind::GraphQl);
}

#[test]
fn operation_documents_and_cursor_wire_states_are_typed() {
    use cynic::QueryBuilder;
    let resolve = GraphQlRequest::with_variables(ResolveTeam::build(ResolveTeamVariables {
        reference: "eng".to_owned(),
        id: None,
        is_uuid: false,
    }));
    assert_eq!(resolve.operation_name.as_deref(), Some("ResolveTeam"));
    assert!(resolve.query.contains("$id: ID,"));
    assert!(resolve.query.contains("$isUuid: Boolean!"));
    assert!(resolve.query.contains("or:"));
    assert!(resolve.query.contains("key: {eqIgnoreCase: $reference}"));
    assert!(resolve.query.contains("name: {eqIgnoreCase: $reference}"));
    assert!(resolve.query.contains("teamById: teams"));
    assert!(resolve.query.contains("id: {eq: $id}"));
    assert!(resolve.query.contains("@include(if: $isUuid)"));
    assert_eq!(
        request_variables(&resolve),
        json!({"reference":"eng","id":null,"isUuid":false})
    );

    for (after, expected) in [
        (Edit::Unchanged, json!({"first":100})),
        (Edit::Clear, json!({"first":100,"after":null})),
        (Edit::Set(String::new()), json!({"first":100,"after":""})),
        (
            Edit::Set("next".to_owned()),
            json!({"first":100,"after":"next"}),
        ),
    ] {
        let request = GraphQlRequest::with_variables(GetAllTeams::build(GetAllTeamsVariables {
            first: Some(100),
            after,
        }));
        assert_eq!(request.operation_name.as_deref(), Some("GetAllTeams"));
        assert!(request.query.contains("$after: String)"));
        assert_eq!(request_variables(&request), expected);
    }
}

#[tokio::test]
async fn disjoint_alias_prioritizes_key_then_id_then_name() {
    let absent = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: &absent,
    };
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
async fn repeated_cursor_fails_without_partial_result() {
    let absent = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: &absent,
    };
    let prepared = prepare_team_lookup("Unknown", &scope).unwrap_or_else(|error| panic!("{error}"));
    for cursor in [None, Some(String::new()), Some("repeat".to_owned())] {
        let empty_resolve: ResolveTeam = serde_json::from_value(json!({"teams":{"nodes":[]}}))
            .unwrap_or_else(|error| panic!("empty ResolveTeam: {error}"));
        let mut pages = 0;
        let error = resolve_team(
            &prepared,
            |_| ready(Ok(empty_resolve)),
            |request| {
                pages += 1;
                let expected_after = if pages == 1 {
                    json!({"first":100})
                } else {
                    json!({"first":100,"after":cursor})
                };
                assert_eq!(request_variables(&request), expected_after);
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
        .unwrap_or_else(|| panic!("repeated cursor should fail"));
        assert_eq!(pages, 2);
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert_eq!(
            error.message,
            "Linear repeated a team pagination cursor on page 2"
        );
        assert_eq!(error.suggestion.as_deref(), Some("Retry the command."));
        assert_eq!(error.context, None);
    }
}

#[tokio::test]
async fn later_page_failure_passes_through_without_partial_result() {
    let absent = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: &absent,
    };
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
        |request| {
            calls += 1;
            if calls == 1 {
                assert_eq!(request_variables(&request), json!({"first":100}));
                ready(Ok(page.clone()))
            } else {
                assert_eq!(
                    request_variables(&request),
                    json!({"first":100,"after":"next"})
                );
                ready(Err(AppError::new(
                    AppErrorKind::GraphQl,
                    "later page failed",
                )))
            }
        },
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("later page should fail"));
    assert_eq!(calls, 2);
    assert_eq!(error.kind, AppErrorKind::GraphQl);
    assert_eq!(error.message, "later page failed");
    assert_eq!(error.context, None);
}

#[tokio::test]
async fn strict_malformed_team_decode_is_an_uncontextualized_difference() {
    // C016E0 c016-team-malformed binds Deno's resolver-level malformed-team
    // text; B deliberately keeps Cynic's strict typed decode instead.
    let body =
        br#"{"data":{"teams":{"nodes":[{"id":"team-eng","key":null,"name":"Engineering"}]}}}"#;
    let failure = parse_response::<ResolveTeam>(body)
        .err()
        .unwrap_or_else(|| panic!("Cynic should reject null key"));
    let app = AppError::from(failure);
    assert_eq!(app.kind, AppErrorKind::Invariant);
    assert_eq!(app.context, None);

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
    let scope = WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: &absent,
    };
    let prepared = prepare_team_lookup("ENG", &scope).unwrap_or_else(|error| panic!("{error}"));
    let original_message = app.message.clone();
    let error = find_team(&prepared, |_| ready(Err(app)))
        .await
        .err()
        .unwrap_or_else(|| panic!("decode failure should pass through"));
    assert_eq!(error.kind, AppErrorKind::Invariant);
    assert_eq!(error.message, original_message);
    assert_eq!(error.context, None);
}
