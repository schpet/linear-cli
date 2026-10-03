use std::path::Path;

use super::*;
use crate::config::test_support::TempTree;
use crate::config::{FileKind, FileSource, RealFileSource};

fn load(tree: &TempTree, root: Option<&Path>) -> SelectedEnv {
    load_env(&tree.inputs(), &RealFileSource, root).unwrap()
}

fn applied<'a>(selected: &'a SelectedEnv, key: &str) -> Option<&'a str> {
    selected.applied.get(key).map(String::as_str)
}

fn reasons(selected: &SelectedEnv) -> Vec<&DiagnosticReason> {
    selected.diagnostics.iter().map(|d| &d.reason).collect()
}

fn invalid(keys: &[&str]) -> DiagnosticReason {
    DiagnosticReason::InvalidLines(keys.iter().map(|key| (*key).to_owned()).collect())
}

fn unexpanded(key: &str, reference: &str) -> DiagnosticReason {
    DiagnosticReason::Unexpanded {
        key: key.to_owned(),
        reference: reference.to_owned(),
    }
}

#[test]
fn values_follow_dotenv_quoting() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        br#"# a comment
LINEAR_A=first
LINEAR_A=last
LINEAR_B="hello\nworld"
LINEAR_C='$WORD'
LINEAR_D=
LINEAR_E=plain # trailing comment
LINEAR_H="a\"b"
LINEAR_I="x\\n"
LINEAR_L='literal\n'
LINEAR_M="cost \$5"
LINEAR_O=a#b
LINEAR_P=cost $5
LINEAR_Q="quoted" # comment
LINEAR_S=it\'s
"#,
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("last"));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("hello\nworld"));
    assert_eq!(applied(&selected, "LINEAR_C"), Some("$WORD"));
    assert_eq!(applied(&selected, "LINEAR_D"), Some(""));
    assert_eq!(applied(&selected, "LINEAR_E"), Some("plain"));
    assert_eq!(applied(&selected, "LINEAR_H"), Some("a\"b"));
    assert_eq!(applied(&selected, "LINEAR_I"), Some("x\\n"));
    assert_eq!(applied(&selected, "LINEAR_L"), Some("literal\\n"));
    assert_eq!(applied(&selected, "LINEAR_M"), Some("cost $5"));
    assert_eq!(applied(&selected, "LINEAR_O"), Some("a#b"));
    assert_eq!(applied(&selected, "LINEAR_P"), Some("cost $5"));
    assert_eq!(applied(&selected, "LINEAR_Q"), Some("quoted"));
    assert_eq!(applied(&selected, "LINEAR_S"), Some("it's"));
    assert!(selected.diagnostics.is_empty());
}

#[test]
fn variable_references_are_never_expanded() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        br#"LINEAR_API_KEY=$SECRET_KEY
LINEAR_TEAM_ID="${SECRET_KEY}/x"
LINEAR_WORKSPACE=lin_api_ab$cd
LINEAR_ISSUE_SORT='${SECRET_KEY}'
"#,
    );
    let mut inputs = tree.inputs();
    inputs
        .process_env
        .insert("SECRET_KEY".to_owned(), "lin_api_real".to_owned());
    let selected = load_env(&inputs, &RealFileSource, None).unwrap();
    assert_eq!(applied(&selected, "LINEAR_API_KEY"), None);
    assert_eq!(applied(&selected, "LINEAR_TEAM_ID"), None);
    assert_eq!(applied(&selected, "LINEAR_WORKSPACE"), None);
    assert_eq!(
        applied(&selected, "LINEAR_ISSUE_SORT"),
        Some("${SECRET_KEY}")
    );
    assert_eq!(
        reasons(&selected),
        [
            &unexpanded("LINEAR_API_KEY", "$SECRET_KEY"),
            &unexpanded("LINEAR_TEAM_ID", "${SECRET_KEY}"),
            &unexpanded("LINEAR_WORKSPACE", "$cd"),
        ]
    );
}

#[test]
fn trailing_whitespace_is_trimmed_before_escapes_are_decoded() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"LINEAR_A=abc\\ \nLINEAR_B=abc\\n\nLINEAR_C=abc  \t# comment\nLINEAR_D=a b\\ \\#c\n",
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("abc "));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("abc\n"));
    assert_eq!(applied(&selected, "LINEAR_C"), Some("abc"));
    assert_eq!(applied(&selected, "LINEAR_D"), Some("a b #c"));
}

#[test]
fn the_last_entry_for_a_key_wins_even_when_it_is_invalid() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"LINEAR_A=old\nLINEAR_A=$BAD\nLINEAR_B=$BAD\nLINEAR_B=good\nLINEAR_C=old\nLINEAR_C='open\n",
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), None);
    assert_eq!(applied(&selected, "LINEAR_B"), Some("good"));
    assert_eq!(applied(&selected, "LINEAR_C"), None);
    assert_eq!(
        reasons(&selected),
        [&invalid(&["LINEAR_C"]), &unexpanded("LINEAR_A", "$BAD")]
    );
}

#[test]
fn malformed_lines_for_read_keys_warn_and_export_takes_any_whitespace() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"export\tLINEAR_A=tab\nLINEAR_B\nexport LINEAR_C\nLINEAR_D=bad\\q\nLINEAR_E=it's\nLINEAR_F=\"a\"b\nOTHER\nexportLINEAR_G=1\n",
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("tab"));
    assert_eq!(selected.applied.len(), 1, "{:?}", selected.applied);
    assert_eq!(
        reasons(&selected),
        [&invalid(&[
            "LINEAR_B", "LINEAR_C", "LINEAR_D", "LINEAR_E", "LINEAR_F"
        ])]
    );
}

