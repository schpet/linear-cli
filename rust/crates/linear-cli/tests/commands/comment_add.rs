use std::fs;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::net::TcpListener;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(target_os = "linux")]
use std::{
    thread,
    time::{Duration, Instant},
};

use linear_cli::commands::comment_add::{self, CommentTarget};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::comment_create::{AddComment, GetDocumentCommentTarget};
use linear_cli::platform::prompt::{PromptOutcome, PromptSession};
use serde_json::{Value, json};

const P: &str = "00000000-0000-4000-9000-000000002801";
const I: &str = "00000000-0000-4000-9000-000000004401";
const D: &str = "00000000-0000-4000-9000-000000005501";
#[cfg(target_os = "linux")]
const CREATED: &str = r#"{"data":{"commentCreate":{"success":true,"comment":{"id":"c1","url":"https://linear.app/acme/comment/c1"}}}}"#;

fn compact(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != ',')
        .collect()
}

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "linear-comment-add-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn body_flags_conflict_before_file_io_and_keep_supplied_text_exactly() {
    let error = comment_add::resolve_body(Some("x"), Some("/definitely/missing")).unwrap_err();
    assert_eq!(
        error.message(),
        "Cannot specify both --body and --body-file"
    );
    let literal = "  **Bold** `code`\nline two ☃\t ";
    assert_eq!(
        comment_add::resolve_body(Some(literal), None).unwrap(),
        Some(literal.to_owned())
    );
    for blank in [" \t\n", "\u{a0}\u{3000}", "\u{2028}"] {
        let error = comment_add::resolve_body(Some(blank), None).unwrap_err();
        assert_eq!(error.message(), "Comment body cannot be empty");
        assert_eq!(
            error.hint(),
            Some("Pass text with --body, or omit it to be prompted.")
        );
    }
    assert_eq!(comment_add::resolve_body(None, None).unwrap(), None);
}

