use linear_cli::auth::{ApiKeyInput, CredentialManifest, CredentialSelectionInputs, hydrate};
use linear_cli::config::{ConfigSecret, OptionSource};
use linear_cli::error::AppErrorKind;
use linear_cli::refs::{
    CycleSelector, LinearUrlParse, LinearUrlRef, WorkspaceScope, expect_team_url, parse_linear_url,
};
use serde_json::Value;
use std::path::PathBuf;

const URL_ACCEPTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-url-accepted.json"
));
const MALFORMED_ESCAPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-malformed-escape.json"
));
const SUGGEST_PLAIN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-suggest-plain.json"
));
const SUGGEST_RAW_EMPTY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-suggest-raw-empty.json"
));
const FOREIGN_BEFORE_KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-foreign-before-key.json"
));
const EMPTY_CONFIG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/runner/f06-teamref-frozen-cases/f06e0-workspace-empty-config.json"
));

macro_rules! e0_case {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../parity/runner/f06-teamref-frozen-cases/",
            $name,
            ".json"
        ))
    };
}

mod shared;
mod team;

fn case(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|error| panic!("frozen E0 JSON: {error}"))
}

fn argument(spec: &Value) -> &str {
    let args = spec["argv"].as_array().unwrap_or_else(|| panic!("E0 argv"));
    let position = args
        .iter()
        .position(|value| value.as_str() == Some("members"))
        .unwrap_or_else(|| panic!("E0 members route"));
    args.get(position + 1)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("E0 argv reference"))
}

fn expected_error(spec: &Value) -> (&str, Option<&str>) {
    let stderr = spec["expected"]["stderr"]["utf8"]
        .as_str()
        .unwrap_or_else(|| panic!("E0 stderr"));
    let body = stderr
        .strip_prefix("✗ Failed to fetch team members: ")
        .and_then(|body| body.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("E0 carrier prefix or LF"));
    match body.split_once("\n  ") {
        Some((message, suggestion)) => (message, Some(suggestion)),
        None => (body, None),
    }
}

fn absent_scope<'a>(key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: key,
    }
}

fn assert_e0_error(raw: &str, scope: &WorkspaceScope<'_>) {
    let spec = case(raw);
    let error = expect_team_url(argument(&spec), scope)
        .err()
        .unwrap_or_else(|| panic!("{} should fail", spec["id"]));
    let (message, suggestion) = expected_error(&spec);
    assert_eq!(error.kind, AppErrorKind::Validation, "{}", spec["id"]);
    assert_eq!(error.message, message, "{}", spec["id"]);
    assert_eq!(error.suggestion.as_deref(), suggestion, "{}", spec["id"]);
    assert_eq!(error.context, None, "{}", spec["id"]);
}

fn assert_e0_prepared(raw: &str, scope: &WorkspaceScope<'_>) {
    let spec = case(raw);
    let prepared = expect_team_url(argument(&spec), scope)
        .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]));
    assert_eq!(
        prepared.as_deref(),
        spec["graphql"]["groups"][0]["steps"][0]["operation"]["variables"]["reference"].as_str(),
        "{}",
        spec["id"]
    );
}

#[test]
fn e0_success_pins_canonical_team_reference() {
    // E0 f06e0-url-accepted SHA 67ca2a98a9b435278999443c4b0e002415fcb0da74acff87edffda251b830862.
    let spec = case(URL_ACCEPTED);
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    let actual = expect_team_url(argument(&spec), &scope).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        actual.as_deref(),
        spec["graphql"]["groups"][0]["steps"][0]["operation"]["variables"]["reference"].as_str()
    );
}

#[test]
fn e0_unsupported_error_is_exact() {
    // E0 f06e0-malformed-escape SHA 6abf2f4b846f7a34931c6dca60e960554e45060893146ae09f61c703670af54b.
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    assert_e0_error(MALFORMED_ESCAPE, &scope);
}

#[test]
fn e0_workspace_suggestions_use_key_provenance_even_for_empty_values() {
    // E0 f06e0-suggest-plain SHA 4e2ba589c1b8a2dde22a77a4052dca23b53c5fe7e2a66b65a2903a2240f4e702.
    let absent = ApiKeyInput::Absent;
    let mut scope = absent_scope(&absent);
    scope.sourced_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    assert_e0_error(SUGGEST_PLAIN, &scope);

    // E0 f06e0-suggest-raw-empty SHA ab7a638a7ca46fb7df85879a69a5a9f689c3db45983c9a3ba3ca4a180befd978.
    let empty = ConfigSecret::new(String::new());
    let key = ApiKeyInput::Raw {
        value: &empty,
        source: OptionSource::ProjectEnv {
            path: PathBuf::from("/fake/.env"),
        },
    };
    let mut scope = absent_scope(&key);
    scope.sourced_workspace = Some("acme");
    assert_e0_error(SUGGEST_RAW_EMPTY, &scope);
}