#[test]
fn an_unclosed_quote_invalidates_only_its_own_line() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"OTHER='unclosed\nLINEAR_A=one\nLINEAR_G=\"oops\nLINEAR_B=two\nOTHER2=\"x\nLINEAR_C=\"three\"\n",
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("one"));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("two"));
    assert_eq!(applied(&selected, "LINEAR_C"), Some("three"));
    assert_eq!(reasons(&selected), [&invalid(&["LINEAR_G"])]);
}

#[test]
fn quoted_values_may_span_lines() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_A=\"one\ntwo\"\nLINEAR_B=after\n");
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("one\ntwo"));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("after"));
}

#[test]
fn only_linear_and_github_keys_are_read_and_export_is_allowed() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"export LINEAR_TEAM_ID = alpha\nGH_TOKEN=example\nGITHUB_TOKEN=example2\nPATH=/bin\nCI=true\n",
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_TEAM_ID"), Some("alpha"));
    assert_eq!(applied(&selected, "GH_TOKEN"), Some("example"));
    assert_eq!(applied(&selected, "GITHUB_TOKEN"), Some("example2"));
    assert!(!selected.applied.contains_key("PATH"));
    assert!(!selected.applied.contains_key("CI"));
}

#[test]
fn process_environment_wins_even_when_empty() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"LINEAR_TEAM_ID=FROM_FILE\nLINEAR_SKIP='unterminated\n",
    );
    let mut inputs = tree.inputs();
    inputs
        .process_env
        .insert("LINEAR_TEAM_ID".to_owned(), String::new());
    inputs
        .process_env
        .insert("LINEAR_SKIP".to_owned(), "set".to_owned());
    let selected = load_env(&inputs, &RealFileSource, None).unwrap();
    assert!(selected.applied.is_empty());
    assert!(selected.diagnostics.is_empty(), "shadowed keys never warn");
}

#[test]
fn windows_line_endings_and_a_byte_order_mark_are_accepted() {
    let tree = TempTree::new();
    tree.write(".env", b"\xef\xbb\xbfLINEAR_A=one\r\nLINEAR_B=two\r\n");
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("one"));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("two"));
}

#[test]
fn cwd_file_wins_over_the_repo_root_file() {
    let tree = TempTree::new();
    tree.write("repo/.env", b"LINEAR_TEAM_ID=ROOT\n");
    let root = tree.join("repo");
    let selected = load(&tree, Some(&root));
    assert_eq!(applied(&selected, "LINEAR_TEAM_ID"), Some("ROOT"));
    assert_eq!(selected.source_path, Some(root.join(".env")));

    tree.write(".env", b"OTHER=1\n");
    let selected = load(&tree, Some(&root));
    assert!(selected.applied.is_empty());
    assert_eq!(selected.source_path, Some(tree.join(".env")));

    assert_eq!(load(&TempTree::new(), None).source_path, None);
}

#[test]
fn ignore_env_file_accepts_one_and_true() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_TEAM_ID=value\n");
    let mut inputs = tree.inputs();
    for (flag, ignored) in [("1", true), ("true", true), ("TRUE", false)] {
        inputs
            .process_env
            .insert("LINEAR_IGNORE_ENV_FILE".to_owned(), flag.to_owned());
        let selected = load_env(&inputs, &RealFileSource, None).unwrap();
        assert_eq!(selected.applied.is_empty(), ignored, "{flag}");
    }
}

#[test]
fn invalid_utf8_and_oversized_files_are_fatal() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_TEAM_ID=A\xffB\n");
    assert!(matches!(
        load_env(&tree.inputs(), &RealFileSource, None),
        Err(error) if matches!(error.failure, ConfigFailure::InvalidUtf8 { .. })
    ));
    tree.write(".env", &vec![b'x'; 1024 * 1024 + 1]);
    tree.write("repo/.env", b"LINEAR_TEAM_ID=root\n");
    assert!(matches!(
        load_env(&tree.inputs(), &RealFileSource, Some(&tree.join("repo"))),
        Err(error) if matches!(&error.failure, ConfigFailure::Oversize { path } if path == &tree.join(".env"))
    ));
    tree.write(".env", &vec![b'x'; 1024 * 1024]);
    assert!(load_env(&tree.inputs(), &RealFileSource, None).is_ok());
}

#[test]
fn unusable_files_warn_in_lookup_order() {
    let tree = TempTree::new();
    tree.mkdir(".env");
    tree.mkdir("repo/.env");
    let root = tree.join("repo");
    let selected = load(&tree, Some(&root));
    assert_eq!(selected.diagnostics.len(), 2);
    assert_eq!(selected.diagnostics[0].path, tree.join(".env"));
    assert_eq!(selected.diagnostics[1].path, root.join(".env"));

    let selected = load(&tree, Some(tree.path()));
    assert_eq!(selected.diagnostics.len(), 1, "the same file is read once");
}

#[test]
fn nonregular_files_are_never_read() {
    struct Fake;
    impl FileSource for Fake {
        fn kind(&self, _path: &Path) -> std::io::Result<Option<FileKind>> {
            Ok(Some(FileKind::Other))
        }
        fn read_bounded(&self, _path: &Path, _max: u64) -> std::io::Result<Vec<u8>> {
            panic!("nonregular file must not be read");
        }
    }
    let tree = TempTree::new();
    let selected = load_env(&tree.inputs(), &Fake, None).unwrap();
    assert_eq!(selected.diagnostics.len(), 1);
    assert!(selected.applied.is_empty());
}
