use linear_cli::commands::prosemirror::to_markdown;
use linear_cli::commands::template_data::JsValue;
use linear_cli::error::AppErrorKind;
use serde_json::{Value, json};

fn markdown(doc: Value) -> String {
    let value: JsValue = serde_json::from_value(doc).expect("JS value");
    to_markdown(&value).unwrap_or_else(|error| panic!("{error}"))
}

fn error(doc: Value) -> String {
    let value: JsValue = serde_json::from_value(doc).expect("JS value");
    let error = to_markdown(&value).expect_err("invalid document");
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(error.suggestion, None);
    error.message
}

fn text(value: &str) -> Value {
    json!({"type": "text", "text": value})
}

fn marked(value: &str, marks: Value) -> Value {
    json!({"type": "text", "text": value, "marks": marks})
}

fn paragraph(content: Vec<Value>) -> Value {
    json!({"type": "paragraph", "content": content})
}

fn doc(content: Vec<Value>) -> Value {
    json!({"type": "doc", "content": content})
}

#[test]
fn renders_the_body_linear_stores_for_an_issue_template() {
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "heading", "attrs": {"level": 2, "id": "f0ba1592"}, "content": [text("Steps to reproduce")]}),
            json!({"type": "ordered_list", "attrs": {"order": 1}, "content": [{"type": "list_item", "content": [{"type": "paragraph"}]}]}),
            json!({"type": "heading", "attrs": {"level": 2}, "content": [text("Expected")]}),
            json!({"type": "heading", "attrs": {"level": 2}, "content": [text("Actual")]}),
        ])),
        "## Steps to reproduce\n\n1. \n\n## Expected\n\n## Actual"
    );
}

#[test]
fn renders_marks_links_and_hard_breaks() {
    assert_eq!(
        markdown(doc(vec![paragraph(vec![
            text("Hello "),
            marked("world", json!([{"type": "bold"}])),
            text(" and "),
            marked("code", json!([{"type": "code"}])),
            text(" "),
            marked(
                "link",
                json!([{"type": "link", "attrs": {"href": "https://example.com"}}])
            ),
            json!({"type": "hard_break"}),
            marked("next", json!([{"type": "em"}, {"type": "strike"}])),
        ])])),
        "Hello **world** and `code` [link](https://example.com)\n~~_next_~~"
    );
}

#[test]
fn renders_nested_lists_todo_lists_code_and_quotes() {
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "bullet_list", "content": [
                {"type": "list_item", "content": [
                    paragraph(vec![text("one")]),
                    {"type": "bullet_list", "content": [
                        {"type": "list_item", "content": [paragraph(vec![text("nested")])]}
                    ]}
                ]},
                {"type": "list_item", "content": [paragraph(vec![text("two")])]}
            ]}),
            json!({"type": "todo_list", "content": [
                {"type": "todo_item", "attrs": {"done": true}, "content": [paragraph(vec![text("done")])]},
                {"type": "todo_item", "content": [paragraph(vec![text("open")])]}
            ]}),
            json!({"type": "code_block", "attrs": {"language": "ts"}, "content": [text("const a = 1")]}),
            json!({"type": "blockquote", "content": [paragraph(vec![text("quoted")])]}),
            json!({"type": "horizontal_rule"}),
        ])),
        [
            "- one\n\n  - nested\n- two",
            "- [x] done\n- [ ] open",
            "```ts\nconst a = 1\n```",
            "> quoted",
            "---",
        ]
        .join("\n\n")
    );
}

#[test]
fn keeps_unknown_nodes_visible_instead_of_dropping_them() {
    assert_eq!(
        markdown(doc(vec![
            paragraph(vec![
                text("cc "),
                json!({"type": "suggestion_userMentions", "attrs": {"id": "u1", "label": "@sam"}}),
                text(" and "),
                json!({"type": "mystery_inline"}),
            ]),
            json!({"type": "embed", "attrs": {"url": "https://example.com"}}),
            json!({"type": "wrapper", "content": [paragraph(vec![text("inside a wrapper")])]}),
        ])),
        "cc @sam and [mystery_inline]\n\n[unsupported embed node]\n\ninside a wrapper"
    );
}

