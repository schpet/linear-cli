use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use linear_cli::config::{
    AssignSelf, ConfigInputs, ConfigOptionError, ConfigOptions, ConfigTier, EndpointSource,
    IssueSort, OptionErrorReason, OptionInputs, OptionKey, OptionSource, OsFamily, PrTemplateCli,
    RawConfigFile, SelectedEnv, Vcs, parse_config_tier,
};

fn tier(path: &str, content: &str) -> ConfigTier {
    parse_config_tier(RawConfigFile {
        path: PathBuf::from(path),
        bytes: content.as_bytes().to_vec(),
    })
    .expect("valid fixture")
}
fn env(values: &[(&str, &str)]) -> ConfigInputs {
    ConfigInputs {
        cwd: PathBuf::from("/repo/sub"),
        os: OsFamily::Unix,
        process_env: values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    }
}
fn dotenv(values: &[(&str, &str)]) -> SelectedEnv {
    SelectedEnv {
        applied: values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
        source_path: Some(PathBuf::from("/repo/.env")),
        diagnostics: Vec::new(),
    }
}
fn snapshot<'a>(
    env: &'a ConfigInputs,
    dotenv: &'a SelectedEnv,
    project: Option<&'a ConfigTier>,
    global: Option<&'a ConfigTier>,
) -> Result<ConfigOptions, ConfigOptionError> {
    ConfigOptions::from_inputs(OptionInputs {
        env,
        dotenv,
        project,
        global,
    })
}

#[test]
fn all_twelve_keys_are_typed_and_keep_provenance() {
    let config = tier(
        "/repo/linear.toml",
        r#"
team_id = "ENG"
api_key = "lin_api_fake"
workspace = "my-workspace"
issue_sort = "manual"
issue_create_ask_project = true
issue_create_assign_self = "auto"
vcs = "jj"
download_images = "YES"
hyperlink_format = "osc8"
attachment_dir = "attachments"
auto_download_attachments = false
pr_template = "  .github/pr.md  "
"#,
    );
    let inputs = env(&[]);
    let selected = dotenv(&[]);
    let options = snapshot(&inputs, &selected, Some(&config), None).expect("valid options");
    assert_eq!(OptionKey::ALL.len(), 12);
    assert_eq!(options.team_id().expect("team").value(), "ENG");
    assert_eq!(
        options.api_key().expect("key").value().expose(),
        "lin_api_fake"
    );
    assert_eq!(
        options.workspace().expect("workspace").value(),
        "my-workspace"
    );
    assert_eq!(
        options.sourced_issue_sort().expect("sort").value(),
        &IssueSort::Manual
    );
    assert_eq!(
        options.issue_create_ask_project().expect("ask").value(),
        &true
    );
    assert_eq!(
        options.issue_create_assign_self().expect("assign").value(),
        &AssignSelf::Auto
    );
    assert_eq!(options.vcs().expect("vcs").value(), &Vcs::Jj);
    assert_eq!(options.download_images().expect("images").value(), &true);
    assert_eq!(options.hyperlink_format().expect("format").value(), "osc8");
    assert_eq!(
        options.attachment_dir().expect("attachments").value(),
        "attachments"
    );
    assert_eq!(
        options.auto_download_attachments().expect("auto").value(),
        &false
    );
    assert_eq!(
        options.sourced_pr_template().expect("template").value(),
        ".github/pr.md"
    );
    assert_eq!(
        options.team_id().expect("team").source(),
        &OptionSource::ProjectConfig {
            path: PathBuf::from("/repo/linear.toml")
        }
    );
    assert_eq!(
        options
            .pr_template(PrTemplateCli::Unset)
            .expect("template")
            .expect("path")
            .path(),
        Path::new("/repo/.github/pr.md")
    );
}

