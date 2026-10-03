use super::*;
use crate::auth::ApiKeyInput;
use crate::refs::WorkspaceScope;

#[test]
fn identifiers_numbers_and_urls_become_references() {
    let key = ApiKeyInput::Absent;
    let scope = WorkspaceScope::new(Some("acme"), key.clone());
    for (input, team, expected) in [
        ("", Some("ENG"), None),
        ("eng-7", None, Some("ENG-7")),
        ("0", Some("ENG"), None),
        ("01", Some("ENG"), None),
        ("1", Some("eng"), Some("ENG-1")),
        (
            "99999999999999999999999999999",
            Some("eng"),
            Some("ENG-99999999999999999999999999999"),
        ),
        ("7", Some("bad_team"), None),
        (
            "https://linear.app/acme/issue/eng-7/title#comment-abcdef12",
            None,
            Some("ENG-7"),
        ),
    ] {
        assert_eq!(
            prepare_issue_reference(input, team, &scope)
                .expect("reference")
                .as_deref(),
            expected,
            "{input:?}"
        );
    }
    let error = prepare_issue_reference("7", None, &scope).expect_err("missing team");
    assert_eq!(error.message(), "Issue number 7 needs a team");
    let error = prepare_issue_reference("https://linear.app/foreign/issue/ENG-7", None, &scope)
        .expect_err("foreign URL");
    assert!(error.message().contains("this is the \"acme\" workspace"));
    let error = prepare_issue_reference("https://linear.app/foreign/team/eng", None, &scope)
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
