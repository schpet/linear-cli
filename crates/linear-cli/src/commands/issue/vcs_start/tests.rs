use super::{GitDone, Previous, free_suffix, parse_change};

#[test]
fn an_existing_branch_gets_the_first_free_suffix() {
    let mut checked = Vec::new();
    let branch = free_suffix("eng-1", |name| {
        checked.push(name.to_owned());
        Ok(name != "eng-1-3")
    })
    .expect("free suffix");
    assert_eq!(branch, "eng-1-3");
    assert_eq!(checked, ["eng-1-1", "eng-1-2", "eng-1-3"]);
}

#[test]
fn the_working_copy_probe_is_one_change_with_an_id() {
    let change = |description: &str, empty: bool| {
        format!(
            "[{{\"change_id\":\"abc\",\"description\":{description:?},\"commit_id\":\"c\"}},{empty}]\n"
        )
    };
    let scratch = parse_change(change("", true).as_bytes()).expect("scratch");
    assert_eq!(scratch.id, "abc");
    assert!(scratch.scratch);
    for (description, empty) in [("notes\n", true), ("", false), (" ", true)] {
        let probed = parse_change(change(description, empty).as_bytes()).expect("change");
        assert!(!probed.scratch, "{description:?} {empty}");
    }
    let two = change("", true).repeat(2);
    for malformed in [
        "",
        "occupied\n",
        two.as_str(),
        "[{\"change_id\":\"\",\"description\":\"\"},true]\n",
        "[{\"description\":\"\"},true]\n",
    ] {
        let error = parse_change(malformed.as_bytes()).expect_err(malformed);
        assert!(
            error
                .message()
                .starts_with("Unexpected output from `jj log`"),
            "{error}"
        );
    }
    assert!(parse_change(b"\xff\n").is_err());
}

#[test]
fn git_results_quote_what_git_named() {
    let created = GitDone::Created {
        branch: "a\"b".to_owned(),
        base: "HEAD".to_owned(),
        commit: "0123456789ab".to_owned(),
        previous: Previous::Detached("fedcba987654".to_owned()),
    };
    assert_eq!(
        created.report(),
        "✓ Created and switched to git branch \"a\\\"b\" from \"HEAD\" (0123456789ab); previous HEAD: detached at fedcba987654\n"
    );
    let kept = GitDone::Switched {
        branch: "topic".to_owned(),
        previous: Previous::Branch("topic".to_owned()),
        unused_base: Some("main".to_owned()),
    };
    assert_eq!(
        kept.report(),
        "✓ Kept git branch \"topic\" checked out\n\
         ✓ Left existing branch history unchanged; --from-ref \"main\" was not used\n"
    );
}