#[test]
fn precedence_and_shadowed_poison_are_strict() {
    let global = tier(
        "/global/linear.toml",
        "team_id = 'global'\nissue_sort = 'manual'",
    );
    let project = tier(
        "/repo/linear.toml",
        "team_id = 'project'\nissue_sort = 'priority'",
    );
    let inputs = env(&[("LINEAR_TEAM_ID", "process")]);
    let selected = dotenv(&[("LINEAR_TEAM_ID", "dotenv")]);
    let options = snapshot(&inputs, &selected, Some(&project), Some(&global)).expect("valid");
    assert_eq!(options.team_id().expect("team").value(), "process");
    assert_eq!(
        options.team_id().expect("team").source(),
        &OptionSource::Env
    );
    let options = snapshot(&env(&[]), &selected, Some(&project), Some(&global)).expect("valid");
    assert_eq!(options.team_id().expect("team").value(), "dotenv");
    assert_eq!(
        options.team_id().expect("team").source(),
        &OptionSource::ProjectEnv {
            path: PathBuf::from("/repo/.env")
        }
    );
    let options = snapshot(&env(&[]), &dotenv(&[]), Some(&project), Some(&global))
        .expect("project beats global");
    assert_eq!(options.team_id().expect("team").value(), "project");
    assert_eq!(
        options.sourced_issue_sort().expect("sort").value(),
        &IssueSort::Priority
    );
    assert_eq!(
        options.issue_sort(Some(IssueSort::Manual)),
        (IssueSort::Manual, Some(OptionSource::Cli))
    );
    let poison = tier("/global/linear.toml", "issue_sort = 'INVALID'");
    let err =
        snapshot(&inputs, &selected, Some(&project), Some(&poison)).expect_err("shadowed poison");
    assert_eq!(err.key, Some(OptionKey::IssueSort));
    assert_eq!(
        err.source,
        OptionSource::GlobalConfig {
            path: PathBuf::from("/global/linear.toml")
        }
    );
    assert!(matches!(err.reason, OptionErrorReason::Invalid(_)), "{err}");
    assert!(
        err.to_string().contains("unknown variant `INVALID`"),
        "{err}"
    );
}

#[test]
fn every_known_key_is_eagerly_checked_and_unknown_keys_are_inert() {
    for key in OptionKey::ALL {
        let content = format!("{} = 42", key.name());
        let project = tier("/repo/linear.toml", &content);
        let err = snapshot(&env(&[]), &dotenv(&[]), Some(&project), None).expect_err("wrong type");
        assert_eq!(err.key, Some(key));
        assert!(matches!(err.reason, OptionErrorReason::Invalid(_)), "{err}");
        assert!(err.to_string().contains("integer `42`"), "{err}");
    }
    let unknown = tier("/repo/linear.toml", "new_future_key = 42");
    snapshot(&env(&[]), &dotenv(&[]), Some(&unknown), None).expect("unknown inert");
}

#[test]
fn booleans_and_enums_reject_invalid_strings_without_trimming() {
    for word in ["true", "YES", "y", "ON", "1", "T"] {
        let inputs = env(&[("LINEAR_DOWNLOAD_IMAGES", word)]);
        assert_eq!(
            snapshot(&inputs, &dotenv(&[]), None, None)
                .expect("true word")
                .download_images()
                .expect("present")
                .value(),
            &true
        );
    }
    for word in ["false", "NO", "n", "OFF", "0", "F"] {
        let inputs = env(&[("LINEAR_DOWNLOAD_IMAGES", word)]);
        assert_eq!(
            snapshot(&inputs, &dotenv(&[]), None, None)
                .expect("false word")
                .download_images()
                .expect("present")
                .value(),
            &false
        );
    }
    for word in ["", " true", "1 ", "maybe"] {
        let inputs = env(&[("LINEAR_DOWNLOAD_IMAGES", word)]);
        let err = snapshot(&inputs, &dotenv(&[]), None, None).expect_err("invalid bool");
        assert!(matches!(err.reason, OptionErrorReason::Invalid(_)), "{err}");
        assert!(err.to_string().ends_with("expected a boolean"), "{err}");
    }
    for (name, value) in [
        ("LINEAR_ISSUE_SORT", "Manual"),
        ("LINEAR_ISSUE_CREATE_ASSIGN_SELF", "Auto"),
        ("LINEAR_VCS", "GIT"),
    ] {
        let inputs = env(&[(name, value)]);
        let err = snapshot(&inputs, &dotenv(&[]), None, None).expect_err("case sensitive");
        assert!(matches!(err.reason, OptionErrorReason::Invalid(_)), "{err}");
    }
}

