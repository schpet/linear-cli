#![cfg(unix)]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use linear_cli::config::{
    ConfigDiagnostic, DiagnosticReason, FileKind, FileSource, IssueSort, NoColor, OptionSource,
    OsFamily, ProcessEnvSnapshot, load_startup, render_diagnostic,
};
use linear_cli::error::AppErrorKind;

enum Entry {
    Bytes(Vec<u8>),
    Directory,
    ReadError(io::ErrorKind),
}

#[derive(Default)]
struct MemFiles(BTreeMap<PathBuf, Entry>);

struct CountFiles {
    inner: MemFiles,
    reads: Cell<usize>,
}

impl FileSource for CountFiles {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
        self.inner.kind(path)
    }

    fn read_bounded(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read_bounded(path, max_bytes)
    }
}

impl MemFiles {
    fn file(mut self, path: &str, bytes: &[u8]) -> Self {
        self.0
            .insert(PathBuf::from(path), Entry::Bytes(bytes.to_vec()));
        self
    }

    fn directory(mut self, path: &str) -> Self {
        self.0.insert(PathBuf::from(path), Entry::Directory);
        self
    }

    fn unreadable(mut self, path: &str) -> Self {
        self.0.insert(
            PathBuf::from(path),
            Entry::ReadError(io::ErrorKind::PermissionDenied),
        );
        self
    }
}

impl FileSource for MemFiles {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
        Ok(match self.0.get(path) {
            Some(Entry::Bytes(_) | Entry::ReadError(_)) => Some(FileKind::Regular),
            Some(Entry::Directory) => Some(FileKind::Directory),
            None => None,
        })
    }

    fn read_bounded(&self, path: &Path, _max_bytes: u64) -> io::Result<Vec<u8>> {
        match self.0.get(path) {
            Some(Entry::Bytes(bytes)) => Ok(bytes.clone()),
            Some(Entry::ReadError(kind)) => Err(io::Error::from(*kind)),
            Some(Entry::Directory) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }
}

fn process(values: &[(&str, &str)]) -> ProcessEnvSnapshot {
    process_in("/work", values)
}

fn process_in(cwd: &str, values: &[(&str, &str)]) -> ProcessEnvSnapshot {
    ProcessEnvSnapshot::from_vars_os(
        PathBuf::from(cwd),
        OsFamily::Unix,
        values
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    )
    .unwrap()
}

#[test]
fn source_precedence_and_redacted_overlay_are_typed() {
    let process = process(&[
        ("XDG_CONFIG_HOME", "/global"),
        ("LINEAR_ISSUE_SORT", "manual"),
        ("NO_COLOR", ""),
    ]);
    let files = MemFiles::default()
        .file(
            "/work/.env",
            b"LINEAR_DEBUG=1\nLINEAR_API_KEY=lin_api_fake_b3\n",
        )
        .file("/work/linear.toml", b"issue_sort = 'priority'\n")
        .file("/global/linear/linear.toml", b"issue_sort = 'manual'\n");
    let report = load_startup(&process, &files);
    assert_eq!(report.settings.no_color, NoColor::Empty);
    assert!(report.settings.debug);
    let ready = report.result.as_ref().unwrap();
    assert_eq!(ready.options.issue_sort(None).0, IssueSort::Manual);
    assert_eq!(
        ready.options.sourced_issue_sort().unwrap().source(),
        &OptionSource::Env
    );
    assert_eq!(ready.child_env.get("LINEAR_DEBUG"), Some("1"));
    assert_eq!(
        ready.child_env.get("LINEAR_API_KEY"),
        Some("lin_api_fake_b3")
    );
    assert_eq!(ready.transport_env.production().ca_bundle, None);
    assert!(!format!("{report:?}").contains("lin_api_fake_b3"));
}

