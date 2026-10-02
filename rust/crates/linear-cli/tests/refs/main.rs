use linear_cli::auth::{ApiKeyInput, CredentialSelectionInputs};
use linear_cli::config::{ConfigSecret, OptionSource};
use linear_cli::refs::{
    CycleSelector, LinearUrlParse, LinearUrlRef, WorkspaceScope, expect_team_url, parse_linear_url,
};
use serde_json::Value;
use std::path::PathBuf;

mod shared;
mod team;

/// Team reference cases: the `team members` argument, workspace inputs,
/// the expected error, and the scripted GraphQL exchanges.
const TEAM_REFERENCES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/team_references.json"
));

fn case(name: &str) -> Value {
    let all: Value = serde_json::from_str(TEAM_REFERENCES)
        .unwrap_or_else(|error| panic!("team reference fixture: {error}"));
    let mut spec = all[name].clone();
    assert!(spec.is_object(), "missing team reference case {name}");
    spec["id"] = Value::from(name);
    spec
}

fn argument(spec: &Value) -> &str {
    spec["reference"]
        .as_str()
        .unwrap_or_else(|| panic!("{} reference", spec["id"]))
}

fn expected_error(spec: &Value) -> (&str, Option<&str>) {
    let message = spec["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("{} error message", spec["id"]));
    (message, spec["error"]["suggestion"].as_str())
}

fn first_reference(spec: &Value) -> Option<&str> {
    spec["steps"][0]["variables"]["reference"].as_str()
}

fn absent_scope<'a>(key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: key.clone(),
    }
}

fn assert_url_error(name: &str, scope: &WorkspaceScope<'_>) {
    let spec = case(name);
    let error = expect_team_url(argument(&spec), scope)
        .err()
        .unwrap_or_else(|| panic!("{} should fail", spec["id"]));
    let (message, suggestion) = expected_error(&spec);
    assert_eq!(error.message(), message, "{}", spec["id"]);
    assert_eq!(error.hint(), suggestion, "{}", spec["id"]);
    assert_eq!(error.to_string(), error.message(), "{}", spec["id"]);
}

fn assert_url_prepared(name: &str, scope: &WorkspaceScope<'_>) {
    let spec = case(name);
    let prepared = expect_team_url(argument(&spec), scope)
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]));
    assert_eq!(
        prepared.as_deref(),
        first_reference(&spec),
        "{}",
        spec["id"]
    );
}

#[test]
fn team_url_prepares_the_canonical_team_reference() {
    let spec = case("url-accepted");
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    let actual = expect_team_url(argument(&spec), &scope).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(actual.as_deref(), first_reference(&spec));
}

#[test]
fn malformed_url_escape_is_unsupported() {
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    assert_url_error("malformed-escape", &scope);
}

#[test]
fn workspace_suggestions_use_key_provenance_even_for_empty_values() {
    let absent = ApiKeyInput::Absent;
    let mut scope = absent_scope(&absent);
    scope.sourced_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    assert_url_error("suggest-plain", &scope);

    let empty = ConfigSecret::new(String::new());
    let key = ApiKeyInput::Raw {
        value: &empty,
        source: OptionSource::ProjectEnv {
            path: PathBuf::from("/fake/.env"),
        },
    };
    let mut scope = absent_scope(&key);
    scope.sourced_workspace = Some("acme");
    assert_url_error("suggest-raw-empty", &scope);
}

#[test]
fn workspace_check_precedes_later_key_conflict_and_empty_config_shadows_default() {
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    assert_url_error("foreign-before-key", &scope);

    let fake = ConfigSecret::new("lin_api_fake_project".to_owned());
    let key = ApiKeyInput::Sourced {
        value: &fake,
        source: OptionSource::ProjectConfig {
            path: PathBuf::from("/fake/linear.toml"),
        },
    };
    let mut scope = absent_scope(&key);
    scope.sourced_workspace = Some("");
    scope.default_workspace = Some("acme");
    assert_url_prepared("workspace-empty-config", &scope);
}

#[test]
fn typed_url_kinds_and_boundaries() {
    assert!(matches!(
        parse_linear_url("plain name"),
        LinearUrlParse::NotLinear
    ));
    assert!(matches!(
        parse_linear_url("\u{feff}  "),
        LinearUrlParse::NotLinear
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/issue/eng-12#comment-abcdef12"), LinearUrlParse::Known(LinearUrlRef::Issue { identifier, comment_id_prefix: Some(comment), .. }) if identifier == "ENG-12" && comment == "abcdef12")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/project/Name-ABCDEF123456"), LinearUrlParse::Known(LinearUrlRef::Project { slug_id, .. }) if slug_id == "abcdef123456")
    );
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/document/X-abcdef123456"),
        LinearUrlParse::Known(LinearUrlRef::Document { .. })
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/initiative/X-abcdef123456"),
        LinearUrlParse::Known(LinearUrlRef::Initiative { .. })
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/upcoming"), LinearUrlParse::Known(LinearUrlRef::Cycle { team_key, cycle: CycleSelector::Next, .. }) if team_key == "ENG")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/ß/projects%2Fall"), LinearUrlParse::Known(LinearUrlRef::Team { team_key, .. }) if team_key == "SS")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/9007199254740992"), LinearUrlParse::Unsupported(reason) if reason == "\"9007199254740992\" is not a cycle number")
    );
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/team/eng/cycle/9007199254740991"),
        LinearUrlParse::Known(LinearUrlRef::Cycle {
            cycle: CycleSelector::Number(9_007_199_254_740_991),
            ..
        })
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/Constructor"), LinearUrlParse::Unsupported(reason) if reason == "\"Constructor\" is not a cycle number")
    );
}

