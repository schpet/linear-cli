use std::path::PathBuf;

use super::*;

fn tier(path: &str, text: &str) -> ConfigTier {
    ConfigTier {
        path: PathBuf::from(path),
        table: text.parse().expect("test TOML"),
    }
}

fn commit() -> CommitContext {
    CommitContext {
        issue: IssueData {
            identifier: "ENG-123".to_owned(),
            title: "Fix login".to_owned(),
            url: "https://linear.app/acme/issue/ENG-123/fix-login".to_owned(),
            description: "Details".to_owned(),
        },
        linkage: "Fixes".to_owned(),
        references: false,
    }
}

#[test]
fn defaults_preserve_commit_and_pull_request_text() {
    assert_default_commit_and_pull_request_text(&TemplateConfig::builtin());
}

fn assert_default_commit_and_pull_request_text(config: &TemplateConfig) {
    let context = commit();
    assert_eq!(
        config
            .render(Slot::CommitDescription, &context)
            .expect("render"),
        "ENG-123 Fix login\n\nLinear-issue: Fixes ENG-123\nLinear-issue-url: https://linear.app/acme/issue/ENG-123/fix-login\n"
    );
    let mut pr = PullRequestContext {
        issues: vec![context.issue.clone()],
        description: "The change".to_owned(),
        linkage: "Fixes".to_owned(),
        references: false,
        issue_clause: "Fixes ENG-123".to_owned(),
    };
    assert_eq!(
        config.render(Slot::PrTitle, &pr).expect("title"),
        "ENG-123 Fix login"
    );
    assert_eq!(
        config.render(Slot::PrBody, &pr).expect("body"),
        "The change\n\nFixes ENG-123\n\nhttps://linear.app/acme/issue/ENG-123/fix-login"
    );
    pr.description.clear();
    assert!(
        config
            .render(Slot::PrBody, &pr)
            .expect("body")
            .starts_with("Fixes")
    );
    pr.issues.push(IssueData {
        identifier: "ENG-124".to_owned(),
        ..context.issue
    });
    let error = config
        .render(Slot::PrTitle, &pr)
        .expect_err("default needs a title");
    assert!(error.to_string().contains("--title"), "{error}");
}

#[test]
fn builtin_defaults_ignore_checkout_line_endings() {
    let lf = DEFAULTS.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    let mixed = crlf.replacen("\r\n", "\n", 1);
    for source in [lf, crlf, mixed] {
        let config = TemplateConfig::builtin_from_source(&source);
        assert_default_commit_and_pull_request_text(&config);
        for (name, entry) in config.entries() {
            assert!(
                !entry.body.contains('\r'),
                "builtin template {name} retains checkout carriage returns"
            );
        }
    }
}