#[test]
fn body_files_strip_a_bom_and_reject_invalid_or_unreadable_files() {
    let dir = scratch();
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    };
    let text = write(
        "bom-text.md",
        b"\xef\xbb\xbf# Title\r\n\n  \xe2\x98\x83 *md* \n",
    );
    assert_eq!(
        comment_add::resolve_body(None, Some(&text)).unwrap(),
        Some("# Title\r\n\n  ☃ *md* \n".to_owned())
    );
    let bom = write("bom.md", b"\xef\xbb\xbf \n");
    let error = comment_add::resolve_body(None, Some(&bom)).unwrap_err();
    assert_eq!(error.message(), format!("Body file is empty: {bom}"));
    assert_eq!(
        error.hint(),
        Some("Write the comment into the file, or use --body.")
    );
    for (name, bytes) in [
        ("truncated.md", &b"x\xe2\x82"[..]),
        ("surrogate.md", b"a\xed\xa0\x80b"),
        ("overlong.md", b"c\xc0\xafd"),
        ("lone-continuation.md", b"\x80"),
    ] {
        let path = write(name, bytes);
        let error = comment_add::resolve_body(None, Some(&path)).unwrap_err();
        assert_eq!(error.message(), "Body file must be valid UTF-8", "{name}");
        assert_eq!(
            error.hint().map(str::to_owned),
            Some(format!("Re-save {path} as UTF-8 text, or use --body."))
        );
    }
    let missing = dir.join("missing.md");
    let missing = missing.to_str().unwrap();
    let error = comment_add::resolve_body(None, Some(missing)).unwrap_err();
    assert_eq!(
        error.message(),
        format!("Failed to read body file: {missing}")
    );
    assert_eq!(
        error.hint(),
        Some("Error: No such file or directory (os error 2)")
    );
    let error = comment_add::resolve_body(None, Some(dir.to_str().unwrap())).unwrap_err();
    assert_eq!(error.hint(), Some("Error: Is a directory (os error 21)"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn input_sets_exactly_one_target_and_omits_absent_optionals() {
    let variables = |target, parent: Option<&str>, id: Option<&str>| {
        let input = comment_add::build_input(target, "Body".into(), parent, id).unwrap();
        serde_json::to_value(comment_add::request(input)).unwrap()["variables"]["input"].clone()
    };
    assert_eq!(
        variables(
            CommentTarget::Issue {
                issue_id: "i".into()
            },
            None,
            None
        ),
        json!({"body":"Body","issueId":"i"})
    );
    assert_eq!(
        variables(
            CommentTarget::Document {
                document_content_id: "".into()
            },
            Some(""),
            None
        ),
        json!({"body":"Body","parentId":"","documentContentId":""})
    );
    assert_eq!(
        variables(
            CommentTarget::Project {
                project_id: P.into()
            },
            Some("p"),
            Some("fixed")
        ),
        json!({"body":"Body","parentId":"p","id":"fixed","projectId":P})
    );
    assert_eq!(
        variables(
            CommentTarget::Initiative {
                initiative_id: I.into()
            },
            None,
            None
        ),
        json!({"body":"Body","initiativeId":I})
    );
    let wire = serde_json::to_value(comment_add::request(
        comment_add::build_input(
            CommentTarget::Project {
                project_id: P.into(),
            },
            "x".into(),
            None,
            None,
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        compact(wire["query"].as_str().unwrap()),
        compact(
            "mutation AddComment($input: CommentCreateInput!) { commentCreate(input: $input) { success comment { id url } } }"
        )
    );
    assert_eq!(wire["operationName"], "AddComment");
}

#[test]
fn parent_comment_links_get_specific_guidance_before_other_linear_urls() {
    let target = || CommentTarget::Project {
        project_id: P.into(),
    };
    let link = "https://linear.app/acme/issue/ENG-1/title#comment-abcdef12";
    let error = comment_add::build_input(target(), "x".into(), Some(link), None).unwrap_err();
    assert_eq!(
        error.message(),
        format!(
            "\"{link}\" links to a comment, but a comment URL only carries the first eight characters of its ID."
        )
    );
    let url = "https://linear.app/acme/issue/ENG-1";
    let error = comment_add::build_input(target(), "x".into(), Some(url), None).unwrap_err();
    assert_eq!(
        error.message(),
        format!("\"{url}\" is a Linear URL, and this command does not take one.")
    );
    assert_eq!(
        error.hint(),
        Some("Pass the UUID of the comment to reply to.")
    );
}

#[test]
fn schema_non_null_payload_fields_decode_strictly_and_only_null_content_id_is_absent() {
    for body in [
        json!({"data":{"commentCreate":{"success":true,"comment":null}}}),
        json!({"data":{"commentCreate":{"success":true}}}),
        json!({"data":{"commentCreate":{"success":true,"comment":{"id":"c1"}}}}),
        json!({"data":{"commentCreate":{"comment":{"id":"c1","url":"u"}}}}),
    ] {
        let error = parse_response::<AddComment>(&serde_json::to_vec(&body).unwrap()).unwrap_err();
        assert!(
            error.to_string().contains("expected operation shape"),
            "{error}"
        );
    }
    let decode = |document: Value| {
        parse_response::<GetDocumentCommentTarget>(
            &serde_json::to_vec(&json!({"data":{"document":document}})).unwrap(),
        )
    };
    assert_eq!(
        decode(json!({"id":D,"title":"T","documentContentId":null}))
            .unwrap()
            .document
            .document_content_id,
        None
    );
    // A selected field must be present even when nullable; omission is malformed.
    let error = decode(json!({"id":D,"title":"T"})).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing field `documentContentId`"),
        "{error}"
    );
    assert!(decode(Value::Null).is_err());
}

#[test]
fn prompt_answer_is_trimmed_and_blank_fails_after_submission() {
    let mut output = Vec::new();
    let mut session = PromptSession::script(&b"  Prompted body \n"[..], &mut output);
    let outcome = comment_add::prompt_body(&mut session).unwrap();
    session.close().unwrap();
    assert_eq!(
        outcome,
        PromptOutcome::Submitted("Prompted body".to_owned())
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "? Comment body\n? Comment body › Prompted body\n"
    );
    let error = comment_add::require_prompted(String::new()).unwrap_err();
    assert_eq!(error.message(), "Comment body cannot be empty");
    assert_eq!(error.hint(), None);
}

/// Serve `replies` in order, then prove no further request arrives.
#[cfg(target_os = "linux")]
fn server(replies: Vec<String>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for body in replies {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "expected request never arrived");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("failed to accept request: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut input = Vec::new();
            let request = loop {
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
                        break serde_json::from_str::<Value>(payload).unwrap();
                    }
                }
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            requests.push(request);
        }
        thread::sleep(Duration::from_millis(300));
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err(), "unexpected extra request");
        requests
    });
    (endpoint, worker)
}

