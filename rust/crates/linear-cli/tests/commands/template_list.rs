use crate::hydrate;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::{Ready, ready};
use std::path::PathBuf;

use linear_cli::auth::{ApiKeyInput, CredentialStore, parse_credentials};
use linear_cli::commands::template::list::{
    Options, TemplateType, prepare, render_text, request, run_with,
};
use linear_cli::config::{
    ConfigInputs, ConfigOptions, OptionInputs, OsFamily, ProcessEnvSnapshot, RawConfigFile,
    SelectedEnv, TransportEnvInputs, parse_config_tier,
};
use linear_cli::error::Error;
use linear_cli::graphql::envelope::{GraphQlRequest, ResponseError, parse_response};
use linear_cli::graphql::operations::team_resolver::{
    GetAllTeams, GetAllTeamsVariables, ResolveTeam, ResolveTeamVariables,
};
use linear_cli::graphql::operations::templates::GetTemplates;
use linear_cli::refs::{PreparedTeamLookup, WorkspaceScope, prepare_team_lookup};
use serde_json::{Value, json};

fn template_response(body: &str) -> GetTemplates {
    parse_response(body.as_bytes()).expect("typed template response")
}

fn template(id: &str, name: &str, template_type: &str, team: Option<Value>) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": null,
        "type": template_type,
        "icon": null,
        "color": null,
        "hasFormFields": false,
        "lastAppliedAt": null,
        "sortOrder": 1,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-02T00:00:00.000Z",
        "team": team,
        "inheritedFrom": null,
        "creator": null,
        "templateData": "{}"
    })
}

fn team_json(id: &str, key: &str) -> Value {
    json!({"id": id, "key": key, "name": format!("{key} team")})
}

fn templates_response(templates: Vec<Value>) -> GetTemplates {
    template_response(&json!({"data": {"templates": templates}}).to_string())
}

fn unexpected_resolve(
    _: GraphQlRequest<ResolveTeamVariables>,
) -> Ready<Result<ResolveTeam, Error>> {
    panic!("ResolveTeam must not be requested")
}

fn unexpected_all_teams(
    _: GraphQlRequest<GetAllTeamsVariables>,
) -> Ready<Result<GetAllTeams, Error>> {
    panic!("GetAllTeams must not be requested")
}

fn unexpected_templates(_: GraphQlRequest<()>) -> Ready<Result<GetTemplates, Error>> {
    panic!("GetTemplates must not be requested")
}

/// `run_with` without `--team`: any team request fails the test.
async fn list_without_team(
    templates: Vec<Value>,
    options: Options,
    columns: usize,
    color: bool,
) -> String {
    let response = templates_response(templates);
    let output = run_with(
        None,
        options,
        columns,
        color,
        unexpected_resolve,
        unexpected_all_teams,
        |_| ready(Ok(response)),
    )
    .await
    .expect("template list");
    String::from_utf8(output).expect("UTF-8 output")
}

fn team_lookup(reference: &str) -> PreparedTeamLookup {
    let absent = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: absent.clone(),
    };
    prepare_team_lookup(reference, &scope).unwrap_or_else(|error| panic!("{error}"))
}

