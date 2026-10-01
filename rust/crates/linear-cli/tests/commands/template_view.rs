use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::{Ready, ready};
use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Utc};
use linear_cli::auth::{CredentialStore, hydrate, parse_credentials};
use linear_cli::commands::template_view::{
    CONTEXT, TemplateReference, prepare, render_text, run_with, template_request,
};
use linear_cli::config::{
    ConfigInputs, ConfigOptions, OptionInputs, OsFamily, ProcessEnvSnapshot, RawConfigFile,
    SelectedEnv, TransportEnvInputs, parse_config_tier,
};
use linear_cli::error::{AppError, AppErrorKind};
use linear_cli::graphql::envelope::{GraphQlRequest, ResponseError, parse_response};
use linear_cli::graphql::operations::templates::{
    GetTemplate, GetTemplateVariables, GetTemplates, Template,
};
use linear_cli::graphql::transport::{RawHttpResponse, TransportFailure, classify_typed};
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde_json::{Value, json};

const BUG_ID: &str = "11111111-1111-4111-8111-111111111111";
const FOOTER: &str = "References are IDs. Map them with `linear team states`, `linear label list`, `linear user list`, or `linear project list`.";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-25T12:00:00Z")
        .expect("fixed clock")
        .with_timezone(&Utc)
}

fn bug_template_data() -> String {
    json!({
        "title": "Bug: ",
        "priority": 2,
        "estimate": 3,
        "labelIds": ["label-bug"],
        "stateId": "state-todo",
        "subIssueData": [
            {
                "title": "Write regression test",
                "description": "Cover the bug with a test",
                "labelIds": ["label-test"]
            },
            {"title": "Fix it"}
        ],
        "descriptionData": {
            "type": "doc",
            "content": [
                {"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "Steps to reproduce"}]},
                {"type": "ordered_list", "attrs": {"order": 1}, "content": [{"type": "list_item", "content": [{"type": "paragraph"}]}]},
                {"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "Expected"}]},
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "Describe the "},
                    {"type": "text", "text": "expected", "marks": [{"type": "bold"}]},
                    {"type": "text", "text": " behaviour."}
                ]}
            ]
        }
    })
    .to_string()
}

fn bug_template() -> Value {
    json!({
        "id": BUG_ID,
        "name": "Bug report",
        "description": "Standard bug intake",
        "type": "issue",
        "icon": null,
        "color": null,
        "hasFormFields": false,
        "lastAppliedAt": null,
        "sortOrder": 0,
        "createdAt": "2024-01-01T12:00:00.000Z",
        "updatedAt": "2024-01-02T12:00:00.000Z",
        "team": {"id": "team-eng-id", "key": "ENG", "name": "Engineering"},
        "inheritedFrom": null,
        "creator": {"id": "user-1", "name": "Sam"},
        "templateData": bug_template_data()
    })
}

fn kickoff_template() -> Value {
    json!({
        "id": "tpl-kickoff",
        "name": "Kickoff",
        "description": null,
        "type": "project",
        "icon": null,
        "color": null,
        "hasFormFields": true,
        "lastAppliedAt": null,
        "sortOrder": 0,
        "createdAt": "2024-01-01T12:00:00.000Z",
        "updatedAt": "2024-01-02T12:00:00.000Z",
        "team": null,
        "inheritedFrom": {"id": "tpl-parent", "name": "Parent kickoff"},
        "creator": null,
        "templateData": json!({
            "name": "Kickoff: ",
            "content": "## Goals\n\n## Milestones\n",
            "labelIds": [],
            "priority": 3,
            "custom": {"flag": true, "nested": {"depth": 2}}
        })
        .to_string()
    })
}

fn broken_template() -> Value {
    let mut broken = kickoff_template();
    broken["id"] = json!("tpl-broken");
    broken["name"] = json!("Broken");
    broken["templateData"] = json!("[1, 2]");
    broken
}

/// A template whose only interesting part is its inner `templateData` text.
fn with_data(template_data: &str) -> Value {
    let mut template = bug_template();
    template["templateData"] = json!(template_data);
    template
}

fn typed_one(template: Value) -> GetTemplate {
    parse_response(
        json!({"data": {"template": template}})
            .to_string()
            .as_bytes(),
    )
    .expect("typed GetTemplate")
}

fn typed_template(template: Value) -> Template {
    typed_one(template).template
}

