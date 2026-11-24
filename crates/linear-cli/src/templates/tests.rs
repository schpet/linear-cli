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
        let config = TemplateConfig::builtin_from_source(&source, false);
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
}

#[test]
fn shipped_issue_reference_links_to_the_supplied_url() {
    let config = TemplateConfig::builtin();
    let url = "https://linear.app/workspace-one/issue/DEMO-42/task";
    let issue = IssueReference::new("DEMO-42", Some(url));
    let expected = DisplayText {
        spans: vec![DisplaySpan {
            text: "DEMO-42".to_owned(),
            url: Some(url.to_owned()),
        }],
    };
    assert_eq!(
        config.format_issue_id(&issue, true).expect("styled"),
        expected
    );
    assert_eq!(expected.plain(), "DEMO-42");
    assert_eq!(
        expected.terminal(),
        format!("\x1b]8;;{url}\x1b\\DEMO-42\x1b]8;;\x1b\\")
    );
    for (issue, styled) in [(issue, false), (IssueReference::new("DEMO-42", None), true)] {
        let shown = config.format_issue_id(&issue, styled).expect("plain");
        assert_eq!(shown.plain(), "DEMO-42");
        assert_eq!(shown.terminal(), "DEMO-42");
    }
}

#[test]
fn shipped_comment_id_links_roots_and_omits_replies() {
    let url = "https://linear.app/workspace-one/issue/DEMO-42/task#comment-bb1bb1bb";
    let issue = IssueReference::new("DEMO-42", None);
    let root = serde_json::json!({ "id": "root", "url": url, "parent": null });
    let reply = serde_json::json!({ "id": "reply", "url": url, "parent": { "id": "root" } });

    for experimental in [false, true] {
        let config = TemplateConfig::builtin_with_experimental(experimental);
        assert_eq!(
            config.format_comment_id(&issue, &root, true).expect("root"),
            DisplayText {
                spans: vec![DisplaySpan {
                    text: "[root]".to_owned(),
                    url: Some(url.to_owned()),
                }],
            },
        );
        assert_eq!(
            config
                .format_comment_id(&issue, &root, false)
                .expect("plain root")
                .terminal(),
            "[root]"
        );
        assert_eq!(
            config
                .format_comment_id(&issue, &reply, true)
                .expect("reply"),
            DisplayText::default(),
        );
    }
}

#[test]
fn shipped_project_name_uses_a_web_link_when_styled() {
    let url = "https://linear.app/workspace-one/project/mobile-app-a0a0a0a0a0a0";
    for experimental in [false, true] {
        let config = TemplateConfig::builtin_with_experimental(experimental);
        assert_eq!(
            config
                .format_project_name("Mobile App", Some(url), true)
                .expect("styled"),
            DisplayText {
                spans: vec![DisplaySpan {
                    text: "Mobile App".to_owned(),
                    url: Some(url.to_owned()),
                }],
            }
        );
        for (url, styled) in [(Some(url), false), (None, true), (Some(""), true)] {
            assert_eq!(
                config
                    .format_project_name("Mobile App", url, styled)
                    .expect("plain")
                    .terminal(),
                "Mobile App"
            );
        }
    }
}

#[test]
fn stable_templates_reject_experimental_authoring() {
    for name in [FORMAT_ISSUE_ID, FORMAT_COMMENT_ID, FORMAT_PROJECT_NAME] {
        let override_layer = tier("project.toml", &format!("[templates]\n{name} = 'override'"));
        let error =
            TemplateConfig::from_tiers([&override_layer]).expect_err("override requires opt-in");
        assert!(
            error
                .message()
                .contains(&format!("project.toml, templates.{name}")),
            "{error}"
        );
        assert_eq!(error.hint(), Some(EXPERIMENTAL_TEMPLATES_HINT));
    }

    for (key, body, feature) in [
        (
            "[template-aliases]\nexample",
            "{% if false %}{% macro demo() %}example{% endmacro %}{% endif %}",
            "macros and call blocks",
        ),
        (
            "[template-aliases]\nexample",
            "{% block content %}{% call fail('not executed') %}example{% endcall %}{% endblock %}",
            "macros and call blocks",
        ),
        (
            "[templates]\npr_title",
            "{% if false %}{{ hyperlink('https://example.test/', 'example') }}{% endif %}",
            "hyperlink",
        ),
        (
            "[templates]\npr_title",
            "{% set make_link = hyperlink %}{{ make_link('https://example.test/', 'example') }}",
            "hyperlink",
        ),
    ] {
        let layer = tier("project.toml", &format!("{key} = {body:?}"));
        let error = TemplateConfig::from_tiers([&layer]).expect_err("feature requires opt-in");
        assert!(error.message().contains("project.toml"), "{error}");
        assert!(error.message().contains(feature), "{error}");
        assert_eq!(error.hint(), Some(EXPERIMENTAL_TEMPLATES_HINT));
        TemplateConfig::from_tiers_with_experimental([&layer], true)
            .expect("opt-in enables feature");
    }
}

