use serde_json::json;

use super::{Comment, threads, wrap_parts};

fn comment(id: &str, created_at: &str, parent: Option<&str>, resolved: bool) -> Comment {
    serde_json::from_value(json!({
        "id": id, "body": id, "quotedText": null, "createdAt": created_at,
        "url": format!("https://linear.app/acme/issue/ENG-1#comment-{id}"),
        "resolvedAt": resolved.then_some("2024-01-09T00:00:00Z"),
        "resolvingCommentId": null, "resolvingUser": null, "user": null, "externalUser": null,
        "parent": parent.map(|id| json!({ "id": id })),
    }))
    .expect("comment")
}

fn ids(comments: &[&Comment]) -> Vec<String> {
    comments.iter().map(|c| c.id.inner().to_owned()).collect()
}

#[test]
fn replies_group_under_their_thread_root_in_time_order() {
    let comments = [
        comment("late-root", "2024-01-05T00:00:00Z", None, false),
        comment("root", "2024-01-01T00:00:00Z", None, false),
        comment("grandchild", "2024-01-04T00:00:00Z", Some("child"), false),
        comment("child", "2024-01-03T00:00:00Z", Some("root"), false),
        comment("resolved", "2024-01-02T00:00:00Z", None, true),
        comment("orphan", "2024-01-06T00:00:00Z", Some("missing"), false),
    ];
    let open = threads(&comments, false).expect("threads");
    assert_eq!(ids(&open.roots), ["root", "late-root"]);
    assert_eq!(open.hidden, 1);
    assert_eq!(ids(&open.replies["root"]), ["child", "grandchild"]);
    assert_eq!(ids(&open.replies["missing"]), ["orphan"]);

    let all = threads(&comments, true).expect("threads");
    assert_eq!(ids(&all.roots), ["root", "resolved", "late-root"]);
    assert_eq!(all.hidden, 0);
}

#[test]
fn a_parent_cycle_is_an_error() {
    let comments = [
        comment("a", "2024-01-01T00:00:00Z", Some("b"), false),
        comment("b", "2024-01-02T00:00:00Z", Some("a"), false),
    ];
    let Err(error) = threads(&comments, true) else {
        panic!("a parent cycle must be refused");
    };
    assert!(error.message().contains("cycle"), "{error}");
}

#[test]
fn comment_headers_wrap_between_parts_and_keep_their_indent() {
    let part = |text: &str| (text.to_owned(), text.len());
    let parts = [
        part("@alice"),
        part("replied 3 days ago"),
        part("[0a1b2c3d-0000-4000-8000-000000000000]"),
    ];
    assert_eq!(
        wrap_parts(&parts, "  ", 80),
        "  @alice replied 3 days ago [0a1b2c3d-0000-4000-8000-000000000000]"
    );
    assert_eq!(
        wrap_parts(&parts, "  ", 60),
        "  @alice replied 3 days ago\n  [0a1b2c3d-0000-4000-8000-000000000000]"
    );
    // A part wider than the terminal gets a line of its own.
    assert_eq!(
        wrap_parts(&parts, "", 10),
        "@alice\nreplied 3 days ago\n[0a1b2c3d-0000-4000-8000-000000000000]"
    );
}