fn typed_list(templates: Vec<Value>) -> GetTemplates {
    parse_response(
        json!({"data": {"templates": templates}})
            .to_string()
            .as_bytes(),
    )
    .expect("typed GetTemplates")
}

fn failure(status: u16, body: Value) -> TransportFailure {
    classify_typed::<GetTemplate>(RawHttpResponse {
        status: StatusCode::from_u16(status).expect("status"),
        headers: HeaderMap::from_iter([(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        )]),
        body: body.to_string().into_bytes(),
    })
    .expect_err("classified failure")
}

fn unexpected_one(
    _: GraphQlRequest<GetTemplateVariables>,
) -> Ready<Result<GetTemplate, TransportFailure>> {
    panic!("GetTemplate must not be requested")
}

fn unexpected_list(_: GraphQlRequest<()>) -> Ready<Result<GetTemplates, TransportFailure>> {
    panic!("GetTemplates must not be requested")
}

fn reference(text: &str) -> TemplateReference {
    TemplateReference::parse(text).unwrap_or_else(|error| panic!("{error}"))
}

/// View one template by ID in text mode at the fixed clock and UTC.
async fn view_text(template: Value) -> Result<String, AppError> {
    let response = typed_one(template);
    let output = run_with(
        &reference(BUG_ID),
        false,
        now,
        &Utc,
        |_| ready(Ok(response)),
        unexpected_list,
    )
    .await?;
    Ok(String::from_utf8(output).expect("UTF-8 output"))
}

/// The pre-fill lines of a text view whose data is `template_data`.
async fn pre_fills(template_data: &str) -> String {
    let text = view_text(with_data(template_data))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let start = text.find("Pre-fills:\n").expect("pre-fill heading") + "Pre-fills:\n".len();
    let end = text.rfind(&format!("\n\n{FOOTER}\n")).expect("footer");
    text[start..end].to_owned()
}

async fn view_error(template_data: &str) -> AppError {
    view_text(with_data(template_data))
        .await
        .expect_err("text view failure")
}

fn config_options(env: &[(&str, &str)]) -> ConfigOptions {
    let process = ConfigInputs {
        cwd: PathBuf::from("/repo"),
        os: OsFamily::Unix,
        process_env: env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    };
    let dotenv = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: vec![],
    };
    ConfigOptions::from_inputs(OptionInputs {
        env: &process,
        dotenv: &dotenv,
        project: None,
        global: None,
    })
    .expect("synthetic options")
}

fn empty_credentials() -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: Vec::new(),
    })
    .expect("credentials TOML");
    hydrate(parse_credentials(tier).expect("manifest"), vec![]).expect("inline store")
}

fn transport_env(values: &[(&str, &str)]) -> TransportEnvInputs {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/repo"),
        OsFamily::Unix,
        values
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    )
    .expect("synthetic process");
    TransportEnvInputs::from_process(&snapshot)
}

#[test]
fn uuid_request_is_one_typed_get_template_with_the_reference_as_typed() {
    let upper = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
    assert_eq!(reference(upper), TemplateReference::Id(upper.to_owned()));
    let wire = template_request(upper);
    assert_eq!(wire.operation_name.as_deref(), Some("GetTemplate"));
    assert_eq!(
        serde_json::to_value(&wire).unwrap()["variables"],
        json!({"id": upper})
    );
    assert_eq!(
        wire.query.split_whitespace().collect::<String>(),
        "queryGetTemplate($id:String!){template(id:$id){idnamedescriptiontypeiconcolorhasFormFieldslastAppliedAtsortOrdercreatedAtupdatedAtteam{idkeyname}inheritedFrom{idname}creator{idname}templateData}}"
    );
    // Not a UUID: a 35-character near miss and a non-Linear URL are names.
    for name in [
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAA",
        "https://example.com/template",
        "",
    ] {
        assert_eq!(reference(name), TemplateReference::Name(name.to_owned()));
    }
}