#[test]
fn pager_is_captured_only_from_process_environment() {
    let files = MemFiles::default().file(
        "/work/.env",
        b"PAGER=dotenv-pager\nNO_COLOR=1\nLINEAR_DEBUG=1\n",
    );
    let absent = load_startup(&process(&[]), &files);
    assert_eq!(absent.result.unwrap().pager, None);
    assert_eq!(absent.settings.no_color, NoColor::Absent);

    for value in ["", "  ", "less -R"] {
        let report = load_startup(&process(&[("PAGER", value)]), &files);
        let ready = report.result.unwrap();
        assert_eq!(ready.pager.as_deref(), Some(std::ffi::OsStr::new(value)));
        assert_eq!(ready.child_env.get("PAGER"), None);
    }

    use std::os::unix::ffi::OsStringExt;
    let invalid = OsString::from_vec(vec![0xff]);
    let process = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        [(OsString::from("PAGER"), invalid.clone())],
    )
    .unwrap();
    let ready = load_startup(&process, &files).result.unwrap();
    assert_eq!(ready.pager, Some(invalid));
}

#[test]
fn windows_child_overlay_lookup_is_case_insensitive() {
    let process = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Windows,
        std::iter::empty::<(OsString, OsString)>(),
    )
    .unwrap();
    let files = MemFiles::default().file("/work/.env", b"LINEAR_API_KEY=lin_api_fake_b3\n");
    let ready = load_startup(&process, &files).result.unwrap();
    assert_eq!(
        ready.child_env.get("linear_api_key"),
        Some("lin_api_fake_b3")
    );
}

#[test]
fn lower_tier_poison_is_fatal_even_when_env_is_valid() {
    let process = process(&[("LINEAR_ISSUE_SORT", "manual")]);
    let files = MemFiles::default().file("/work/linear.toml", b"issue_sort = 12\n");
    let report = load_startup(&process, &files);
    let error = report.result.unwrap_err().app_error();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(
        error.display_message(),
        "invalid config option issue_sort from project config /work/linear.toml: invalid type: integer `12`, expected manual or priority"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Fix issue_sort in project config /work/linear.toml.")
    );
}

#[test]
fn malformed_first_candidate_does_not_fall_through() {
    let files = MemFiles::default()
        .file("/work/linear.toml", b"issue_sort = [\n")
        .file("/work/.linear.toml", b"issue_sort = 'priority'\n");
    let report = load_startup(&process(&[]), &files);
    let message = report.result.unwrap_err().app_error().display_message();
    assert!(
        message.starts_with(
            "invalid config file /work/linear.toml: invalid TOML at line 1, column 15: "
        ),
        "{message}"
    );
}

#[test]
fn selected_config_is_read_once() {
    let files = CountFiles {
        inner: MemFiles::default().file("/work/linear.toml", b"issue_sort = 'priority'\n"),
        reads: Cell::new(0),
    };
    assert!(load_startup(&process(&[]), &files).result.is_ok());
    assert_eq!(files.reads.get(), 1);
}

#[test]
fn global_then_project_then_options_then_endpoint_errors_are_reported_first() {
    let files = MemFiles::default()
        .file("/work/.env", b"LINEAR_VCS=bad\n")
        .file("/global/linear/linear.toml", b"vcs = [\n")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[("XDG_CONFIG_HOME", "/global")]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .app_error()
            .display_message()
            .contains("/global/linear/linear.toml")
    );

    let files = MemFiles::default()
        .file("/work/.env", b"LINEAR_VCS=bad\n")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .app_error()
            .display_message()
            .contains("/work/linear.toml")
    );

    let files = MemFiles::default().file("/work/.env", b"LINEAR_VCS=bad\n");
    let report = load_startup(&process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .app_error()
            .display_message()
            .contains("LINEAR_VCS")
    );
}

#[test]
fn dotenv_warning_survives_a_later_config_failure() {
    let files = MemFiles::default()
        .directory("/work/.env")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[]), &files);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].path, PathBuf::from("/work/.env"));
    assert!(report.result.is_err());
}

