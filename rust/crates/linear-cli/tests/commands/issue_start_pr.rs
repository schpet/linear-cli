//! Public source command contracts; all processes, paths and responses are fake.
use linear_cli::{
    commands::{issue_pull_request as pr, issue_start as start},
    config::{
        ChildEnvOverlay, FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily,
        ProcessEnvSnapshot,
    },
    error::AppError,
    graphql::{
        bulk_error::{ObservedExchangeFailure, SourceException, SourceExceptionKind},
        operations::workflow_states::WorkflowState,
    },
    platform::{
        gh_script::GhRunner,
        vcs_script::{Captured, ChildOutcome, CommandSpec, ProcessRunner, Program},
    },
};
use std::{
    collections::VecDeque,
    io,
    path::{Path, PathBuf},
};
struct EmptyFiles;
impl FileSource for EmptyFiles {
    fn kind(&self, _: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _: &Path, _: u64) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::NotFound.into())
    }
}
struct NoGit;
impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
    }
}
fn overlay() -> ChildEnvOverlay {
    let env = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/fake-start-pr"),
        OsFamily::Unix,
        std::iter::empty(),
    )
    .unwrap();
    linear_cli::config::load_startup(&env, &EmptyFiles, &NoGit)
        .result
        .unwrap()
        .child_env
}
fn captured(code: i32, stdout: &[u8], stderr: &[u8]) -> Captured {
    Captured {
        outcome: ChildOutcome::Code(code),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}
struct Runner {
    outcomes: VecDeque<Captured>,
    calls: Vec<CommandSpec>,
}
impl Runner {
    fn new(outcomes: Vec<Captured>) -> Self {
        Self {
            outcomes: outcomes.into(),
            calls: vec![],
        }
    }
}
impl ProcessRunner for Runner {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        _: &Path,
        _: &ChildEnvOverlay,
    ) -> Result<Captured, AppError> {
        self.calls.push(spec.clone());
        Ok(self.outcomes.pop_front().expect("unexpected process"))
    }
    fn inherit(
        &mut self,
        _: &CommandSpec,
        _: &Path,
        _: &ChildEnvOverlay,
    ) -> Result<ChildOutcome, AppError> {
        panic!("start may not inherit Git/jj")
    }
}
#[test]
fn team_preempts_conflict_and_picker_filters_preserve_assignee_shapes() {
    assert_eq!(
        start::team_and_flags(None, true, true).unwrap_err().message,
        "Could not determine team ID"
    );
    assert_eq!(
        start::team_and_flags(Some("ENG"), true, true)
            .unwrap_err()
            .message,
        "Cannot specify both --all-assignees and --unassigned"
    );
    for (all, unassigned, expected) in [
        (false, false, serde_json::json!({"isMe":{"eq":true}})),
        (false, true, serde_json::json!({"null":true})),
        (true, false, serde_json::Value::Null),
    ] {
        let value = serde_json::to_value(start::filter("ENG", all, unassigned)).unwrap();
        assert_eq!(value["team"], serde_json::json!({"key":{"eq":"ENG"}}));
        assert_eq!(
            value["state"],
            serde_json::json!({"type":{"in":["unstarted"]}})
        );
        if all {
            assert!(value.get("assignee").is_none());
        } else {
            assert_eq!(value["assignee"], expected);
        }
    }
    assert_eq!(start::branch_name(Some(""), "returned"), "returned");
    assert_eq!(start::branch_name(Some(" "), "returned"), " ");
}
#[test]
fn git_switch_and_suffix_failures_keep_exact_argv_and_correct_branch_diagnostics() {
    let cwd = Path::new("/dummy");
    let env = overlay();
    let mut runner = Runner::new(vec![captured(
        7,
        b"hidden",
        b" \xef\xbb\xbfDUMMY switch\n ",
    )]);
    let err = start::existing_git(
        &mut runner,
        start::ExistingBranch::Switch,
        "B",
        Some("ignored"),
        cwd,
        &env,
    )
    .unwrap_err();
    assert_eq!(err.message, "Failed to switch to branch 'B': DUMMY switch");
    assert_eq!(
        runner.calls,
        vec![CommandSpec::new(Program::Git, &["checkout", "B"])]
    );
    let mut runner = Runner::new(vec![
        captured(0, b"", b""),
        captured(0, b"", b""),
        captured(19, b"", b""),
        captured(7, b"", b" DUMMY create \n"),
    ]);
    let err = start::existing_git(
        &mut runner,
        start::ExistingBranch::Suffix,
        "B",
        Some(""),
        cwd,
        &env,
    )
    .unwrap_err();
    assert_eq!(err.message, "Failed to create branch 'B-3': DUMMY create");
    assert_eq!(
        runner.calls,
        vec![
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "B-1"]),
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "B-2"]),
            CommandSpec::new(Program::Git, &["rev-parse", "--verify", "B-3"]),
            CommandSpec::new(Program::Git, &["checkout", "-b", "B-3", "HEAD"])
        ]
    );
}
#[test]
fn jj_probes_ignore_status_and_failures_decode_without_trimming_plus_extra_lf() {
    let cwd = Path::new("/dummy");
    let env = overlay();
    let mut runner = Runner::new(vec![
        captured(7, b"\xef\xbb\xbf \n", b""),
        captured(8, b"diff --git fake", b""),
        captured(9, b"", b"\xef\xbb\xbfDUMMY\xff \n"),
    ]);
    let mut err = vec![];
    assert_eq!(
        start::prepare_jj(&mut runner, cwd, &env, &mut err)
            .unwrap_err()
            .message,
        "Failed to create new jj change"
    );
    assert_eq!(err, "DUMMY\u{fffd} \n\n".as_bytes());
    assert_eq!(
        runner.calls.last().unwrap(),
        &CommandSpec::new(Program::Jj, &["new"])
    );
    let mut runner = Runner::new(vec![captured(7, b"", b"\xef\xbb\xbfDUMMY\xff \n")]);
    let mut err = vec![];
    assert_eq!(
        start::describe_jj(
            &mut runner,
            "ENG-1",
            "second 界",
            "https://dummy",
            cwd,
            &env,
            &mut err
        )
        .unwrap_err()
        .message,
        "Failed to set jj description"
    );
    assert_eq!(err, "DUMMY\u{fffd} \n\n".as_bytes());
    assert_eq!(
        runner.calls[0],
        CommandSpec::new(
            Program::Jj,
            &[
                "describe",
                "-m",
                "ENG-1 second 界\n\nLinear-issue: Fixes ENG-1\nLinear-issue-url: https://dummy"
            ]
        )
    );
}
#[test]
fn state_chooses_lowest_started_with_stable_ties_and_raw_sdk_not_preferred() {
    let state = |id: &str, kind: &str, position: f64| WorkflowState {
        id: cynic::Id::new(id),
        name: id.to_owned(),
        state_type: kind.to_owned(),
        position,
    };
    let chosen = start::started(vec![
        state("higher", "started", 10.0),
        state("first", "started", 1.0),
        state("second", "started", 1.0),
        state("other", "unstarted", -50.0),
    ])
    .unwrap();
    assert_eq!(chosen.id.inner(), "first");
    assert!(start::started(vec![state("bad", "unstarted", f64::NAN)]).is_err());
    assert_eq!(
        start::started(vec![]).unwrap_err().message,
        "No 'started' state found in workflow"
    );
    let failure = ObservedExchangeFailure::Ordinary(SourceException {
        kind: SourceExceptionKind::Client,
        message: "raw SDK query+metadata".into(),
        preferred_message: Some("preferred".into()),
    });
    assert_eq!(
        start::post_failure(failure),
        "ClientError: raw SDK query+metadata"
    );
    let wire = serde_json::to_value(start::update_request("ENG-1", "state1")).unwrap();
    assert_eq!(wire["operationName"], "UpdateIssueState");
    assert_eq!(
        wire["variables"],
        serde_json::json!({"issueId":"ENG-1","stateId":"state1"})
    );
    assert!(
        wire["query"]
            .as_str()
            .unwrap()
            .contains("stateId: $stateId")
    );
}
#[test]
fn mixed_output_refusal_is_stdin_tty_and_actual_fifo_only() {
    assert!(start::check_prompt_topology(true, true).is_err());
    for (stdin, fifo) in [(false, true), (true, false), (false, false)] {
        assert!(start::check_prompt_topology(stdin, fifo).is_ok());
    }
}
#[test]
fn pr_body_and_argv_preserve_js_trimend_bom_empty_title_and_raw_values() {
    let args = pr::args(
        "ENG-1",
        "default",
        "URL",
        Some("\u{feff} leading\r\n\u{feff}"),
        pr::Options {
            title: Some(""),
            base: Some(""),
            head: Some(" "),
            draft: true,
            web: true,
        },
    );
    assert_eq!(
        args,
        [
            "pr",
            "create",
            "--title",
            "ENG-1 ",
            "--body",
            "\u{feff} leading\n\nURL",
            "--head",
            " ",
            "--draft",
            "--web"
        ]
    );
    assert_eq!(pr::body(Some(" \u{feff}\r\n"), "URL"), "URL");
    assert_eq!(pr::body(Some("text\u{0085}"), "URL"), "text\u{0085}\n\nURL");
}
#[test]
fn pr_template_follows_symlink_keeps_lossy_bom_crlf_and_refuses_nul_missing_directory() {
    let root = std::env::temp_dir().join(format!("c071-c081-template-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("template");
    std::fs::write(&file, b"\xef\xbb\xbfDUMMY\xff\r\n").unwrap();
    assert_eq!(
        pr::read_template(&file).unwrap(),
        "\u{feff}DUMMY\u{fffd}\r\n"
    );
    #[cfg(unix)]
    {
        let link = root.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(
            pr::read_template(&link).unwrap(),
            pr::read_template(&file).unwrap()
        );
    }
    for (path, reason) in [
        (root.join("missing"), "does not exist"),
        (root.clone(), "is a directory, not a file"),
    ] {
        let error = pr::read_template(&path).unwrap_err();
        assert!(error.message.contains(reason));
        assert_eq!(error.suggestion.as_deref(), Some(pr::TEMPLATE_SUGGESTION));
    }
    std::fs::write(&file, b"DUMMY\0").unwrap();
    assert!(
        pr::read_template(&file)
            .unwrap_err()
            .message
            .ends_with("is not a text file")
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn gh_non_success_is_handled_not_forwarded_and_has_no_retry() {
    struct Gh {
        calls: usize,
        success: bool,
    }
    impl GhRunner for Gh {
        fn create(
            &mut self,
            args: &[String],
            _: &Path,
            _: &ChildEnvOverlay,
        ) -> Result<bool, AppError> {
            self.calls += 1;
            assert_eq!(args, ["pr"]);
            Ok(self.success)
        }
    }
    for success in [true, false] {
        let mut gh = Gh { calls: 0, success };
        let result = pr::create(&mut gh, &["pr".into()], Path::new("/dummy"), &overlay());
        assert_eq!(result.is_ok(), success);
        if let Err(error) = result {
            assert_eq!(error.message, "Failed to create pull request");
        }
        assert_eq!(gh.calls, 1);
    }
}

#[test]
fn start_list_full_wire_equals_original_source_in_filter_and_final_lf() {
    use linear_cli::graphql::operations::issue_read::GetIssuesForStateVariables;
    let request = start::list_request(GetIssuesForStateVariables {
        sort: Some(linear_cli::commands::issue_read::sort_payload(true)),
        filter: start::filter("ENG", false, false),
        first: Some(50),
        after: None,
    });
    let source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/c071-list-wire.json")).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), source);
}