#[test]
fn prepare_refuses_a_linear_url_before_credentials_with_one_context() {
    let url = "https://linear.app/acme/issue/ENG-1";
    let store = empty_credentials();
    // Unusable if reached: the URL failure must win over it and credentials.
    let bad_policy = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    for (env, workspace) in [
        (vec![], None),
        (vec![("LINEAR_API_KEY", "lin_api_fake")], Some("beta")),
        (vec![], Some("missing")),
    ] {
        let error = prepare(&config_options(&env), &store, workspace, &bad_policy, url)
            .err()
            .expect("URL refused");
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert_eq!(
            error.display_message(),
            format!(
                "Failed to view template: \"{url}\" is a Linear URL, and this command does not take one."
            )
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Pass a template name or UUID.")
        );
    }

    let no_key = prepare(&config_options(&[]), &store, None, &bad_policy, BUG_ID)
        .err()
        .expect("no key");
    assert_eq!(no_key.context.as_deref(), Some(CONTEXT));
    assert_eq!(
        no_key.display_message(),
        "Failed to view template: No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`."
    );

    let conflict = prepare(
        &config_options(&[("LINEAR_API_KEY", "lin_api_fake")]),
        &store,
        Some("beta"),
        &transport_env(&[]),
        "Bug report",
    )
    .err()
    .expect("workspace conflict");
    assert_eq!(
        conflict.display_message(),
        "Failed to view template: Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag."
    );

    let prepared = prepare(
        &config_options(&[("LINEAR_API_KEY", "lin_api_fake")]),
        &store,
        None,
        &transport_env(&[]),
        "AAAAAAAA-aaaa-4AAA-8aaa-AAAAAAAAAAAA",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        prepared.reference,
        TemplateReference::Id("AAAAAAAA-aaaa-4AAA-8aaa-AAAAAAAAAAAA".to_owned())
    );
}

#[tokio::test]
async fn by_id_renders_metadata_pre_fills_and_the_body_as_copyable_markdown() {
    let requests = RefCell::new(Vec::new());
    let response = typed_one(bug_template());
    let output = run_with(
        &reference(BUG_ID),
        false,
        now,
        &Utc,
        |request| {
            requests
                .borrow_mut()
                .push(serde_json::to_value(&request).unwrap()["variables"].clone());
            ready(Ok(response))
        },
        unexpected_list,
    )
    .await
    .expect("text view");
    assert_eq!(*requests.borrow(), [json!({"id": BUG_ID})]);
    // Deno styles the body with charmd; this prints its Markdown source
    // (C022-TEMPLATE-BODY-MARKDOWN), so the bold markers stay visible.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        format!(
            "Bug report
Issue template · Team ENG (Engineering)
ID: {BUG_ID}
Description: Standard bug intake
Created by: Sam
Updated: 1/2/2024

Pre-fills:
  title: Bug:\x20
  priority: 2 (high)
  estimate: 3
  labelIds: label-bug
  stateId: state-todo
  subIssueData: 2 items
    - Write regression test
        description: Cover the bug with a test
        labelIds: label-test
    - Fix it
  descriptionData:
    ## Steps to reproduce

    1.\x20

    ## Expected

    Describe the **expected** behaviour.

{FOOTER}
"
        )
    );
}

#[tokio::test]
async fn by_name_matches_case_insensitively_with_one_list_request_and_shows_every_key() {
    let list = typed_list(vec![bug_template(), kickoff_template(), broken_template()]);
    let mut requests = 0;
    let output = run_with(
        &reference("kickoff"),
        false,
        now,
        &Utc,
        unexpected_one,
        |request| {
            requests += 1;
            assert_eq!(request.operation_name.as_deref(), Some("GetTemplates"));
            assert_eq!(
                serde_json::to_value(&request).unwrap().get("variables"),
                None
            );
            ready(Ok(list))
        },
    )
    .await
    .expect("text view by name");
    assert_eq!(requests, 1);
    // Byte-identical to the frozen Deno snapshot: no rich-text body.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        format!(
            "Kickoff
Project template · Workspace
ID: tpl-kickoff
Form template: yes (its form is filled in inside Linear; applying it from the CLI creates the entity with the form unanswered)
Inherited from: Parent kickoff (tpl-parent)
Updated: 1/2/2024

Pre-fills:
  name: Kickoff:\x20
  content:
    ## Goals

    ## Milestones
  labelIds: (none)
  priority: 3 (medium)
  custom:
    flag: true
    nested:
      depth: 2

{FOOTER}
"
        )
    );
}