#[test]
fn repo_root_supplies_dotenv_and_config() {
    let files = MemFiles::default()
        .directory("/repo/.git")
        .file("/repo/.env", b"LINEAR_TEAM_ID=ROOT\n")
        .file("/repo/.config/linear.toml", b"vcs = 'jj'\n");
    let ready = load_startup(&process_in("/repo/sub", &[]), &files)
        .result
        .unwrap();
    assert_eq!(ready.options.team_id().unwrap().value(), "ROOT");
    assert_eq!(
        ready.options.vcs().unwrap().source(),
        &OptionSource::ProjectConfig {
            path: PathBuf::from("/repo/.config/linear.toml")
        }
    );
}

#[test]
fn outside_a_repository_cwd_dot_config_is_not_a_candidate() {
    let files = MemFiles::default().file("/work/.config/linear.toml", b"vcs = 'jj'\n");
    let ready = load_startup(&process(&[]), &files).result.unwrap();
    assert!(ready.options.vcs().is_none());
}

#[test]
fn poisoned_candidate_and_endpoint_error_are_explicit() {
    let report = load_startup(
        &process(&[]),
        &MemFiles::default().unreadable("/work/linear.toml"),
    );
    assert_eq!(
        report.result.unwrap_err().app_error().display_message(),
        "cannot read config file /work/linear.toml: permission denied"
    );
    let report = load_startup(
        &process(&[]),
        &MemFiles::default().file("/work/linear.toml", &vec![b'x'; 1024 * 1024 + 1]),
    );
    assert_eq!(
        report.result.unwrap_err().app_error().display_message(),
        "invalid config file /work/linear.toml: too large"
    );
    let report = load_startup(
        &process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]),
        &MemFiles::default(),
    );
    let error = report.result.unwrap_err().app_error();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(
        error.display_message(),
        "invalid LINEAR_GRAPHQL_ENDPOINT from process environment: expected an http(s) URL without credentials or fragment"
    );
}

#[test]
fn warning_templates_have_exact_color_bytes() {
    let diagnostic = ConfigDiagnostic {
        path: PathBuf::from("/work/.env"),
        reason: DiagnosticReason::InvalidLines(vec!["LINEAR_TEAM_ID".to_owned()]),
    };
    let body = "Warning: Ignoring LINEAR_TEAM_ID in /work/.env: the line could not be parsed.";
    let suggestion = "  Check for an unclosed quote or a malformed KEY=value line.";
    assert_eq!(
        render_diagnostic(&diagnostic, true),
        format!("\x1b[33m{body}\x1b[39m\n\x1b[90m{suggestion}\x1b[39m\n")
    );
    assert_eq!(
        render_diagnostic(&diagnostic, false),
        format!("{body}\n{suggestion}\n")
    );
}

static NEXT_BINARY: AtomicU64 = AtomicU64::new(0);

struct BinaryTree {
    root: PathBuf,
}

impl BinaryTree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "linear-config-binary-{}-{}",
            std::process::id(),
            NEXT_BINARY.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["cwd", "home", "bin", "repo"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        Self {
            root: fs::canonicalize(root).expect("canonical private config sandbox"),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn file(&self, relative: &str, contents: &[u8]) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .env_clear()
            .current_dir(self.path("cwd"))
            .env("HOME", self.path("home"))
            .env("XDG_CONFIG_HOME", self.path("home"))
            .env("APPDATA", self.path("home"))
            .env("PATH", self.path("bin"))
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC");
        command
    }
}

impl Drop for BinaryTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn binary_config_validation_precedes_parser_usage() {
    let tree = BinaryTree::new();
    tree.file("cwd/linear.toml", b"issue_sort = 'alphabetical'\n");
    let output = tree
        .command()
        .arg("frobnicate")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        format!(
            "✗ invalid config option issue_sort from project config {}: unknown variant `alphabetical`, expected `manual` or `priority`\n  Fix issue_sort in project config {}.\n",
            tree.path("cwd/linear.toml").display(),
            tree.path("cwd/linear.toml").display()
        ).as_bytes()
    );
}

