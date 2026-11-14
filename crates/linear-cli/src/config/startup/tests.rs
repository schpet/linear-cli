use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::*;
use crate::config::{
    ConfigDiagnostic, DiagnosticReason, FileKind, FileSource, IssueSort, OptionSource, OsFamily,
    ProcessEnvSnapshot, fixture_path,
};

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
        self.0.insert(
            PathBuf::from(fixture_path(path)),
            Entry::Bytes(bytes.to_vec()),
        );
        self
    }

    fn directory(mut self, path: &str) -> Self {
        self.0
            .insert(PathBuf::from(fixture_path(path)), Entry::Directory);
        self
    }

    fn unreadable(mut self, path: &str) -> Self {
        self.0.insert(
            PathBuf::from(fixture_path(path)),
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
        PathBuf::from(fixture_path(cwd)),
        OsFamily::Unix,
        values
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    )
    .unwrap()
}

#[test]
fn env_beats_files_and_debug_output_hides_the_key() {
    let global = fixture_path("/global");
    let process = process(&[
        ("XDG_CONFIG_HOME", global.as_str()),
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
    assert!(!report.settings.no_color);
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
    assert_eq!(ready.network_env.client_config().ca_bundle, None);
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
    assert!(!absent.settings.no_color);

    for value in ["", "  ", "less -R"] {
        let report = load_startup(&process(&[("PAGER", value)]), &files);
        let ready = report.result.unwrap();
        assert_eq!(ready.pager.as_deref(), Some(std::ffi::OsStr::new(value)));
        assert_eq!(ready.child_env.get("PAGER"), None);
    }
}

#[cfg(unix)]
#[test]
fn a_pager_that_is_not_utf8_is_kept_as_is() {
    use std::os::unix::ffi::OsStringExt;
    let invalid = OsString::from_vec(vec![0xff]);
    let process = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        [(OsString::from("PAGER"), invalid.clone())],
    )
    .unwrap();
    let ready = load_startup(&process, &MemFiles::default()).result.unwrap();
    assert_eq!(ready.pager, Some(invalid));
}

#[test]
fn windows_child_overlay_lookup_is_case_insensitive() {
    let process = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from(fixture_path("/work")),
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
    let error = report.result.unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "invalid config option issue_sort from project config {}: invalid type: integer `12`, expected manual or priority",
            fixture_path("/work/linear.toml")
        )
    );
    let hint = format!(
        "Fix issue_sort in project config {}.",
        fixture_path("/work/linear.toml")
    );
    assert_eq!(error.hint(), Some(hint.as_str()));
}

#[test]
fn malformed_first_candidate_does_not_fall_through() {
    let files = MemFiles::default()
        .file("/work/linear.toml", b"issue_sort = [\n")
        .file("/work/.linear.toml", b"issue_sort = 'priority'\n");
    let report = load_startup(&process(&[]), &files);
    let message = report.result.unwrap_err().to_string();
    assert!(
        message.starts_with(&format!(
            "invalid config file {}: invalid TOML at line 1, column 15: ",
            fixture_path("/work/linear.toml")
        )),
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
    let global = fixture_path("/global");
    let files = MemFiles::default()
        .file("/work/.env", b"LINEAR_VCS=bad\n")
        .file("/global/linear/linear.toml", b"vcs = [\n")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[("XDG_CONFIG_HOME", global.as_str())]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .to_string()
            .contains(&fixture_path("/global/linear/linear.toml"))
    );

    let files = MemFiles::default()
        .file("/work/.env", b"LINEAR_VCS=bad\n")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .to_string()
            .contains(&fixture_path("/work/linear.toml"))
    );

    let files = MemFiles::default().file("/work/.env", b"LINEAR_VCS=bad\n");
    let report = load_startup(&process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .to_string()
            .contains("LINEAR_VCS")
    );
}

#[test]
fn dotenv_warning_survives_a_later_config_failure() {
    let files = MemFiles::default()
        .directory("/work/.env")
        .file("/work/linear.toml", b"vcs = [\n");
    let report = load_startup(&process(&[]), &files);
    let [StartupDiagnostic::Dotenv(diagnostic)] = report.diagnostics.as_slice() else {
        panic!("one dotenv warning: {:?}", report.diagnostics);
    };
    assert_eq!(diagnostic.path, PathBuf::from(fixture_path("/work/.env")));
    assert!(report.result.is_err());
}