#[tokio::test]
async fn json_is_the_typed_projection_and_never_parses_template_data() {
    for template_data in [bug_template_data(), "not-json".to_owned()] {
        let response = typed_one(with_data(&template_data));
        let output = run_with(
            &reference(BUG_ID),
            true,
            || panic!("JSON output must not read the clock"),
            &Utc,
            |_| ready(Ok(response)),
            unexpected_list,
        )
        .await
        .expect("JSON view");
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!(
                r#"{{
  "id": "{BUG_ID}",
  "name": "Bug report",
  "description": "Standard bug intake",
  "type": "issue",
  "icon": null,
  "color": null,
  "hasFormFields": false,
  "lastAppliedAt": null,
  "sortOrder": 0,
  "createdAt": "2024-01-01T12:00:00.000Z",
  "updatedAt": "2024-01-02T12:00:00.000Z",
  "team": {{
    "id": "team-eng-id",
    "key": "ENG",
    "name": "Engineering"
  }},
  "inheritedFrom": null,
  "creator": {{
    "id": "user-1",
    "name": "Sam"
  }},
  "templateData": {}
}}
"#,
                serde_json::to_string(&template_data).unwrap()
            )
        );
    }

    // By name, JSON is the same projection of the matched list entry.
    let list = typed_list(vec![broken_template()]);
    let output = run_with(
        &reference("BROKEN"),
        true,
        now,
        &Utc,
        unexpected_one,
        |_| ready(Ok(list)),
    )
    .await
    .expect("JSON by name");
    let parsed: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(parsed["id"], "tpl-broken");
    assert_eq!(parsed["templateData"], "[1, 2]");
}

