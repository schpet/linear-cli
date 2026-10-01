use linear_cli::config::{
    ConfigInputs, ConfigOptions, OptionInputs, OsFamily, PrTemplateCli, RawConfigFile, SelectedEnv,
    StartupOptionPolicy, parse_config_tier,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn config(
    raw: &str,
    env: &[(&str, &str)],
    global: Option<&str>,
    policy: StartupOptionPolicy,
) -> Result<ConfigOptions, linear_cli::config::ConfigOptionError> {
    let env = ConfigInputs {
        cwd: PathBuf::from("/dummy/sub"),
        os: OsFamily::Unix,
        process_env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    };
    let dotenv = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: vec![],
    };
    let tier = |path: &str, s: &str| {
        parse_config_tier(RawConfigFile {
            path: PathBuf::from(path),
            bytes: s.as_bytes().to_vec(),
        })
        .unwrap()
    };
    let project = tier("/dummy/.linear.toml", raw);
    let global = global.map(|raw| tier("/global/.linear.toml", raw));
    ConfigOptions::from_inputs_with_startup_policy(
        OptionInputs {
            env: &env,
            dotenv: &dotenv,
            project: Some(&project),
            global: global.as_ref(),
        },
        policy,
    )
}
#[test]
fn pr_validates_only_highest_present_raw_option_at_action_with_override_bypass() {
    let options = config(
        "pr_template = 5",
        &[],
        None,
        StartupOptionPolicy::PullRequestTemplate,
    )
    .unwrap();
    assert!(
        options
            .pull_request_template(PrTemplateCli::Disabled)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        options
            .pull_request_template(PrTemplateCli::Path(" explicit "))
            .unwrap()
            .unwrap()
            .path(),
        Path::new("explicit")
    );
    assert_eq!(
        options
            .pull_request_template(PrTemplateCli::Unset)
            .unwrap_err()
            .message,
        "Invalid pull request template: 5"
    );
    assert_eq!(
        options
            .pull_request_template(PrTemplateCli::Path(" "))
            .unwrap_err()
            .message,
        "Invalid pull request template: \" \""
    );
    let higher = config(
        "pr_template = 5",
        &[("LINEAR_PR_TEMPLATE", "env.md")],
        Some("pr_template = false"),
        StartupOptionPolicy::PullRequestTemplate,
    )
    .unwrap();
    assert_eq!(
        higher
            .pull_request_template(PrTemplateCli::Unset)
            .unwrap()
            .unwrap()
            .path(),
        Path::new("env.md")
    );
    let empty = config(
        "pr_template = \"valid\"",
        &[("LINEAR_PR_TEMPLATE", "")],
        None,
        StartupOptionPolicy::PullRequestTemplate,
    )
    .unwrap();
    assert_eq!(
        empty
            .pull_request_template(PrTemplateCli::Unset)
            .unwrap_err()
            .message,
        "Invalid pull request template: \"\""
    );
    let relative = config(
        "pr_template = \".github/template.md\"",
        &[],
        None,
        StartupOptionPolicy::PullRequestTemplate,
    )
    .unwrap();
    assert_eq!(
        relative
            .pull_request_template(PrTemplateCli::Unset)
            .unwrap()
            .unwrap()
            .path(),
        Path::new("/dummy/.github/template.md")
    );
}
#[test]
fn unrelated_callers_retain_all_tier_eager_validation_and_start_sort_stays_typed() {
    assert!(
        config(
            "pr_template = 5",
            &[("LINEAR_PR_TEMPLATE", "valid")],
            None,
            StartupOptionPolicy::Eager
        )
        .is_err()
    );
    assert!(
        config(
            "issue_sort = \"bad\"",
            &[],
            None,
            StartupOptionPolicy::Eager
        )
        .is_err()
    );
    let options = config(
        "issue_sort = \"bad\"",
        &[],
        None,
        StartupOptionPolicy::IssueSort,
    )
    .unwrap();
    assert_eq!(
        options.issue_read_sort(None).unwrap_err().message,
        "Invalid issue sort: \"bad\""
    );
}