fn empty_resolve() -> ResolveTeam {
    serde_json::from_value(json!({"teams": {"nodes": []}})).expect("typed ResolveTeam")
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

#[tokio::test]
async fn query_and_json_keep_selected_graphql_fields_and_js_number_bytes() {
    let wire = request();
    assert_eq!(wire.operation_name.as_deref(), Some("GetTemplates"));
    assert_eq!(serde_json::to_value(&wire).unwrap().get("variables"), None);
    assert_eq!(
        wire.query.split_whitespace().collect::<String>(),
        "queryGetTemplates{templates{idnamedescriptiontypeiconcolorhasFormFieldslastAppliedAtsortOrdercreatedAtupdatedAtteam{idkeyname}inheritedFrom{idname}creator{idname}templateData}}"
    );

    let response = template_response(
        r#"{"data":{"templates":[{"id":"t1","name":"A","description":null,"type":"issue","icon":null,"color":null,"hasFormFields":false,"lastAppliedAt":null,"sortOrder":1e21,"createdAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-02T00:00:00.000Z","team":null,"inheritedFrom":null,"creator":null,"templateData":"not-json","serverOnly":"omitted"}]}}"#,
    );
    let output = run_with(
        None,
        Options {
            template_type: None,
            json: true,
        },
        120,
        false,
        |_| ready(Err(Error::new("unexpected team lookup"))),
        |_| ready(Err(Error::new("unexpected team page"))),
        |request| {
            assert_eq!(
                serde_json::to_value(&request).unwrap().get("variables"),
                None
            );
            ready(Ok(response))
        },
    )
    .await
    .expect("template JSON");
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\"sortOrder\": 1e+21"), "{text}");
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed[0]["templateData"], "not-json");
    assert_eq!(parsed[0]["team"], Value::Null);
    assert!(parsed[0].get("serverOnly").is_none());
    assert_eq!(parsed[0].as_object().unwrap().len(), 15);
}

#[test]
fn strict_typed_boundary_and_empty_text_are_visible_through_public_api() {
    let valid = r#"{"data":{"templates":[{"id":"t1","name":"A","description":null,"type":"issue","icon":null,"color":null,"hasFormFields":false,"lastAppliedAt":null,"sortOrder":1,"createdAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-02T00:00:00.000Z","team":null,"inheritedFrom":null,"creator":null,"templateData":"{}"}]}}"#;
    for (from, to) in [
        ("\"name\":\"A\"", "\"name\":null"),
        ("\"templateData\":\"{}\"", "\"templateData\":{}"),
        ("\"sortOrder\":1", "\"sortOrder\":null"),
    ] {
        let body = valid.replace(from, to);
        assert!(
            matches!(
                parse_response::<GetTemplates>(body.as_bytes()),
                Err(ResponseError::UnexpectedShape(_))
            ),
            "{to} should be rejected"
        );
    }
    assert_eq!(render_text(&[], 120, false), "No templates found.\n");
}

#[tokio::test]
async fn type_filter_stable_scope_order_and_padded_text_are_public_behavior() {
    let response = json!({"data": {"templates": [
        template("team", "A", "issue", Some(json!({"id":"eng", "key":"ENG", "name":"Engineering"}))),
        template("other", "Q", "unknown", None),
        template("workspace", "A", "issue", None),
        template("project", "B", "project", None)
    ]}});
    let parsed = template_response(&response.to_string());
    let output = run_with(
        None,
        Options {
            template_type: Some(TemplateType::Issue),
            json: true,
        },
        120,
        false,
        |_| ready(Err(Error::new("unexpected lookup"))),
        |_| ready(Err(Error::new("unexpected page"))),
        |_| ready(Ok(parsed)),
    )
    .await
    .expect("filtered list");
    let rows: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["id"], "workspace");
    assert_eq!(rows[1]["id"], "team");

    let typed = template_response(&response.to_string());
    let expected = format!(
        "{:<36} {:<4} {:<5} {:<9}\n{:<36} {:<4} {:<5} {:<9}\n{:<36} {:<4} {:<5} {:<9}\n\n2 templates found.\n",
        "ID",
        "NAME",
        "TYPE",
        "TEAM",
        "workspace",
        "A",
        "issue",
        "Workspace",
        "team",
        "A",
        "issue",
        "ENG"
    );
    let selected = typed
        .templates
        .into_iter()
        .filter(|entry| entry.template_type == "issue")
        .collect::<Vec<_>>();
    let mut selected = selected;
    selected.sort_by_key(|entry| entry.team.is_some());
    assert_eq!(render_text(&selected, 120, false), expected);
    let colored = render_text(&selected, 120, true);
    assert!(colored.starts_with("\x1b[4mID"));
    assert!(colored.contains("\x1b[24m \x1b[4mNAME"));
}