#[test]
fn url_normalization_and_error_order() {
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    assert_eq!(
        expect_team_url("linear.app/acme/team/eng", &scope).unwrap_or_else(|e| panic!("{e}")),
        Some("ENG".to_owned())
    );
    assert_eq!(
        expect_team_url("https://linear.app:443/acme/team/eng", &scope)
            .unwrap_or_else(|e| panic!("{e}")),
        Some("ENG".to_owned())
    );
    assert_eq!(
        expect_team_url("https://linear.app:123/acme/team/eng", &scope)
            .unwrap_or_else(|e| panic!("{e}")),
        None
    );
    assert_eq!(
        expect_team_url("https://user@linear.app/acme/team/eng", &scope)
            .unwrap_or_else(|e| panic!("{e}")),
        None
    );
    assert_eq!(
        expect_team_url("https://linear.app/acme/other/../team/eng", &scope)
            .unwrap_or_else(|e| panic!("{e}")),
        Some("ENG".to_owned())
    );
    let error = expect_team_url("https://linear.app/foreign/project/X-ABCDEF123456", &scope)
        .err()
        .unwrap_or_else(|| panic!("foreign URL"));
    assert!(
        error
            .message()
            .starts_with("That URL is for the \"foreign\" workspace")
    );
    let error = expect_team_url("https://linear.app/acme/project/X-ABCDEF123456", &scope)
        .err()
        .unwrap_or_else(|| panic!("wrong kind"));
    assert_eq!(
        error.message(),
        "\"https://linear.app/acme/project/X-ABCDEF123456\" is a project URL, not a team URL."
    );
    // An unknown cycle selector is rejected as an invalid URL before the
    // workspace is compared.
    let error = expect_team_url(
        "https://linear.app/foreign/team/eng/cycle/Constructor",
        &scope,
    )
    .err()
    .unwrap_or_else(|| panic!("inherited alias"));
    assert_eq!(
        error.message(),
        "\"https://linear.app/foreign/team/eng/cycle/Constructor\" is a Linear URL, but \"Constructor\" is not a cycle number."
    );
}

#[test]
fn url_preparation_tracks_request_variables_across_normalization() {
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    for name in [
        "dot-segment",
        "escaped-segment",
        "schemeless",
        "default-port",
        "url-segment-name",
        "url-uuid",
        "url-miss-original",
        "url-ambiguity-key",
    ] {
        let spec = case(name);
        let prepared = expect_team_url(argument(&spec), &scope)
            .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
            .unwrap_or_else(|| panic!("{} should be a URL", spec["id"]));
        assert_eq!(
            Some(prepared.as_str()),
            first_reference(&spec),
            "{}",
            spec["id"]
        );
    }

    let fake = ConfigSecret::new("lin_api_fake_project".to_owned());
    let key = ApiKeyInput::Sourced {
        value: &fake,
        source: OptionSource::ProjectConfig {
            path: PathBuf::from("/fake/linear.toml"),
        },
    };
    let mut scope = absent_scope(&key);
    scope.sourced_workspace = Some("project");
    assert_url_prepared("workspace-config", &scope);
}

#[test]
fn fallthrough_and_refusal_cases_keep_exact_boundaries() {
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    for name in [
        "port-fallthrough",
        "userinfo-fallthrough",
        "lookalike-fallthrough",
    ] {
        let spec = case(name);
        assert_eq!(
            expect_team_url(argument(&spec), &scope).unwrap_or_else(|e| panic!("{e}")),
            None,
            "{}",
            spec["id"]
        );
    }
    // Dot segments past the team key and unknown descendants are refused
    // before any team lookup.
    scope.cli_workspace = Some("acme");
    for name in ["dot-parent", "dot-percent", "unsupported-foreign"] {
        assert_url_error(name, &scope);
    }
}

#[test]
fn workspace_names_are_trimmed_and_case_folded() {
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some(" Å ");
    assert_eq!(
        expect_team_url("https://linear.app/å/team/eng", &scope)
            .unwrap_or_else(|error| panic!("{error}")),
        Some("ENG".to_owned())
    );
    scope.cli_workspace = Some(" ");
    scope.default_workspace = Some("acme");
    assert_eq!(
        expect_team_url("https://linear.app/foreign/team/eng", &scope)
            .unwrap_or_else(|error| panic!("{error}")),
        Some("ENG".to_owned())
    );
}