#[test]
fn stable_partials_can_check_and_inherit_a_variable_named_hyperlink() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_title = '''{% include "contents" %} {% set hyperlink = "Inherited" %}{% include "contents" %}'''
[template-aliases]
contents = '''{% if hyperlink is defined %}{{ hyperlink }}{% else %}Absent{% endif %}'''
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("partial context");
    assert_eq!(
        config
            .render(Slot::PrTitle, &PullRequestContext::default())
            .expect("render"),
        "Absent Inherited"
    );
}

#[test]
fn stable_templates_keep_constant_imports_and_literal_experimental_syntax() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_title = '''{% import "values" as values %}{% from "values" import hyperlink %}{{ values.hyperlink }} {{ hyperlink }} {{ "hyperlink()" }} {% raw %}{% macro example() %}{{ hyperlink() }}{% endmacro %}{% endraw %}{# {% call demo() %} #}'''
[template-aliases]
values = '''{% set hyperlink = "reference" %}'''
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("ordinary imports");
    assert!(!config.experimental_enabled());
    let title = config
        .render(Slot::PrTitle, &PullRequestContext::default())
        .expect("render");
    assert_eq!(
        title,
        "reference reference hyperlink() {% macro example() %}{{ hyperlink() }}{% endmacro %}"
    );
}

#[test]
fn stable_templates_preserve_legacy_format_slot_aliases() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_title = '''{% set note = "Legacy title" %}{% include "format_issue_id" %} {% include "format_comment_id" %} {% include "format_project_name" %}'''
[template-aliases]
format_issue_id = '''{{ note }}'''
format_comment_id = '''{{ note | lower }}'''
format_project_name = '''{{ note | upper }}'''
"#,
    );
    let config = TemplateConfig::from_tiers([&layer]).expect("legacy template alias");
    assert_eq!(
        config
            .render(Slot::PrTitle, &PullRequestContext::default())
            .expect("render"),
        "Legacy title legacy title LEGACY TITLE"
    );
    config
        .validate_alias(FORMAT_ISSUE_ID)
        .expect("legacy alias remains an alias");
    config
        .validate_alias(FORMAT_COMMENT_ID)
        .expect("legacy alias remains an alias");
    config
        .validate_alias(FORMAT_PROJECT_NAME)
        .expect("legacy alias remains an alias");

    let url = "https://linear.app/workspace-one/issue/DEMO-42";
    let issue = IssueReference::new("DEMO-42", Some(url));
    let display = config
        .format_issue_id(&issue, true)
        .expect("shipped display");
    assert_eq!(display.plain(), "DEMO-42");
    assert_eq!(
        display.spans.first().and_then(|span| span.url.as_deref()),
        Some(url)
    );
    let comment_url = "https://linear.app/workspace-one/issue/DEMO-42#comment-01234567";
    let comment = serde_json::json!({ "id": "abc", "url": comment_url, "parent": null });
    let display = config
        .format_comment_id(&issue, &comment, true)
        .expect("shipped comment display");
    assert_eq!(display.plain(), "[abc]");
    assert_eq!(
        display.spans.first().and_then(|span| span.url.as_deref()),
        Some(comment_url)
    );

    let project_url = "https://linear.app/workspace-one/project/mobile-app-a0a0a0a0a0a0";
    let display = config
        .format_project_name("Mobile App", Some(project_url), true)
        .expect("shipped project display");
    assert_eq!(display.plain(), "Mobile App");
    assert_eq!(
        display.spans.first().and_then(|span| span.url.as_deref()),
        Some(project_url)
    );

    for name in [FORMAT_ISSUE_ID, FORMAT_COMMENT_ID, FORMAT_PROJECT_NAME] {
        let collision = tier(
            "project.toml",
            &format!("[template-aliases]\n{name} = 'legacy'"),
        );
        let error = TemplateConfig::from_tiers_with_experimental([&collision], true)
            .expect_err("experimental templates reserve each format slot");
        assert!(
            error
                .message()
                .contains(&format!("template-aliases.{name}")),
            "{error}"
        );
        assert!(
            error
                .hint()
                .is_some_and(|hint| hint.contains("Rename the alias")),
            "{error}"
        );
    }
}

