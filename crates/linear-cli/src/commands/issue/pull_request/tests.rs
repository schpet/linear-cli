use super::{Covered, Linkage, TEMPLATE_SUGGESTION, body, check_title, read_template, title};
use crate::graphql::operations::issue::IssueDetails;

fn covered(identifier: &str) -> Covered {
    Covered {
        identifier: identifier.to_owned(),
        details: IssueDetails {
            title: format!("{identifier} title"),
            url: format!("https://linear.app/acme/issue/{identifier}/slug"),
        },
    }
}

fn issues(identifiers: &[&str]) -> Vec<Covered> {
    identifiers.iter().map(|id| covered(id)).collect()
}

fn url(identifier: &str) -> String {
    format!("https://linear.app/acme/issue/{identifier}/slug")
}

#[test]
fn one_issue_gets_the_trimmed_template_the_clause_then_the_issue_url() {
    let one = issues(&["ENG-7"]);
    let eng7 = url("ENG-7");
    assert_eq!(
        body(Some(" leading\r\n \t"), &one, Linkage::Fixes),
        format!(" leading\n\nFixes ENG-7\n\n{eng7}")
    );
    assert_eq!(
        body(Some(" \r\n"), &one, Linkage::Fixes),
        format!("Fixes ENG-7\n\n{eng7}")
    );
    assert_eq!(
        body(None, &one, Linkage::Fixes),
        format!("Fixes ENG-7\n\n{eng7}")
    );
}

#[test]
fn one_issue_with_references_adds_the_clause_before_the_url() {
    let one = issues(&["ENG-7"]);
    let eng7 = url("ENG-7");
    assert_eq!(
        body(None, &one, Linkage::References),
        format!("References ENG-7\n\n{eng7}")
    );
    assert_eq!(
        body(Some("## Summary\n"), &one, Linkage::References),
        format!("## Summary\n\nReferences ENG-7\n\n{eng7}")
    );
}

#[test]
fn several_issues_get_one_clause_and_every_url_in_order() {
    let urls = |ids: &[&str]| ids.iter().map(|id| url(id)).collect::<Vec<_>>().join("\n");
    for (ids, clause) in [
        (&["ENG-7", "DES-5"][..], "ENG-7 and DES-5"),
        (&["ENG-7", "DES-5", "ENG-256"], "ENG-7, DES-5, and ENG-256"),
        (
            &["ENG-7", "DES-5", "ENG-256", "OPS-4"],
            "ENG-7, DES-5, ENG-256, and OPS-4",
        ),
    ] {
        let several = issues(ids);
        assert_eq!(
            body(None, &several, Linkage::Fixes),
            format!("Fixes {clause}\n\n{}", urls(ids))
        );
        assert_eq!(
            body(
                Some("## Summary\nDescribe the change.\n\n"),
                &several,
                Linkage::References
            ),
            format!(
                "## Summary\nDescribe the change.\n\nReferences {clause}\n\n{}",
                urls(ids)
            )
        );
    }
}

#[test]
fn a_given_title_is_used_verbatim_and_one_issue_defaults_to_its_id_and_title() {
    let one = issues(&["ENG-7"]);
    assert_eq!(title(&one, None), "ENG-7 ENG-7 title");
    assert_eq!(title(&one, Some(" Custom ")), " Custom ");
    let several = issues(&["ENG-7", "DES-5"]);
    assert_eq!(
        title(&several, Some("  Fix ENG-7 and DES-5 ")),
        "  Fix ENG-7 and DES-5 "
    );
}

#[test]
fn several_issues_need_a_title() {
    let one = ["ENG-7".to_owned()];
    let two = ["ENG-7".to_owned(), "DES-5".to_owned()];
    assert!(check_title(&one, None).is_ok());
    assert!(check_title(&one, Some("One")).is_ok());
    assert!(check_title(&two, Some("Both")).is_ok());
    let error = check_title(&two, None).expect_err("a title is required");
    assert_eq!(
        error.message(),
        "A title is required for a pull request covering several issues: ENG-7, DES-5"
    );
    assert_eq!(
        error.hint(),
        Some(
            "Pass --title \"<summary covering all issues>\". The title is used exactly as supplied."
        )
    );
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
        assert_eq!(
            read_template(&link).expect("linked template"),
            "Summary\r\n"
        );
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
    assert!(
        error.message().ends_with("is not valid UTF-8 text"),
        "{error}"
    );
    std::fs::write(&file, b"Summary\0").expect("write template");
    let error = read_template(&file).expect_err("binary");
    assert!(error.message().ends_with("is not a text file"), "{error}");
}