#[test]
fn outer_template_data_and_template_must_match_the_typed_schema() {
    // C022-STRICT-TEMPLATE-DECODE: Deno prints these in JSON mode.
    let body = json!({"data": {"template": bug_template()}}).to_string();
    for (from, to) in [
        (
            format!(
                "\"templateData\":{}",
                serde_json::to_string(&bug_template_data()).unwrap()
            ),
            "\"templateData\":{\"title\":\"raw\"}".to_owned(),
        ),
        (
            format!(
                "\"templateData\":{}",
                serde_json::to_string(&bug_template_data()).unwrap()
            ),
            "\"templateData\":null".to_owned(),
        ),
    ] {
        assert!(body.contains(&from));
        let changed = body.replace(&from, &to);
        assert!(
            matches!(
                parse_response::<GetTemplate>(changed.as_bytes()),
                Err(ResponseError::UnexpectedShape(_))
            ),
            "{to}"
        );
    }
    assert!(matches!(
        parse_response::<GetTemplate>(br#"{"data":{"template":null}}"#),
        Err(ResponseError::UnexpectedShape(_))
    ));
}

#[tokio::test]
async fn any_raw_no_template_found_message_becomes_not_found_with_the_typed_reference() {
    let upper = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
    let cases = [
        (
            200,
            json!({"errors": [{"message": format!("No template found with id {upper}")}], "data": null}),
        ),
        // Second error, whatever the first says.
        (
            200,
            json!({"errors": [
                {"message": "Something else"},
                {"message": "no template found"}
            ], "data": null}),
        ),
        // Uppercase on HTTP 400, and on HTTP 500.
        (
            400,
            json!({"errors": [{"message": format!("NO TEMPLATE FOUND WITH ID {upper}")}], "data": null}),
        ),
        (500, json!({"errors": [{"message": "No template found"}]})),
        // The raw message decides, not the presentable one.
        (
            200,
            json!({"errors": [{
                "message": "No template found",
                "extensions": {"userPresentableMessage": "Choose another template"}
            }], "data": null}),
        ),
    ];
    for (status, body) in cases {
        let error = run_with(
            &reference(upper),
            false,
            now,
            &Utc,
            |request| {
                assert_eq!(
                    serde_json::to_value(&request).unwrap()["variables"],
                    json!({"id": upper})
                );
                ready(Err(failure(status, body.clone())))
            },
            unexpected_list,
        )
        .await
        .expect_err("missing template");
        assert_eq!(error.kind, AppErrorKind::NotFound, "{body}");
        assert_eq!(
            error.display_message(),
            format!("Failed to view template: Template not found: {upper}")
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Run `linear template list` to see every template.")
        );
    }
}

#[tokio::test]
async fn other_failures_pass_through_once_with_the_command_context() {
    let cases = [
        (
            json!({"errors": [{"message": "Entity not found: Template"}], "data": null}),
            200,
            "Entity not found: Template",
        ),
        (
            json!({"errors": [{
                "message": "Entity not found",
                "extensions": {"userPresentableMessage": "This template cannot be opened"}
            }], "data": null}),
            200,
            "This template cannot be opened",
        ),
        (
            json!({"errors": [{"message": "unavailable"}]}),
            500,
            "unavailable",
        ),
    ];
    for (body, status, message) in cases {
        let error = run_with(
            &reference(BUG_ID),
            false,
            now,
            &Utc,
            |_| ready(Err(failure(status, body.clone()))),
            unexpected_list,
        )
        .await
        .expect_err("GraphQL failure");
        assert_eq!(error.kind, AppErrorKind::GraphQl);
        assert_eq!(error.context.as_deref(), Some(CONTEXT));
        assert_eq!(
            error.display_message(),
            format!("Failed to view template: {message}")
        );
        assert_eq!(error.suggestion, None);
    }

    // A non-2xx body without GraphQL errors is never a template miss.
    let http = run_with(
        &reference(BUG_ID),
        false,
        now,
        &Utc,
        |_| ready(Err(failure(500, json!({"message": "No template found"})))),
        unexpected_list,
    )
    .await
    .expect_err("HTTP failure");
    assert_eq!(http.kind, AppErrorKind::Transport);
    assert_eq!(http.context.as_deref(), Some(CONTEXT));
    assert_eq!(http.display_message().matches(CONTEXT).count(), 1);

    let list = run_with(
        &reference("Bug report"),
        false,
        now,
        &Utc,
        unexpected_one,
        |_| {
            ready(Err(failure(
                200,
                json!({"errors": [{"message": "No template found"}], "data": null}),
            )))
        },
    )
    .await
    .expect_err("list failure");
    // The translation belongs to the UUID path only.
    assert_eq!(list.kind, AppErrorKind::GraphQl);
    assert_eq!(
        list.display_message(),
        "Failed to view template: No template found"
    );
}

async fn name_error(reference_text: &str, templates: Vec<Value>) -> AppError {
    let list = typed_list(templates);
    run_with(
        &reference(reference_text),
        false,
        now,
        &Utc,
        unexpected_one,
        |_| ready(Ok(list)),
    )
    .await
    .expect_err("name lookup failure")
}

fn named(id: &str, name: &str, team: Option<(&str, &str)>) -> Value {
    let mut template = bug_template();
    template["id"] = json!(id);
    template["name"] = json!(name);
    template["team"] = match team {
        Some((id, key)) => json!({"id": id, "key": key, "name": key}),
        None => Value::Null,
    };
    template
}

#[tokio::test]
async fn unknown_names_list_deduplicated_root_collated_names() {
    let error = name_error(
        "Nope",
        vec![bug_template(), kickoff_template(), broken_template()],
    )
    .await;
    assert_eq!(error.kind, AppErrorKind::NotFound);
    assert_eq!(
        error.display_message(),
        "Failed to view template: Template not found: Nope"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Available templates: \"Broken\", \"Bug report\", \"Kickoff\". Run `linear template list` to see every template."
        )
    );

    let error = name_error(
        "Nope",
        vec![
            named("1", "beta", None),
            named("2", "Zulu", None),
            named("3", "Álpha", None),
            named("4", "beta", Some(("team-eng", "ENG"))),
        ],
    )
    .await;
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Available templates: \"Álpha\", \"beta\", \"Zulu\". Run `linear template list` to see every template."
        )
    );

    let empty = name_error("", vec![]).await;
    assert_eq!(
        empty.display_message(),
        "Failed to view template: Template not found: "
    );
    assert_eq!(
        empty.suggestion.as_deref(),
        Some("No templates are available here. Run `linear template list` to see every template.")
    );
}

#[tokio::test]
async fn ambiguous_names_list_ids_in_response_order() {
    let error = name_error(
        "bug REPORT",
        vec![
            named("tpl-z", "Bug report", Some(("team-eng", "ENG"))),
            named("tpl-other", "Other", None),
            named("tpl-a", "BUG REPORT", None),
        ],
    )
    .await;
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(
        error.display_message(),
        "Failed to view template: Template name \"bug REPORT\" is ambiguous: it matches 2 templates"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Pass the template ID instead: tpl-z (issue, ENG), tpl-a (issue, Workspace)")
    );
}

