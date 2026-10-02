use linear_cli::refs::{is_linear_uuid, reject_linear_url};

#[test]
fn url_guard_rejects_known_and_unsupported_urls_without_workspace_selection() {
    for input in [
        "https://linear.app/acme/issue/ENG-12",
        "https://linear.app/foreign/team/eng",
        "https://linear.app/acme/team/eng/secret",
        "linear.app/acme/issue/ENG-12",
        "  https://linear.app/acme/issue/ENG-12  ",
    ] {
        let error = reject_linear_url(input, "a template name or UUID").expect_err(input);
        assert_eq!(
            error.message(),
            format!("\"{input}\" is a Linear URL, and this command does not take one.")
        );
        assert_eq!(error.hint(), Some("Pass a template name or UUID."));
        assert_eq!(error.to_string(), error.message());
    }
    for input in [
        "A template",
        "linear.example/acme/issue/ENG-12",
        "https://notlinear.app/acme/issue/ENG-12",
    ] {
        reject_linear_url(input, "a template name or UUID").expect(input);
    }
}

#[test]
fn uuid_shape_matches_linear_without_version_or_variant_rules() {
    assert!(is_linear_uuid("ABCDEF01-2345-6789-abCD-ef0123456789"));
    assert!(is_linear_uuid("00000000-0000-0000-0000-000000000000"));
    for input in [
        "abcdef01-2345-6789-abcd-ef012345678",
        "abcdef012345-6789-abcd-ef0123456789",
        "abcdef01-2345-6789-abcd-ef012345678g",
        "abcdef01-2345-6789-abcd-ef0123456789\n",
        "abcdef01-2345-6789-abcd-ef012345678é",
    ] {
        assert!(!is_linear_uuid(input), "{input}");
    }
}
