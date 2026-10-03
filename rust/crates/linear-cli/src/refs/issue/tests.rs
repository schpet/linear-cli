use super::*;
use crate::auth::ApiKeyInput;
use crate::refs::WorkspaceScope;

#[test]
fn references_preserve_presence_numbers_and_workspace_order() {
    let key = ApiKeyInput::Absent;
    let scope = WorkspaceScope {
        api_key: key.clone(),
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
            error.message(),
            "an integer id was provided, but no team is set"
        );
        assert_eq!(error.hint(), Some("Run `linear config` to set a team."));
    }
    let error =
        prepare_issue_reference(Some("https://linear.app/foreign/issue/ENG-7"), None, &scope)
            .expect_err("foreign URL");
    assert!(error.message().contains("this is the \"acme\" workspace"));
    let error = prepare_issue_reference(Some("https://linear.app/foreign/team/eng"), None, &scope)
        .expect_err("workspace before kind");
    assert!(error.message().contains("this is the \"acme\" workspace"));
}
#[test]
fn word_boundaries_are_ascii_only_including_non_ascii_neighbors() {
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
