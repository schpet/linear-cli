use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use linear_cli::config::{
    ConfigFailure, ConfigInputs, DiagnosticReason, FileKind, FileSource, GitProbeResult, OsFamily,
    ReadCandidate, RealFileSource, load_env, read_config_candidate,
};

use super::discover::Probe;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempTree(PathBuf);
impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "linear-cli-r02a1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, suffix: &str, bytes: &[u8]) {
        let path = self.0.join(suffix);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn inputs(&self) -> ConfigInputs {
        ConfigInputs {
            cwd: self.0.clone(),
            os: OsFamily::Unix,
            process_env: BTreeMap::new(),
        }
    }
}
impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn usable_unrelated_cwd_file_blocks_root_fallback() {
    let tree = TempTree::new();
    tree.write(".env", b"PATH=$PATH\n");
    tree.write("repo/.env", b"LINEAR_TEAM_ID=ROOT\n");
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: tree.0.join("repo").display().to_string(),
    });
    let selected = load_env(&tree.inputs(), &RealFileSource, &probe).unwrap();
    assert!(selected.applied.is_empty());
    assert_eq!(selected.source_path, Some(tree.0.join(".env")));
    assert_eq!(probe.count.get(), 0);
}

#[test]
fn selected_assignments_preserve_literal_quotes_and_suppress_skipped_duplicate() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        br#"LINEAR_A=$SELF
LINEAR_A=safe
LINEAR_B="hello\nworld"
LINEAR_C='$WORD'
LINEAR_D=
LINEAR_E=\$LITERAL
LINEAR_F=$SKIP
LINEAR_G='oops
LINEAR_H="a\qb"
LINEAR_I="a\"b"
LINEAR_J="x\\n"
LINEAR_K=\${FOO}
LINEAR_L="tab\tend"
LINEAR_M="cr\rend"
LINEAR_N='literal\n'
LINEAR_O=${}
LINEAR_P=${}}
"#,
    );
    let probe = Probe::new(GitProbeResult::SpawnFailure);
    let selected = load_env(&tree.inputs(), &RealFileSource, &probe).unwrap();
    assert_eq!(
        selected.applied.get("LINEAR_A").map(String::as_str),
        Some("safe")
    );
    assert_eq!(
        selected.applied.get("LINEAR_B").map(String::as_str),
        Some("hello\nworld")
    );
    assert_eq!(
        selected.applied.get("LINEAR_C").map(String::as_str),
        Some("$WORD")
    );
    assert_eq!(
        selected.applied.get("LINEAR_D").map(String::as_str),
        Some("")
    );
    assert_eq!(
        selected.applied.get("LINEAR_E").map(String::as_str),
        Some("\\$LITERAL")
    );
    assert_eq!(
        selected.applied.get("LINEAR_H").map(String::as_str),
        Some("a\\qb")
    );
    assert_eq!(
        selected.applied.get("LINEAR_I").map(String::as_str),
        Some("a\\")
    );
    assert_eq!(
        selected.applied.get("LINEAR_J").map(String::as_str),
        Some("x\\\n")
    );
    assert_eq!(
        selected.applied.get("LINEAR_L").map(String::as_str),
        Some("tab\tend")
    );
    assert_eq!(
        selected.applied.get("LINEAR_M").map(String::as_str),
        Some("cr\rend")
    );
    assert_eq!(
        selected.applied.get("LINEAR_N").map(String::as_str),
        Some("literal\\n")
    );
    assert_eq!(
        selected.applied.get("LINEAR_O").map(String::as_str),
        Some("${}")
    );
    assert_eq!(selected.diagnostics.len(), 2);
    assert_eq!(
        selected.diagnostics[0].reason,
        DiagnosticReason::SkippedExpansion(vec![
            "LINEAR_F".to_owned(),
            "LINEAR_K".to_owned(),
            "LINEAR_P".to_owned(),
        ])
    );
    assert_eq!(
        selected.diagnostics[1].reason,
        DiagnosticReason::UnterminatedQuote(vec!["LINEAR_G".to_owned()])
    );
}

#[test]
fn present_empty_process_env_shadows_dotenv() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_TEAM_ID=FROM_FILE\n");
    let mut inputs = tree.inputs();
    inputs
        .process_env
        .insert("LINEAR_TEAM_ID".to_owned(), String::new());
    let selected = load_env(
        &inputs,
        &RealFileSource,
        &Probe::new(GitProbeResult::SpawnFailure),
    )
    .unwrap();
    assert!(selected.applied.is_empty());
}

