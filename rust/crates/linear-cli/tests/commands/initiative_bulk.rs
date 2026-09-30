use linear_cli::{
    auth::ApiKeyInput,
    commands::{
        initiative_bulk::{self as command, BulkInput, BulkOutcome, Mode, Target},
        initiative_view::Reference,
    },
    graphql::transport::{
        ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
        TransportConfig,
    },
    refs::WorkspaceScope,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn read_request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut input = Vec::new();
    loop {
        let mut bytes = [0; 8192];
        let count = stream.read(&mut bytes).unwrap();
        assert!(count > 0);
        input.extend_from_slice(&bytes[..count]);
        if let Some((headers, payload)) =
            std::str::from_utf8(&input).unwrap().split_once("\r\n\r\n")
        {
            let length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            if payload.len() >= length {
                return serde_json::from_str(&payload[..length]).unwrap();
            }
        }
    }
}
fn reply(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
fn accept(listener: &TcpListener) -> TcpStream {
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "expected request never arrived"
                );
                thread::sleep(Duration::from_millis(2))
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
}
fn transport(endpoint: &str, deadline: Duration, cap: usize) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(deadline).unwrap(),
            max_response_bytes: ResponseCap::new(cap).unwrap(),
        },
    )
    .unwrap()
}
struct Reply {
    status: u16,
    body: String,
    delay: Duration,
}
impl Reply {
    fn data(value: Value) -> Self {
        Self {
            status: 200,
            body: value.to_string(),
            delay: Duration::ZERO,
        }
    }
}
fn server(
    replies: Vec<Reply>,
    deadline: Duration,
    cap: usize,
) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        let mut workers = Vec::new();
        for expected in replies {
            let mut stream = accept(&listener);
            requests.push(read_request(&mut stream));
            workers.push(thread::spawn(move || {
                thread::sleep(expected.delay);
                if expected.status != 0 {
                    let result = reply(&mut stream, expected.status, &expected.body);
                    if expected.delay.is_zero() {
                        result.unwrap();
                    }
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert!(listener.accept().is_err(), "unexpected extra request");
        requests
    });
    (transport(&endpoint, deadline, cap), worker)
}
fn scope() -> WorkspaceScope<'static> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: &ApiKeyInput::Absent,
    }
}
fn uuid(number: usize) -> String {
    format!("00000000-0000-4000-9000-{number:012}")
}
fn target(id: &str) -> Target {
    Target::prepare(id.to_owned(), &scope())
}
fn temporary() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "linear-bulk-public-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
#[test]
fn additive_collection_preserves_js_tokenization_and_exact_ordered_dedupe() {
    let dir = temporary();
    let file = dir.join("ids.txt");
    std::fs::write(&file, "A\u{feff}File\u{85}Joined,A\r\nB").unwrap();
    let argv = vec!["A".into(), " Raw, argv ".into(), "A".into()];
    let input = BulkInput {
        argv: Some(&argv),
        file: Some(&file),
        stdin: true,
    };
    assert_eq!(
        command::collect_ids(&input, &mut "B\u{feff}Stdin\u{85}Joined,A\nC".as_bytes()).unwrap(),
        vec![
            "A",
            " Raw, argv ",
            "File\u{85}Joined",
            "B",
            "Stdin\u{85}Joined",
            "C"
        ]
    );
    assert!(input.requested());
    assert!(
        !BulkInput {
            argv: Some(&[]),
            file: None,
            stdin: false
        }
        .requested()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn each_invalid_input_rejects_valid_neighbors_before_any_ids_are_returned() {
    let dir = temporary();
    let file = dir.join("ids.bin");
    std::fs::write(&file, [uuid(1).as_bytes(), b",\xff"].concat()).unwrap();
    let argv = vec![uuid(2)];
    let error = command::collect_ids(
        &BulkInput {
            argv: Some(&argv),
            file: Some(&file),
            stdin: false,
        },
        &mut std::io::empty(),
    )
    .unwrap_err();
    assert!(error.message.contains("Bulk file must be valid UTF-8"));
    let stdin = [uuid(1).as_bytes(), b",\xfe"].concat();
    let error = command::collect_ids(
        &BulkInput {
            argv: Some(&argv),
            file: None,
            stdin: true,
        },
        &mut stdin.as_slice(),
    )
    .unwrap_err();
    assert_eq!(error.message, "Bulk stdin must be valid UTF-8");
    let missing = dir.join("missing");
    assert_eq!(
        command::collect_ids(
            &BulkInput {
                argv: None,
                file: Some(&missing),
                stdin: false
            },
            &mut std::io::empty()
        )
        .unwrap_err()
        .message,
        format!("File not found: {}", missing.display())
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn all_reachable_optional_detail_failures_discard_partial_fields_and_still_mutate() {
    let errors = vec![
        Reply {
            status: 503,
            body: "unavailable".into(),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: "not JSON".into(),
            delay: Duration::ZERO,
        },
        Reply::data(json!({})),
        Reply::data(
            json!({"data":{"initiative":{"id":7,"name":"CORRUPT","archivedAt":"already"}}}),
        ),
        Reply::data(
            json!({"errors":[{"message":"failed"}],"data":{"initiative":{"id":uuid(1),"name":"CORRUPT","archivedAt":"already"}}}),
        ),
        Reply {
            status: 0,
            body: String::new(),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: "x".repeat(1024),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: "{}".into(),
            delay: Duration::from_millis(140),
        },
    ];
    for mode in [Mode::Archive, Mode::Delete] {
        for error in &errors {
            let detail = Reply {
                status: error.status,
                body: error.body.clone(),
                delay: error.delay,
            };
            let field = if mode == Mode::Archive {
                "initiativeArchive"
            } else {
                "initiativeDelete"
            };
            let (transport, worker) = server(
                vec![
                    detail,
                    Reply::data(json!({"data":{field:{"success":true}}})),
                ],
                Duration::from_millis(80),
                256,
            );
            let id = uuid(1);
            let result = command::run_item(&transport, target(&id), mode).await;
            assert_eq!(result.name, Some(id.clone()));
            assert_eq!(result.outcome, BulkOutcome::Succeeded);
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 2);
            assert!(
                requests[0]["query"]
                    .as_str()
                    .unwrap()
                    .contains("GetInitiativeNameForBulk")
            );
            assert!(requests[1]["query"].as_str().unwrap().contains("mutation"));
            assert_eq!(requests[1]["variables"], json!({"id":id}));
        }
    }
}
#[tokio::test]
async fn first_empty_slug_id_stops_before_name_and_url_miss_never_falls_through() {
    for mode in [Mode::Archive, Mode::Delete] {
        let (transport, worker) = server(
            vec![Reply::data(
                json!({"data":{"initiatives":{"nodes":[{"id":"","slugId":"x"},{"id":"later","slugId":"x"}]}}}),
            )],
            Duration::from_secs(2),
            65536,
        );
        assert_eq!(
            command::resolve(&transport, &Reference::NameOrSlug("x".into()), mode)
                .await
                .unwrap(),
            None
        );
        assert_eq!(worker.join().unwrap().len(), 1);
        let (transport, worker) = server(
            vec![Reply::data(json!({"data":{"initiatives":{"nodes":[]}}}))],
            Duration::from_secs(2),
            65536,
        );
        assert_eq!(
            command::resolve(&transport, &Reference::UrlSlug("123456789abc".into()), mode)
                .await
                .unwrap(),
            None
        );
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["variables"],
            json!({"slugId":"123456789abc","includeArchived":mode==Mode::Delete})
        );
    }
}
#[tokio::test]
async fn required_detail_shapes_and_false_mutations_fail_with_exact_single_context() {
    for mode in [Mode::Archive, Mode::Delete] {
        for detail in [
            json!({"data":{"initiative":{"id":"i","slugId":"s","name":17}}}),
            json!({"data":{"initiative":{"id":"i"}}}),
        ] {
            let (transport, worker) =
                server(vec![Reply::data(detail)], Duration::from_secs(2), 65536);
            let error = command::fetch_single(&transport, &uuid(1), mode)
                .await
                .unwrap_err();
            assert_eq!(
                error.context.as_deref(),
                Some("Failed to fetch initiative details")
            );
            assert_eq!(worker.join().unwrap().len(), 1);
        }
        let field = if mode == Mode::Archive {
            "initiativeArchive"
        } else {
            "initiativeDelete"
        };
        let (transport, worker) = server(
            vec![Reply::data(json!({"data":{field:{"success":false}}}))],
            Duration::from_secs(2),
            65536,
        );
        let error = command::submit_single(&transport, &uuid(1), "", mode)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("{}: {}", mode.context(), mode.context())
        );
        worker.join().unwrap();
        let (transport, worker) = server(
            vec![Reply::data(json!({"data":{field:{}}}))],
            Duration::from_secs(2),
            65536,
        );
        assert!(
            command::submit_single(&transport, &uuid(1), "", mode)
                .await
                .is_err()
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn legal_blank_single_fields_and_optional_nulls_are_preserved() {
    for mode in [Mode::Archive, Mode::Delete] {
        let node = if mode == Mode::Archive {
            json!({"id":"","slugId":"","name":"","archivedAt":""})
        } else {
            json!({"id":"","slugId":"","name":"","projects":null})
        };
        let (transport, worker) = server(
            vec![Reply::data(json!({"data":{"initiative":node}}))],
            Duration::from_secs(2),
            65536,
        );
        let detail = command::fetch_single(&transport, &uuid(1), mode)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.name(), "");
        assert!(!detail.already_archived());
        assert_eq!(detail.linked_warning(), None);
        worker.join().unwrap();
    }
}
#[derive(Default)]
struct BatchState {
    requests: Vec<Value>,
    active: HashSet<usize>,
    max_active: usize,
    mutation_replies: HashSet<usize>,
    effects: Vec<usize>,
    records: BTreeMap<usize, bool>,
    violations: Vec<String>,
}
#[tokio::test]
async fn seven_items_prove_five_barrier_order_progress_counts_and_exact_effects() {
    for mode in [Mode::Archive, Mode::Delete] {
        let ids: Vec<_> = (1..=7).map(uuid).collect();
        let initial = BatchState {
            records: (1..=7)
                .map(|index| (index, index == 6 && mode == Mode::Archive))
                .collect(),
            ..BatchState::default()
        };
        let state = Arc::new((Mutex::new(initial), Condvar::new()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
        let state_server = state.clone();
        let server_ids = ids.clone();
        let expected = if mode == Mode::Archive { 13 } else { 14 };
        let worker = thread::spawn(move || {
            let mut handlers = Vec::new();
            for _ in 0..expected {
                let mut stream = accept(&listener);
                let request = read_request(&mut stream);
                let index = server_ids
                    .iter()
                    .position(|id| Some(id.as_str()) == request["variables"]["id"].as_str())
                    .unwrap()
                    + 1;
                let mutation = request["query"].as_str().unwrap().contains("mutation");
                {
                    let mut s = state_server.0.lock().unwrap();
                    if index > 5 && !s.mutation_replies.contains(&5) {
                        s.violations
                            .push("second batch began before fifth mutation completed".into());
                    }
                    if !mutation {
                        s.active.insert(index);
                        s.max_active = s.max_active.max(s.active.len());
                    }
                    s.requests.push(request);
                }
                let state = state_server.clone();
                handlers.push(thread::spawn(move||{
                    if !mutation && index==5 {
                        let s=state.0.lock().unwrap();let(s,timeout)=state.1.wait_timeout_while(s,Duration::from_secs(4),|s|s.mutation_replies.len()<4).unwrap();assert!(!timeout.timed_out());assert_eq!(s.mutation_replies.len(),4);drop(s);thread::sleep(Duration::from_millis(60));
                    }
                    let id=uuid(index);let field=if mode==Mode::Archive{"initiativeArchive"}else{"initiativeDelete"};
                    let body=if index==7 {json!({"errors":[{"message":"raw","extensions":{"userPresentableMessage":"Human line\r\nnext","duplicate":"already exists"}}],"data":{"initiative":{"id":id,"name":"CORRUPT","archivedAt":"already"}}})}
                        else if mutation {json!({"data":{field:{"success":index!=6}}})}
                        else {json!({"data":{"initiative":{"id":id,"name":format!("Name {index}"),"archivedAt":if index==6 && mode==Mode::Archive{Some("2026-09-01")}else{None}}}})};
                    let mut s=state.0.lock().unwrap();reply(&mut stream,200,&body.to_string()).unwrap();
                    if mutation {s.active.remove(&index);s.mutation_replies.insert(index);if index<=5{s.effects.push(index);s.records.insert(index,true);}state.1.notify_all();}
                    else if index==6 && mode==Mode::Archive {s.active.remove(&index);}
                }));
            }
            for handler in handlers {
                handler.join().unwrap();
            }
            assert!(listener.accept().is_err());
        });
        let transport = transport(&endpoint, Duration::from_secs(5), 65536);
        let mut progress = Vec::new();
        let results = command::execute(
            &transport,
            ids.iter().map(|id| target(id)).collect(),
            mode,
            |event| {
                progress.push(event);
                Ok(())
            },
        )
        .await
        .unwrap();
        worker.join().unwrap();
        assert_eq!(
            results.iter().map(|row| &row.id).collect::<Vec<_>>(),
            ids.iter().collect::<Vec<_>>()
        );
        assert_eq!(results.len(), 7);
        assert_eq!(progress.len(), 7);
        assert!(progress[..5].iter().all(|p| p.succeeded == 0));
        assert!(progress[5..].iter().all(|p| p.succeeded == 5));
        assert_eq!(progress.last().unwrap().completed, 7);
        let s = state.0.lock().unwrap();
        assert!(s.violations.is_empty(), "{:?}", s.violations);
        assert_eq!(s.max_active, 5);
        assert!(s.active.is_empty());
        assert_eq!(s.requests.len(), expected);
        let mut effects = s.effects.clone();
        effects.sort();
        assert_eq!(effects, vec![1, 2, 3, 4, 5]);
        assert_eq!(
            s.records,
            (1..=7)
                .map(|index| (index, index <= 5 || (index == 6 && mode == Mode::Archive)))
                .collect::<BTreeMap<_, _>>()
        );
        assert_eq!(results[6].name, None);
        assert_eq!(
            results[6].outcome,
            BulkOutcome::Failed("Human line  next".into())
        );
        assert_eq!(
            results.iter().filter(|row| row.succeeded()).count(),
            if mode == Mode::Archive { 6 } else { 5 }
        );
        let (output, failed) = command::summary(&results, mode);
        assert!(failed);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Human line  next"));
        assert!(!output.contains("CORRUPT"));
        assert!(!output.contains("duplicate"));
        if mode == Mode::Delete {
            assert_eq!(
                results[5].outcome,
                BulkOutcome::Failed("Delete operation failed".into())
            );
            assert!(output.find("Name 6").unwrap() < output.find("Human line").unwrap());
        }
    }
}
#[tokio::test]
async fn bulk_false_uses_resolved_id_but_thrown_row_keeps_input_without_name() {
    for mode in [Mode::Archive, Mode::Delete] {
        for throws in [false, true] {
            let id = uuid(1);
            let field = if mode == Mode::Archive {
                "initiativeArchive"
            } else {
                "initiativeDelete"
            };
            let mutation = if throws {
                json!({"errors":[{"message":"raw","extensions":{"userPresentableMessage":"Friendly"}}],"data":{"partial":"duplicate"}})
            } else {
                json!({"data":{field:{"success":false}}})
            };
            let (transport, worker) = server(
                vec![
                    Reply::data(
                        json!({"data":{"initiatives":{"nodes":[{"id":id,"slugId":"target"}]}}}),
                    ),
                    Reply::data(
                        json!({"data":{"initiative":{"id":id,"name":"Display name","archivedAt":null}}}),
                    ),
                    Reply::data(mutation),
                ],
                Duration::from_secs(2),
                65536,
            );
            let row = command::run_item(&transport, target("Original input"), mode).await;
            if throws {
                assert_eq!(row.id, "Original input");
                assert_eq!(row.name, None);
                assert_eq!(row.outcome, BulkOutcome::Failed("Friendly".into()));
            } else {
                assert_eq!(row.id, id);
                assert_eq!(row.name, Some("Display name".into()));
                assert_eq!(
                    row.outcome,
                    BulkOutcome::Failed(
                        if mode == Mode::Archive {
                            "Archive operation failed"
                        } else {
                            "Delete operation failed"
                        }
                        .into()
                    )
                );
            }
            assert_eq!(worker.join().unwrap().len(), 3);
        }
    }
}