#[test]
fn e0_workspace_check_precedes_later_key_conflict_and_empty_config_shadows_default() {
    // E0 f06e0-foreign-before-key SHA 11ed4c1f249b9649f04015fd2f63ff7f2231db793c1de9217335ada4b68d8524.
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    assert_e0_error(FOREIGN_BEFORE_KEY, &scope);

    // E0 f06e0-workspace-empty-config SHA 5fd88570c7b8256b8ff17c62e3b4387c7630628474a0c373632715b149cee028.
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
    assert_e0_prepared(EMPTY_CONFIG, &scope);
}

#[test]
fn typed_url_kinds_and_source_backed_boundaries() {
    assert!(matches!(
        parse_linear_url("plain name"),
        LinearUrlParse::NotLinear
    ));
    assert!(matches!(
        parse_linear_url("\u{feff}  "),
        LinearUrlParse::NotLinear
    ));
    assert!(matches!(
        parse_linear_url("\u{0085}"),
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
fn url_normalization_and_error_order_follow_frozen_source() {
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
            .message
            .starts_with("That URL is for the \"foreign\" workspace")
    );
    let error = expect_team_url("https://linear.app/acme/project/X-ABCDEF123456", &scope)
        .err()
        .unwrap_or_else(|| panic!("wrong kind"));
    assert_eq!(
        error.message,
        "\"https://linear.app/acme/project/X-ABCDEF123456\" is a project URL, not a team URL."
    );
    // Deliberate source-backed difference: Deno's plain alias object inherits
    // `constructor` and checks workspace before kind; Rust rejects it as invalid.
    let error = expect_team_url(
        "https://linear.app/foreign/team/eng/cycle/Constructor",
        &scope,
    )
    .err()
    .unwrap_or_else(|| panic!("inherited alias"));
    assert_eq!(
        error.message,
        "\"https://linear.app/foreign/team/eng/cycle/Constructor\" is a Linear URL, but \"Constructor\" is not a cycle number."
    );
}

#[test]
fn e0_url_preparation_tracks_request_variables_across_normalization() {
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    // Each file's full hash is in the E0 manifest; these are the exact public
    // request variables, before B's team lookup. E0 dot-segment e757d6cea304dae6d787dee9f2ab433f3c5af846f4db578634a0b5ec0e5d64f6,
    // escaped-segment cd7bec18aee82d0cc4ce643cad7f282a4b084a9c5f3bf529fb421343f0363991, schemeless 14a1952195b6d68205632c2e2ed71ddb674fe7f1bc641442c173bcae39b10d28,
    // default-port 0830700415cc243fd14d12a30dfd1dd0465e5bc2c6d48625b2575b50654d5e11,
    // URL-segment-name a4a7ad04d7e0011325cc02633cfc043dedeb559ed295e370981fb0acbc8e4af3,
    // URL UUID c07c2bdfd0159a3f419327a09bbcaf71faee7faa952e9147498b9d947a2842bd,
    // URL miss c126a5d46ca6d0f0255a2b327de9be542acc4007fa586147b3d06b10918017ae,
    // and URL ambiguity 532e642da580033f234bb94a87a5a9f83d29b2c2872d7a4ba97f8010a5b5441c.
    for raw in [
        e0_case!("f06e0-dot-segment"),
        e0_case!("f06e0-escaped-segment"),
        e0_case!("f06e0-schemeless"),
        e0_case!("f06e0-default-port"),
        e0_case!("f06e0-url-segment-name"),
        e0_case!("f06e0-url-uuid"),
        e0_case!("f06e0-url-miss-original"),
        e0_case!("f06e0-url-ambiguity-key"),
    ] {
        let spec = case(raw);
        let prepared = expect_team_url(argument(&spec), &scope)
            .unwrap_or_else(|error| panic!("{}: {error}", spec["id"]))
            .unwrap_or_else(|| panic!("{} should be a URL", spec["id"]));
        assert_eq!(
            Some(prepared.as_str()),
            spec["graphql"]["groups"][0]["steps"][0]["operation"]["variables"]["reference"]
                .as_str(),
            "{}",
            spec["id"]
        );
    }

    // E0 workspace-config SHA 23d63ca650ffe7d3e6e4fe80e63e0831163a5a4cf3123b50d2df8c92393116ab.
    let fake = ConfigSecret::new("lin_api_fake_project".to_owned());
    let key = ApiKeyInput::Sourced {
        value: &fake,
        source: OptionSource::ProjectConfig {
            path: PathBuf::from("/fake/linear.toml"),
        },
    };
    let mut scope = absent_scope(&key);
    scope.sourced_workspace = Some("project");
    assert_e0_prepared(e0_case!("f06e0-workspace-config"), &scope);
}

#[test]
fn e0_fallthrough_and_refusal_cases_keep_exact_boundaries() {
    let fake = ConfigSecret::new("lin_api_fake".to_owned());
    let key = ApiKeyInput::Raw {
        value: &fake,
        source: OptionSource::Env,
    };
    let mut scope = absent_scope(&key);
    // E0 port-fallthrough 96f3f56ef0a40961caf373d6d83b21cdfd2f0fa671fca6ab22737269e805fa20, userinfo-fallthrough b149856ca7f820984da5f3622883ce9ebb7ecc7f7e3b423c28a92d69a10b8668,
    // lookalike-fallthrough c7587a949761d5a236ccbf02678d4804a20cacfbd1a961f2a00edc7d00060cfa: B receives the original text unchanged.
    for raw in [
        e0_case!("f06e0-port-fallthrough"),
        e0_case!("f06e0-userinfo-fallthrough"),
        e0_case!("f06e0-lookalike-fallthrough"),
    ] {
        let spec = case(raw);
        assert_eq!(
            expect_team_url(argument(&spec), &scope).unwrap_or_else(|e| panic!("{e}")),
            None,
            "{}",
            spec["id"]
        );
    }
    // E0 dot-parent e35fd50a6896d3dc37f268e142d25e165947266dff3bd80916f9bad0cb8f4bdd, dot-percent 3992010d08a0157ca03f69b62ca813e0d76ddd9ddf64dc2fb4d8f47f345b4520 and unsupported-foreign e6c65e070e04bb9573074cebfbe235f1be37db75b34b5c8ba2fc008139ec119e
    // refuse before a team lookup, with the exact resolver-level error text.
    scope.cli_workspace = Some("acme");
    for raw in [
        e0_case!("f06e0-dot-parent"),
        e0_case!("f06e0-dot-percent"),
        e0_case!("f06e0-unsupported-foreign"),
    ] {
        assert_e0_error(raw, &scope);
    }
}

#[test]
fn unicode_workspace_and_javascript_whitespace_are_source_backed() {
    let key = ApiKeyInput::Absent;
    let mut scope = absent_scope(&key);
    scope.cli_workspace = Some("\u{feff}Å\u{feff}");
    assert_eq!(
        expect_team_url("https://linear.app/å/team/eng", &scope)
            .unwrap_or_else(|error| panic!("{error}")),
        Some("ENG".to_owned())
    );
    scope.cli_workspace = Some("\u{feff}");
    scope.default_workspace = Some("acme");
    assert_eq!(
        expect_team_url("https://linear.app/foreign/team/eng", &scope)
            .unwrap_or_else(|error| panic!("{error}")),
        Some("ENG".to_owned())
    );
    scope.cli_workspace = Some("\u{0085}");
    let error = expect_team_url("https://linear.app/foreign/team/eng", &scope)
        .err()
        .unwrap_or_else(|| panic!("NEL is not JavaScript whitespace"));
    assert!(error.message.contains("this is the \"\u{0085}\" workspace"));
}

#[test]
fn scope_borrows_existing_credential_selection_inputs() {
    let store = hydrate(CredentialManifest::empty(), Vec::new())
        .unwrap_or_else(|error| panic!("empty fake store: {error}"));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: Some(("acme", OptionSource::Env)),
    };
    let scope = WorkspaceScope::from_selection(&inputs, &store);
    let error = expect_team_url("https://linear.app/foreign/team/eng", &scope)
        .err()
        .unwrap_or_else(|| panic!("sourced workspace mismatch"));
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("this is the \"acme\" workspace"));
}