#[test]
fn missing_cwd_uses_root_only_on_successful_nonempty_probe() {
    let tree = TempTree::new();
    tree.write("repo/.env", b"LINEAR_TEAM_ID=ROOT\n");
    let root = tree.0.join("repo");
    let good = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: root.display().to_string(),
    });
    let selected = load_env(&tree.inputs(), &RealFileSource, &good).unwrap();
    assert_eq!(
        selected.applied.get("LINEAR_TEAM_ID").map(String::as_str),
        Some("ROOT")
    );
    assert_eq!(good.count.get(), 1);
    let bad = Probe::new(GitProbeResult::Completed {
        success: false,
        stdout: root.display().to_string(),
    });
    assert!(
        load_env(&tree.inputs(), &RealFileSource, &bad)
            .unwrap()
            .applied
            .is_empty()
    );
}

#[test]
fn ignored_file_does_not_probe_git() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_TEAM_ID=ignored\n");
    let mut inputs = tree.inputs();
    inputs
        .process_env
        .insert("LINEAR_IGNORE_ENV_FILE".to_owned(), "true".to_owned());
    let probe = Probe::new(GitProbeResult::SpawnFailure);
    assert!(
        load_env(&inputs, &RealFileSource, &probe)
            .unwrap()
            .applied
            .is_empty()
    );
    assert_eq!(probe.count.get(), 0);
    inputs
        .process_env
        .insert("LINEAR_IGNORE_ENV_FILE".to_owned(), "1".to_owned());
    assert!(
        load_env(&inputs, &RealFileSource, &probe)
            .unwrap()
            .applied
            .is_empty()
    );
    inputs
        .process_env
        .insert("LINEAR_IGNORE_ENV_FILE".to_owned(), "TRUE".to_owned());
    assert_eq!(
        load_env(&inputs, &RealFileSource, &probe)
            .unwrap()
            .applied
            .get("LINEAR_TEAM_ID")
            .map(String::as_str),
        Some("ignored")
    );
}

#[test]
fn export_prefix_comments_and_process_shadow_follow_selected_parser() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        b"export LINEAR_TEAM_ID =alpha # comment\nGH_TOKEN=example\nGITHUB_TOKEN=example2\nPATH=$PATH\nLINEAR_SKIP=$EXPAND\nLINEAR_TEAM_ID=last\n",
    );
    let mut inputs = tree.inputs();
    inputs
        .process_env
        .insert("LINEAR_SKIP".to_owned(), String::new());
    let selected = load_env(
        &inputs,
        &RealFileSource,
        &Probe::new(GitProbeResult::SpawnFailure),
    )
    .unwrap();
    assert_eq!(
        selected.applied.get("LINEAR_TEAM_ID").map(String::as_str),
        Some("last")
    );
    assert_eq!(
        selected.applied.get("GH_TOKEN").map(String::as_str),
        Some("example")
    );
    assert_eq!(
        selected.applied.get("GITHUB_TOKEN").map(String::as_str),
        Some("example2")
    );
    assert!(!selected.applied.contains_key("PATH"));
    assert!(!selected.applied.contains_key("LINEAR_SKIP"));
    assert!(selected.diagnostics.is_empty());
}

#[test]
fn empty_git_stdout_never_selects_root_dotenv() {
    let tree = TempTree::new();
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: "  \n".to_owned(),
    });
    let selected = load_env(&tree.inputs(), &RealFileSource, &probe).unwrap();
    assert_eq!(probe.count.get(), 1);
    assert_eq!(selected.source_path, None);
    assert!(selected.diagnostics.is_empty());
}

#[test]
fn strict_utf8_rejection_and_bom_source_shape() {
    let tree = TempTree::new();
    tree.write(".env", b"LINEAR_TEAM_ID=A\xffB\n");
    let result = load_env(
        &tree.inputs(),
        &RealFileSource,
        &Probe::new(GitProbeResult::SpawnFailure),
    );
    assert!(
        matches!(result, Err(error) if matches!(error.failure, ConfigFailure::InvalidUtf8 { .. }))
    );
    tree.write(".env", b"\xef\xbb\xbfLINEAR_TEAM_ID=BOM\n");
    let result = load_env(
        &tree.inputs(),
        &RealFileSource,
        &Probe::new(GitProbeResult::SpawnFailure),
    )
    .unwrap();
    assert!(result.applied.is_empty());
}

#[test]
fn bounded_reads_are_fatal_for_env_and_poison_config() {
    let tree = TempTree::new();
    let oversized = vec![b'x'; 1024 * 1024 + 1];
    tree.write(".env", &oversized);
    tree.write("linear.toml", &oversized);
    assert!(matches!(
        load_env(
            &tree.inputs(),
            &RealFileSource,
            &Probe::new(GitProbeResult::SpawnFailure)
        ),
        Err(error) if matches!(error.failure, ConfigFailure::Oversize { .. })
    ));
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("linear.toml")),
        ReadCandidate::TooLarge { .. }
    ));
    tree.write(".env", &vec![b'x'; 1024 * 1024]);
    assert!(
        load_env(
            &tree.inputs(),
            &RealFileSource,
            &Probe::new(GitProbeResult::SpawnFailure)
        )
        .is_ok()
    );
    tree.write("linear.toml", &vec![b'x'; 1024 * 1024]);
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("linear.toml")),
        ReadCandidate::Contents(_)
    ));
}