#[test]
fn escapes_literal_markdown_punctuation_but_not_code() {
    assert_eq!(
        markdown(doc(vec![
            paragraph(vec![
                text("*literal* `tick` [x] a_b # 1 > 2 "),
                marked("a ` b", json!([{"type": "code"}])),
                text(" "),
                marked("*bold*", json!([{"type": "bold"}])),
            ]),
            paragraph(vec![text("- not a list")]),
            paragraph(vec![text("1. not ordered")]),
            paragraph(vec![text("2026 roadmap")]),
            json!({"type": "code_block", "content": [text("```md\nfenced inside\n```")]}),
        ])),
        [
            "\\*literal\\* \\`tick\\` \\[x\\] a\\_b \\# 1 \\> 2 `` a ` b `` **\\*bold\\***",
            "\\- not a list",
            "1\\. not ordered",
            "2026 roadmap",
            "````\n```md\nfenced inside\n```\n````",
        ]
        .join("\n\n")
    );
}

#[test]
fn rejects_values_that_are_not_a_document() {
    assert_eq!(
        error(json!({"type": "paragraph", "content": []})),
        "Expected a ProseMirror document, got a \"paragraph\" node"
    );
    assert_eq!(
        error(json!({"type": "doc", "content": "not an array"})),
        "Invalid ProseMirror node at doc: \"content\" must be an array"
    );
    assert_eq!(
        error(json!("## markdown")),
        "Invalid ProseMirror node at doc: expected an object with a string \"type\""
    );
}

#[test]
fn reports_the_first_invalid_path_children_before_marks_and_root_type() {
    assert_eq!(
        error(json!({"type": "doc", "content": [
            paragraph(vec![text("ok"), json!({"type": 7})])
        ]})),
        "Invalid ProseMirror node at doc.content[0].content[1]: expected an object with a string \"type\""
    );
    assert_eq!(
        error(json!({"type": "doc", "content": [{"type": "text", "marks": {}}]})),
        "Invalid ProseMirror node at doc.content[0]: \"marks\" must be an array"
    );
    assert_eq!(
        error(json!({"type": "doc", "content": [{"type": "text", "text": 1}]})),
        "Invalid ProseMirror node at doc.content[0]: \"text\" must be a string"
    );
    assert_eq!(
        error(
            json!({"type": "doc", "content": [{"type": "text", "text": "x", "marks": [{"attrs": {}}]}]})
        ),
        "Invalid ProseMirror mark at doc.content[0].marks[0]: expected an object with a string \"type\""
    );
    // Content is checked before marks, text and children before marks.
    assert_eq!(
        error(json!({"type": "p", "content": 1, "marks": 1, "text": 1})),
        "Invalid ProseMirror node at doc: \"content\" must be an array"
    );
    assert_eq!(
        error(json!({"type": "p", "marks": [1], "content": [{"type": false}]})),
        "Invalid ProseMirror node at doc.content[0]: expected an object with a string \"type\""
    );
    // The whole tree is read before the root type is judged.
    assert_eq!(
        error(json!({"type": "bad", "content": [[]]})),
        "Invalid ProseMirror node at doc.content[0]: expected an object with a string \"type\""
    );
    // Null content, marks and text are absent; the empty document is empty.
    assert_eq!(
        markdown(json!({"type": "doc", "content": null, "marks": null, "text": null})),
        ""
    );
}