#[tokio::test]
async fn text_parses_template_data_lazily_with_one_context() {
    let list = typed_list(vec![bug_template(), kickoff_template(), broken_template()]);
    let error = run_with(
        &reference("Broken"),
        false,
        now,
        &Utc,
        unexpected_one,
        |_| ready(Ok(list)),
    )
    .await
    .expect_err("array templateData");
    assert_eq!(
        error.display_message(),
        "Failed to view template: Template data for \"Broken\" (tpl-broken) is not a JSON object"
    );

    for (data, suffix) in [
        ("not-json", "is not valid JSON"),
        ("", "is not valid JSON"),
        ("{} trailing", "is not valid JSON"),
        ("42", "is not a JSON object"),
        ("null", "is not a JSON object"),
        ("\"{}\"", "is not a JSON object"),
        // JSON.parse accepts these; Rust rejects them (named text deviations).
        ("{\"value\":1e400}", "is not valid JSON"),
        ("{\"value\":\"\\ud800\"}", "is not valid JSON"),
    ] {
        let error = view_error(data).await;
        assert_eq!(
            error.display_message(),
            format!(
                "Failed to view template: Template data for \"Bug report\" ({BUG_ID}) {suffix}"
            ),
            "{data}"
        );
        assert_eq!(error.suggestion, None);
    }
}

#[tokio::test]
async fn pre_fills_follow_js_object_order_and_duplicate_replacement() {
    assert_eq!(
        pre_fills(
            r#"{"b":1,"10":"ten","2":"two","b":2,"01":"zero-one","4294967295":"limit","0":"zero"}"#
        )
        .await,
        "  0: zero\n  2: two\n  10: ten\n  b: 2\n  01: zero-one\n  4294967295: limit"
    );
    assert_eq!(
        pre_fills(
            r#"{"b":1,"4294967294":"max-index","-1":"negative","2":"two","4294967295":"over-limit"}"#
        )
        .await,
        "  2: two\n  4294967294: max-index\n  b: 1\n  -1: negative\n  4294967295: over-limit"
    );
    assert_eq!(pre_fills("{}").await, "  (nothing)");
}