#[test]
fn oversized_root_dotenv_fails_without_applying_earlier_assignment() {
    let tree = TempTree::new();
    let mut bytes = b"LINEAR_TEAM_ID=part\n".to_vec();
    bytes.resize(1024 * 1024 + 1, b'x');
    tree.write("repo/.env", &bytes);
    let root = tree.0.join("repo");
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: root.display().to_string(),
    });
    assert!(matches!(
        load_env(&tree.inputs(), &RealFileSource, &probe),
        Err(error) if matches!(&error.failure, ConfigFailure::Oversize { path } if path == &root.join(".env"))
    ));
    assert_eq!(probe.count.get(), 1);
}

#[test]
fn oversized_cwd_file_does_not_probe_or_apply_valid_root_file() {
    let tree = TempTree::new();
    tree.write(".env", &vec![b'x'; 1024 * 1024 + 1]);
    tree.write("repo/.env", b"LINEAR_TEAM_ID=root\n");
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: tree.0.join("repo").display().to_string(),
    });
    assert!(matches!(
        load_env(&tree.inputs(), &RealFileSource, &probe),
        Err(error) if matches!(&error.failure, ConfigFailure::Oversize { path } if path == &tree.0.join(".env"))
    ));
    assert_eq!(probe.count.get(), 0);
}

#[test]
fn bare_cr_and_unicode_line_separators_do_not_become_assignments() {
    let tree = TempTree::new();
    tree.write(
        ".env",
        "LINEAR_CR=bad\rTRAIL\nLINEAR_LS=bad\u{2028}TRAIL\nLINEAR_PS=bad\u{2029}TRAIL\nLINEAR_OK=ok\n".as_bytes(),
    );
    let selected = load_env(
        &tree.inputs(),
        &RealFileSource,
        &Probe::new(GitProbeResult::SpawnFailure),
    )
    .unwrap();
    assert_eq!(selected.applied.len(), 1);
    assert_eq!(
        selected.applied.get("LINEAR_OK").map(String::as_str),
        Some("ok")
    );
}

#[test]
fn config_file_reader_distinguishes_missing_empty_and_directory() {
    let tree = TempTree::new();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("missing.toml")),
        ReadCandidate::Absent
    ));
    tree.write("empty.toml", b"");
    match read_config_candidate(&RealFileSource, &tree.0.join("empty.toml")) {
        ReadCandidate::Contents(file) => assert!(file.bytes.is_empty()),
        ReadCandidate::Absent | ReadCandidate::TooLarge { .. } | ReadCandidate::Poisoned { .. } => {
            panic!("empty file must be present")
        }
    }
    fs::create_dir(tree.0.join("directory.toml")).unwrap();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.0.join("directory.toml")),
        ReadCandidate::Poisoned { .. }
    ));
}

#[test]
fn unusable_cwd_warning_precedes_root_warning_and_no_duplicate_root_read() {
    let tree = TempTree::new();
    fs::create_dir(tree.0.join(".env")).unwrap();
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: tree.0.display().to_string(),
    });
    let selected = load_env(&tree.inputs(), &RealFileSource, &probe).unwrap();
    assert_eq!(probe.count.get(), 1);
    assert_eq!(selected.diagnostics.len(), 1);
    assert_eq!(selected.diagnostics[0].path, tree.0.join(".env"));
}

#[test]
fn unusable_cwd_and_distinct_root_warn_in_candidate_order() {
    let tree = TempTree::new();
    fs::create_dir(tree.0.join(".env")).unwrap();
    fs::create_dir_all(tree.0.join("repo/.env")).unwrap();
    let root = tree.0.join("repo");
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: root.display().to_string(),
    });
    let selected = load_env(&tree.inputs(), &RealFileSource, &probe).unwrap();
    assert_eq!(selected.diagnostics.len(), 2);
    assert_eq!(selected.diagnostics[0].path, tree.0.join(".env"));
    assert_eq!(selected.diagnostics[1].path, root.join(".env"));
}

#[test]
fn nonregular_metadata_does_not_block() {
    struct Fake;
    impl FileSource for Fake {
        fn kind(&self, _path: &std::path::Path) -> std::io::Result<Option<FileKind>> {
            Ok(Some(FileKind::Other))
        }
        fn read_bounded(&self, _path: &std::path::Path, _max: u64) -> std::io::Result<Vec<u8>> {
            panic!("nonregular file must not be read");
        }
    }
    let tree = TempTree::new();
    let selected = load_env(
        &tree.inputs(),
        &Fake,
        &Probe::new(GitProbeResult::SpawnFailure),
    )
    .unwrap();
    assert_eq!(selected.diagnostics.len(), 1);
    assert!(selected.applied.is_empty());
}
