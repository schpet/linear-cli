use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{ExistingBranch, existing_git, filter};
use crate::config::{
    ChildEnvOverlay, FileKind, FileSource, OsFamily, ProcessEnvSnapshot, load_startup,
};
use crate::error::Error;
use crate::platform::vcs_script::{Captured, ChildOutcome, CommandSpec, ProcessRunner, Program};

struct NoFiles;

impl FileSource for NoFiles {
    fn kind(&self, _: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _: &Path, _: u64) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::NotFound.into())
    }
}

fn no_overlay() -> ChildEnvOverlay {
    let env = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        std::iter::empty(),
    )
    .expect("empty environment");
    load_startup(&env, &NoFiles)
        .result
        .expect("startup without files")
        .child_env
}

fn exit(code: i32, stderr: &[u8]) -> Captured {
    Captured {
        outcome: ChildOutcome::Code(code),
        stdout: Vec::new(),
        stderr: stderr.to_vec(),
    }
}

/// Answers each captured command with the next scripted outcome.
struct Script {
    outcomes: VecDeque<Captured>,
    calls: Vec<CommandSpec>,
}

impl Script {
    fn new(outcomes: Vec<Captured>) -> Self {
        Self {
            outcomes: outcomes.into(),
            calls: Vec::new(),
        }
    }
}

impl ProcessRunner for Script {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        _: &Path,
        _: &ChildEnvOverlay,
    ) -> Result<Captured, Error> {
        self.calls.push(spec.clone());
        Ok(self.outcomes.pop_front().expect("unexpected command"))
    }
    fn inherit(
        &mut self,
        _: &CommandSpec,
        _: &Path,
        _: &ChildEnvOverlay,
    ) -> Result<ChildOutcome, Error> {
        panic!("issue start captures every command")
    }
}

#[test]
fn an_existing_branch_gets_the_first_free_suffix() {
    let mut git = Script::new(vec![exit(0, b""), exit(0, b""), exit(1, b""), exit(0, b"")]);
    let output = existing_git(
        &mut git,
        ExistingBranch::Suffix,
        "eng-1",
        None,
        Path::new("/work"),
        &no_overlay(),
    )
    .expect("new branch");
    assert_eq!(
        String::from_utf8(output).expect("UTF-8"),
        "✓ Created and switched to branch 'eng-1-3'\n"
    );
    assert_eq!(
        git.calls,
        [
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "eng-1-1"]),
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "eng-1-2"]),
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "eng-1-3"]),
            CommandSpec::new(Program::Git, &["checkout", "-b", "eng-1-3", "HEAD"]),
        ]
    );
}

#[test]
fn a_failed_switch_reports_git_stderr() {
    let mut git = Script::new(vec![exit(7, b" not a branch\n ")]);
    let error = existing_git(
        &mut git,
        ExistingBranch::Switch,
        "eng-1",
        None,
        Path::new("/work"),
        &no_overlay(),
    )
    .expect_err("switch fails");
    assert_eq!(
        error.message(),
        "Failed to switch to branch 'eng-1': not a branch"
    );
    assert_eq!(
        git.calls,
        [CommandSpec::new(Program::Git, &["checkout", "eng-1"])]
    );
}

#[test]
fn the_picker_lists_unstarted_issues_for_the_chosen_assignees() {
    for (all, unassigned, assignee) in [
        (false, false, Some(json!({"isMe": {"eq": true}}))),
        (false, true, Some(json!({"null": true}))),
        (true, false, None),
    ] {
        let value = serde_json::to_value(filter("ENG", all, unassigned)).expect("filter");
        assert_eq!(value["team"], json!({"key": {"eq": "ENG"}}));
        assert_eq!(value["state"], json!({"type": {"in": ["unstarted"]}}));
        assert_eq!(value.get("assignee").cloned(), assignee);
    }
}