#[test]
fn empty_strings_are_present_and_cli_overrides_are_closed() {
    let inputs = env(&[
        ("LINEAR_TEAM_ID", ""),
        ("LINEAR_API_KEY", ""),
        ("LINEAR_ATTACHMENT_DIR", ""),
    ]);
    let options = snapshot(&inputs, &dotenv(&[]), None, None).expect("valid empty strings");
    assert_eq!(options.team_id().expect("present").value(), "");
    assert_eq!(options.api_key().expect("present").value().expose(), "");
    assert_eq!(options.attachment_dir().expect("present").value(), "");
    assert_eq!(options.issue_sort(None), (IssueSort::Priority, None));
    assert_eq!(
        options.issue_sort(Some(IssueSort::Manual)),
        (IssueSort::Manual, Some(OptionSource::Cli))
    );
    assert!(
        options
            .pr_template(PrTemplateCli::Disabled)
            .expect("disabled")
            .is_none()
    );
    let template = options
        .pr_template(PrTemplateCli::Path(" ./x "))
        .expect("cli path")
        .expect("present");
    assert_eq!(template.path(), Path::new("./x"));
    assert_eq!(template.source(), &OptionSource::Cli);
    let error = options
        .pr_template(PrTemplateCli::Path("  "))
        .expect_err("empty CLI path");
    assert_eq!(error.source, OptionSource::Cli);
    assert_eq!(error.reason, OptionErrorReason::EmptyTemplate);
}

#[test]
fn template_paths_resolve_against_their_config_file() {
    let global = tier(
        "/global/linear.toml",
        "pr_template = '../../x'\nattachment_dir = 'files'",
    );
    let options = snapshot(&env(&[]), &dotenv(&[]), None, Some(&global)).expect("global");
    assert_eq!(
        options
            .pr_template(PrTemplateCli::Unset)
            .expect("path")
            .expect("present")
            .path(),
        Path::new("/x")
    );
    assert_eq!(
        options.attachment_dir().expect("attachment").value(),
        "files"
    );
    let relative_global = tier("../cfg/linear/linear.toml", "pr_template = 't.md'");
    let options = snapshot(&env(&[]), &dotenv(&[]), None, Some(&relative_global))
        .expect("relative global path");
    assert_eq!(
        options
            .pr_template(PrTemplateCli::Unset)
            .expect("path")
            .expect("present")
            .path(),
        Path::new("/repo/cfg/linear/t.md")
    );
    let env_options = snapshot(
        &env(&[("LINEAR_PR_TEMPLATE", "relative.md")]),
        &dotenv(&[]),
        None,
        None,
    )
    .expect("env path");
    assert_eq!(
        env_options
            .pr_template(PrTemplateCli::Unset)
            .expect("path")
            .expect("present")
            .path(),
        Path::new("relative.md")
    );
    let selected = dotenv(&[("LINEAR_PR_TEMPLATE", "dotenv.md")]);
    let dotenv_options = snapshot(&env(&[]), &selected, None, None).expect("dotenv path");
    assert_eq!(
        dotenv_options
            .pr_template(PrTemplateCli::Unset)
            .expect("path")
            .expect("present")
            .path(),
        Path::new("dotenv.md")
    );
    let inputs = env(&[("LINEAR_PR_TEMPLATE", "\u{feff}x\u{feff}")]);
    let options = snapshot(&inputs, &dotenv(&[]), None, None).expect("trimmed");
    assert_eq!(
        options.sourced_pr_template().expect("template").value(),
        "x"
    );
    let inputs = env(&[("LINEAR_PR_TEMPLATE", "\u{85}")]);
    assert_eq!(
        snapshot(&inputs, &dotenv(&[]), None, None)
            .expect("NEL not trimmed")
            .sourced_pr_template()
            .expect("template")
            .value(),
        "\u{85}"
    );
}

#[test]
fn empty_template_is_rejected_in_every_tier_including_shadowed() {
    let global = tier("/global/linear.toml", "pr_template = '   '");
    let project = tier("/repo/linear.toml", "pr_template = 'valid.md'");
    let inputs = env(&[("LINEAR_PR_TEMPLATE", "also-valid.md")]);
    let error =
        snapshot(&inputs, &dotenv(&[]), Some(&project), Some(&global)).expect_err("shadowed empty");
    assert_eq!(
        error.source,
        OptionSource::GlobalConfig {
            path: PathBuf::from("/global/linear.toml")
        }
    );
    assert_eq!(error.reason, OptionErrorReason::EmptyTemplate);
    let inputs = env(&[("LINEAR_PR_TEMPLATE", "")]);
    assert_eq!(
        snapshot(&inputs, &dotenv(&[]), None, None)
            .expect_err("env empty")
            .reason,
        OptionErrorReason::EmptyTemplate
    );
    let selected = dotenv(&[("LINEAR_PR_TEMPLATE", "")]);
    assert_eq!(
        snapshot(&env(&[]), &selected, None, None)
            .expect_err("dotenv empty")
            .reason,
        OptionErrorReason::EmptyTemplate
    );
}

