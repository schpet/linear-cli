use serde_json::json;

use super::{CommentTarget, build_input, check_parent, request, resolve_body};

#[test]
fn body_text_is_kept_exactly_and_blank_text_is_refused() {
    let error = resolve_body(Some("x"), Some("/definitely/missing")).expect_err("conflict");
    assert_eq!(
        error.message(),
        "Cannot specify both --body and --body-file"
    );
    let literal = "  **Bold** `code`\nline two ☃\t ";
    assert_eq!(
        resolve_body(Some(literal), None).expect("body"),
        Some(literal.to_owned())
    );
    for blank in [" \t\n", "\u{a0}\u{3000}", "\u{2028}"] {
        let error = resolve_body(Some(blank), None).expect_err("blank");
        assert_eq!(error.message(), "Comment body cannot be empty");
    }
    assert_eq!(resolve_body(None, None).expect("no body"), None);
}

#[test]
fn body_files_drop_a_byte_order_mark_and_must_be_utf8_text() {
    let dir = tempfile::tempdir().expect("temp dir");
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("write body file");
        path.to_str().expect("UTF-8 temp path").to_owned()
    };
    let text = write(
        "text.md",
        b"\xef\xbb\xbf# Title\r\n\n  \xe2\x98\x83 *md* \n",
    );
    assert_eq!(
        resolve_body(None, Some(&text)).expect("body"),
        Some("# Title\r\n\n  ☃ *md* \n".to_owned())
    );
    let blank = write("blank.md", b"\xef\xbb\xbf \n");
    let error = resolve_body(None, Some(&blank)).expect_err("blank file");
    assert_eq!(error.message(), format!("Body file is empty: {blank}"));
    for (name, bytes) in [
        ("truncated.md", &b"x\xe2\x82"[..]),
        ("surrogate.md", b"a\xed\xa0\x80b"),
        ("overlong.md", b"c\xc0\xafd"),
    ] {
        let path = write(name, bytes);
        let error = resolve_body(None, Some(&path)).expect_err("invalid UTF-8");
        assert_eq!(error.message(), "Body file must be valid UTF-8", "{name}");
    }
    let missing = dir.path().join("missing.md");
    let missing = missing.to_str().expect("UTF-8 temp path");
    let error = resolve_body(None, Some(missing)).expect_err("missing file");
    assert_eq!(
        error.message(),
        format!("Failed to read body file: {missing}")
    );
}

#[test]
fn input_names_exactly_one_target_and_omits_absent_optionals() {
    let input = |target, parent: Option<&str>, id: Option<&str>| {
        let request = request(build_input(target, "Body".into(), parent, id));
        serde_json::to_value(request).expect("request")["variables"]["input"].clone()
    };
    assert_eq!(
        input(
            CommentTarget::Issue {
                issue_id: "i".into()
            },
            None,
            None
        ),
        json!({"body": "Body", "issueId": "i"})
    );
    assert_eq!(
        input(
            CommentTarget::Document {
                document_content_id: "d".into()
            },
            Some("p"),
            None
        ),
        json!({"body": "Body", "parentId": "p", "documentContentId": "d"})
    );
    assert_eq!(
        input(
            CommentTarget::Project {
                project_id: "pr".into()
            },
            None,
            Some("fixed")
        ),
        json!({"body": "Body", "id": "fixed", "projectId": "pr"})
    );
    assert_eq!(
        input(
            CommentTarget::Initiative {
                initiative_id: "in".into()
            },
            None,
            None
        ),
        json!({"body": "Body", "initiativeId": "in"})
    );
}

#[test]
fn a_parent_comment_link_is_explained_before_other_linear_urls() {
    let link = "https://linear.app/acme/issue/ENG-1/title#comment-abcdef12";
    let error = check_parent(Some(link)).expect_err("comment link");
    assert_eq!(
        error.message(),
        format!(
            "\"{link}\" links to a comment, but a comment URL only carries the first eight characters of its ID."
        )
    );
    let url = "https://linear.app/acme/issue/ENG-1";
    let error = check_parent(Some(url)).expect_err("Linear URL");
    assert_eq!(
        error.message(),
        format!("\"{url}\" is a Linear URL, and this command does not take one.")
    );
    assert_eq!(
        error.hint(),
        Some("Pass the UUID of the comment to reply to.")
    );
    check_parent(Some("7d2e4f1a-3b5c-4d6e-8f90-a1b2c3d4e5f6")).expect("comment UUID");
}