#[tokio::test]
async fn scalars_use_javascript_number_and_priority_spelling() {
    assert_eq!(
        pre_fills(r#"{"large":1e21,"small":1e-7,"fraction":1.25,"big":9007199254740993,"flag":false,"none":null}"#)
            .await,
        "  large: 1e+21\n  small: 1e-7\n  fraction: 1.25\n  big: 9007199254740992\n  flag: false\n  none: null"
    );
    assert_eq!(
        pre_fills(r#"{"priority":-0}"#).await,
        "  priority: 0 (none)"
    );
    assert_eq!(
        pre_fills(r#"{"priority":4.0}"#).await,
        "  priority: 4 (low)"
    );
    for (value, expected) in [("5", "5"), ("2.5", "2.5"), ("-1", "-1")] {
        assert_eq!(
            pre_fills(&format!("{{\"priority\":{value}}}")).await,
            format!("  priority: {expected}")
        );
    }
    // A string priority takes the string branch.
    assert_eq!(pre_fills(r#"{"priority":"2"}"#).await, "  priority: 2");
}

#[tokio::test]
async fn arrays_items_and_labels_render_recursively() {
    assert_eq!(
        pre_fills(r#"{"empty":[],"strings":["a","","b"],"blank":[""]}"#).await,
        "  empty: (none)\n  strings: a, , b\n  blank: "
    );
    assert_eq!(
        pre_fills(r#"{"subIssueData":[{"priority":3},1,null,"x",[1,"y"],{"a":{"b":[]}}],"flag":true,"meta":{"a":1}}"#)
            .await,
        "  subIssueData: 6 items
    - (untitled)
        priority: 3 (medium)
    - 1
    - null
    - \"x\"
    - [1,\"y\"]
    - (untitled)
        a:
          b: (none)
  flag: true
  meta:
    a: 1"
    );
    assert_eq!(
        pre_fills(r#"{"subIssueData":[{"title":"X","name":"X","priority":2}]}"#).await,
        "  subIssueData: 1 item\n    - X\n        priority: 2 (high)"
    );
    assert_eq!(
        pre_fills(r#"{"subIssueData":[{"title":"","name":"X"}]}"#).await,
        "  subIssueData: 1 item\n    - X\n        title: "
    );
    assert_eq!(
        pre_fills(r#"{"issueData":[{"title":"T","name":"N"},{"title":1,"name":"N"}]}"#).await,
        "  issueData: 2 items\n    - T\n        name: N\n    - N\n        title: 1"
    );
    assert_eq!(
        pre_fills(r#"{"items":[{"s":"quote \" back \\ tab \t nl \n ctl \u0001 ls \u2028"}, "ctl \u001f"]}"#)
            .await,
        "  items: 2 items\n    - (untitled)\n        s:\n          quote \" back \\ tab \t nl \n           ctl \u{1} ls\n    - \"ctl \\u001f\""
    );
}

#[tokio::test]
async fn multiline_strings_split_on_line_feed_and_trim_js_whitespace() {
    assert_eq!(
        pre_fills(r#"{"description":"first\r\nsecond\rthird"}"#).await,
        "  description:\n    first\r\n    second\rthird"
    );
    assert_eq!(
        pre_fills(r#"{"description":"one\rtwo"}"#).await,
        "  description: one\rtwo"
    );
    assert_eq!(
        pre_fills(
            "{\"description\":\"\u{a0}first\u{a0}\\n\\nsecond\u{feff}\\n\u{2028}third\u{2028}\"}"
        )
        .await,
        "  description:\n    \u{a0}first\u{a0}\n\n    second\u{feff}\n    \u{2028}third"
    );
}

#[tokio::test]
async fn rich_text_bodies_render_last_at_every_depth_as_raw_markdown() {
    let paragraph = |text: &str| json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]});
    let data = json!({
        "descriptionData": paragraph("* literal\n1. list"),
        "title": "T",
        "subIssueData": [{
            "descriptionData": paragraph("nested *body*"),
            "title": "Child",
            "contentData": "plain"
        }],
        "contentData": null
    });
    // C022-TEMPLATE-BODY-MARKDOWN: `\*` and `1\.` stay visible so the text can
    // be pasted as Markdown; Deno's charmd rendering shows `* literal`.
    assert_eq!(
        pre_fills(&data.to_string()).await,
        "  title: T
  subIssueData: 1 item
    - Child
        descriptionData:
          nested \\*body\\*
        contentData: plain
  descriptionData:
    \\* literal
    1\\. list
  contentData: null"
    );
}

#[tokio::test]
async fn prose_mirror_errors_surface_in_render_order_without_a_key_path() {
    let error = view_error(
        r#"{"descriptionData":{"type":"bad"},"subIssueData":[{"descriptionData":{"type":"also-bad"}}]}"#,
    )
    .await;
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(
        error.display_message(),
        "Failed to view template: Expected a ProseMirror document, got a \"also-bad\" node"
    );

    let error = view_error(
        r#"{"contentData":{"type":"doc","content":[{"type":"text","text":"X","marks":[42]}]}}"#,
    )
    .await;
    assert_eq!(
        error.display_message(),
        "Failed to view template: Invalid ProseMirror mark at doc.content[0].marks[0]: expected an object with a string \"type\""
    );
}

#[tokio::test]
async fn header_lines_use_injected_clock_and_zone() {
    let mut template = bug_template();
    template["description"] = json!("");
    template["lastAppliedAt"] = json!("2026-09-25T11:30:00.000Z");
    template["updatedAt"] = json!("2024-01-02T05:00:00.000Z");
    template["type"] = json!("éxotic");
    template["templateData"] = json!("{}");
    let typed = typed_template(template);
    let pacific = FixedOffset::west_opt(8 * 3600).expect("offset");
    assert_eq!(
        render_text(&typed, now(), &pacific).expect("text"),
        format!(
            "Bug report
Éxotic template · Team ENG (Engineering)
ID: {BUG_ID}
Created by: Sam
Last applied: 30 minutes ago
Updated: 1/1/2024

Pre-fills:
  (nothing)

{FOOTER}"
        )
    );
    let utc = render_text(&typed, now(), &Utc).expect("text");
    assert!(utc.contains("\nUpdated: 1/2/2024\n"), "{utc}");

    let mut astral = bug_template();
    astral["type"] = json!("𝒾ssue");
    astral["templateData"] = json!("{}");
    let text = render_text(&typed_template(astral), now(), &Utc).expect("text");
    assert!(text.contains("\n𝒾ssue template · "), "{text}");
}
