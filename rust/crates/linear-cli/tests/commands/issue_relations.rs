use linear_cli::cli::RootCommand;
use linear_cli::cli::issue::{IssueCommand, IssueRelationCommand, RelationType};
use linear_cli::commands::{issue_id, issue_relations};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::issue_id::GetIssueId;
use linear_cli::graphql::operations::issue_relations::{
    CreateIssueRelation, DeleteIssueRelation, FindIssueRelation, ListIssueRelations,
};
use serde_json::{Value, json};

#[test]
fn public_cli_relation_enum_is_case_insensitive_and_rejects_other_values() {
    for (word, expected) in [
        ("blocks", RelationType::Blocks),
        ("BlOcKeD-bY", RelationType::BlockedBy),
        ("RELATED", RelationType::Related),
        ("duplicate", RelationType::Duplicate),
    ] {
        let words =
            ["issue", "relation", "add", "eng-1", word, "eng-2"].map(std::ffi::OsString::from);
        let parsed = crate::parse(&words).unwrap();
        let RootCommand::Issue(issue) = parsed.command else {
            panic!("issue")
        };
        let IssueCommand::Relation(relation) = issue.command else {
            panic!("relation")
        };
        let IssueRelationCommand::Add(add) = relation.command else {
            panic!("add")
        };
        assert_eq!(add.relation_type, expected);
    }
    for words in [
        vec!["issue", "relation", "add", "A-1", "similar", "B-2"],
        vec!["issue", "relation", "add", "A-1", "blocKs", "B-2"],
        vec!["issue", "relation", "add", "A-1", "blocks"],
        vec![
            "issue", "relation", "delete", "A-1", "blocks", "B-2", "extra",
        ],
        vec!["issue", "relation", "list", "A-1", "--json"],
    ] {
        assert!(
            crate::parse(
                &words
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }
}

#[test]
fn directional_inputs_are_exhaustive_and_keep_related_and_duplicate_directional() {
    for (kind, api, from, to) in [
        (RelationType::Blocks, "blocks", "a", "b"),
        (RelationType::BlockedBy, "blocks", "b", "a"),
        (RelationType::Related, "related", "a", "b"),
        (RelationType::Duplicate, "duplicate", "a", "b"),
    ] {
        let input = issue_relations::directional_input(kind, "a".into(), "b".into());
        let request = serde_json::to_value(issue_relations::create_request(input)).unwrap();
        assert_eq!(
            request["variables"],
            json!({"input":{"issueId":from,"relatedIssueId":to,"type":api}})
        );
        assert_eq!(request["operationName"], "CreateIssueRelation");
    }
}

#[test]
fn list_output_uses_response_identifier_and_preserves_order_and_unknown_types() {
    let base = json!({"identifier":"MOVED-8","title":"Title","relations":{"nodes":[]},"inverseRelations":{"nodes":[]}});
    let outgoing =
        json!({"id":"r","type":"similar","relatedIssue":{"identifier":"ENG-2","title":"Other"}});
    let incoming =
        json!({"id":"s","type":"blocks","issue":{"identifier":"ENG-3","title":"Blocker"}});
    for (out, inc, expected) in [
        (
            vec![],
            vec![],
            "Relations for MOVED-8: Title\n\n  No relations\n",
        ),
        (
            vec![outgoing.clone()],
            vec![],
            "Relations for MOVED-8: Title\n\nOutgoing:\n  MOVED-8 similar ENG-2: Other\n",
        ),
        (
            vec![],
            vec![incoming.clone()],
            "Relations for MOVED-8: Title\n\nIncoming:\n  MOVED-8 blocked-by ENG-3: Blocker\n",
        ),
        (
            vec![outgoing],
            vec![incoming],
            "Relations for MOVED-8: Title\n\nOutgoing:\n  MOVED-8 similar ENG-2: Other\n\nIncoming:\n  MOVED-8 blocked-by ENG-3: Blocker\n",
        ),
    ] {
        let mut issue = base.clone();
        issue["relations"]["nodes"] = json!(out);
        issue["inverseRelations"]["nodes"] = json!(inc);
        let data: ListIssueRelations =
            parse_response(json!({"data":{"issue":issue}}).to_string().as_bytes()).unwrap();
        assert_eq!(
            issue_relations::list_output(&data.issue),
            expected.as_bytes()
        );
    }
    let data:ListIssueRelations=parse_response(br#"{"data":{"issue":{"identifier":"A-1","title":"","relations":{"nodes":[]},"inverseRelations":{"nodes":[{"id":"x","type":"future-kind","issue":{"identifier":"B-2","title":""}}]}}}}"#).unwrap();
    assert_eq!(
        issue_relations::list_output(&data.issue),
        b"Relations for A-1: \n\nIncoming:\n  A-1 future-kind B-2: \n"
    );
}

#[test]
fn selected_schema_fields_are_strict_even_on_false_success() {
    for issue in [
        json!(null),
        json!({}),
        json!({"identifier":"A-1","title":"T","relations":{"nodes":null},"inverseRelations":{"nodes":[]}}),
        json!({"identifier":"A-1","title":"T","relations":{"nodes":[null]},"inverseRelations":{"nodes":[]}}),
    ] {
        assert!(
            parse_response::<ListIssueRelations>(
                json!({"data":{"issue":issue}}).to_string().as_bytes()
            )
            .is_err()
        );
    }
    for payload in [
        json!(null),
        json!({"success":true,"issueRelation":null}),
        json!({"success":false,"issueRelation":null}),
        json!({"success":true,"issueRelation":{}}),
        json!({"success":"true","issueRelation":{"id":"x"}}),
    ] {
        assert!(
            parse_response::<CreateIssueRelation>(
                json!({"data":{"issueRelationCreate":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
    for issue in [
        json!(null),
        json!({"relations":{"nodes":null}}),
        json!({"relations":{"nodes":[{"id":"r","type":"blocks","relatedIssue":null}]}}),
    ] {
        assert!(
            parse_response::<FindIssueRelation>(
                json!({"data":{"issue":issue}}).to_string().as_bytes()
            )
            .is_err()
        );
    }
    for data in [
        json!({"issueRelationDelete":null}),
        json!({"issueRelationDelete":{}}),
        json!({"issueRelationDelete":{"success":0}}),
    ] {
        assert!(
            parse_response::<DeleteIssueRelation>(json!({"data":data}).to_string().as_bytes())
                .is_err()
        );
    }
    for issue in [json!(null), json!({}), json!({"id":null}), json!({"id":7})] {
        assert!(
            parse_response::<GetIssueId>(json!({"data":{"issue":issue}}).to_string().as_bytes())
                .is_err()
        );
    }
}

#[test]
fn minimal_lookup_and_first_page_requests_have_no_extra_selections() {
    let compact = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace() && *c != ',')
            .collect::<String>()
    };
    for (actual, document, variables) in [
        (
            serde_json::to_value(issue_id::request("ENG-1")).unwrap(),
            "query GetIssueId($id:String!){issue(id:$id){id}}",
            json!({"id":"ENG-1"}),
        ),
        (
            serde_json::to_value(issue_relations::find_request("uuid")).unwrap(),
            "query FindIssueRelation($issueId:String!){issue(id:$issueId){relations{nodes{id type relatedIssue{id}}}}}",
            json!({"issueId":"uuid"}),
        ),
        (
            serde_json::to_value(issue_relations::delete_request("first")).unwrap(),
            "mutation DeleteIssueRelation($id:String!){issueRelationDelete(id:$id){success}}",
            json!({"id":"first"}),
        ),
    ] {
        assert_eq!(
            compact(actual["query"].as_str().unwrap()),
            compact(document)
        );
        assert_eq!(actual["variables"], variables);
    }
}

use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
pub(super) fn sequence(replies: Vec<Value>) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut input = Vec::new();
            loop {
                let mut buf = [0; 8192];
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                input.extend_from_slice(&buf[..n]);
                if let Some((head, body)) =
                    std::str::from_utf8(&input).unwrap().split_once("\r\n\r\n")
                {
                    let size = head
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if body.len() >= size {
                        requests.push(serde_json::from_str(body).unwrap());
                        break;
                    }
                }
            }
            let body = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        thread::sleep(Duration::from_millis(30));
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err(), "no extra request");
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, server)
}
fn id_reply(id: &str) -> Value {
    json!({"data":{"issue":{"id":id}}})
}
#[tokio::test]
async fn add_looks_up_equal_identifiers_twice_in_order() {
    let (transport, server) = sequence(vec![
        id_reply("a"),
        id_reply("a"),
        json!({"data":{"issueRelationCreate":{"success":true,"issueRelation":{"id":"r"}}}}),
    ]);
    assert_eq!(
        issue_relations::add(&transport, RelationType::Related, "ENG-1", "ENG-1")
            .await
            .unwrap(),
        "✓ Created relation: ENG-1 related ENG-1\n".as_bytes()
    );
    let requests = server.join().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|r| r["operationName"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("GetIssueId"),
            json!("GetIssueId"),
            json!("CreateIssueRelation")
        ]
    );
    assert_eq!(requests[0]["variables"], json!({"id":"ENG-1"}));
    assert_eq!(requests[1]["variables"], json!({"id":"ENG-1"}));
    assert_eq!(
        requests[2]["variables"],
        json!({"input":{"issueId":"a","relatedIssueId":"a","type":"related"}})
    );
}
#[tokio::test]
async fn delete_first_exact_outgoing_match_and_directional_no_matches() {
    for kind in [
        RelationType::Blocks,
        RelationType::BlockedBy,
        RelationType::Related,
        RelationType::Duplicate,
    ] {
        let api = issue_relations::directional_input(kind, "a".into(), "b".into());
        let nodes = json!([
            {"id":"wrong-type","type":"similar","relatedIssue":{"id":api.related_issue_id}},
            {"id":"wrong-target","type":api.relation_type.spelling(),"relatedIssue":{"id":"other"}},
            {"id":"first","type":api.relation_type.spelling(),"relatedIssue":{"id":api.related_issue_id}},
            {"id":"later","type":api.relation_type.spelling(),"relatedIssue":{"id":api.related_issue_id}}
        ]);
        let (transport, server) = sequence(vec![
            id_reply("a"),
            id_reply("b"),
            json!({"data":{"issue":{"relations":{"nodes":nodes}}}}),
            json!({"data":{"issueRelationDelete":{"success":true}}}),
        ]);
        assert_eq!(
            issue_relations::delete(&transport, kind, "ENG-1", "ENG-2")
                .await
                .unwrap(),
            format!("✓ Deleted relation: ENG-1 {} ENG-2\n", kind.spelling()).as_bytes()
        );
        let requests = server.join().unwrap();
        assert_eq!(requests[2]["variables"], json!({"issueId":api.issue_id}));
        assert_eq!(requests[3]["variables"], json!({"id":"first"}));
        let (transport, server) = sequence(vec![
            id_reply("a"),
            id_reply("b"),
            json!({"data":{"issue":{"relations":{"nodes":[]}}}}),
        ]);
        let error = issue_relations::delete(&transport, kind, "ENG-1", "ENG-2")
            .await
            .unwrap_err();
        assert_eq!(
            error.message(),
            format!(
                "Relation not found: {} between ENG-1 and ENG-2",
                kind.spelling()
            )
        );
        assert_eq!(server.join().unwrap().len(), 3);
    }
}
#[tokio::test]
async fn lookup_not_found_stops_the_sequence_and_false_success_is_an_error() {
    for second in [false, true] {
        let mut replies = vec![];
        if second {
            replies.push(id_reply("a"));
        }
        replies.push(json!({"errors":[{"message":"oops","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}));
        let (transport, server) = sequence(replies);
        let error = issue_relations::add(&transport, RelationType::Blocks, "ENG-1", "ENG-2")
            .await
            .unwrap_err();
        assert_eq!(
            error.message(),
            if second {
                "Issue not found: ENG-2"
            } else {
                "Issue not found: ENG-1"
            }
        );
        assert_eq!(server.join().unwrap().len(), if second { 2 } else { 1 });
    }
    let (transport, server) = sequence(vec![
        id_reply("a"),
        id_reply("b"),
        json!({"data":{"issueRelationCreate":{"success":false,"issueRelation":{"id":"r"}}}}),
    ]);
    assert_eq!(
        issue_relations::add(&transport, RelationType::Blocks, "A-1", "B-2")
            .await
            .unwrap_err()
            .message(),
        "Failed to create relation"
    );
    assert_eq!(server.join().unwrap().len(), 3);
}

#[tokio::test]
async fn empty_lookup_ids_are_not_found_and_stop_before_any_write() {
    for delete in [false, true] {
        for second in [false, true] {
            let mut replies = vec![];
            if second {
                replies.push(id_reply("a"));
            }
            replies.push(id_reply(""));
            let (transport, server) = sequence(replies);
            let error = if delete {
                issue_relations::delete(&transport, RelationType::Blocks, "ENG-1", "ENG-2")
                    .await
                    .unwrap_err()
            } else {
                issue_relations::add(&transport, RelationType::Blocks, "ENG-1", "ENG-2")
                    .await
                    .unwrap_err()
            };
            assert_eq!(
                error.message(),
                if second {
                    "Issue not found: ENG-2"
                } else {
                    "Issue not found: ENG-1"
                }
            );
            assert_eq!(server.join().unwrap().len(), if second { 2 } else { 1 });
        }
    }
}