#[test]
fn list_markers_escape_only_at_js_line_starts_before_whitespace() {
    let paragraph_of = |value: &str| markdown(doc(vec![paragraph(vec![text(value)])]));
    assert_eq!(paragraph_of("  - a\n\t+ b"), "  \\- a\n\t\\+ b");
    assert_eq!(paragraph_of("12. a\r3. b"), "12\\. a\r3\\. b");
    assert_eq!(
        paragraph_of("x\u{2028}- a\u{2029}1.\u{a0}b"),
        "x\u{2028}\\- a\u{2029}1\\.\u{a0}b"
    );
    // The lookahead may see the line terminator itself, but not the end.
    assert_eq!(paragraph_of("-\n1."), "\\-\n1.");
    for unchanged in ["-a", "1.5 b", "a - b", "1 . b", "٣. b", "-"] {
        assert_eq!(paragraph_of(unchanged), unchanged);
    }
}

#[test]
fn attributes_follow_the_source_fallbacks() {
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "heading", "attrs": {"level": 9}, "content": [text("six")]}),
            json!({"type": "heading", "attrs": {"level": 2.7}, "content": [text("two")]}),
            json!({"type": "heading", "attrs": {"level": "3"}, "content": [text("one")]}),
            json!({"type": "heading", "attrs": {"level": 0}}),
        ])),
        // The empty last heading's trailing space is trimmed with the document.
        "###### six\n\n## two\n\n# one\n\n#"
    );
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "ordered_list", "attrs": {"order": 1.5}, "content": [
                {"type": "list_item", "content": [paragraph(vec![text("a")])]},
                {"type": "list_item", "content": [paragraph(vec![text("b")]), paragraph(vec![text("c")])]}
            ]})
        ])),
        "1.5. a\n2.5. b\n\n     c"
    );
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "ordered_list", "attrs": {"order": 1e21}, "content": [
                {"type": "list_item", "content": [paragraph(vec![text("x")])]}
            ]})
        ])),
        "1e+21. x"
    );
    // Todo markers read done or checked from any child type; a non-item child
    // renders as a block.
    assert_eq!(
        markdown(doc(vec![json!({"type": "todo_list", "content": [
            {"type": "list_item", "attrs": {"checked": true}, "content": [paragraph(vec![text("a")])]},
            {"type": "paragraph", "attrs": {"done": "true"}, "content": [text("b\nc")]}
        ]})])),
        "- [x] a\n- [ ] b\n      c"
    );
    assert_eq!(
        markdown(doc(vec![paragraph(vec![
            marked(
                "plain",
                json!([{"type": "link", "attrs": {"href": ""}}, {"type": "underline"}])
            ),
            json!({"type": "image", "attrs": {"alt": "a*", "src": "https://x/y.png"}}),
            json!({"type": "mention", "attrs": {"label": "@a_b"}, "marks": [{"type": "strong"}]}),
            json!({"type": "emoji", "text": "*"}),
            json!({"type": "span", "content": [text("in")]}),
            json!({"type": "paragraph"}),
        ])])),
        "plain![a*](https://x/y.png)**@a\\_b**\\*in[paragraph]"
    );
    assert_eq!(
        markdown(doc(vec![
            json!({"type": "code_block", "content": [text("a ```` b"), {"type": "hard_break"}, text("c")]}),
            json!({"type": "blockquote", "content": [paragraph(vec![text("a")]), paragraph(vec![text("b")])]}),
            json!({"type": "unknown_leaf", "text": "_t_", "marks": [{"type": "italic"}]}),
            json!({"type": "doc"}),
        ])),
        "`````\na ```` bc\n`````\n\n> a\n> \n> b\n\n_\\_t\\__\n\n[unsupported doc node]"
    );
}

#[test]
fn trailing_js_whitespace_is_trimmed_from_the_whole_document() {
    assert_eq!(
        markdown(doc(vec![paragraph(vec![text("body \u{feff}\u{3000}\n")])])),
        "body"
    );
    assert_eq!(
        markdown(doc(vec![paragraph(vec![text("body\u{feff}")])])),
        "body"
    );
    assert_eq!(
        markdown(doc(vec![paragraph(vec![text("body\u{85}")])])),
        "body\u{85}"
    );
    assert_eq!(markdown(doc(vec![])), "");
}
