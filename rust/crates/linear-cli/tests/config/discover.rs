use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use linear_cli::config::{
    ConfigInputs, GitProbeResult, GitRootProbe, OsFamily, discover_config_paths,
};

pub(super) struct Probe {
    pub result: GitProbeResult,
    pub count: Cell<usize>,
}

impl Probe {
    pub fn new(result: GitProbeResult) -> Self {
        Self {
            result,
            count: Cell::new(0),
        }
    }
}

impl GitRootProbe for Probe {
    fn probe(&self) -> GitProbeResult {
        self.count.set(self.count.get() + 1);
        self.result.clone()
    }
}

pub(super) fn inputs() -> ConfigInputs {
    ConfigInputs {
        cwd: PathBuf::from("/tmp/riir-project/sub"),
        os: OsFamily::Unix,
        process_env: BTreeMap::new(),
    }
}

#[test]
fn config_probe_accepts_empty_stdout_after_nonzero_git() {
    let probe = Probe::new(GitProbeResult::Completed {
        success: false,
        stdout: String::new(),
    });
    let paths = discover_config_paths(&inputs(), &probe).unwrap();
    assert_eq!(probe.count.get(), 1);
    assert_eq!(paths.project.len(), 5);
    assert_eq!(
        paths.project[4].path,
        PathBuf::from("/tmp/riir-project/sub/.config/linear.toml")
    );
}

#[test]
fn config_probe_omits_git_candidates_only_on_spawn_failure() {
    let probe = Probe::new(GitProbeResult::SpawnFailure);
    let paths = discover_config_paths(&inputs(), &probe).unwrap();
    assert_eq!(paths.project.len(), 2);
    assert_eq!(probe.count.get(), 1);
}

#[test]
fn root_and_global_candidate_order() {
    let mut inputs = inputs();
    inputs
        .process_env
        .insert("XDG_CONFIG_HOME".to_owned(), "/tmp/xdg".to_owned());
    inputs
        .process_env
        .insert("HOME".to_owned(), "/tmp/home".to_owned());
    let probe = Probe::new(GitProbeResult::Completed {
        success: true,
        stdout: "/tmp/riir-project\n".to_owned(),
    });
    let paths = discover_config_paths(&inputs, &probe).unwrap();
    assert_eq!(
        paths.global[0].path,
        PathBuf::from("/tmp/xdg/linear/linear.toml")
    );
    assert_eq!(
        paths.project[0].path,
        PathBuf::from("/tmp/riir-project/sub/linear.toml")
    );
    assert_eq!(
        paths.project[2].path,
        PathBuf::from("/tmp/riir-project/linear.toml")
    );
    assert_eq!(
        paths.project[4].path,
        PathBuf::from("/tmp/riir-project/.config/linear.toml")
    );
}

#[test]
fn falsey_xdg_falls_back_to_home_and_windows_uses_appdata() {
    let mut inputs = inputs();
    inputs
        .process_env
        .insert("XDG_CONFIG_HOME".to_owned(), String::new());
    inputs
        .process_env
        .insert("HOME".to_owned(), "/tmp/home".to_owned());
    inputs
        .process_env
        .insert("APPDATA".to_owned(), "/tmp/roaming".to_owned());
    let probe = Probe::new(GitProbeResult::SpawnFailure);
    assert_eq!(
        discover_config_paths(&inputs, &probe).unwrap().global[0].path,
        PathBuf::from("/tmp/home/.config/linear/linear.toml")
    );
    inputs.os = OsFamily::Windows;
    assert_eq!(
        discover_config_paths(&inputs, &probe).unwrap().global[0].path,
        PathBuf::from("/tmp/roaming/linear/linear.toml")
    );
}

#[test]
fn absent_global_bases_produce_no_global_candidate() {
    let mut inputs = inputs();
    let probe = Probe::new(GitProbeResult::SpawnFailure);
    assert!(
        discover_config_paths(&inputs, &probe)
            .unwrap()
            .global
            .is_empty()
    );
    inputs.os = OsFamily::Windows;
    assert!(
        discover_config_paths(&inputs, &probe)
            .unwrap()
            .global
            .is_empty()
    );
    inputs
        .process_env
        .insert("APPDATA".to_owned(), String::new());
    assert!(
        discover_config_paths(&inputs, &probe)
            .unwrap()
            .global
            .is_empty()
    );
}