#[tokio::test]
async fn network_error_has_one_command_context() {
    let error = run_with(
        None,
        Options::default(),
        120,
        false,
        |_| ready(Err(Error::new("unexpected lookup"))),
        |_| ready(Err(Error::new("unexpected page"))),
        |_| ready(Err(Error::new("templates unavailable"))),
    )
    .await
    .expect_err("request failure");
    assert_eq!(
        error.to_string(),
        "Failed to list templates: templates unavailable"
    );
}

#[test]
fn prepare_turns_a_team_url_into_its_key_and_builds_the_transport() {
    let prepared = prepare(
        &config_options(&[("LINEAR_API_KEY", "lin_api_fake")]),
        &empty_credentials(),
        None,
        &transport_env(&[]),
        Some("https://linear.app/acme/team/eng"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let team = prepared.team.expect("prepared team");
    assert_eq!(team.original(), "https://linear.app/acme/team/eng");
    assert_eq!(team.lookup(), "ENG");
    assert_eq!(
        prepared.transport.endpoint().origin(),
        "https://api.linear.app"
    );
}

#[tokio::test]
async fn team_resolves_before_one_template_request_and_keeps_workspace_and_team_id() {
    let team = team_lookup("eng");
    let resolved: ResolveTeam = serde_json::from_value(json!({
        "teams": {"nodes": [{"id": "team-eng", "key": "ENG", "name": "Engineering"}]}
    }))
    .expect("typed ResolveTeam");
    let templates = templates_response(vec![
        template("workspace", "Z", "issue", None),
        // Its ID is the raw reference text, but it is not the resolved team.
        template(
            "reference-text",
            "A",
            "issue",
            Some(team_json("eng", "DES")),
        ),
        template("resolved", "A", "issue", Some(team_json("team-eng", "ENG"))),
    ]);
    let requests = RefCell::new(Vec::new());
    let output = run_with(
        Some(&team),
        Options {
            template_type: None,
            json: true,
        },
        120,
        false,
        |request| {
            requests.borrow_mut().push(request.operation_name.clone());
            assert_eq!(
                serde_json::to_value(&request).unwrap()["variables"],
                json!({"reference": "eng", "id": null, "isUuid": false})
            );
            ready(Ok(resolved))
        },
        unexpected_all_teams,
        |request| {
            requests.borrow_mut().push(request.operation_name.clone());
            ready(Ok(templates))
        },
    )
    .await
    .expect("team-filtered list");
    assert_eq!(
        *requests.borrow(),
        [
            Some("ResolveTeam".to_owned()),
            Some("GetTemplates".to_owned())
        ]
    );
    let rows: Value = serde_json::from_slice(&output).unwrap();
    let ids: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["resolved", "workspace"]);
}

#[tokio::test]
async fn team_lookup_failures_have_one_context_and_never_request_templates() {
    let team = team_lookup("eng");
    let page: GetAllTeams = serde_json::from_value(json!({"teams": {
        "nodes": [{"id": "team-des", "key": "DES", "name": "Design"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}))
    .expect("typed GetAllTeams");
    let mut pages = 0;
    let miss = run_with(
        Some(&team),
        Options::default(),
        120,
        false,
        |_| ready(Ok(empty_resolve())),
        |request| {
            pages += 1;
            assert_eq!(
                serde_json::to_value(&request).unwrap()["variables"],
                json!({"first": 100})
            );
            ready(Ok(page.clone()))
        },
        unexpected_templates,
    )
    .await
    .expect_err("team miss");
    assert_eq!(pages, 1);
    assert_eq!(miss.kind(), linear_cli::error::ErrorKind::NotFound);
    assert!(miss.to_string().starts_with("Failed to list templates: "));
    assert_eq!(
        miss.to_string(),
        "Failed to list templates: Team not found: eng"
    );
    assert_eq!(
        miss.hint(),
        Some("Valid team keys: DES (Design). Run `linear team list` to see all teams.")
    );

    let resolve_failure = run_with(
        Some(&team),
        Options::default(),
        120,
        false,
        |_| ready(Err(Error::new("team lookup failed"))),
        unexpected_all_teams,
        unexpected_templates,
    )
    .await
    .expect_err("ResolveTeam failure");
    assert!(
        resolve_failure
            .to_string()
            .starts_with(&format!("{}: ", "Failed to list templates"))
    );
    assert_eq!(
        resolve_failure.to_string(),
        "Failed to list templates: team lookup failed"
    );

    let page_failure = run_with(
        Some(&team),
        Options::default(),
        120,
        false,
        |_| ready(Ok(empty_resolve())),
        |_| ready(Err(Error::new("team page failed"))),
        unexpected_templates,
    )
    .await
    .expect_err("GetAllTeams failure");
    assert_eq!(
        page_failure.to_string(),
        "Failed to list templates: team page failed"
    );
}

#[tokio::test]
async fn unfiltered_list_keeps_unknown_types_and_ties_lowercased_names_stably() {
    // Byte order would give Alpha, Beta, alpha; root collation of the raw
    // names would put alpha before Alpha. Lowercased names tie, so input
    // order decides. The unknown type sorts after issue despite its name.
    let output = list_without_team(
        vec![
            template("beta", "Beta", "issue", None),
            template("upper", "Alpha", "issue", None),
            template("lower", "alpha", "issue", None),
            template("unknown", "Aardvark", "roadmap", None),
        ],
        Options::default(),
        120,
        false,
    )
    .await;
    assert_eq!(
        output,
        format!(
            "{:<36} {:<8} {:<7} {:<9}\n{:<36} {:<8} {:<7} {:<9}\n{:<36} {:<8} {:<7} {:<9}\n{:<36} {:<8} {:<7} {:<9}\n{:<36} {:<8} {:<7} {:<9}\n\n4 templates found.\n",
            "ID",
            "NAME",
            "TYPE",
            "TEAM",
            "upper",
            "Alpha",
            "issue",
            "Workspace",
            "lower",
            "alpha",
            "issue",
            "Workspace",
            "beta",
            "Beta",
            "issue",
            "Workspace",
            "unknown",
            "Aardvark",
            "roadmap",
            "Workspace"
        )
    );
}

#[tokio::test]
async fn narrow_text_floors_name_keeps_long_team_marks_forms_and_counts_one() {
    let mut form = template(
        "tpl-1",
        "A very long template name here",
        "issue",
        Some(team_json("team-long", "VERYLONGTEAMKEY123")),
    );
    form["hasFormFields"] = json!(true);
    // TYPE is "issue (form)" (12) and TEAM is capped at 15, so the fixed
    // columns are 36 + 12 + 15 + 3 separators = 66.
    let plain = |name_width: usize, name: &str| {
        format!(
            "{:<36} {:<name_width$} {:<12} {:<15}\n{:<36} {name} issue (form) VERYLONGTEAMKEY123\n\n1 template found.\n",
            "ID", "NAME", "TYPE", "TEAM", "tpl-1"
        )
    };
    for (columns, name_width, name) in [
        (40, 20, "A very long templ..."),
        (92, 25, "A very long template n..."),
        (200, 30, "A very long template name here"),
    ] {
        assert_eq!(
            list_without_team(vec![form.clone()], Options::default(), columns, false).await,
            plain(name_width, name),
            "{columns} columns"
        );
    }

    let colored = list_without_team(vec![form], Options::default(), 40, true).await;
    assert_eq!(
        colored,
        format!(
            "\x1b[4m{:<36}\x1b[24m \x1b[4m{:<20}\x1b[24m \x1b[4m{:<12}\x1b[24m \x1b[4m{:<15}\x1b[0m\n{:<36} A very long templ... issue (form) VERYLONGTEAMKEY123\n\n1 template found.\n",
            "ID", "NAME", "TYPE", "TEAM", "tpl-1"
        )
    );
}
