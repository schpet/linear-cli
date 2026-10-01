use linear_cli::{
    error::AppError,
    platform::prompt::{PlainOption, PromptKey, PromptOutcome, PromptSession},
};
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
fn options() -> Vec<PlainOption> {
    vec![
        PlainOption {
            label: "Workflow".into(),
            value: "state".into(),
            script_token: "state".into(),
        },
        PlainOption {
            label: "Labels".into(),
            value: "labels".into(),
            script_token: "labels".into(),
        },
    ]
}
#[test]
fn checkbox_script_membership_returns_declaration_order_and_retains_next_prompt() {
    let mut prompt =
        PromptSession::script_cr_or_lf(Cursor::new(b"labels,state\rnext\r"), Vec::new());
    assert_eq!(
        prompt
            .checkbox("Fields", &options(), false)
            .expect("members"),
        PromptOutcome::Submitted(vec!["state".into(), "labels".into()])
    );
    assert_eq!(
        prompt.text("Next", 0, |_| Ok(())).expect("retained"),
        PromptOutcome::Submitted("next".into())
    );
    for input in [b"missing\r".to_vec(), b"state,state\r".to_vec()] {
        let mut prompt = PromptSession::script_cr_or_lf(Cursor::new(input), Vec::new());
        assert!(prompt.checkbox("Fields", &options(), false).is_err())
    }
    let mut prompt = PromptSession::script(Cursor::new(b"\n"), Vec::new());
    assert_eq!(
        prompt.checkbox("Fields", &options(), false).expect("none"),
        PromptOutcome::Submitted(Vec::new())
    );
}
#[test]
fn checkbox_keys_toggle_search_recovery_and_abort_share_existing_owner() {
    let mut keys = VecDeque::from([
        PromptKey::Down,
        PromptKey::Character(' '),
        PromptKey::Up,
        PromptKey::Character(' '),
        PromptKey::Enter,
    ]);
    let mut prompt =
        PromptSession::<Cursor<Vec<u8>>, Vec<u8>>::keys(Vec::new(), 80, 24, move || {
            Ok(keys.pop_front().expect("key"))
        })
        .expect("owner");
    assert_eq!(
        prompt
            .checkbox("Fields", &options(), false)
            .expect("selected"),
        PromptOutcome::Submitted(vec!["state".into(), "labels".into()])
    );
    let mut keys = VecDeque::from([
        PromptKey::Character('x'),
        PromptKey::Backspace,
        PromptKey::Character(' '),
        PromptKey::Enter,
    ]);
    let mut prompt =
        PromptSession::<Cursor<Vec<u8>>, Vec<u8>>::keys(Vec::new(), 80, 24, move || {
            Ok(keys.pop_front().expect("key"))
        })
        .expect("owner");
    assert_eq!(
        prompt
            .checkbox("Fields", &options(), true)
            .expect("recover"),
        PromptOutcome::Submitted(vec!["state".into()])
    );
    let mut prompt = PromptSession::<Cursor<Vec<u8>>, Vec<u8>>::keys(Vec::new(), 80, 24, || {
        Ok(PromptKey::Interrupt)
    })
    .expect("owner");
    assert_eq!(
        prompt.checkbox("Fields", &options(), true).expect("abort"),
        PromptOutcome::Interrupted
    );
}
#[test]
fn eager_phase_drives_both_tasks_and_abort_drops_them_before_owner_returns() {
    use linear_cli::platform::network_owner;
    struct Guard(Arc<AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicUsize::new(0));
    let (ready, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        let first_ready = ready.clone();
        let first_drop = dropped.clone();
        let second_drop = dropped.clone();
        let (phase, _first, _second) = network_owner::pair(
            scope,
            async move {
                let _guard = Guard(first_drop);
                first_ready.send("auto").expect("notify");
                futures_util::future::pending::<Result<bool, AppError>>().await
            },
            async move {
                let _guard = Guard(second_drop);
                ready.send("team").expect("notify");
                futures_util::future::pending::<Result<bool, AppError>>().await
            },
        )
        .expect("spawn");
        // Test-only deadlock watchdog, not a production request/prompt deadline.
        let mut actual = vec![
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("eager first"),
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("eager second"),
        ];
        actual.sort();
        assert_eq!(actual, ["auto", "team"]);
        phase.close().expect("abort then join");
        assert_eq!(dropped.load(Ordering::SeqCst), 2);
    });
}
#[test]
fn early_phase_return_joins_without_waiting_for_unconsumed_request() {
    use linear_cli::platform::network_owner;
    std::thread::scope(|scope| {
        let (phase, first, _second) = network_owner::pair(
            scope,
            async {
                Err::<bool, _>(AppError::new(
                    linear_cli::error::AppErrorKind::GraphQl,
                    "team failed",
                ))
            },
            futures_util::future::pending::<Result<bool, AppError>>(),
        )
        .expect("spawn");
        assert_eq!(first.take().expect_err("failure").message, "team failed");
        drop(phase);
    });
}

#[test]
fn project_result_is_awaitable_while_both_other_preloads_remain_pending() {
    use linear_cli::platform::network_owner;
    std::thread::scope(|scope| {
        let (phase, _states, _labels, projects) = network_owner::triple(
            scope,
            futures_util::future::pending::<Result<bool, AppError>>(),
            futures_util::future::pending::<Result<bool, AppError>>(),
            async { Ok::<_, AppError>("ready projects") },
        )
        .unwrap();
        assert_eq!(projects.take().unwrap(), "ready projects");
        phase.close().unwrap();
    });
}
