use linear_cli::commands::{comment_add, issue_upload, upload};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::upload::{AttachmentCreate, FileUpload, UploadFileHeader};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "linear-upload-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
fn file(name: &str, mime: &'static str, public: bool) -> upload::UploadedFile {
    upload::UploadedFile {
        file: upload::PreparedFile {
            filename: name.into(),
            size: 13,
            content_type: mime,
            public,
        },
        asset_url: "https://uploads.linear.app/fake/asset".into(),
    }
}
#[test]
fn comment_id_body_links_and_reply_fields_preserve_source() {
    for id in [
        "123e4567-e89b-42d3-a456-426614174000",
        "123E4567-E89B-42D3-B456-426614174000",
    ] {
        issue_upload::validate_comment_id(Some(id)).unwrap();
    }
    for id in [
        "",
        "uuid",
        "123e4567-e89b-12d3-a456-426614174000",
        "123e4567-e89b-42d3-c456-426614174000",
    ] {
        assert!(issue_upload::validate_comment_id(Some(id)).is_err());
    }
    let files = vec![
        file("x.png", "image/png", false),
        file("n.txt", "text/plain", false),
    ];
    let body = issue_upload::compose_body(Some(" Text\n "), &files);
    assert_eq!(
        body,
        " Text\n \n\n![x.png](https://uploads.linear.app/fake/asset)\n[n.txt](https://uploads.linear.app/fake/asset)"
    );
    let input = comment_add::build_input(
        comment_add::CommentTarget::Issue {
            issue_id: "ENG-1".into(),
        },
        body,
        Some("parent"),
        Some("123e4567-e89b-42d3-a456-426614174000"),
    )
    .unwrap();
    let value = serde_json::to_value(comment_add::request(input)).unwrap();
    assert_eq!(value["variables"]["input"]["issueId"], "ENG-1");
    assert_eq!(value["variables"]["input"]["parentId"], "parent");
    assert!(value["variables"]["input"].get("projectId").is_none());
    assert_eq!(
        issue_upload::comment_output("ENG-1", "url"),
        "✓ Comment added to ENG-1\nurl\n".as_bytes()
    );
}
#[test]
fn all_paths_and_public_types_prevalidate_but_sizes_wait_for_upload() {
    let dir = scratch();
    let png = dir.join("Picture.PNG");
    std::fs::write(&png, b"pixel").unwrap();
    let txt = dir.join("note.txt");
    std::fs::write(&txt, b"note").unwrap();
    let large = dir.join("large.png");
    std::fs::File::create(&large)
        .unwrap()
        .set_len(upload::MAX_FILE_SIZE + 1)
        .unwrap();
    let strings = |paths: Vec<&Path>| {
        paths
            .iter()
            .map(|x| x.to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert!(
        upload::prevalidate(&strings(vec![&png, &dir.join("missing")]), false)
            .unwrap_err()
            .message
            .starts_with("File not found:")
    );
    assert_eq!(
        upload::prevalidate(&strings(vec![&png, &txt]), true)
            .unwrap_err()
            .message,
        "Cannot upload text/plain to a public URL"
    );
    upload::prevalidate(&strings(vec![&png, &large]), true).unwrap();
    assert_eq!(
        upload::prepare(&large, true).unwrap_err().message,
        "File too large: 100.00MB exceeds limit of 100MB"
    );
    let f = std::fs::File::create(dir.join("limit.bin")).unwrap();
    f.set_len(upload::MAX_FILE_SIZE).unwrap();
    assert_eq!(
        upload::prepare(&dir.join("limit.bin"), false).unwrap().size,
        104857600
    );
    assert!(upload::validate_file(&dir).is_err());
    assert_eq!(upload::mime_type(&png), "image/png");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn mime_and_public_policy_table_is_complete_for_rasters_and_documents() {
    for (name, expected, allowed) in [
        ("x.JPEG", "image/jpeg", true),
        ("x.webp", "image/webp", true),
        ("x.TIF", "image/tiff", true),
        ("x.svg", "image/svg+xml", false),
        ("x.ico", "image/x-icon", false),
        (
            "x.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            false,
        ),
        ("x.tsx", "text/typescript", false),
        ("x.unknown", "application/octet-stream", false),
        (".png", "application/octet-stream", false),
    ] {
        assert_eq!(upload::mime_type(Path::new(name)), expected);
        assert_eq!(upload::resolve_public(expected, true).is_ok(), allowed);
        assert!(!upload::resolve_public(expected, false).unwrap());
    }
}
#[test]
fn signed_headers_reproduce_exact_case_record_then_fetch_combining() {
    let h = |key: &str, value: &str| UploadFileHeader {
        key: key.into(),
        value: value.into(),
    };
    let headers = upload::signed_headers(
        "text/plain",
        &[
            h("content-type", "application/x-signed"),
            h("x-token", "first"),
            h("x-token", "last"),
        ],
    )
    .unwrap();
    assert_eq!(headers["content-type"], "application/x-signed");
    assert_eq!(headers["x-token"], "last");
    assert!(!headers.contains_key("authorization"));
    let headers = upload::signed_headers(
        "text/plain",
        &[
            h("Content-Type", "application/x-signed"),
            h("X-Token", " first "),
            h("x-token", "last"),
        ],
    )
    .unwrap();
    assert_eq!(
        headers
            .get_all("content-type")
            .iter()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["text/plain", "application/x-signed"]
    );
    assert_eq!(
        headers
            .get_all("x-token")
            .iter()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["first", "last"]
    );
    for value in [" \t\r\nvalue\r\n\t ", "\rvalue\n", "\t value \t"] {
        let headers = upload::signed_headers("text/plain", &[h("x-trim", value)]).unwrap();
        assert_eq!(
            headers["x-trim"], "value",
            "Fetch trims SP/TAB/CR/LF at edges"
        );
    }
    for pair in [
        h("bad\nname", "secret"),
        h("token", "secret\r\nnew: header"),
    ] {
        let error = upload::signed_headers("text/plain", &[pair]).unwrap_err();
        assert!(!error.message.contains("secret"));
    }
}
#[test]
fn upload_and_attachment_requests_select_only_source_fields_and_omit_options() {
    let file = file("note.txt", "text/plain", false);
    let query = serde_json::to_value(upload::request(&file.file)).unwrap();
    assert_eq!(
        query["variables"],
        json!({"contentType":"text/plain","filename":"note.txt","size":13,"makePublic":false})
    );
    assert_eq!(query["operationName"], "FileUpload");
    let compact = query["query"]
        .as_str()
        .unwrap()
        .chars()
        .filter(|x| !x.is_whitespace() && *x != ',')
        .collect::<String>();
    assert!(compact.contains("uploadFile{assetUrluploadUrlheaders{keyvalue}}"));
    assert!(!compact.contains("metaData"));
    for title in [None, Some(""), Some("Custom")] {
        let query =
            serde_json::to_value(issue_upload::attach_request("uuid", &file, title, None)).unwrap();
        assert_eq!(
            query["variables"]["input"]["title"],
            title.filter(|x| !x.is_empty()).unwrap_or("note.txt")
        );
        assert!(query["variables"]["input"].get("commentBody").is_none());
    }
}
#[test]
fn upstream_upload_and_attachment_required_shapes_are_strict_but_blanks_legal() {
    for payload in [
        json!({}),
        json!({"success":true,"uploadFile":{"assetUrl":"a","uploadUrl":"u","headers":null}}),
        json!({"success":true,"uploadFile":{"assetUrl":"a","uploadUrl":"u","headers":[{"key":"x"}]}}),
    ] {
        assert!(
            parse_response::<FileUpload>(
                json!({"data":{"fileUpload":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
    let x: FileUpload =
        parse_response(br#"{"data":{"fileUpload":{"success":false,"uploadFile":null}}}"#).unwrap();
    assert!(!x.file_upload.success);
    let x:FileUpload=parse_response(br#"{"data":{"fileUpload":{"success":true,"uploadFile":{"assetUrl":"","uploadUrl":"","headers":[]}}}}"#).unwrap();
    assert_eq!(x.file_upload.upload_file.unwrap().asset_url, "");
    assert!(
        parse_response::<AttachmentCreate>(
            br#"{"data":{"attachmentCreate":{"success":true,"attachment":null}}}"#
        )
        .is_err()
    );
    let x:AttachmentCreate=parse_response(br#"{"data":{"attachmentCreate":{"success":true,"attachment":{"id":"","url":"","title":""}}}}"#).unwrap();
    assert!(x.attachment_create.attachment.title.is_empty());
}
#[test]
fn sidebar_output_uses_returned_title_and_quotes_inline_hint_path() {
    let attachment = linear_cli::graphql::operations::upload::CreatedAttachment {
        id: cynic::Id::new("id"),
        url: "url".into(),
        title: "API title".into(),
    };
    let file = file("a'b image.png", "image/png", true);
    let output = String::from_utf8(issue_upload::attach_output(
        &attachment,
        "ENG-1",
        "a'b image.png",
        &file,
    ))
    .unwrap();
    assert!(output.starts_with("✓ Sidebar link attachment created: API title\nurl\n"));
    assert!(
        output.ends_with("linear issue comment add ENG-1 --attach 'a'\\''b image.png' --public\n")
    );
    assert_eq!(issue_upload::quote_shell("safe/a.txt"), "safe/a.txt");
}

use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
    TransportConfig,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};
#[derive(Debug)]
struct Seen {
    method: String,
    path: String,
    header_pairs: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Seen {
    fn header_values(&self, name: &str) -> Vec<&str> {
        self.header_pairs
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
    fn contains_header(&self, name: &str) -> bool {
        self.header_pairs
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    }
}
#[derive(Clone)]
enum Reply {
    Json(serde_json::Value),
    Upload {
        false_success: bool,
    },
    UploadConfigured {
        headers: Vec<(&'static str, &'static str)>,
        fragment: &'static str,
    },
    SlowPut,
    Put {
        status: u16,
        location: Option<&'static str>,
    },
}
fn receive(mut stream: &TcpStream, slow_body: bool) -> Seen {
    stream
        .set_nonblocking(false)
        .expect("blocking accepted mock stream");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut input = Vec::new();
    let (header_end, size) = loop {
        let mut buffer = [0; 4096];
        let n = if slow_body {
            // Leave every PUT body byte for the controlled three-chunk reader.
            stream.read(&mut buffer[..1]).unwrap()
        } else {
            stream.read(&mut buffer).unwrap()
        };
        assert!(n > 0);
        input.extend_from_slice(&buffer[..n]);
        assert!(input.len() < 65536);
        if let Some(at) = input.windows(4).position(|x| x == b"\r\n\r\n") {
            let head = std::str::from_utf8(&input[..at]).unwrap();
            let size = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            break (at + 4, size);
        }
    };
    while input.len() - header_end < size {
        let mut buffer = [0; 4096];
        let remaining = size - (input.len() - header_end);
        let chunk_limit = if slow_body {
            thread::sleep(Duration::from_secs(1));
            remaining.min(size.div_ceil(3))
        } else {
            buffer.len().min(remaining)
        };
        let n = stream.read(&mut buffer[..chunk_limit]).unwrap();
        assert!(n > 0);
        input.extend_from_slice(&buffer[..n]);
        assert!(input.len() < 65536);
    }
    let head = std::str::from_utf8(&input[..header_end]).unwrap();
    let mut lines = head.lines();
    let parts = lines.next().unwrap().split_whitespace().collect::<Vec<_>>();
    let mut header_pairs = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            header_pairs.push((name.to_owned(), value.trim().to_owned()));
        }
    }
    Seen {
        method: parts[0].into(),
        path: parts[1].into(),
        header_pairs,
        body: input[header_end..header_end + size].to_vec(),
    }
}
fn script(
    replies: Vec<Reply>,
    remove_after_metadata: Option<PathBuf>,
) -> (GraphQlTransport, thread::JoinHandle<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let origin = endpoint.clone();
    let worker = thread::spawn(move || {
        let mut rows = Vec::new();
        for reply in replies {
            let started = Instant::now();
            let stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            started.elapsed() < Duration::from_secs(3),
                            "expected request timed out"
                        );
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            };
            let mut stream = stream;
            let row = receive(&stream, matches!(&reply, Reply::SlowPut));
            let json_reply = matches!(
                &reply,
                Reply::Json(_) | Reply::Upload { .. } | Reply::UploadConfigured { .. }
            );
            let (status, location, body) = match reply {
                Reply::Json(value) => (200, None, value.to_string().into_bytes()),
                Reply::Upload { false_success } => {
                    if let Some(path) = &remove_after_metadata {
                        std::fs::remove_file(path).unwrap();
                    }
                    (200,None,json!({"data":{"fileUpload":{"success":!false_success,"uploadFile":if false_success{serde_json::Value::Null}else{json!({"assetUrl":"https://uploads.linear.app/fake/asset","uploadUrl":format!("{origin}/signed?token=fake"),"headers":[{"key":"content-type","value":"application/x-signed"},{"key":"x-token","value":"fake"}]})}}}}).to_string().into_bytes())
                }
                Reply::UploadConfigured { headers, fragment } => {
                    let headers = headers
                        .into_iter()
                        .map(|(key, value)| json!({"key":key,"value":value}))
                        .collect::<Vec<_>>();
                    let data = json!({"data":{"fileUpload":{"success":true,"uploadFile":{
                        "assetUrl":"https://uploads.linear.app/fake/asset",
                        "uploadUrl":format!("{origin}/signed?token=fake#{fragment}"),
                        "headers":headers
                    }}}});
                    (200, None, data.to_string().into_bytes())
                }
                Reply::SlowPut => (200, None, Vec::new()),
                Reply::Put { status, location } => (
                    status,
                    location,
                    if status >= 400 {
                        b"upload refused".to_vec()
                    } else {
                        Vec::new()
                    },
                ),
            };
            write!(
                stream,
                "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                if status == 403 { "Forbidden" } else { "OK" },
                body.len()
            )
            .unwrap();
            if json_reply {
                write!(stream, "Content-Type: application/json\r\n").unwrap();
            }
            if let Some(location) = location {
                write!(stream, "Location: {location}\r\n").unwrap();
            }
            stream.write_all(b"\r\n").unwrap();
            stream.write_all(&body).unwrap();
            rows.push(row);
        }
        thread::sleep(Duration::from_millis(20));
        assert!(listener.accept().is_err(), "no retry or extra request");
        rows
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&format!("{endpoint}/graphql")).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, worker)
}
#[tokio::test]
async fn full_issue_upload_pipelines_preserve_raw_duplicate_headers_fragments_redirect_bytes_and_final_mutation()
 {
    for sidebar in [false, true] {
        for differently_cased in [false, true] {
            let dir = scratch();
            let path = dir.join("note.txt");
            let bytes = b"\x00\xffbinary\n";
            std::fs::write(&path, bytes).unwrap();
            let mut replies = Vec::new();
            if sidebar {
                replies.push(Reply::Json(json!({"data":{"issue":{"id":"uuid"}}})));
            }
            let headers = if differently_cased {
                vec![
                    ("Content-Type", "application/x-signed"),
                    ("x-token", "first"),
                    ("x-token", "fake"),
                ]
            } else {
                vec![
                    ("content-type", "first"),
                    ("content-type", "application/x-signed"),
                    ("x-token", "first"),
                    ("x-token", "fake"),
                ]
            };
            replies.push(Reply::UploadConfigured {
                headers,
                fragment: "client-side-upload",
            });
            replies.push(Reply::Put {
                status: 307,
                location: Some("/next?token=redirect#client-side-redirect"),
            });
            replies.push(Reply::Put {
                status: 200,
                location: None,
            });
            replies.push(Reply::Json(if sidebar {
                json!({"data":{"attachmentCreate":{"success":true,"attachment":{"id":"a","url":"asset","title":"API title"}}}})
            } else {
                json!({"data":{"commentCreate":{"success":true,"comment":{"id":"c","url":"comment"}}}})
            }));
            let (transport, server) = script(replies, None);
            if sidebar {
                assert_eq!(
                    issue_upload::lookup(&transport, "ENG-1").await.unwrap(),
                    "uuid"
                );
            }
            let prepared = upload::prepare(&path, false).unwrap();
            let file = upload::upload(&transport, &path, prepared).await.unwrap();
            if sidebar {
                let attachment =
                    issue_upload::attach(&transport, "uuid", &file, Some("title"), Some("body"))
                        .await
                        .unwrap();
                assert_eq!(attachment.title, "API title");
            } else {
                let body = issue_upload::compose_body(None, &[file]);
                let input = comment_add::build_input(
                    comment_add::CommentTarget::Issue {
                        issue_id: "ENG-1".into(),
                    },
                    body,
                    None,
                    None,
                )
                .unwrap();
                assert_eq!(
                    comment_add::create(&transport, input).await.unwrap().url,
                    "comment"
                );
            }
            let rows = server.join().unwrap();
            assert_eq!(rows.len(), if sidebar { 5 } else { 4 });
            assert_eq!(
                rows.last().unwrap().method,
                "POST",
                "final mutation follows both complete PUTs"
            );
            let puts = rows
                .iter()
                .filter(|row| row.method == "PUT")
                .collect::<Vec<_>>();
            assert_eq!(puts.len(), 2);
            assert_eq!(
                puts.iter().map(|row| row.path.as_str()).collect::<Vec<_>>(),
                vec!["/signed?token=fake", "/next?token=redirect"]
            );
            for put in puts {
                assert_eq!(put.body, bytes);
                let raw_content_types = put
                    .header_pairs
                    .iter()
                    .filter(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                    .map(|(name, value)| (name.as_str(), value.as_str()))
                    .collect::<Vec<_>>();
                assert_eq!(
                    raw_content_types,
                    if differently_cased {
                        vec![
                            ("content-type", "text/plain"),
                            ("content-type", "application/x-signed"),
                        ]
                    } else {
                        vec![("content-type", "application/x-signed")]
                    }
                );
                assert_eq!(
                    put.header_values("x-token"),
                    vec!["fake"],
                    "same literal key replaces its earlier value"
                );
                assert!(!put.contains_header("authorization"));
                assert!(!put.contains_header("user-agent"));
            }
            for row in rows.iter().filter(|row| row.method == "POST") {
                assert_eq!(row.header_values("authorization"), vec!["lin_api_fake"]);
                assert_eq!(
                    row.header_values("user-agent"),
                    vec![linear_cli::graphql::transport::USER_AGENT_VALUE]
                );
            }
            let operations = rows
                .iter()
                .filter(|row| row.method == "POST")
                .map(|row| {
                    serde_json::from_slice::<serde_json::Value>(&row.body).unwrap()["operationName"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                operations,
                if sidebar {
                    vec!["GetIssueId", "FileUpload", "AttachmentCreate"]
                } else {
                    vec!["FileUpload", "AddComment"]
                }
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
#[tokio::test]
async fn rejected_put_false_metadata_and_post_metadata_read_failure_stop_before_final_mutation() {
    for failure in ["put", "false", "read"] {
        let dir = scratch();
        let path = dir.join("note.txt");
        std::fs::write(&path, b"bytes").unwrap();
        let prepared = upload::prepare(&path, false).unwrap();
        let mut replies = vec![Reply::Upload {
            false_success: failure == "false",
        }];
        if failure == "put" {
            replies.push(Reply::Put {
                status: 403,
                location: None,
            });
        }
        let (transport, server) = script(replies, (failure == "read").then(|| path.clone()));
        let error = upload::upload(&transport, &path, prepared)
            .await
            .unwrap_err();
        assert_eq!(
            server.join().unwrap().len(),
            if failure == "put" { 2 } else { 1 }
        );
        match failure {
            "put" => assert_eq!(
                error.message,
                "Failed to upload file: 403 Forbidden - upload refused"
            ),
            "false" => assert_eq!(error.message, "Failed to get upload URL from Linear"),
            "read" => assert!(error.message.starts_with("Failed to read upload file:")),
            _ => unreachable!(),
        };
        std::fs::remove_dir_all(dir).unwrap();
    }
}
#[tokio::test]
async fn signed_put_relative_redirect_preserves_bytes_and_303_becomes_bodyless_get() {
    for status in [301, 302, 303, 307, 308] {
        let (transport, server) = script(
            vec![
                Reply::Put {
                    status,
                    location: Some("/next?token=redirect"),
                },
                Reply::Put {
                    status: 200,
                    location: None,
                },
            ],
            None,
        );
        let headers = upload::signed_headers("text/plain", &[]).unwrap();
        let endpoint = transport
            .endpoint()
            .url()
            .join("/signed?token=secret")
            .unwrap();
        transport
            .put_signed(endpoint.as_str(), headers, b"bytes".to_vec())
            .await
            .unwrap();
        let rows = server.join().unwrap();
        assert_eq!(rows[0].method, "PUT");
        assert_eq!(rows[0].body, b"bytes");
        assert_eq!(rows[1].path, "/next?token=redirect");
        assert_eq!(rows[1].method, if status != 303 { "PUT" } else { "GET" });
        assert_eq!(
            rows[1].body,
            if status != 303 {
                b"bytes".to_vec()
            } else {
                Vec::new()
            }
        );
        assert_eq!(rows[1].contains_header("content-type"), status != 303);
    }
}

#[test]
fn native_full_leaf_flags_preserve_repeatable_uploads_and_reply_aliases() {
    for reply_flag in ["--parent", "--reply-to", "-p"] {
        let args = [
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body",
            "Text",
            reply_flag,
            "parent",
            "--attach",
            "one.png",
            "-a",
            "two.txt",
            "--id",
            "123e4567-e89b-42d3-a456-426614174000",
        ];
        assert!(
            linear_cli::cli::parse(
                &args
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>()
            )
            .is_ok()
        );
    }
    let args = [
        "issue",
        "attach",
        "ENG-1",
        "file.png",
        "--title",
        "Title",
        "--comment",
        "Comment",
        "--public",
    ];
    assert!(
        linear_cli::cli::parse(
            &args
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        )
        .is_ok()
    );
    for args in [
        vec!["issue", "attach", "ENG-1"],
        vec!["issue", "attach", "ENG-1", "file.txt", "--json"],
        vec!["issue", "comment", "add", "ENG-1", "--json"],
    ] {
        assert!(
            linear_cli::cli::parse(
                &args
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }
}

#[tokio::test]
async fn slow_signed_put_outlives_two_second_graphql_deadline_and_still_creates_comment() {
    let dir = scratch();
    let path = dir.join("note.txt");
    let bytes = b"slowly";
    std::fs::write(&path, bytes).unwrap();
    let (transport, server) = script(
        vec![
            Reply::Upload {
                false_success: false,
            },
            Reply::SlowPut,
            Reply::Json(
                json!({"data":{"commentCreate":{"success":true,"comment":{"id":"c","url":"comment"}}}}),
            ),
        ],
        None,
    );
    let started = Instant::now();
    // Test watchdog only: production signed uploads have no total deadline.
    let comment = tokio::time::timeout(Duration::from_secs(6), async {
        let prepared = upload::prepare(&path, false).unwrap();
        let file = upload::upload(&transport, &path, prepared).await.unwrap();
        let body = issue_upload::compose_body(None, &[file]);
        let input = comment_add::build_input(
            comment_add::CommentTarget::Issue {
                issue_id: "ENG-1".into(),
            },
            body,
            None,
            None,
        )
        .unwrap();
        comment_add::create(&transport, input).await.unwrap()
    })
    .await
    .expect("bounded fixture completes after the three-second PUT");
    assert!(started.elapsed() >= Duration::from_secs(3));
    assert_eq!(comment.url, "comment");
    let rows = server.join().unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.method.as_str())
            .collect::<Vec<_>>(),
        vec!["POST", "PUT", "POST"]
    );
    assert_eq!(rows[1].body, bytes);
    assert!(!rows[1].contains_header("authorization"));
    let metadata: serde_json::Value = serde_json::from_slice(&rows[0].body).unwrap();
    let final_mutation: serde_json::Value = serde_json::from_slice(&rows[2].body).unwrap();
    assert_eq!(metadata["operationName"], "FileUpload");
    assert_eq!(final_mutation["operationName"], "AddComment");
    std::fs::remove_dir_all(dir).unwrap();
}

struct WireReply {
    status: u16,
    location: Option<String>,
    body: Vec<u8>,
}
fn wire_fixture(replies: Vec<WireReply>) -> (String, thread::JoinHandle<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut rows = Vec::new();
        for reply in replies {
            let started = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(started.elapsed() < Duration::from_secs(3));
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            rows.push(receive(&stream, false));
            write!(
                stream,
                "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                reply.body.len()
            )
            .unwrap();
            if let Some(location) = reply.location {
                write!(stream, "Location: {location}\r\n").unwrap();
            }
            stream.write_all(b"\r\n").unwrap();
            stream.write_all(&reply.body).unwrap();
        }
        thread::sleep(Duration::from_millis(20));
        assert!(listener.accept().is_err(), "no extra request or retry");
        rows
    });
    (origin, worker)
}
fn direct_transport(cap: usize) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse("http://127.0.0.1:1/graphql").unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(cap).unwrap(),
        },
    )
    .unwrap()
}
#[tokio::test]
async fn signed_cross_origin_redirect_strips_returned_credentials_but_retains_explicit_user_agent()
{
    let (second, second_server) = wire_fixture(vec![WireReply {
        status: 200,
        location: None,
        body: Vec::new(),
    }]);
    let (first, first_server) = wire_fixture(vec![WireReply {
        status: 307,
        location: Some(format!("{second}/next?kept=yes#ignored")),
        body: Vec::new(),
    }]);
    let headers = upload::signed_headers(
        "text/plain",
        &[
            UploadFileHeader {
                key: "Authorization".into(),
                value: "signed-auth".into(),
            },
            UploadFileHeader {
                key: "Proxy-Authorization".into(),
                value: "signed-proxy".into(),
            },
            UploadFileHeader {
                key: "WWW-Authenticate".into(),
                value: "signed-www".into(),
            },
            UploadFileHeader {
                key: "User-Agent".into(),
                value: "explicit-signed-agent".into(),
            },
        ],
    )
    .unwrap();
    direct_transport(64)
        .put_signed(
            &format!("{first}/first?secret=redacted"),
            headers,
            b"bytes".to_vec(),
        )
        .await
        .unwrap();
    let first = first_server.join().unwrap();
    let second = second_server.join().unwrap();
    assert_eq!(first[0].header_values("authorization"), vec!["signed-auth"]);
    for name in ["authorization", "proxy-authorization", "www-authenticate"] {
        assert!(!second[0].contains_header(name));
    }
    assert_eq!(
        second[0].header_values("user-agent"),
        vec!["explicit-signed-agent"]
    );
    assert_eq!(second[0].body, b"bytes");
    assert_eq!(second[0].path, "/next?kept=yes");
}
#[tokio::test]
async fn signed_redirect_limit_allows_twenty_hops_and_rejects_the_twenty_first() {
    for success in [true, false] {
        let mut replies = (0..20)
            .map(|_| WireReply {
                status: 307,
                location: Some("/next?token=fake".into()),
                body: Vec::new(),
            })
            .collect::<Vec<_>>();
        replies.push(WireReply {
            status: if success { 200 } else { 307 },
            location: (!success).then(|| "/extra?secret=hidden".into()),
            body: Vec::new(),
        });
        let (origin, server) = wire_fixture(replies);
        let result = direct_transport(64)
            .put_signed(
                &format!("{origin}/first"),
                upload::signed_headers("text/plain", &[]).unwrap(),
                b"bytes".to_vec(),
            )
            .await;
        if success {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert!(
                error
                    .message
                    .starts_with("Too many signed upload redirects;")
            );
            assert!(
                error
                    .message
                    .contains("no comment or attachment was created")
            );
            assert!(!error.message.contains("hidden"));
        }
        let rows = server.join().unwrap();
        assert_eq!(rows.len(), 21);
        assert!(
            rows.iter()
                .all(|row| row.method == "PUT" && row.body == b"bytes")
        );
    }
}
#[tokio::test]
async fn signed_failure_body_cap_network_and_invalid_redirects_are_typed_and_sanitized() {
    for (status, location, body, expected) in [
        (
            403,
            None,
            vec![b'x'; 65],
            "Signed upload response exceeded 64 bytes",
        ),
        (
            307,
            Some("file:///private?secret=hidden".into()),
            Vec::new(),
            "Invalid signed upload redirect",
        ),
        (
            307,
            Some("http://user:secret@localhost/path".into()),
            Vec::new(),
            "Invalid signed upload redirect",
        ),
    ] {
        let (origin, server) = wire_fixture(vec![WireReply {
            status,
            location,
            body,
        }]);
        let error = direct_transport(64)
            .put_signed(
                &format!("{origin}/first?secret=hidden"),
                upload::signed_headers("text/plain", &[]).unwrap(),
                b"bytes".to_vec(),
            )
            .await
            .unwrap_err();
        assert!(error.message.starts_with(expected));
        assert!(
            error
                .message
                .contains("object may already be stored remotely")
        );
        assert!(!error.message.contains("hidden"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = direct_transport(64)
        .put_signed(
            &format!("{origin}/path?secret=hidden"),
            upload::signed_headers("text/plain", &[]).unwrap(),
            b"bytes".to_vec(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .message
            .starts_with(&format!("Signed upload failed at {origin};"))
    );
    assert!(!error.message.contains("hidden"));
    for url in [
        "",
        "file:///tmp/file",
        "https://user:secret@localhost/path",
        "relative/path",
    ] {
        let error = direct_transport(64)
            .put_signed(
                url,
                upload::signed_headers("text/plain", &[]).unwrap(),
                b"bytes".to_vec(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.message, "Invalid signed upload URL");
    }
}

#[test]
fn upload_spinner_prefix_preserves_source_clear_symbol_reset_and_space_bytes() {
    let first = linear_cli::platform::spinner::frame(0);
    assert_eq!(first.as_bytes(), b"\r\x1b[K\xe2\xa0\x8b\x1b[0m ");
    assert_eq!(
        format!("{first}Uploading note.txt...").as_bytes(),
        b"\r\x1b[K\xe2\xa0\x8b\x1b[0m Uploading note.txt..."
    );
    assert_eq!(linear_cli::platform::spinner::frame(10), first);
}
