use super::{Read, from_branch, from_trailers, one, scan, trunk_is_root};
use crate::cli::values::IssueRef;

#[test]
fn identifiers_start_and_end_on_ascii_word_boundaries() {
    for (text, expected) in [
        ("eng-123_x", &[][..]),
        ("éENG-5", &["ENG-5"]),
        ("ENG-0", &[]),
        ("ENG-123abc", &[]),
        ("feature/eng-7-x", &["ENG-7"]),
        ("ENG-01", &[]),
        ("_ENG-7", &[]),
        ("ENG-9é", &["ENG-9"]),
        ("x/42-99999999999999999999", &["42-99999999999999999999"]),
        ("sam/eng-7-add-utf-8-support", &["ENG-7", "UTF-8"]),
        ("Fixes ENG-7, References OPS-12", &["ENG-7", "OPS-12"]),
        ("ENG-1-2", &["ENG-1"]),
        ("", &[]),
    ] {
        assert_eq!(scan(text), expected, "{text}");
    }
}

#[test]
fn a_git_branch_names_its_first_issue_id() {
    let read = from_branch("sam/eng-7-add-utf-8-support").expect("an issue");
    assert_eq!(read.identifiers, ["ENG-7"]);
    assert_eq!(
        read.notice(),
        "Using issue ENG-7 from the git branch name \"sam/eng-7-add-utf-8-support\"\n"
    );
    let none = from_branch("main").expect_err("no issue");
    assert_eq!(
        none.to_string(),
        "No issue ID in the git branch name \"main\""
    );
}

fn changes(lines: &[(&str, &[(&str, &str)])]) -> Vec<u8> {
    let mut out = String::new();
    for (change, trailers) in lines {
        out.push_str(&serde_json::json!([change, trailers]).to_string());
        out.push('\n');
    }
    out.into_bytes()
}

#[test]
fn jj_trailers_name_every_distinct_issue_in_order() {
    let read = from_trailers(&changes(&[
        ("aaa", &[]),
        (
            "bbb",
            &[
                ("Linear-issue", "Fixes ENG-10"),
                ("Co-authored-by", "A <a@example.com>"),
                ("linear-issue", "References eng-7"),
            ],
        ),
        ("ccc", &[("Linear-issue", "Fixes OPS-2, ENG-7, ENG-9")]),
    ]))
    .expect("issues");
    assert_eq!(read.identifiers, ["ENG-7", "ENG-9", "ENG-10", "OPS-2"]);
    assert_eq!(
        read.notice(),
        "Using issues ENG-7, ENG-9, ENG-10, OPS-2 from Linear-issue trailers in trunk()..@\n"
    );
    let single =
        from_trailers(&changes(&[("bbb", &[("Linear-issue", "Fixes ENG-7")])])).expect("one issue");
    assert_eq!(
        single.notice(),
        "Using issue ENG-7 from Linear-issue trailers in trunk()..@\n"
    );
}

#[test]
fn jj_trailers_must_each_name_an_issue_and_one_must_exist() {
    let nothing = from_trailers(&changes(&[(
        "bbb",
        &[
            ("Linear-issue", "Fixes ENG-7"),
            ("Linear-issue", "Fixes it"),
        ],
    )]))
    .expect_err("a trailer names nothing");
    assert_eq!(
        nothing.to_string(),
        "Cannot read @jj: the Linear-issue trailer \"Fixes it\" of change bbb names no issue"
    );
    for stdout in [
        &b""[..],
        &changes(&[("aaa", &[("Co-authored-by", "ENG-7")])]),
    ] {
        let none = from_trailers(stdout).expect_err("no trailer");
        assert_eq!(
            none.to_string(),
            "Cannot read @jj: no Linear-issue trailer in trunk()..@ names an issue"
        );
    }
    for bad in [
        &b"[1]\n"[..],
        b"not json\n",
        b"[\"a\",[[\"k\"]]]\n",
        b"\xff\n",
    ] {
        let error = from_trailers(bad).expect_err("malformed");
        assert!(
            error
                .to_string()
                .starts_with("Unexpected output from `jj log`"),
            "{error}"
        );
    }
}

#[test]
fn the_trunk_probe_is_one_boolean() {
    assert!(trunk_is_root(b"true\n").expect("root"));
    assert!(!trunk_is_root(b"false\n").expect("a trunk"));
    for bad in [&b""[..], b"true\nfalse\n", b"yes\n"] {
        assert!(trunk_is_root(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_reference_for_one_issue_refuses_several_and_lists_them() {
    let read = |ids: &[&str]| Read {
        identifiers: ids.iter().map(|&id| id.to_owned()).collect(),
        source: "here".to_owned(),
    };
    assert_eq!(one(&IssueRef::Jj, &read(&["ENG-7"])).expect("one"), "ENG-7");
    let several = one(&IssueRef::Jj, &read(&["ENG-7", "ENG-9"])).expect_err("several");
    assert_eq!(
        several.to_string(),
        "@jj names several issues: ENG-7, ENG-9"
    );
}
