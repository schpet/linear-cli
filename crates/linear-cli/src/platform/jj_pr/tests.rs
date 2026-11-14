use super::{
    Bookmark, Commit, Head, Remote, Source, choose_bookmark, choose_remote, is_hosted,
    parse_commit, parse_lines, parse_remotes, source,
};

fn commit(id: &str, parents: &[&str], described: bool, empty: bool) -> Commit {
    Commit {
        id: id.to_owned(),
        parents: parents.iter().map(|&parent| parent.to_owned()).collect(),
        described,
        empty,
        conflict: false,
    }
}

fn bookmark(name: &str, target: &[Option<&str>]) -> Bookmark {
    Bookmark {
        name: name.to_owned(),
        remote: None,
        target: target.iter().map(|id| id.map(str::to_owned)).collect(),
    }
}

fn remote(name: &str) -> Remote {
    Remote {
        name: name.to_owned(),
        url: format!("git@github.com:acme/{name}.git"),
    }
}

#[test]
fn jj_commit_and_bookmark_json_is_read_strictly() {
    let line =
        br#"[{"commit_id":"abc","parents":["p1"],"change_id":"x","description":" \n"},true,true]
"#;
    let parsed = parse_commit(line).expect("one commit");
    assert_eq!(parsed.id, "abc");
    assert_eq!(parsed.parents, ["p1"]);
    assert!(!parsed.described, "whitespace is no description");
    assert!(parsed.empty && parsed.conflict);
    for bad in [
        &b""[..],
        b"[1,2]\n",
        b"not json\n",
        &[line.as_slice(), line.as_slice()].concat(),
    ] {
        let error = parse_commit(bad).expect_err("malformed");
        assert!(
            error
                .to_string()
                .starts_with("Unexpected output from `jj log`"),
            "{error}"
        );
    }
    let bookmarks: Vec<Bookmark> = parse_lines(
        br#"{"name":"a","target":["1"]}
{"name":"a","remote":"origin","target":["1"]}
{"name":"gone","target":[null]}
{"name":"split","target":["1","2",null]}
"#,
        "jj bookmark list",
    )
    .expect("bookmarks");
    let commits: Vec<Option<&str>> = bookmarks.iter().map(Bookmark::commit).collect();
    assert_eq!(
        commits,
        [Some("1"), None, None, None],
        "only a resolved local bookmark"
    );
}

#[test]
fn the_head_is_the_working_copy_unless_it_is_an_empty_scratch_change() {
    let none = [];
    let from = |working_copy: &Commit| source(None, working_copy, &none);
    assert_eq!(
        from(&commit("w", &["p"], false, false)).expect("dirty"),
        Source::WorkingCopy
    );
    assert_eq!(
        from(&commit("w", &["p"], true, true)).expect("described"),
        Source::WorkingCopy
    );
    assert_eq!(
        from(&commit("w", &["p"], false, true)).expect("scratch"),
        Source::Parent("p".to_owned())
    );
    let merge = from(&commit("w", &["p", "q"], false, true)).expect_err("empty merge");
    assert!(merge.to_string().contains("has 2 parents"), "{merge}");
    assert_eq!(
        merge.hint(),
        Some("Pass --head with the bookmark to open the pull request from.")
    );
}

#[test]
fn an_existing_bookmark_named_by_head_decides_the_revision() {
    let bookmarks = [
        bookmark("topic", &[Some("t")]),
        bookmark("split", &[Some("a"), Some("b")]),
    ];
    let merge = commit("w", &["p", "q"], false, true);
    assert_eq!(
        source(Some("topic"), &merge, &bookmarks).expect("explicit"),
        Source::Bookmark("t".to_owned())
    );
    let conflicted = source(Some("split"), &merge, &bookmarks).expect_err("conflicted");
    assert_eq!(
        conflicted.to_string(),
        "The jj bookmark split is deleted or conflicted"
    );
    // A name that is not a bookmark yet is created on the automatic head.
    assert_eq!(
        source(Some("new"), &commit("w", &["p"], false, true), &bookmarks).expect("new name"),
        Source::Parent("p".to_owned())
    );
}