#[cfg(target_os = "linux")]
struct Run {
    output: Output,
    requests: Vec<Value>,
}

#[cfg(target_os = "linux")]
fn run(args: &[&str], stdin: &[u8], key: bool, files: &[(&str, &[u8])], replies: &[&str]) -> Run {
    let dir = scratch();
    for (name, bytes) in files {
        fs::write(dir.join(name), bytes).unwrap();
    }
    let (endpoint, worker) = server(replies.iter().map(|body| (*body).to_owned()).collect());
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command
        .args(args)
        .env_clear()
        .current_dir(&dir)
        .env("HOME", &dir)
        .env("XDG_CONFIG_HOME", &dir)
        .env("PATH", &dir)
        .env("NO_COLOR", "1")
        .env("LINEAR_IGNORE_ENV_FILE", "1")
        .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if key {
        command.env("LINEAR_API_KEY", "lin_api_fake");
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    let requests = worker.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
    Run { output, requests }
}

#[cfg(target_os = "linux")]
fn assert_failure(run: &Run, stdout: &str, stderr: &str) {
    assert_eq!(run.output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&run.output.stdout), stdout);
    assert_eq!(String::from_utf8_lossy(&run.output.stderr), stderr);
}

#[cfg(target_os = "linux")]
fn operation(request: &Value) -> &str {
    request["operationName"].as_str().unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn empty_project_name_falls_back_to_first_slug_then_not_found() {
    // The second slug result must never replace the empty first result.
    let result = run(
        &["project", "comment", "add", "Roadmap", "-b", "Hi"],
        b"",
        true,
        &[],
        &[
            r#"{"data":{"projects":{"nodes":[{"id":""}]}}}"#,
            r#"{"data":{"projects":{"nodes":[{"id":""},{"id":"must-not-use"}]}}}"#,
        ],
    );
    assert_failure(
        &result,
        "",
        "✗ Failed to add comment: Project not found: Roadmap\n  Pass a project UUID, slug ID (from `linear project list`), or exact project name.\n",
    );
    assert_eq!(result.requests.len(), 2);
    assert_eq!(operation(&result.requests[0]), "GetProjectIdByName");
    assert_eq!(result.requests[0]["variables"], json!({"name":"Roadmap"}));
    assert_eq!(operation(&result.requests[1]), "GetProjectIdBySlugId");
    assert_eq!(result.requests[1]["variables"], json!({"slugId":"Roadmap"}));
}

#[cfg(target_os = "linux")]
#[test]
fn empty_project_url_id_is_not_found_without_mutation() {
    let target = "https://linear.app/acme/project/roadmap-0000000028ff";
    let result = run(
        &["project", "comment", "add", target, "-b", "Hi"],
        b"",
        true,
        &[],
        &[r#"{"data":{"projects":{"nodes":[{"id":""}]}}}"#],
    );
    assert_failure(
        &result,
        "",
        &format!(
            "✗ Failed to add comment: Project not found: {target}\n  Pass a project UUID, slug ID (from `linear project list`), or exact project name.\n"
        ),
    );
    assert_eq!(result.requests.len(), 1);
    assert_eq!(operation(&result.requests[0]), "GetProjectIdBySlugId");
    assert_eq!(
        result.requests[0]["variables"],
        json!({"slugId":"0000000028ff"})
    );
}

#[cfg(target_os = "linux")]
#[test]
fn empty_initiative_plain_slug_falls_back_to_name_and_preserves_empty_name_id() {
    for id in [I, ""] {
        let name =
            json!({"data":{"initiatives":{"nodes":[{"id":id,"name":"Growth","slugId":"growth"}]}}})
                .to_string();
        let result = run(
            &["initiative", "comment", "add", "Growth", "-b", "Hi"],
            b"",
            true,
            &[],
            &[
                r#"{"data":{"initiatives":{"nodes":[{"id":""},{"id":"must-not-use"}]}}}"#,
                &name,
                CREATED,
            ],
        );
        assert_eq!(result.output.status.code(), Some(0));
        assert_eq!(
            result.output.stdout,
            "✓ Comment added to initiative Growth\nhttps://linear.app/acme/comment/c1\n".as_bytes()
        );
        assert!(result.output.stderr.is_empty());
        assert_eq!(result.requests.len(), 3);
        assert_eq!(operation(&result.requests[0]), "ResolveInitiativeBySlug");
        assert_eq!(
            result.requests[0]["variables"],
            json!({"slugId":"Growth","includeArchived":false})
        );
        assert_eq!(operation(&result.requests[1]), "ResolveInitiativeByName");
        assert_eq!(result.requests[1]["variables"], json!({"name":"Growth"}));
        assert_eq!(operation(&result.requests[2]), "AddComment");
        assert_eq!(
            result.requests[2]["variables"],
            json!({"input":{"body":"Hi","initiativeId":id}})
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn empty_initiative_url_id_reaches_one_comment_mutation() {
    let target = "https://linear.app/acme/initiative/growth-0000000044ff";
    let result = run(
        &["initiative", "comment", "add", target, "-b", "Hi"],
        b"",
        true,
        &[],
        &[r#"{"data":{"initiatives":{"nodes":[{"id":""}]}}}"#, CREATED],
    );
    assert_eq!(result.output.status.code(), Some(0));
    assert_eq!(
        result.output.stdout,
        format!("✓ Comment added to initiative {target}\nhttps://linear.app/acme/comment/c1\n")
            .as_bytes()
    );
    assert!(result.output.stderr.is_empty());
    assert_eq!(result.requests.len(), 2);
    assert_eq!(operation(&result.requests[0]), "ResolveInitiativeBySlug");
    assert_eq!(
        result.requests[0]["variables"],
        json!({"slugId":"0000000044ff","includeArchived":false})
    );
    assert_eq!(operation(&result.requests[1]), "AddComment");
    assert_eq!(
        result.requests[1]["variables"],
        json!({"input":{"body":"Hi","initiativeId":""}})
    );
}

#[cfg(target_os = "linux")]
#[test]
fn invalid_body_file_fails_before_target_lookup_for_every_caller() {
    let stderr = "✗ Failed to add comment: Body file must be valid UTF-8\n  Re-save bad.md as UTF-8 text, or use --body.\n";
    for args in [
        &[
            "project",
            "comment",
            "add",
            "https://linear.app/acme/project/roadmap-0000000028ff",
        ][..],
        &["initiative", "comment", "add", "Growth"],
        &["document", "comment", "add", D],
    ] {
        let mut args = args.to_vec();
        args.extend(["--body-file", "bad.md"]);
        let run = run(&args, b"", true, &[("bad.md", b"c\xc0\xafd")], &[]);
        assert_failure(&run, "", stderr);
        assert!(run.requests.is_empty());
    }
}