#[test]
fn repo_root_supplies_dotenv_and_config() {
    let files = MemFiles::default()
        .directory("/repo/.git")
        .file("/repo/.env", b"LINEAR_TEAM_ID=ROOT\n")
        .file("/repo/.config/linear.toml", b"issue_sort = 'manual'\n");
    let ready = load_startup(&process_in("/repo/sub", &[]), &files)
        .result
        .unwrap();
    assert_eq!(ready.options.team_key(), Some("ROOT"));
    assert_eq!(
        ready.options.sourced_issue_sort().map(|sort| sort.source()),
        Some(&OptionSource::ProjectConfig {
            path: PathBuf::from(fixture_path("/repo/.config/linear.toml"))
        })
    );
}

#[test]
fn outside_a_repository_cwd_dot_config_is_not_a_candidate() {
    let files = MemFiles::default().file("/work/.config/linear.toml", b"issue_sort = 'manual'\n");
    let ready = load_startup(&process(&[]), &files).result.unwrap();
    assert!(ready.options.sourced_issue_sort().is_none());
}

fn retired(diagnostics: &[StartupDiagnostic]) -> Vec<OptionSource> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            StartupDiagnostic::RetiredVcs(source) => Some(source.clone()),
            StartupDiagnostic::Dotenv(_) => None,
        })
        .collect()
}

#[test]
fn the_retired_vcs_setting_loads_and_warns_once_per_source_that_sets_it() {
    let global = fixture_path("/global");
    let files = MemFiles::default()
        .file("/work/.env", b"LINEAR_VCS=git\n")
        .file("/work/linear.toml", b"vcs = 'jj'\n")
        .file("/global/linear/linear.toml", b"vcs = 'git'\n");
    let path = |path: &str| PathBuf::from(fixture_path(path));
    // The process environment hides the .env value, which is then not loaded.
    let report = load_startup(
        &process(&[("XDG_CONFIG_HOME", global.as_str()), ("LINEAR_VCS", "jj")]),
        &files,
    );
    assert!(report.result.is_ok());
    assert_eq!(
        retired(&report.diagnostics),
        [
            OptionSource::Env,
            OptionSource::ProjectConfig {
                path: path("/work/linear.toml")
            },
            OptionSource::GlobalConfig {
                path: path("/global/linear/linear.toml")
            },
        ]
    );
    let report = load_startup(&process(&[]), &files);
    assert!(report.result.is_ok());
    assert_eq!(
        retired(&report.diagnostics),
        [
            OptionSource::ProjectEnv {
                path: path("/work/.env")
            },
            OptionSource::ProjectConfig {
                path: path("/work/linear.toml")
            },
        ]
    );
    let report = load_startup(&process(&[]), &MemFiles::default());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let ignored = load_startup(&process(&[("LINEAR_IGNORE_ENV_FILE", "1")]), &files);
    assert_eq!(
        retired(&ignored.diagnostics).len(),
        1,
        "an ignored .env is not read"
    );
}

#[test]
fn the_retired_vcs_setting_still_rejects_what_was_never_valid() {
    for (env, file, said) in [
        (
            "GIT",
            "",
            "invalid config option LINEAR_VCS from process environment",
        ),
        (
            "",
            "",
            "invalid config option LINEAR_VCS from process environment",
        ),
        (
            "hg",
            "",
            "invalid config option LINEAR_VCS from process environment",
        ),
        (
            "jj",
            "vcs = 1\n",
            "invalid config option vcs from project config",
        ),
        (
            "jj",
            "vcs = 'Jj'\n",
            "invalid config option vcs from project config",
        ),
    ] {
        let files = MemFiles::default().file("/work/linear.toml", file.as_bytes());
        let report = load_startup(&process(&[("LINEAR_VCS", env)]), &files);
        let error = report.result.unwrap_err();
        assert!(error.to_string().contains(said), "{error}");
        assert!(error.to_string().contains("git"), "{error}");
        assert!(
            error.hint().is_some_and(|hint| hint.starts_with("Fix ")),
            "{error:?}"
        );
        assert!(
            retired(&report.diagnostics).is_empty(),
            "{:?}",
            report.diagnostics
        );
    }
}