#[test]
fn custom_template_and_data_line_endings_are_preserved() {
    let layer = tier(
        "project.toml",
        r#"[templates]
commit_description = "  {{ issue.title }}\r\n{% include 'row' %}\r\n"
[template-aliases]
row = "{{ issue.description }}\r\n"
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("load custom line endings");
    let mut context = commit();
    context.issue.title = "First\r\n{{ 7 * 7 }}\rlast".to_owned();
    context.issue.description = "Details\r\nmore".to_owned();
    assert_eq!(
        config
            .render(Slot::CommitDescription, &context)
            .expect("custom render"),
        format!(
            "  {}\r\n{}\r\n\r\n",
            context.issue.title, context.issue.description
        )
    );
    assert_eq!(
        TemplateConfig::builtin()
            .render(Slot::CommitDescription, &context)
            .expect("builtin render with custom data"),
        format!(
            "ENG-123 {}\n\nLinear-issue: Fixes ENG-123\nLinear-issue-url: https://linear.app/acme/issue/ENG-123/fix-login\n",
            context.issue.title
        )
    );
}

#[test]
fn layers_replace_only_one_name_and_keep_origins() {
    let global = tier(
        "global.toml",
        r#"[templates]
commit_description = 'global {{ issue.identifier }}'
pr_title = 'global title'
[template-aliases]
policy = 'organization policy'
"#,
    );
    let project = tier(
        "project.toml",
        r#"[templates]
commit_description = '{% include "policy" %}: {{ issue.title | upper }}'
"#,
    );
    let config = TemplateConfig::from_tiers([&global, &project]).expect("layers");
    assert_eq!(
        config
            .render(Slot::CommitDescription, &commit())
            .expect("render"),
        "organization policy: FIX LOGIN"
    );
    assert_eq!(
        config
            .entries()
            .get("pr_title")
            .expect("entry")
            .origin
            .source,
        "global.toml"
    );
    assert_eq!(
        config
            .entries()
            .get("commit_description")
            .expect("entry")
            .origin
            .source,
        "project.toml"
    );
}

#[test]
fn builtin_includes_and_block_inheritance_keep_shipped_definition() {
    let layer = tier(
        "project.toml",
        r#"[templates]
commit_description = '{% extends "builtin/commit_description" %}{% block content %}{{ super() }}Footer{% endblock %}'
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("load");
    let original = TemplateConfig::builtin()
        .render(Slot::CommitDescription, &commit())
        .expect("original");
    assert_eq!(
        config
            .render(Slot::CommitDescription, &commit())
            .expect("render"),
        format!("{original}Footer")
    );
    assert_eq!(
        config
            .render_named("builtin/commit_description", &commit())
            .expect("builtin"),
        original
    );
    for text in [
        "[templates]\n'builtin/commit_description' = 'changed'",
        "[template-aliases]\n'builtin/policy' = 'changed'",
        "[template-aliases]\ncommit_description = 'changed'",
        "[template-aliases]\nskill_not_a_command = 'changed'",
    ] {
        assert!(
            TemplateConfig::from_tiers([&tier("bad.toml", text)]).is_err(),
            "{text}"
        );
    }
}

#[test]
fn canonical_empty_hooks_exist_and_keep_overrides() {
    let layer = tier(
        "project.toml",
        "[templates]\nskill_issue_comment = 'local policy'",
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("load");
    assert_eq!(
        config
            .render_named("skill_issue_comment", &SkillContext::default())
            .expect("hook"),
        "local policy"
    );
    for name in ["skill_milestone_update", "builtin/skill_milestone_update"] {
        assert_eq!(
            config
                .render_named(name, &SkillContext::default())
                .expect("empty hook"),
            ""
        );
    }
    assert!(
        TemplateConfig::from_tiers([&tier(
            "bad.toml",
            "[templates]\nskill_isssue_comment = 'typo'"
        )])
        .is_err()
    );
}

#[test]
fn every_lower_tier_and_unused_partial_is_syntax_checked() {
    let lower = tier("lower.toml", "[template-aliases]\nunused = '{% if %}'");
    let higher = tier("higher.toml", "[template-aliases]\nunused = 'valid'");
    let error = TemplateConfig::from_tiers([&lower, &higher]).expect_err("invalid lower layer");
    assert!(error.to_string().contains("lower.toml"), "{error}");
    for text in [
        "templates = 'bad'",
        "[templates]\npr_title = 4",
        "[templates]\npr_titel = 'typo'",
        "[template-aliases]\npolicy = { nested = 'bad' }",
    ] {
        assert!(
            TemplateConfig::from_tiers([&tier("bad.toml", text)]).is_err(),
            "{text}"
        );
    }
}

#[test]
fn direct_unknown_fields_fail_even_in_unselected_branches() {
    for body in [
        "{% if false %}{{ issue.typo }}{% endif %}",
        "{{ credential }}",
        "{{ issues }}",
    ] {
        let mut config = TemplateConfig::builtin();
        let mut layer = tier("bad.toml", "[templates]");
        layer
            .table
            .get_mut("templates")
            .expect("templates")
            .as_table_mut()
            .expect("table")
            .insert(
                "commit_description".to_owned(),
                toml::Value::String(body.to_owned()),
            );
        assert!(config.apply_tier(&layer).is_err(), "{body}");
    }
}

#[test]
fn runtime_errors_are_buffered_and_report_the_partial_source() {
    let layer = tier(
        "organization.toml",
        r#"[templates]
commit_description = 'prefix {% include "policy" %}'
[template-aliases]
policy = 'prefix {{ issue["missing"] }}'
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("compile");
    let error = config
        .render(Slot::CommitDescription, &commit())
        .expect_err("strict missing field");
    let message = error.to_string();
    assert!(message.contains("organization.toml"), "{message}");
    assert!(message.contains("template-aliases.policy"), "{message}");
    assert!(message.contains("undefined value"), "{message}");
    assert!(message.contains("policy"), "{message}");
    assert!(!message.contains("prefix"), "no source text: {message}");
}

#[test]
fn partials_can_receive_lexical_loop_data() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_body = '{% for issue in issues %}{% include "row" %}{% endfor %}'
[template-aliases]
row = '{{ issue.identifier }};'
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("load lexical partial");
    let pr = PullRequestContext {
        issues: vec![commit().issue],
        ..PullRequestContext::default()
    };
    assert_eq!(
        config.render(Slot::PrBody, &pr).expect("render"),
        "ENG-123;"
    );
}

#[test]
fn only_explicit_capabilities_are_available() {
    for body in [
        "prefix {{ issue.title | tojson }}",
        "prefix {{ range(3) }}",
        "prefix {% include '/etc/passwd' %}",
        "prefix {% include 'missing' %}",
        "prefix {% include 'looping' %}",
    ] {
        let mut config = TemplateConfig::builtin();
        let layer = tier(
            "project.toml",
            &format!(
                "[templates]\ncommit_description = {body:?}\n[template-aliases]\nlooping = '{{% include \"looping\" %}}'"
            ),
        );
        // Unknown globals are caught by metadata; filters/includes by strict execution.
        match config.apply_tier(&layer) {
            Ok(()) => assert!(
                config.render(Slot::CommitDescription, &commit()).is_err(),
                "{body}"
            ),
            Err(error) => assert!(
                error.to_string().contains("unknown context field range"),
                "{error}"
            ),
        }
    }
    assert!(
        TemplateConfig::from_tiers([&tier(
            "bad.toml",
            "[template-aliases]\nmacro = '{% macro f() %}text{% endmacro %}'"
        )])
        .is_err()
    );
}

#[test]
fn data_is_inert_and_whitespace_is_preserved() {
    let layer = tier(
        "project.toml",
        "[templates]\ncommit_description = '  {{ issue.title }}  '",
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("load");
    let mut context = commit();
    context.issue.title = "<tag>{{ 7 * 7 }} {% include '/etc/passwd' %}&".to_owned();
    assert_eq!(
        config
            .render(Slot::CommitDescription, &context)
            .expect("render"),
        format!("  {}  ", context.issue.title)
    );
}

#[test]
fn diagnostic_does_not_print_source_or_other_config_values() {
    let layer = tier(
        "private.toml",
        "api_key = 'very-secret-api-key'\n[templates]\npr_title = '{% if %} private-template-text'",
    );
    let error = TemplateConfig::from_tiers([&layer]).expect_err("bad syntax");
    let message = format!("{error:?}");
    assert!(message.contains("private.toml"), "{message}");
    assert!(message.contains("templates.pr_title"), "{message}");
    assert!(message.contains("line 1"), "{message}");
    assert!(!message.contains("very-secret"), "{message}");
    assert!(!message.contains("private-template-text"), "{message}");
}

#[test]
fn failed_override_does_not_mutate_the_registry() {
    let mut config = TemplateConfig::builtin();
    let before = config
        .render(Slot::CommitDescription, &commit())
        .expect("before");
    let layer = tier(
        "bad.toml",
        "[templates]\ncommit_description = 'replacement'\npr_body = '{% if %}'",
    );
    assert!(config.apply_tier(&layer).is_err());
    assert_eq!(
        config
            .render(Slot::CommitDescription, &commit())
            .expect("after"),
        before
    );
}

#[test]
fn excessive_nested_loops_stop_at_the_fuel_limit() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_body = '{% for a in issues %}{% for b in issues %}{% for c in issues %}x{% endfor %}{% endfor %}{% endfor %}'
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("compile");
    let pr = PullRequestContext {
        issues: vec![commit().issue; 100],
        ..PullRequestContext::default()
    };
    let error = config.render(Slot::PrBody, &pr).expect_err("fuel limit");
    assert!(error.to_string().contains("fuel"), "{error}");
}

#[test]
fn builtin_detection_follows_the_effective_body() {
    let mut config = TemplateConfig::builtin();
    assert!(config.is_builtin(Slot::PrTitle));
    let body = config
        .entries()
        .get("pr_title")
        .expect("title entry")
        .body
        .clone();
    config
        .apply_tier(&tier(
            "project.toml",
            &format!("[templates]\npr_title = {body:?}"),
        ))
        .expect("same body");
    assert!(config.is_builtin(Slot::PrTitle));
    config
        .apply_tier(&tier(
            "project.toml",
            "[templates]\npr_title = 'custom title'",
        ))
        .expect("custom body");
    assert!(!config.is_builtin(Slot::PrTitle));
    assert!(config.is_builtin(Slot::CommitDescription));
}

#[test]
fn command_hook_names_are_unique_and_shipped_names_have_known_roles() {
    fn walk(
        command: &clap::Command,
        path: &mut Vec<String>,
        names: &mut BTreeSet<String>,
        count: &mut usize,
    ) {
        for child in command
            .get_subcommands()
            .filter(|child| !child.is_hide_set())
        {
            assert!(
                !child.get_name().contains('_'),
                "command segments cannot contain underscores"
            );
            path.push(child.get_name().to_owned());
            let parts: Vec<_> = path.iter().map(String::as_str).collect();
            let name = skill_hook(&parts);
            assert!(names.insert(name.clone()), "duplicate hook {name}");
            *count += 1;
            walk(child, path, names, count);
            path.pop();
        }
    }
    let mut names = BTreeSet::new();
    let mut count = 0;
    walk(
        &crate::cli::Cli::command(),
        &mut Vec::new(),
        &mut names,
        &mut count,
    );
    assert_eq!(count, names.len());
    assert_eq!(names, command_hooks());
    let shipped: toml::Table = DEFAULTS.parse().expect("shipped TOML");
    let templates = shipped
        .get("templates")
        .expect("templates")
        .as_table()
        .expect("table");
    for name in templates.keys().filter(|name| name.starts_with("skill_")) {
        assert!(names.contains(name), "orphan shipped hook {name}");
    }
    let guide_names: BTreeSet<_> = templates
        .keys()
        .filter(|name| name.as_str() == "guide" || name.starts_with("guide_"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        guide_names,
        BTreeSet::from([
            "guide",
            "guide_targeted",
            "guide_intro",
            "guide_how_to_work",
            "guide_common_tasks",
            "guide_markdown_rules",
            "guide_exit_status",
            "guide_help_notes",
            "guide_commands",
            "guide_graphql",
        ])
    );
    for partial in ["guide_custom", "skill_custom", "guide", "guide_targeted"] {
        let text = format!("[template-aliases]\n{partial}='text'");
        assert!(TemplateConfig::from_tiers([&tier("reserved.toml", &text)]).is_err());
    }
}