#[test]
fn stable_document_templates_reject_hyperlink_invocations_in_include_expressions() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_title = '''{% include hyperlink("https://example.test/", "body") %}'''
[template-aliases]
body = 'Unexpected title'
"#,
    );
    let error = TemplateConfig::from_tiers([&layer])
        .and_then(|config| config.render(Slot::PrTitle, &PullRequestContext::default()))
        .expect_err("cannot use hyperlink to select a template");
    assert!(error.message().contains("templates.pr_title"), "{error}");
    assert_eq!(error.hint(), Some(EXPERIMENTAL_TEMPLATES_HINT));
}

#[test]
fn display_templates_support_macros_includes_multiple_links_and_plain_fallbacks() {
    let layer = tier(
        "project.toml",
        r#"[templates]
format_issue_id = '''{% from "formats" import ident %}{{ issue.identifier }} {{ ident(issue) }} {{ ident(issue) | upper }}	{{ issue.workspace_url_key }}'''
[template-aliases]
formats = '''{% macro ident(issue) %}[{{ hyperlink(issue.url, issue.identifier | lower, "été") }}]{% endmacro %}'''
row = '''{% import "formats" as fmt %}{{ fmt.ident(issue) }}{% include "other" %}'''
other = '''	{{ hyperlink("example://workspace-one/issue/DEMO-42", issue.identifier) }}	{{ issue.title }}'''
"#,
    );
    let config =
        TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compile templates");
    let url = "https://linear.app/workspace-one/issue/DEMO-42/task";
    let issue = IssueReference::new("DEMO-42", Some(url));
    let styled = config.format_issue_id(&issue, true).expect("styled");
    assert_eq!(styled.plain(), "DEMO-42 [demo-42] [demo-42] workspace-one");
    assert_eq!(styled.terminal().matches("\x1b]8;;https://").count(), 2);
    let plain = config.format_issue_id(&issue, false).expect("fallback");
    assert_eq!(plain.terminal(), "DEMO-42 [été] [ÉTÉ] workspace-one");

    let context = serde_json::json!({
        "issue": { "url": url, "identifier": "DEMO-42", "title": "Å\t界" }
    });
    let row = config.render_issue_row("row", &context, true).expect("row");
    assert_eq!(row.plain(), "[demo-42]\tDEMO-42\tÅ 界");
    let urls = row
        .spans
        .iter()
        .filter_map(|span| span.url.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(urls, [url, "example://workspace-one/issue/DEMO-42"]);
    config.validate_alias("row").expect("registered alias");
    for name in ["format_issue_id", "builtin/format_issue_id", "missing"] {
        assert!(matches!(
            config
                .validate_alias(name)
                .expect_err("not an alias")
                .kind(),
            crate::error::ErrorKind::Invalid
        ));
    }
}

#[test]
fn issue_id_presets_select_the_target_from_each_issue() {
    for preset in ["web", "app"] {
        let layer = tier(
            "project.toml",
            &format!(
                r#"[templates]
format_issue_id = '{{% include "builtin/issue_id/{preset}" %}}'
[template-aliases]
"issue_id/{preset}" = 'shadowed'
"#
            ),
        );
        let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("preset");
        for workspace in ["workspace-one", "workspace-two"] {
            let web = format!("https://linear.app/{workspace}/issue/ENG-42/title");
            let app = format!("linear://{workspace}/issue/ENG-42");
            let issue = IssueReference::new("ENG-42", Some(&web));
            let target = if preset == "web" { &web } else { &app };
            assert_eq!(
                config
                    .format_issue_id(&issue, true)
                    .expect("styled")
                    .terminal(),
                format!("\x1b]8;;{target}\x1b\\ENG-42\x1b]8;;\x1b\\")
            );
            assert_eq!(
                config
                    .format_issue_id(&issue, false)
                    .expect("pipe")
                    .terminal(),
                "ENG-42"
            );
        }
        assert_eq!(
            config
                .format_issue_id(&IssueReference::new("ENG-42", None), true)
                .expect("missing URL")
                .terminal(),
            "ENG-42"
        );
    }
}

#[test]
fn comment_id_presets_use_the_api_url_for_roots_and_replies() {
    let url_suffix =
        "workspace-one/issue/DEMO-42/task%20slug?from=comments&label=%2F#comment-01234567";
    let web = format!("https://linear.app/{url_suffix}");
    let app = format!("linear://linear.app/{url_suffix}");
    let issue = IssueReference::new("DEMO-42", None);
    for (preset, target) in [("web", &web), ("app", &app)] {
        let layer = tier(
            "project.toml",
            &format!(
                r#"[templates]
format_comment_id = '{{% include "builtin/comment_id/{preset}" %}}'
[template-aliases]
"comment_id/{preset}" = 'shadowed'
"#
            ),
        );
        let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("preset");
        for parent in [
            serde_json::Value::Null,
            serde_json::json!({ "id": "parent" }),
        ] {
            let comment = serde_json::json!({ "id": "api-id", "url": web, "parent": parent });
            let styled = config
                .format_comment_id(&issue, &comment, true)
                .expect("styled");
            assert_eq!(
                styled.spans.first().and_then(|span| span.url.as_ref()),
                Some(target)
            );
            let plain = config
                .format_comment_id(&issue, &comment, false)
                .expect("plain");
            assert_eq!(plain.terminal(), "[api-id]");
        }
        let comment = serde_json::json!({ "id": "api-id" });
        assert_eq!(
            config
                .format_comment_id(&issue, &comment, true)
                .expect("missing URL")
                .terminal(),
            "[api-id]"
        );
    }
}

#[test]
fn app_comment_preset_in_an_issue_view_accepts_raw_comments() {
    let layer = tier(
        "project.toml",
        r#"[templates]
issue_view = '''{% for comment in issue.comments %}{% include "builtin/comment_id/app" %}{% if not loop.last %}
{% endif %}{% endfor %}'''
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("preset");
    let context = serde_json::json!({"issue": { "comments": [
        { "id": "actual", "url": "https://linear.app/acme/issue/ENG-7/slug#comment-ba5ed001" },
        { "id": "no-anchor", "url": "https://linear.app/acme/issue/ENG-7/slug" },
        { "id": "no-comment-anchor", "url": "https://linear.app/acme/issue/ENG-7/slug#body" },
        { "id": "empty-anchor", "url": "https://linear.app/acme/issue/ENG-7/slug#comment-" },
        { "id": "wrong-host", "url": "https://example.test/acme/issue/ENG-7/slug#comment-ba5ed001" },
        { "id": "wrong-path", "url": "https://linear.app/acme/project/other#comment-ba5ed001" },
        { "id": "malformed", "url": "https://linear.app/acme/issue/ENG-7/\n#comment-ba5ed001" },
    ]}});
    let mut text = String::new();
    for part in config.render_view(&context, true).expect("formatted view") {
        part.write_plain(&mut text);
    }
    assert_eq!(
        text,
        "[\\[actual\\]](<linear://linear.app/acme/issue/ENG-7/slug#comment-ba5ed001>)\n[no-anchor]\n[no-comment-anchor]\n[empty-anchor]\n[wrong-host]\n[wrong-path]\n[malformed]"
    );
}

#[test]
fn project_name_presets_are_reusable_in_views_and_rows() {
    let suffix = "workspace-one/project/mobile%20app/overview?from=%2F#summary";
    let web = format!("https://linear.app/{suffix}");
    let app = format!("linear://linear.app/{suffix}");
    let name = "[Mobile App]";
    let context = serde_json::json!({"issue": {"project": {"name": name, "url": web}}});
    for (preset, target) in [("web", &web), ("app", &app)] {
        let layer = tier(
            "project.toml",
            &format!(
                r#"[templates]
format_project_name = '{{% include "builtin/project_name/{preset}" %}}'
issue_view = '{{{{ format_project_name(issue.project) }}}}'
[template-aliases]
"project_name/{preset}" = 'shadowed'
row = '{{{{ format_project_name(issue.project) }}}}'
"#
            ),
        );
        let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("preset");
        let styled = config
            .format_project_name(name, Some(&web), true)
            .expect("styled");
        assert_eq!(
            styled.terminal(),
            format!("\x1b]8;;{target}\x1b\\{name}\x1b]8;;\x1b\\")
        );
        assert_eq!(
            config
                .format_project_name(name, Some(&web), false)
                .expect("unstyled")
                .terminal(),
            name
        );
        assert_eq!(
            config.render_issue_row("row", &context, true).expect("row"),
            styled
        );
        let view = config.render_view(&context, true).expect("view");
        let mut text = String::new();
        for part in view {
            part.write_plain(&mut text);
        }
        assert_eq!(text, format!("[\\[Mobile App\\]](<{target}>)"));

        let context = serde_json::json!({"issue": {"project": {"name": name}}});
        assert_eq!(
            config
                .render_issue_row("row", &context, true)
                .expect("row without project URL")
                .terminal(),
            name
        );
    }
}

#[test]
fn app_project_name_preset_only_converts_canonical_project_urls() {
    let layer = tier(
        "project.toml",
        r#"[templates]
format_project_name = '{% include "builtin/project_name/app" %}'
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("preset");
    for url in [
        "https://example.test/acme/project/mobile",
        "http://linear.app/acme/project/mobile",
        "https://linear.app/acme/issue/ENG-7/mobile",
        "https://linear.app/acme/project/?name=mobile",
        "https://linear.app/acme/project/mobile/../other",
        "https://linear.app/acme/project/%2e%2E/issue/ENG-7",
        "https://linear.app/acme/project/mobile\\issues",
        "https://linear.app/acme/project/mobile%GG",
    ] {
        assert_eq!(
            config
                .format_project_name("Mobile App", Some(url), true)
                .expect("plain fallback")
                .terminal(),
            "Mobile App",
            "{url}"
        );
    }
}

#[test]
fn project_name_templates_check_direct_context_fields() {
    for field in ["project.title", "issue.project.name"] {
        let layer = tier(
            "project.toml",
            &format!(
                "[templates]\nformat_project_name = '{{% if false %}}{{{{ {field} }}}}{{% endif %}}'"
            ),
        );
        let error = TemplateConfig::from_tiers_with_experimental([&layer], true)
            .expect_err("unknown context field");
        assert!(error.message().contains(field), "{error}");
    }
}

#[test]
fn project_name_callable_composes_with_view_labels_and_row_links() {
    let layer = tier(
        "project.toml",
        r#"[templates]
format_project_name = '📁 {{ hyperlink(project.url, project.name) }}'
issue_view = '{{ label("field_label", "Project: " ~ format_project_name(issue.project)) }}'
[template-aliases]
row = '{{ hyperlink(issue.url, format_project_name(issue.project)) }}'
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("format");
    let url = "https://linear.app/acme/project/mobile";
    let issue_url = "https://linear.app/acme/issue/ENG-1/mobile";
    let context = serde_json::json!({
        "issue": {"url": issue_url, "project": {"name": "[Mobile]", "url": url}}
    });
    let mut text = String::new();
    for part in config.render_view(&context, true).expect("labeled project") {
        part.write_plain(&mut text);
    }
    assert_eq!(text, format!("Project: 📁 [\\[Mobile\\]](<{url}>)"));
    assert_eq!(
        config
            .render_issue_row("row", &context, true)
            .expect("linked row")
            .terminal(),
        format!("\x1b]8;;{issue_url}\x1b\\📁 [Mobile]\x1b]8;;\x1b\\")
    );
}

#[test]
fn project_name_callable_uses_the_calling_templates_fuel_budget() {
    let layer = tier(
        "project.toml",
        r#"[templates]
format_project_name = '{% for letter in project.name %}{% set upper = letter | upper %}{% endfor %}Mobile'
[template-aliases]
row = '{% for index in repetitions %}{% set result = format_project_name(issue.project) %}{% endfor %}'
"#,
    );
    let name = "a".repeat(1_000);
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("format");
    assert_eq!(
        config
            .format_project_name(&name, None, false)
            .expect("single render fits")
            .plain(),
        "Mobile"
    );
    let context = serde_json::json!({
        "issue": {"project": {"name": name}},
        "repetitions": vec![0; 1_000],
    });
    let error = config
        .render_issue_row("row", &context, false)
        .expect_err("nested renders share the caller's fuel");
    assert!(error.message().contains("fuel"), "{error}");
}

#[test]
fn project_name_callable_distinguishes_null_from_unexpected_projects() {
    let layer = tier(
        "project.toml",
        r#"[template-aliases]
row = '{{ format_project_name(issue.project) }}'
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("format");
    let issue = serde_json::json!({"issue": {"project": null}});
    assert_eq!(
        config
            .render_issue_row("row", &issue, true)
            .expect("no project"),
        DisplayText::default()
    );
    for issue in [
        serde_json::json!({}),
        serde_json::json!({"project": "Mobile"}),
        serde_json::json!({"project": {"name": "Mobile", "url": 42}}),
    ] {
        config
            .render_issue_row("row", &serde_json::json!({"issue": issue}), true)
            .expect_err("invalid project argument");
    }
}

#[test]
fn only_hyperlink_spans_can_emit_terminal_control_sequences() {
    let layer = tier(
        "project.toml",
        r#"[template-aliases]
row = '''{% macro cell(value) %}	{{ value | lower }}{% endmacro %}{{ cell(title) ~ suffix }}	{{ hyperlink(url, label) }}	{{ label }}{{ "\u001b]8;;https://example.test/author\u0007" }}'''
"#,
    );
    let config =
        TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compile templates");
    let context = serde_json::json!({
        "title": "Å\x1b[2J\t界\r\n\u{009b}x\u{001f}0123456789012345678901234567890123456789\u{001e}",
        "suffix": "\r\n\t\u{0007}",
        "label": "同じ\n\u{0008}",
        "url": "https://example.test/linked",
    });
    let row = config.render_issue_row("row", &context, true).expect("row");
    assert_eq!(
        row.plain(),
        "\tå�[2j 界 �x�0123456789012345678901234567890123456789�  �\t同じ �\t同じ ��]8;;https://example.test/author�"
    );
    assert_eq!(
        row.spans.iter().filter(|span| span.url.is_some()).count(),
        1
    );
    assert_eq!(row.terminal().matches('\x1b').count(), 4);
}

#[test]
fn document_hyperlinks_render_plain_safe_fallbacks() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_body = '''First line
{% from "formats" import link %}{{ link(issues[0]) }}
{{ hyperlink(issues[0].url, issues[0].title) }}'''
[template-aliases]
formats = '''{% macro link(issue) %}{{ hyperlink(issue.url, issue.identifier, issue.title | upper) }}{% endmacro %}'''
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compiled");
    let pr = PullRequestContext {
        issues: vec![IssueData {
            identifier: "DEMO-42".into(),
            url: "https://linear.app/workspace-one/issue/DEMO-42/task".into(),
            title: "<Été\x1b[2J>".into(),
            ..IssueData::default()
        }],
        ..PullRequestContext::default()
    };
    assert_eq!(
        config.render(Slot::PrBody, &pr).expect("plain"),
        "First line\n<ÉTÉ�[2J>\n<Été�[2J>"
    );
}

#[test]
fn document_templates_retain_explicit_html_autoescape() {
    let layer = tier(
        "project.toml",
        r#"[templates]
pr_body = '''{{ description }}|{% autoescape true %}{{ description }} {{ hyperlink("https://example.test/", "label", description) }}{% endautoescape %}'''
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compiled");
    let context = PullRequestContext {
        description: "<review>&".into(),
        ..PullRequestContext::default()
    };
    assert_eq!(
        config.render(Slot::PrBody, &context).expect("document"),
        "<review>&|&lt;review&gt;&amp; &lt;review&gt;&amp;"
    );
}

#[test]
fn hyperlink_errors_use_template_origins_and_do_not_echo_urls() {
    let layer = tier(
        "project.toml",
        r#"
[template-aliases]
row = '{% include "cannot_link" %}'
cannot_link = '{{ hyperlink(url, "text", "fallback") }}'
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compile");
    for url in [
        "/workspace-one/issue/DEMO-42",
        "https:///example.test",
        "https://[invalid]",
        "https://example.test/token%broken",
        "https://example.test/a path",
        "https://example.test/\nprivate-url",
        "https://example.test/\x1b]8;;private-url",
        "https://example.test/\u{009c}",
        "https://example.test\\private-url",
        "1http://example.test/",
    ] {
        for styled in [false, true] {
            let error = config
                .render_issue_row("row", &serde_json::json!({ "url": url }), styled)
                .expect_err("invalid url");
            assert!(
                error
                    .message()
                    .contains("project.toml, template-aliases.cannot_link"),
                "{error}"
            );
            assert!(
                error.message().contains("must be a valid absolute URL"),
                "{error}"
            );
            assert!(!error.message().contains(url), "{error}");
        }
    }
    for (call, reason) in [
        ("hyperlink()", "expected hyperlink"),
        ("hyperlink(42, 'text')", "url must be a string"),
        ("hyperlink(url, 42)", "text must be a string"),
        ("hyperlink(url, 'text', none)", "fallback must be a string"),
        (
            "hyperlink(url, 'text', 'fallback', 'surplus')",
            "expected hyperlink",
        ),
    ] {
        let body = format!("{{{{ {call} }}}}");
        let config = TemplateConfig::from_tiers_with_experimental(
            [&tier(
                "project.toml",
                &format!("[template-aliases]\nrow = {body:?}"),
            )],
            true,
        )
        .expect("compile");
        let error = config
            .render_issue_row(
                "row",
                &serde_json::json!({ "url": "https://example.test/" }),
                true,
            )
            .expect_err("invalid args");
        assert!(error.message().contains(reason), "{error}");
    }
}

#[test]
fn template_diagnostics_and_macro_repetition_are_bounded() {
    let layer = tier(
        "project.toml",
        r#"
[template-aliases]
fail = '{{ fail(label) }}'
large = '''{% set item = hyperlink(url, label) %}{{ item * 1000 }}'''
indent = '''{{ "text" | indent(width=999999999) }}'''
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compile");
    let error = config
        .render_issue_row(
            "fail",
            &serde_json::json!({ "label": "message\x1b[2J\n" }),
            true,
        )
        .expect_err("fail reports bad input");
    assert!(!error.message().chars().any(char::is_control), "{error}");
    let error = config
        .render_issue_row(
            "large",
            &serde_json::json!({ "url": "https://example.test/", "label": "x".repeat(5_000) }),
            true,
        )
        .expect_err("expanded output limit");
    assert!(error.message().contains("output size limit"), "{error}");
    let error = config
        .render_named("indent", &())
        .expect_err("indent fuel limit");
    assert!(error.message().contains("fuel"), "{error}");
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

#[test]
fn view_sections_survive_macro_capture_and_bound_repeated_output() {
    let layer = tier(
        "project.toml",
        r#"[templates]
issue_view = '''{% macro comments() %}{{ sections.comments }}{% endmacro %}{{ comments() * issue.priority }}'''
"#,
    );
    let config = TemplateConfig::from_tiers_with_experimental([&layer], true).expect("compile");
    let section = ViewSection::value("x".repeat(5 * 1024 * 1024), ViewSectionKind::Comments);
    let render = |copies| {
        config.render_view(
            &minijinja::context! {
                sections => minijinja::context! { comments => section.clone() },
                issue => minijinja::context! { priority => copies },
            },
            true,
        )
    };
    let once = render(1).expect("a full fetched section fits");
    assert!(matches!(
        once.as_slice(),
        [ViewPart::Section(ViewSectionKind::Comments, _)]
    ));
    let repeated = render(2).expect_err("repetitions are charged for the source bytes");
    assert!(repeated.message().contains("size limit"), "{repeated}");
}