#[test]
fn a_bookmark_at_the_head_is_reused_and_none_is_ever_moved() {
    let head = |name: &str, create| Head {
        name: name.to_owned(),
        create,
    };
    let choose =
        |explicit, bookmarks: &[Bookmark]| choose_bookmark(explicit, "eng-7-fix", "h", bookmarks);
    assert_eq!(
        choose(None, &[]).expect("none yet"),
        head("eng-7-fix", true)
    );
    assert_eq!(
        choose(
            None,
            &[
                bookmark("mine", &[Some("h")]),
                bookmark("other", &[Some("x")])
            ]
        )
        .expect("sole"),
        head("mine", false)
    );
    assert_eq!(
        choose(
            None,
            &[
                bookmark("mine", &[Some("h")]),
                bookmark("eng-7-fix", &[Some("h")])
            ]
        )
        .expect("the suggested name wins"),
        head("eng-7-fix", false)
    );
    let several = choose(
        None,
        &[bookmark("b", &[Some("h")]), bookmark("a", &[Some("h")])],
    )
    .expect_err("two candidates");
    assert_eq!(
        several.to_string(),
        "Several jj bookmarks point at the pull request revision: a, b"
    );
    let elsewhere = choose(None, &[bookmark("eng-7-fix", &[Some("old")])]).expect_err("taken");
    assert_eq!(
        elsewhere.to_string(),
        "The jj bookmark eng-7-fix already exists at another revision"
    );
    let deleted = choose(None, &[bookmark("eng-7-fix", &[None])]).expect_err("deleted");
    assert_eq!(
        deleted.to_string(),
        "The jj bookmark eng-7-fix is deleted or conflicted"
    );
    assert_eq!(
        choose(Some("topic"), &[bookmark("mine", &[Some("h")])]).expect("new explicit"),
        head("topic", true)
    );
    assert_eq!(
        choose(Some("topic"), &[bookmark("topic", &[Some("h")])]).expect("existing explicit"),
        head("topic", false)
    );
}

#[test]
fn the_push_remote_follows_jj_defaults() {
    let all = || vec![remote("fork"), remote("origin")];
    assert_eq!(
        choose_remote(Some("fork"), all()).expect("configured").name,
        "fork"
    );
    assert_eq!(choose_remote(None, all()).expect("origin").name, "origin");
    assert_eq!(
        choose_remote(None, vec![remote("fork")])
            .expect("only")
            .name,
        "fork"
    );
    let missing = choose_remote(Some("gone"), all()).expect_err("missing");
    assert!(
        missing.to_string().contains("names the remote gone"),
        "{missing}"
    );
    let ambiguous = choose_remote(None, vec![remote("a"), remote("b")]).expect_err("ambiguous");
    assert_eq!(
        ambiguous.to_string(),
        "Cannot tell which remote to push to: a, b"
    );
    let none = choose_remote(None, Vec::new()).expect_err("none");
    assert_eq!(
        none.to_string(),
        "This jj repository has no git remote to push to"
    );
}

#[test]
fn remotes_are_listed_by_name_and_url_and_only_hosted_urls_name_a_repository() {
    let remotes = parse_remotes(
        "origin https://github.com/acme/widgets.git (push: git@github.com:acme/widgets.git)\nfork git@github.com:me/widgets.git\n",
    )
    .expect("remotes");
    assert_eq!(remotes[0].url, "https://github.com/acme/widgets.git");
    assert_eq!(remotes[1].name, "fork");
    assert!(parse_remotes("origin\n").is_err());
    for url in [
        "https://github.com/acme/widgets.git",
        "ssh://git@github.com/acme/widgets",
        "git@github.com:acme/widgets.git",
        "github.example.com:acme/widgets",
    ] {
        assert!(is_hosted(url), "{url}");
    }
    for url in [
        "/srv/git/widgets.git",
        "../widgets",
        "file:///srv/widgets.git",
        "C:\\repos\\widgets",
        "widgets",
    ] {
        assert!(!is_hosted(url), "{url}");
    }
}
