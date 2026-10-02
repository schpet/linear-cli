//! Actual public CLI request-order and handled-error contracts, synthetic API only.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Reply {
    operation: &'static str,
    status: u16,
    mime: &'static str,
    body: String,
}
impl Reply {
    fn data(operation: &'static str, data: Value) -> Self {
        Self {
            operation,
            status: 200,
            mime: "application/json",
            body: json!({"data":data}).to_string(),
        }
    }
}
struct Server {
    endpoint: String,
    done: Arc<AtomicBool>,
    worker: thread::JoinHandle<Vec<Value>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&done);
        let worker = thread::spawn(move || {
            let mut requests = vec![];
            let start = Instant::now();
            while !stop.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(8),
                            "CLI did not complete"
                        );
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted mock stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = vec![];
                let request = loop {
                    let mut chunk = [0; 8192];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    let text = std::str::from_utf8(&bytes).unwrap();
                    if let Some((header, body)) = text.split_once("\r\n\r\n") {
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        if body.len() >= length {
                            break serde_json::from_str::<Value>(&body[..length]).unwrap();
                        }
                    }
                };
                let index = requests.len();
                requests.push(request);
                let fallback = Reply {
                    operation: "UNEXPECTED",
                    status: 200,
                    mime: "application/json",
                    body: "{\"errors\":[{\"message\":\"unexpected additional request\"}]}"
                        .to_owned(),
                };
                let reply = replies.get(index).unwrap_or(&fallback);
                if reply.operation != "UNEXPECTED" {
                    assert_eq!(requests[index]["operationName"], reply.operation);
                }
                write!(stream,"HTTP/1.1 {} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.status,reply.mime,reply.body.len(),reply.body).unwrap();
            }
            assert!(
                requests.len() >= replies.len(),
                "missing source resolver prefix"
            );
            requests
        });
        Self {
            endpoint,
            done,
            worker,
        }
    }
    fn finish(self) -> Vec<Value> {
        self.done.store(true, Ordering::SeqCst);
        self.worker.join().unwrap()
    }
}
fn scope() -> Reply {
    Reply::data(
        "ResolveTeam",
        json!({"teams":{"nodes":[{"id":"team-id","key":"ENG","name":"Engineering"}]}}),
    )
}
fn state() -> Reply {
    Reply::data(
        "GetWorkflowStatesInScope",
        json!({"workflowStates":{"nodes":[{"id":"state-id","name":"Ready","type":"unstarted","team":{"key":"ENG"}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}),
    )
}
fn project() -> Reply {
    Reply::data(
        "GetProjectIdByName",
        json!({"projects":{"nodes":[{"id":"project-id"}]}}),
    )
}
fn cycle() -> Reply {
    Reply::data(
        "GetTeamCyclesForLookup",
        json!({"team":{"key":"ENG","cyclesEnabled":true,"cycles":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}},"activeCycle":{"id":"cycle-id","number":7,"name":"Current"}}}),
    )
}
fn milestone() -> Reply {
    Reply::data(
        "GetProjectMilestonesForLookup",
        json!({"project":{"projectMilestones":{"nodes":[{"id":"milestone-id","name":"M1"}]}}}),
    )
}
fn user() -> Reply {
    Reply::data(
        "LookupUser",
        json!({"users":{"nodes":[{"id":"user-id","email":"user@example.invalid","displayName":"Dummy","name":"dummy"}]}}),
    )
}
fn invoke(server: &Server, args: &[&str], sort: Option<&str>) -> std::process::Output {
    let sandbox = super::startup::BinarySandbox::new();
    let mut command = sandbox.command();
    command
        .env("LINEAR_API_KEY", "lin_api_fake")
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_GRAPHQL_ENDPOINT", &server.endpoint)
        .env("NO_COLOR", "1")
        .env("TZ", "UTC");
    if let Some(sort) = sort {
        command.env("LINEAR_ISSUE_SORT", sort);
    }
    command.args(args).output().unwrap()
}
#[test]
fn query_invalid_configured_sort_is_deferred_after_exact_resolver_prefix() {
    let server = Server::new(vec![scope(), state(), project(), cycle(), milestone()]);
    let output = invoke(
        &server,
        &[
            "issue",
            "query",
            "--team",
            "eng",
            "--state",
            "Ready",
            "--project",
            "Plan",
            "--cycle",
            "active",
            "--milestone",
            "M1",
            "--assignee",
            "dummy",
            "--created-after",
            "yesterday",
            "--json",
        ],
        Some("invalid"),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "✗ Failed to query issues: Invalid issue sort: \"invalid\"\n  Use one of: manual, priority (via --sort, the issue_sort config option, or the LINEAR_ISSUE_SORT environment variable)\n"
    );
    let requests = server.finish();
    assert_eq!(
        requests
            .iter()
            .map(|r| r["operationName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "ResolveTeam",
            "GetWorkflowStatesInScope",
            "GetProjectIdByName",
            "GetTeamCyclesForLookup",
            "GetProjectMilestonesForLookup"
        ]
    );
    assert_eq!(
        requests[3]["variables"],
        json!({"teamId":"team-id","after":null})
    );
}
#[test]
fn mine_configured_sort_fails_before_explicit_team_but_other_routes_stay_eager() {
    let server = Server::new(vec![]);
    let output = invoke(
        &server,
        &["issue", "mine", "--team", "eng"],
        Some("invalid"),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("✗ Failed to list issues: Invalid issue sort: \"invalid\"\n")
    );
    assert!(server.finish().is_empty());
    let sandbox = super::startup::BinarySandbox::new();
    let output = sandbox
        .command()
        .env("LINEAR_ISSUE_SORT", "invalid")
        .args(["team", "list", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("invalid config option LINEAR_ISSUE_SORT")
    );
}
#[test]
fn date_failures_drop_only_final_issue_read_after_source_resolvers() {
    for mode in ["mine", "query", "search"] {
        let mut replies = if mode == "mine" {
            vec![scope(), project(), cycle(), milestone(), state()]
        } else {
            vec![scope(), state(), project(), cycle(), milestone(), user()]
        };
        let expected = replies.iter().map(|r| r.operation).collect::<Vec<_>>();
        let server = Server::new(std::mem::take(&mut replies));
        let mut args = vec![
            "issue",
            if mode == "mine" { "mine" } else { "query" },
            "--team",
            "eng",
            "--state",
            "Ready",
            "--project",
            "Plan",
            "--cycle",
            "active",
            "--milestone",
            "M1",
            "--created-after",
            "2026-02-30",
            "--updated-after",
            "yesterday",
        ];
        if mode != "mine" {
            args.extend(["--assignee", "dummy", "--json"]);
        }
        if mode == "search" {
            args.extend(["--search", " term "]);
        }
        let output = invoke(&server, &args, None);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("Invalid date for --created-after: \"2026-02-30\""));
        assert!(!stderr.contains("--updated-after:"));
        let requests = server.finish();
        assert_eq!(
            requests
                .iter()
                .map(|r| r["operationName"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
#[test]
fn earlier_resolver_failure_precedes_dates_and_query_search_sort_override() {
    let server = Server::new(vec![Reply {
        operation: "ResolveTeam",
        status: 200,
        mime: "application/json",
        body: "{\"errors\":[{\"message\":\"resolver failed\"}]}".to_owned(),
    }]);
    let output = invoke(
        &server,
        &[
            "issue",
            "query",
            "--team",
            "eng",
            "--created-after",
            "yesterday",
            "--json",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "✗ Failed to query issues: resolver failed\n"
    );
    assert_eq!(server.finish().len(), 1);
    let server = Server::new(vec![Reply::data(
        "SearchIssues",
        json!({"searchIssues":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null},"totalCount":0}}),
    )]);
    let output = invoke(
        &server,
        &[
            "issue",
            "query",
            "--all-teams",
            "--search",
            " term ",
            "--json",
        ],
        Some("invalid"),
    );
    assert!(output.status.success());
    let requests = server.finish();
    assert_eq!(requests[0]["variables"], json!({"term":"term","first":50}));
}
#[test]
fn handled_raw_error_fallback_keeps_empty_first_and_nonjson_body_without_class_prefix() {
    for (status, mime, body) in [
        (
            200,
            "application/json",
            "{\"errors\":[{\"message\":\"\"},{\"message\":\"boom\"}],\"extensions\":{\"value\":9007199254740993}}",
        ),
        (500, "text/plain", "raw fixture failure"),
    ] {
        let server = Server::new(vec![Reply {
            operation: "GetIssuesForQuery",
            status,
            mime,
            body: body.to_owned(),
        }]);
        let output = invoke(&server, &["issue", "query", "--all-teams", "--json"], None);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("✗ Failed to query issues: "));
        assert!(stderr.contains("request") && stderr.contains("GetIssuesForQuery"));
        assert!(stderr.contains(if status == 500 {
            "raw fixture failure"
        } else {
            "boom"
        }));
        assert!(!stderr.contains("ClientError:"));
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn default_team_cycle_url_error_follows_one_team_lookup() {
    let server = Server::new(vec![scope()]);
    let output = invoke(
        &server,
        &[
            "issue",
            "mine",
            "--cycle",
            "https://linear.app/dummy/issue/ENG-1/wrong",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.starts_with("✗ Failed to list issues: "), "{error}");
    assert!(error.contains("cycle URL, number, or name"), "{error}");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["operationName"], "ResolveTeam");
    assert_eq!(
        requests[0]["variables"],
        json!({"reference":"ENG","id":null,"isUuid":false})
    );
}
