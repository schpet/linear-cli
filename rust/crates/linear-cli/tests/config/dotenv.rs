use std::fs;
use std::path::Path;

use linear_cli::config::{
    ConfigFailure, DiagnosticReason, FileKind, FileSource, ReadCandidate, RealFileSource,
    SelectedEnv, load_env, read_config_candidate,
};

use super::TempTree;

fn load(tree: &TempTree, root: Option<&Path>) -> SelectedEnv {
    load_env(&tree.inputs(), &RealFileSource, root).unwrap()
}

fn applied<'a>(selected: &'a SelectedEnv, key: &str) -> Option<&'a str> {
    selected.applied.get(key).map(String::as_str)
}

#[test]
fn values_follow_common_dotenv_quoting() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        br#"# a comment
LINEAR_A=first
LINEAR_A=last
LINEAR_B="hello\nworld"
LINEAR_C='$WORD'
LINEAR_D=
LINEAR_E=plain value # trailing comment
LINEAR_F=$SKIP
LINEAR_G='oops
LINEAR_H="a\"b"
LINEAR_I="x\\n"
LINEAR_J=${FOO}
LINEAR_K="tab\tend"
LINEAR_L='literal\n'
LINEAR_M="cost \$5"
LINEAR_N="$HOME/x"
LINEAR_O=a#b
"#,
    );
    let selected = load(&tree, None);
    assert_eq!(applied(&selected, "LINEAR_A"), Some("last"));
    assert_eq!(applied(&selected, "LINEAR_B"), Some("hello\nworld"));
    assert_eq!(applied(&selected, "LINEAR_C"), Some("$WORD"));
    assert_eq!(applied(&selected, "LINEAR_D"), Some(""));
    assert_eq!(applied(&selected, "LINEAR_E"), Some("plain value"));
    assert_eq!(applied(&selected, "LINEAR_H"), Some("a\"b"));
    assert_eq!(applied(&selected, "LINEAR_I"), Some("x\\n"));
    assert_eq!(applied(&selected, "LINEAR_K"), Some("tab\tend"));
    assert_eq!(applied(&selected, "LINEAR_L"), Some("literal\\n"));
    assert_eq!(applied(&selected, "LINEAR_M"), Some("cost $5"));
    assert_eq!(applied(&selected, "LINEAR_O"), Some("a#b"));
    assert_eq!(
        selected
            .diagnostics
            .iter()
            .map(|d| &d.reason)
            .collect::<Vec<_>>(),
        [
            &DiagnosticReason::SkippedExpansion(vec![
                "LINEAR_F".to_owned(),
                "LINEAR_J".to_owned(),
                "LINEAR_N".to_owned(),
            ]),
            &DiagnosticReason::UnterminatedQuote(vec!["LINEAR_G".to_owned()]),
        ]
    );
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
    tree.write(".env", b"LINEAR_TEAM_ID=FROM_FILE\nLINEAR_SKIP=$EXPAND\n");
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
    let root = tree.0.join("repo");
    let selected = load(&tree, Some(&root));
    assert_eq!(applied(&selected, "LINEAR_TEAM_ID"), Some("ROOT"));
    assert_eq!(selected.source_path, Some(root.join(".env")));

    tree.write(".env", b"OTHER=1\n");
    let selected = load(&tree, Some(&root));
    assert!(selected.applied.is_empty());
    assert_eq!(selected.source_path, Some(tree.0.join(".env")));

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
        load_env(&tree.inputs(), &RealFileSource, Some(&tree.0.join("repo"))),
        Err(error) if matches!(&error.failure, ConfigFailure::Oversize { path } if path == &tree.0.join(".env"))
    ));
    tree.write(".env", &vec![b'x'; 1024 * 1024]);
    assert!(load_env(&tree.inputs(), &RealFileSource, None).is_ok());
}

#[test]
fn unusable_files_warn_in_lookup_order() {
    let tree = TempTree::new();
    tree.mkdir(".env");
    tree.mkdir("repo/.env");
    let root = tree.0.join("repo");
    let selected = load(&tree, Some(&root));
    assert_eq!(selected.diagnostics.len(), 2);
    assert_eq!(selected.diagnostics[0].path, tree.0.join(".env"));
    assert_eq!(selected.diagnostics[1].path, root.join(".env"));

    let selected = load(&tree, Some(&tree.0));
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

#[test]
fn config_file_reader_distinguishes_missing_empty_large_and_directory() {
    let tree = TempTree::new();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("missing.toml")),
        ReadCandidate::Absent
    ));
    tree.write("empty.toml", b"");
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("empty.toml")),
        ReadCandidate::Contents(file) if file.bytes.is_empty()
    ));
    tree.write("large.toml", &vec![b'x'; 1024 * 1024 + 1]);
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("large.toml")),
        ReadCandidate::TooLarge { .. }
    ));
    fs::create_dir(tree.0.join("directory.toml")).unwrap();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("directory.toml")),
        ReadCandidate::Poisoned { .. }
    ));
}
