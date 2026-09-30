use linear_cli::auth::ApiKeyInput;
use linear_cli::platform::vcs::{parse_git_branch, parse_jj_trailers};
use linear_cli::refs::{
    IssueReference, WorkspaceScope, find_issue_identifier, prepare_issue_reference,
};

#[test]
fn references_preserve_presence_numbers_and_workspace_order() {
    let key = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        api_key: &key,
        cli_workspace: Some("acme"),
        sourced_workspace: None,
        default_workspace: None,
    };
    for (input, team, expected) in [
        (None, None, IssueReference::Inferred),
        (Some(""), Some("ENG"), IssueReference::Unresolved),
        (
            Some("eng-7"),
            None,
            IssueReference::Identifier("ENG-7".into()),
        ),
        (Some("0"), Some("ENG"), IssueReference::Unresolved),
        (Some("01"), Some("ENG"), IssueReference::Unresolved),
        (
            Some("1"),
            Some("eng"),
            IssueReference::Identifier("ENG-1".into()),
        ),
        (
            Some("99999999999999999999999999999"),
            Some("eng"),
            IssueReference::Identifier("ENG-99999999999999999999999999999".into()),
        ),
        (Some("7"), Some("bad_team"), IssueReference::Unresolved),
        (
            Some("https://linear.app/acme/issue/eng-7/title#comment-abcdef12"),
            None,
            IssueReference::Identifier("ENG-7".into()),
        ),
    ] {
        assert_eq!(
            prepare_issue_reference(input, team, &scope).expect("reference"),
            expected,
            "{input:?}"
        );
    }
    for team in [None, Some("")] {
        let error = prepare_issue_reference(Some("7"), team, &scope).expect_err("missing team");
        assert_eq!(
            error.message,
            "an integer id was provided, but no team is set"
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Run `linear config` to set a team.")
        );
    }
    let error =
        prepare_issue_reference(Some("https://linear.app/foreign/issue/ENG-7"), None, &scope)
            .expect_err("foreign URL");
    assert!(error.message.contains("this is the \"acme\" workspace"));
    let error = prepare_issue_reference(Some("https://linear.app/foreign/team/eng"), None, &scope)
        .expect_err("workspace before kind");
    assert!(error.message.contains("this is the \"acme\" workspace"));
}
#[test]
fn ascii_boundaries_match_js_including_non_ascii_neighbors() {
    for (text, expected) in [
        ("eng-123_x", None),
        ("éENG-5", Some("ENG-5")),
        ("ENG-0", None),
        ("ENG-123abc", None),
        ("feature/eng-7-x", Some("ENG-7")),
        ("ENG-01", None),
        ("_ENG-7", None),
        ("Fixes ABC-123Fixes DEF-456", Some("DEF-456")),
        ("ENG-9é", Some("ENG-9")),
        ("x/42-99999999999999999999", Some("42-99999999999999999999")),
    ] {
        assert_eq!(find_issue_identifier(text).as_deref(), expected, "{text}");
    }
}
#[test]
fn raw_jj_joining_and_newline_parser_remain_source_compatible() {
    for (text, expected) in [
        ("Fixes ABC-123Fixes DEF-456", Some("DEF-456")),
        ("Fixes ABC-123 References DEF-456", Some("ABC-123")),
        (
            "Fixes ABC-123\nFixes DEF-456\n\nFixes XYZ-9",
            Some("DEF-456"),
        ),
        ("\n\nNo issue\nFixes XYZ-9", Some("XYZ-9")),
        ("", None),
        ("\u{feff}ENG-7\u{feff}", Some("ENG-7")),
    ] {
        assert_eq!(parse_jj_trailers(text).as_deref(), expected);
    }
}
#[test]
fn git_nonzero_is_detached_only_for_the_source_substring() {
    assert_eq!(
        parse_git_branch(false, "ENG-7", "fatal: not a symbolic ref\n").expect("detached"),
        None
    );
    assert_eq!(
        parse_git_branch(true, "\u{feff}feature/eng-7-x\n", "warning")
            .expect("branch")
            .as_deref(),
        Some("ENG-7")
    );
    assert_eq!(parse_git_branch(true, "\n", "").expect("empty"), None);
    let error = parse_git_branch(false, "ENG-7", "\u{feff}fatal: denied\n").expect_err("fatal");
    assert_eq!(error.message, "Failed to get current branch: fatal: denied");
}
