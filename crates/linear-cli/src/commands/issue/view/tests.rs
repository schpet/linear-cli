use pulldown_cmark::{Event, Parser, Tag};
use serde_json::json;

use super::{Comment, DisplayText, push_markdown_link, threads};

#[test]
fn issue_markup_keeps_a_literal_label_paired_with_its_destination() {
    let label = " [label](https://example.com) <*literal*> &amp; `code` ";
    let url = "https://linear.app/workspace-one/issue/DEMO-42?q=(\"a b\")&end=<>&entity=&amp;";
    let mut markdown = "- **".to_owned();
    push_markdown_link(&mut markdown, label, url);
    markdown.push_str(" [archived]**: Work");
    let mut parsed = Parser::new(&markdown);
    let target = parsed.find_map(|event| match event {
        Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url),
        _ => None,
    });
    assert_eq!(target.as_deref(), Some(url));
    let text: String = parsed
        .by_ref()
        .take_while(|event| !matches!(event, Event::End(pulldown_cmark::TagEnd::Link)))
        .map(|event| match event {
            Event::Text(text) => text,
            other => panic!("unexpected event inside link {other:?}"),
        })
        .map(|text| text.into_string())
        .collect();
    assert_eq!(text, label);
    assert!(parsed.any(|event| matches!(event, Event::Text(text) if text.starts_with(" "))));
}

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

#[test]
fn only_visible_comments_get_labels_in_thread_order() {
    let comments = [
        comment("late-root", "2024-01-05T00:00:00Z", None, false),
        comment("root", "2024-01-01T00:00:00Z", None, false),
        comment("grandchild", "2024-01-04T00:00:00Z", Some("child"), false),
        comment("child", "2024-01-03T00:00:00Z", Some("root"), false),
        comment("resolved", "2024-01-02T00:00:00Z", None, true),
        comment(
            "resolved-reply",
            "2024-01-06T00:00:00Z",
            Some("resolved"),
            false,
        ),
        comment("orphan", "2024-01-06T00:00:00Z", Some("missing"), false),
    ];
    let mut formatted = Vec::new();
    let mut label = |comment: &Comment| {
        formatted.push(comment.id.inner().to_owned());
        Ok(DisplayText::default())
    };
    let open = threads(&comments, false, &mut label).expect("threads");
    assert_eq!(open.hidden, 1);

    let all = threads(&comments, true, label).expect("threads");
    assert_eq!(all.hidden, 0);
    assert_eq!(
        formatted,
        [
            "root",
            "child",
            "grandchild",
            "late-root",
            "root",
            "child",
            "grandchild",
            "resolved",
            "resolved-reply",
            "late-root",
        ]
    );
}

#[test]
fn a_parent_cycle_is_an_error() {
    let comments = [
        comment("a", "2024-01-01T00:00:00Z", Some("b"), false),
        comment("b", "2024-01-02T00:00:00Z", Some("a"), false),
    ];
    let Err(error) = threads(&comments, true, |_| Ok(DisplayText::default())) else {
        panic!("a parent cycle must be refused");
    };
    assert!(error.message().contains("cycle"), "{error}");
}
