use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::config::OsFamily;

fn inputs() -> ConfigInputs {
    ConfigInputs {
        cwd: PathBuf::from("/tmp/project/sub"),
        os: OsFamily::Unix,
        process_env: BTreeMap::new(),
    }
}

#[test]
fn project_candidates_are_cwd_then_repo_root() {
    let paths = discover_config_paths(&inputs(), Some(Path::new("/tmp/project")));
    let project = paths
        .project
        .iter()
        .map(|candidate| candidate.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        project,
        [
            "/tmp/project/sub/linear.toml",
            "/tmp/project/sub/.linear.toml",
            "/tmp/project/linear.toml",
            "/tmp/project/.linear.toml",
            "/tmp/project/.config/linear.toml",
        ]
        .map(PathBuf::from)
    );
}

#[test]
fn without_a_repository_only_cwd_candidates_are_used() {
    let paths = discover_config_paths(&inputs(), None);
    assert_eq!(paths.project.len(), 2);
}

#[test]
fn cwd_at_the_repo_root_lists_each_candidate_once() {
    let paths = discover_config_paths(&inputs(), Some(Path::new("/tmp/project/sub")));
    assert_eq!(paths.project.len(), 3);
    assert_eq!(
        paths.project[2].path,
        PathBuf::from("/tmp/project/sub/.config/linear.toml")
    );
}

#[test]
fn global_config_uses_xdg_then_home_and_appdata_on_windows() {
    let mut inputs = inputs();
    inputs
        .process_env
        .insert("HOME".to_owned(), "/tmp/home".to_owned());
    inputs
        .process_env
        .insert("APPDATA".to_owned(), "/tmp/roaming".to_owned());
    assert_eq!(
        discover_config_paths(&inputs, None).global[0].path,
        PathBuf::from("/tmp/home/.config/linear/linear.toml")
    );
    inputs
        .process_env
        .insert("XDG_CONFIG_HOME".to_owned(), "/tmp/xdg".to_owned());
    assert_eq!(
        discover_config_paths(&inputs, None).global[0].path,
        PathBuf::from("/tmp/xdg/linear/linear.toml")
    );
    inputs
        .process_env
        .insert("XDG_CONFIG_HOME".to_owned(), String::new());
    assert_eq!(
        discover_config_paths(&inputs, None).global[0].path,
        PathBuf::from("/tmp/home/.config/linear/linear.toml")
    );
    inputs.os = OsFamily::Windows;
    assert_eq!(
        discover_config_paths(&inputs, None).global[0].path,
        PathBuf::from("/tmp/roaming/linear/linear.toml")
    );
}

#[test]
fn absent_global_bases_produce_no_global_candidate() {
    let mut inputs = inputs();
    assert!(discover_config_paths(&inputs, None).global.is_empty());
    inputs.os = OsFamily::Windows;
    inputs
        .process_env
        .insert("APPDATA".to_owned(), String::new());
    assert!(discover_config_paths(&inputs, None).global.is_empty());
}