#[test]
fn endpoint_default_empty_valid_invalid_and_redaction() {
    let options = snapshot(&env(&[]), &dotenv(&[]), None, None).expect("default");
    assert_eq!(options.endpoint().source(), &EndpointSource::Default);
    assert_eq!(
        options.endpoint().value().url().as_str(),
        "https://api.linear.app/graphql"
    );
    let inputs = env(&[("LINEAR_GRAPHQL_ENDPOINT", "")]);
    let options = snapshot(&inputs, &dotenv(&[]), None, None).expect("empty default");
    assert_eq!(options.endpoint().source(), &EndpointSource::Default);
    let inputs = env(&[(
        "LINEAR_GRAPHQL_ENDPOINT",
        "https://example.com/graphql?token=lin_api_fake",
    )]);
    let options = snapshot(&inputs, &dotenv(&[]), None, None).expect("valid URL");
    assert_eq!(options.endpoint().source(), &EndpointSource::Env);
    assert!(!format!("{options:?}").contains("lin_api_fake"));
    let selected = dotenv(&[("LINEAR_GRAPHQL_ENDPOINT", "https://example.com/graphql")]);
    let options = snapshot(&env(&[]), &selected, None, None).expect("dotenv endpoint");
    assert_eq!(
        options.endpoint().source(),
        &EndpointSource::ProjectEnv {
            path: PathBuf::from("/repo/.env")
        }
    );
    let inputs = env(&[("LINEAR_GRAPHQL_ENDPOINT", "lin_api_fake://invalid")]);
    let err = snapshot(&inputs, &dotenv(&[]), None, None).expect_err("invalid endpoint");
    assert_eq!(err.reason, OptionErrorReason::InvalidEndpoint);
    assert!(!format!("{err:?} {err}").contains("lin_api_fake"));
}

#[test]
fn dotenv_only_applied_values_count_and_secret_never_formats() {
    let selected = dotenv(&[
        ("LINEAR_TEAM_ID", "dotenv"),
        ("LINEAR_API_KEY", "lin_api_fake"),
    ]);
    let options = snapshot(&env(&[]), &selected, None, None).expect("dotenv");
    assert_eq!(
        options.team_id().expect("team").source(),
        &OptionSource::ProjectEnv {
            path: PathBuf::from("/repo/.env")
        }
    );
    assert!(
        !format!(
            "{options:?} {:?} {:?}",
            options.api_key(),
            options.api_key().expect("key").value()
        )
        .contains("lin_api_fake")
    );
    let selected = dotenv(&[]);
    let inputs = env(&[("LINEAR_API_KEY", "lin_api_fake")]);
    let options = snapshot(&inputs, &selected, None, None).expect("process");
    assert_eq!(options.api_key().expect("key").source(), &OptionSource::Env);
    assert!(!format!("{options:?}").contains("lin_api_fake"));
}

#[test]
fn windows_dotenv_lookup_is_case_insensitive() {
    let mut inputs = env(&[]);
    inputs.os = OsFamily::Windows;
    let selected = SelectedEnv {
        applied: BTreeMap::from([("LINEAR_team_id".to_owned(), "ENG".to_owned())]),
        source_path: Some(PathBuf::from("C:/repo/.env")),
        diagnostics: Vec::new(),
    };
    let options = snapshot(&inputs, &selected, None, None).expect("Windows case");
    assert_eq!(options.team_id().expect("team").value(), "ENG");
    inputs
        .process_env
        .insert("LINEAR_TEAM_ID".to_owned(), "process".to_owned());
    let options = snapshot(&inputs, &selected, None, None)
        .expect("process wins without validating unapplied dotenv duplicate");
    assert_eq!(options.team_id().expect("team").value(), "process");
}

#[test]
fn relative_injected_cwd_is_rejected_before_path_resolution() {
    let mut inputs = env(&[]);
    inputs.cwd = PathBuf::from("relative");
    let error = snapshot(&inputs, &dotenv(&[]), None, None).expect_err("cwd must be absolute");
    assert_eq!(error.reason, OptionErrorReason::InvalidCwd);
}