#[test]
fn e0_sourced_key_and_default_provenance_errors_are_exact() {
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

    // E0 suggest-config-key fb894dacf2e054da46b8853d0bab68f34ba5247800775f1705ff1f9d549fb33e.
    let mut scope = absent_scope(&sourced);
    scope.sourced_workspace = Some("project");
    assert_e0_error(e0_case!("f06e0-suggest-config-key"), &scope);

    // E0 suggest-config-empty-key 9ca3837e08966609219ed90b60a9b44ee93fb60c12bab34bc8a82f1ca2b5db09.
    let mut scope = absent_scope(&empty_sourced);
    scope.sourced_workspace = Some("acme");
    scope.default_workspace = Some("acme");
    assert_e0_error(e0_case!("f06e0-suggest-config-empty-key"), &scope);

    // E0 unvalidated-default a6345ee6b96f1c83a0497c7cbb81428b77c3bf55d1c76b637638da8879265285.
    let mut scope = absent_scope(&sourced);
    scope.default_workspace = Some("ghost");
    assert_e0_error(e0_case!("f06e0-unvalidated-default"), &scope);
}

#[test]
fn e0_workspace_priority_and_case_rows_are_exact() {
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

    // E0 workspace-env-mismatch-case 8147435ca055d1a3a1943c9799cf2230aa7693faf8b46683e56f25425f1a0c34.
    let mut scope = absent_scope(&raw);
    scope.sourced_workspace = Some(" Acme ");
    assert_e0_error(e0_case!("f06e0-workspace-env-mismatch-case"), &scope);

    // E0 wrong-kind-foreign a98e910b502049bf1e6f40f90db85eebf900e5c010d1b43471ff6d1773f56306.
    let mut scope = absent_scope(&raw);
    scope.cli_workspace = Some("acme");
    assert_e0_error(e0_case!("f06e0-wrong-kind-foreign"), &scope);

    // E0 workspace-cli-over-env e3a4375f327777a90ff55e347dc7b4b1f8637cfd30839af667e68f37d08a3cb9.
    let mut scope = absent_scope(&absent);
    scope.cli_workspace = Some("acme");
    scope.sourced_workspace = Some("beta");
    scope.default_workspace = Some("acme");
    assert_e0_prepared(e0_case!("f06e0-workspace-cli-over-env"), &scope);

    // E0 workspace-cli-precedes-config b993e631c4b4e94da224e231215033be150fefc8c792e081adfd5b9b5a4d60d8.
    let mut scope = absent_scope(&sourced);
    scope.cli_workspace = Some("cli");
    scope.sourced_workspace = Some("project");
    assert_e0_prepared(e0_case!("f06e0-workspace-cli-precedes-config"), &scope);

    // E0 workspace-env-trim-case c972c15b1959a6b58e11fd1b0a3d693276e1186f0ccf07f515c8bd767bad4b8a.
    let mut scope = absent_scope(&absent);
    scope.sourced_workspace = Some(" Beta ");
    scope.default_workspace = Some("acme");
    assert_e0_prepared(e0_case!("f06e0-workspace-env-trim-case"), &scope);

    // E0 workspace-env-empty f42cb9844655e7ad8cb0fe6aa938809efc190f0ab180b685ef2f3476713e9007.
    let mut scope = absent_scope(&raw);
    scope.sourced_workspace = Some("");
    scope.default_workspace = Some("acme");
    assert_e0_prepared(e0_case!("f06e0-workspace-env-empty"), &scope);

    // E0 workspace-whitespace-cli 4fb09875d4c94731847579449707e53704fd2c3cebbab13b3bf82a3ba34db351.
    let mut scope = absent_scope(&sourced);
    scope.cli_workspace = Some("   ");
    scope.sourced_workspace = Some("acme");
    assert_e0_prepared(e0_case!("f06e0-workspace-whitespace-cli"), &scope);
}

#[test]
fn source_backed_hosts_anchors_and_slug_rules() {
    // Frozen linear-url.ts SHA a64eb8e7ecbfe05b5bf030b817df6d89cd060b145bfdcfcf255a6e47b50cb5f5,
    // lines 117-128, 204-320. These cases are not claimed as E0 observations.
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
fn source_backed_descendant_refusals_are_specific() {
    // Frozen linear-url.ts SHA a64eb8e7ecbfe05b5bf030b817df6d89cd060b145bfdcfcf255a6e47b50cb5f5,
    // lines 154-201 and 247-320. E0 covers only selected team URL descendants.
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
