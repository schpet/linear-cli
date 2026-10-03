use super::{TEMPLATE_SUGGESTION, body, read_template};

#[test]
fn the_body_is_the_trimmed_template_then_the_issue_url() {
    assert_eq!(body(Some(" leading\r\n \t"), "URL"), " leading\n\nURL");
    assert_eq!(body(Some(" \r\n"), "URL"), "URL");
    assert_eq!(body(None, "URL"), "URL");
}

#[test]
fn templates_must_be_readable_utf8_text_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("template");
    std::fs::write(&file, b"\xef\xbb\xbfSummary\r\n").expect("write template");
    assert_eq!(read_template(&file).expect("template"), "Summary\r\n");
    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&file, &link).expect("symlink");
        assert_eq!(read_template(&link).expect("linked template"), "Summary\r\n");
    }
    for (path, reason) in [
        (dir.path().join("missing"), "does not exist"),
        (dir.path().to_owned(), "is a directory, not a file"),
    ] {
        let error = read_template(&path).expect_err(reason);
        assert!(error.message().contains(reason), "{error}");
        assert_eq!(error.hint(), Some(TEMPLATE_SUGGESTION));
    }
    std::fs::write(&file, b"Summary\xff").expect("write template");
    let error = read_template(&file).expect_err("invalid UTF-8");
    assert!(error.message().ends_with("is not valid UTF-8 text"), "{error}");
    std::fs::write(&file, b"Summary\0").expect("write template");
    let error = read_template(&file).expect_err("binary");
    assert!(error.message().ends_with("is not a text file"), "{error}");
}