#[test]
fn binary_warning_color_and_offline_version_are_exact() {
    let tree = BinaryTree::new();
    tree.file("cwd/.env", b"LINEAR_TEAM_ID='unterminated\n");
    let diagnostic = ConfigDiagnostic {
        path: tree.path("cwd/.env"),
        reason: DiagnosticReason::InvalidLines(vec!["LINEAR_TEAM_ID".to_owned()]),
    };
    for (value, color) in [(None, true), (Some(""), true), (Some("1"), false)] {
        let mut command = tree.command();
        command.arg("-V");
        if let Some(value) = value {
            command.env("NO_COLOR", value);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"linear 3.0.0-alpha.1\n");
        assert_eq!(
            output.stderr,
            render_diagnostic(&diagnostic, color).as_bytes()
        );
    }
}

#[test]
fn binary_reads_config_from_the_repository_root() {
    let tree = BinaryTree::new();
    tree.file("cwd/.jj/repo", b"");
    tree.file("cwd/linear.toml", b"vcs = 'invalid'\n");
    fs::create_dir_all(tree.path("cwd/sub")).unwrap();
    let output = tree
        .command()
        .current_dir(tree.path("cwd/sub"))
        .arg("-V")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, format!(
        "✗ invalid config option vcs from project config {}: unknown variant `invalid`, expected `git` or `jj`\n  Fix vcs in project config {}.\n",
        tree.path("cwd/linear.toml").display(),
        tree.path("cwd/linear.toml").display(),
    ).as_bytes());
}

#[test]
fn binary_endpoint_error_precedes_help() {
    let tree = BinaryTree::new();
    let output = tree
        .command()
        .arg("--help")
        .env("NO_COLOR", "1")
        .env("LINEAR_GRAPHQL_ENDPOINT", "bad")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, "✗ invalid LINEAR_GRAPHQL_ENDPOINT from process environment: expected an http(s) URL without credentials or fragment\n  Set a valid LINEAR_GRAPHQL_ENDPOINT or remove it.\n".as_bytes());
}

#[test]
fn binary_offline_markdown_needs_no_credential() {
    let tree = BinaryTree::new();
    let output = tree
        .command()
        .arg("markdown")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("+++ [Server log]")
    );
}

#[test]
fn binary_offline_markdown_ignores_transport_settings() {
    let tree = BinaryTree::new();
    let baseline = tree.command().arg("markdown").output().expect("binary");
    assert_eq!(baseline.status.code(), Some(0));
    for (name, value) in [
        ("HTTPS_PROXY", "http://proxy.example.invalid:3128"),
        ("SSL_CERT_FILE", "/missing/sentinel-ca.pem"),
        ("DENO_CERT", "/missing/sentinel-ca.pem"),
    ] {
        let output = tree
            .command()
            .arg("markdown")
            .env(name, value)
            .output()
            .expect("binary");
        assert_eq!(output.status.code(), Some(0), "{name}");
        assert_eq!(output.stdout, baseline.stdout, "{name}");
        assert!(output.stderr.is_empty(), "{name}");
    }
}

#[test]
fn ci_comes_only_from_the_process_environment() {
    let files = MemFiles::default().file("/work/.env", b"CI=true\n");
    let from_file = load_startup(&process(&[]), &files).result.unwrap();
    assert_eq!(from_file.ci, None);
    assert_eq!(from_file.child_env.get("CI"), None);

    let from_process = load_startup(&process(&[("CI", "")]), &files)
        .result
        .unwrap();
    assert_eq!(from_process.ci.as_deref(), Some(""));
    assert_eq!(from_process.child_env.get("CI"), None);
}