#[test]
fn the_retired_vcs_warning_survives_another_options_error() {
    let files = MemFiles::default().file("/work/linear.toml", b"vcs = 'jj'\nissue_sort = 'x'\n");
    let report = load_startup(&process(&[]), &files);
    assert!(
        report
            .result
            .unwrap_err()
            .to_string()
            .contains("issue_sort")
    );
    assert_eq!(retired(&report.diagnostics).len(), 1);
    let report = load_startup(
        &process(&[("LINEAR_VCS", "jj"), ("LINEAR_GRAPHQL_ENDPOINT", "bad")]),
        &MemFiles::default(),
    );
    assert!(report.result.is_err());
    assert_eq!(retired(&report.diagnostics), [OptionSource::Env]);
}

#[test]
fn poisoned_candidate_and_endpoint_error_are_explicit() {
    let report = load_startup(
        &process(&[]),
        &MemFiles::default().unreadable("/work/linear.toml"),
    );
    assert_eq!(
        report.result.unwrap_err().to_string(),
        format!(
            "cannot read config file {}: permission denied",
            fixture_path("/work/linear.toml")
        )
    );
    let report = load_startup(
        &process(&[]),
        &MemFiles::default().file("/work/linear.toml", &vec![b'x'; 1024 * 1024 + 1]),
    );
    assert_eq!(
        report.result.unwrap_err().to_string(),
        format!(
            "invalid config file {}: too large",
            fixture_path("/work/linear.toml")
        )
    );
    let report = load_startup(
        &process(&[("LINEAR_GRAPHQL_ENDPOINT", "bad")]),
        &MemFiles::default(),
    );
    let error = report.result.unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid LINEAR_GRAPHQL_ENDPOINT from process environment: expected an http(s) URL without credentials or fragment"
    );
}

#[test]
fn warning_templates_are_colored_only_on_request() {
    let diagnostic = ConfigDiagnostic {
        path: PathBuf::from("/work/.env"),
        reason: DiagnosticReason::InvalidLines(vec!["LINEAR_TEAM_ID".to_owned()]),
    };
    let body = "Warning: Ignoring LINEAR_TEAM_ID in /work/.env: the line could not be parsed.";
    let suggestion = "  Check for an unclosed quote or a malformed KEY=value line.";
    let diagnostic = StartupDiagnostic::Dotenv(diagnostic);
    let colored = render_diagnostic(&diagnostic, true);
    assert!(colored.contains(body) && colored.contains(suggestion));
    assert!(colored.contains('\x1b'));
    assert_eq!(
        render_diagnostic(&diagnostic, false),
        format!("{body}\n{suggestion}\n")
    );
}

#[test]
fn the_retired_vcs_warning_names_the_spelling_and_source_and_the_replacements() {
    let hint = "Commands name their tool instead: `linear issue jj pr` and `linear issue jj start` for jj, `linear issue gh pr` and `linear issue git start` for git.";
    for (source, said) in [
        (
            OptionSource::Env,
            "Warning: LINEAR_VCS from process environment was removed in 4.0 and does nothing.\n  Remove LINEAR_VCS. ",
        ),
        (
            OptionSource::ProjectEnv {
                path: PathBuf::from("/work/.env"),
            },
            "Warning: LINEAR_VCS from .env file /work/.env was removed in 4.0 and does nothing.\n  Remove LINEAR_VCS. ",
        ),
        (
            OptionSource::ProjectConfig {
                path: PathBuf::from("/work/.linear.toml"),
            },
            "Warning: vcs from project config /work/.linear.toml was removed in 4.0 and does nothing.\n  Remove vcs. ",
        ),
        (
            OptionSource::GlobalConfig {
                path: PathBuf::from("/home/linear.toml"),
            },
            "Warning: vcs from global config /home/linear.toml was removed in 4.0 and does nothing.\n  Remove vcs. ",
        ),
    ] {
        let rendered = render_diagnostic(&StartupDiagnostic::RetiredVcs(source), false);
        assert_eq!(rendered, format!("{said}{hint}\n"));
    }
}