#[test]
fn scope_uses_the_sourced_workspace() {
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: Some(("acme", OptionSource::Env)),
    };
    let scope = WorkspaceScope::new(inputs, None);
    let error = expect_team_url("https://linear.app/foreign/team/eng", &scope)
        .err()
        .unwrap_or_else(|| panic!("sourced workspace mismatch"));
    assert!(error.message().contains("this is the \"acme\" workspace"));
}

#[test]
fn sourced_key_and_default_provenance_errors_are_exact() {
    let fake = ConfigSecret::new("lin_api_fake_project".to_owned());
    let empty = ConfigSecret::new(String::new());
    let config_source = OptionSource::ProjectConfig {
        path: PathBuf::from("/fake/linear.toml"),
    };
    let sourced = ApiKeyInput::Sourced {
        value: &fake,
        source: config_source.clone(),
    };
    let empty_sourced = ApiKeyInput::Sourced {
        value: &empty,
        source: config_source,
    };

    let mut scope = absent_scope(&sourced);
    scope.sourced_workspace = Some("project");
    assert_url_error("suggest-config-key", &scope);

    let mut scope = absent_scope(&empty_sourced);
    scope.sourced_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    assert_url_error("suggest-config-empty-key", &scope);

    let mut scope = absent_scope(&sourced);
    scope.default_workspace = Some("ghost");
    assert_url_error("unvalidated-default", &scope);
}

#[test]
fn workspace_priority_and_case_handling() {
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let raw = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let absent = ApiKeyInput::Absent;
    let sourced = ApiKeyInput::Sourced {
        value: &fake,
        source: OptionSource::ProjectConfig {
            path: PathBuf::from("/fake/linear.toml"),
        },
    };

    let mut scope = absent_scope(&raw);
    scope.sourced_workspace = Some(" Acme ");
    assert_url_error("workspace-env-mismatch-case", &scope);

    let mut scope = absent_scope(&raw);
    scope.cli_workspace = Some("acme");
    assert_url_error("wrong-kind-foreign", &scope);

    let mut scope = absent_scope(&absent);
    scope.cli_workspace = Some("acme");
    scope.sourced_workspace = Some("beta");
    scope.default_workspace = Some("acme");
    assert_url_prepared("workspace-cli-over-env", &scope);

    let mut scope = absent_scope(&sourced);
    scope.cli_workspace = Some("cli");
    scope.sourced_workspace = Some("project");
    assert_url_prepared("workspace-cli-precedes-config", &scope);

    let mut scope = absent_scope(&absent);
    scope.sourced_workspace = Some(" Beta ");
    scope.default_workspace = Some("acme");
    assert_url_prepared("workspace-env-trim-case", &scope);

    let mut scope = absent_scope(&raw);
    scope.sourced_workspace = Some("");
    scope.default_workspace = Some("acme");
    assert_url_prepared("workspace-env-empty", &scope);

    let mut scope = absent_scope(&sourced);
    scope.cli_workspace = Some("   ");
    scope.sourced_workspace = Some("acme");
    assert_url_prepared("workspace-whitespace-cli", &scope);
}

#[test]
fn hosts_anchors_and_slug_rules() {
    assert!(matches!(
        parse_linear_url("http://www.linear.app./acme/team/eng#"),
        LinearUrlParse::Known(LinearUrlRef::Team { team_key, .. }) if team_key == "ENG"
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/issue/eng-12#comment-abcdef12"),
        LinearUrlParse::Known(LinearUrlRef::Issue { comment_id_prefix: Some(prefix), .. }) if prefix == "abcdef12"
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/project/ABCDEF123456/activity#project-update-abcdef12"),
        LinearUrlParse::Known(LinearUrlRef::Project { slug_id, .. }) if slug_id == "abcdef123456"
    ));
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-0"),
        LinearUrlParse::Unsupported("\"ENG-0\" is not an issue identifier".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-01"),
        LinearUrlParse::Unsupported("\"ENG-01\" is not an issue identifier".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/project/no-id"),
        LinearUrlParse::Unsupported("\"no-id\" does not end in a Linear slug ID".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-1#other"),
        LinearUrlParse::Unsupported("\"#other\" is not a comment link".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/document/X-abcdef123456#other"),
        LinearUrlParse::Unsupported("\"#other\" is not a link this command can use".to_owned())
    );
}

#[test]
fn descendant_refusals_are_specific() {
    for (input, reason) in [
        (
            "https://linear.app/acme/team/eng/secret",
            "\"secret\" is not a team page this command can use",
        ),
        (
            "https://linear.app/acme/team/eng/Cycle/3",
            "\"Cycle/3\" is not a team page this command can use",
        ),
        (
            "https://linear.app/acme/project/X-abcdef123456/secret",
            "\"secret\" is not a page this command can use",
        ),
        (
            "https://linear.app/acme/team/eng/cycle",
            "it does not name a cycle",
        ),
        (
            "https://linear.app/acme/team/eng/cycle/3/extra",
            "\"3/extra\" is not a cycle page",
        ),
    ] {
        assert_eq!(
            parse_linear_url(input),
            LinearUrlParse::Unsupported(reason.to_owned()),
            "{input}"
        );
    }
}

mod initiative;

mod issue;
