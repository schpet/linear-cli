use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::commands::user::list::run_with;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::organization_members::GetOrganizationMembers;
use serde_json::{Value, json};

fn member(id: &str, display_name: &str, active: bool) -> Value {
    json!({
        "id": id,
        "name": display_name,
        "displayName": display_name,
        "email": format!("{id}@example.invalid"),
        "active": active,
        "initials": "XX",
        "description": null,
        "timezone": null,
        "lastSeen": null,
        "statusEmoji": null,
        "statusLabel": null,
        "guest": false,
        "isAssignable": true,
        "admin": false,
        "owner": false,
        "isMe": false,
        "url": format!("https://linear.app/example/profiles/{id}")
    })
}

fn page(nodes: Vec<Value>, has_next_page: bool, cursor: Option<&str>) -> GetOrganizationMembers {
    let body = json!({"data":{"viewer":{"organization":{"users":{
        "nodes": nodes,
        "pageInfo":{"hasNextPage":has_next_page,"endCursor":cursor}
    }}}}});
    parse_response(body.to_string().as_bytes()).expect("typed workspace user page")
}

#[tokio::test]
async fn request_walks_all_pages_then_sorts_and_filters_a_graphql_connection() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![member("mona", "Mona", true), member("old", "Old", false)],
            true,
            Some("next"),
        ),
        page(vec![member("aaron", "aaron", true)], false, Some("last")),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).expect("request JSON"));
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        false,
        true,
    )
    .await
    .expect("members JSON");
    let text = String::from_utf8(output).expect("UTF-8");
    let value: Value = serde_json::from_str(&text).expect("JSON connection");
    assert_eq!(value["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(value["nodes"][0]["id"], "aaron");
    assert_eq!(value["nodes"][1]["id"], "mona");
    assert_eq!(
        value["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"last"})
    );
    assert!(text.starts_with("{\n  \"nodes\": ["));
    assert!(text.ends_with("\n"));

    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]["variables"],
        json!({"includeDisabled":false,"first":100})
    );
    assert_eq!(
        requests[1]["variables"],
        json!({"includeDisabled":false,"first":100,"after":"next"})
    );
    let query = requests[0]["query"]
        .as_str()
        .expect("typed operation query");
    assert!(query.contains("GetOrganizationMembers"));
    assert!(query.contains("$includeDisabled: Boolean!"));
    assert!(query.contains("users("));
    for argument in [
        "includeDisabled: $includeDisabled",
        "first: $first",
        "after: $after",
    ] {
        assert!(query.contains(argument), "missing {argument} in {query}");
    }
}

#[tokio::test]
async fn all_retains_inactive_and_case_fold_ties_keep_server_order() {
    let output = run_with(
        |_| {
            ready(Ok(page(
                vec![
                    member("upper", "ADA", true),
                    member("lower", "ada", true),
                    member("inactive", "Zed", false),
                ],
                false,
                None,
            )))
        },
        true,
        true,
    )
    .await
    .expect("all members");
    let value: Value = serde_json::from_slice(&output).expect("JSON");
    let ids: Vec<_> = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["upper", "lower", "inactive"]);
}

#[tokio::test]
async fn empty_cursor_is_sent_as_an_explicit_after_variable_once() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(vec![member("first", "First", true)], true, Some("")),
        page(vec![member("second", "Second", true)], false, Some("last")),
    ])));
    let variables = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        {
            let pages = Rc::clone(&pages);
            let variables = Rc::clone(&variables);
            move |request| {
                variables.borrow_mut().push(
                    serde_json::to_value(&request).expect("request JSON")["variables"].clone(),
                );
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        false,
        true,
    )
    .await
    .expect("empty cursor can advance");
    let value: Value = serde_json::from_slice(&output).expect("JSON connection");
    assert_eq!(value["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(value["pageInfo"]["endCursor"], "last");
    assert_eq!(
        *variables.borrow(),
        [
            json!({"includeDisabled":false,"first":100}),
            json!({"includeDisabled":false,"first":100,"after":""}),
        ]
    );
}

#[tokio::test]
async fn text_distinguishes_empty_raw_page_from_all_inactive_and_renders_markers() {
    let empty = run_with(|_| ready(Ok(page(vec![], false, None))), false, false)
        .await
        .expect("empty");
    assert_eq!(empty, b"No members found in this workspace.\n");

    let inactive = run_with(
        |_| ready(Ok(page(vec![member("old", "Old", false)], false, None))),
        false,
        false,
    )
    .await
    .expect("inactive");
    assert_eq!(
        inactive,
        b"No active members found in this workspace. Use --all to include inactive members.\n"
    );

    let mut rich = member("rich", "", true);
    rich["name"] = json!("Fallback");
    rich["description"] = json!("Engineer");
    rich["timezone"] = json!("America/Los_Angeles");
    rich["statusEmoji"] = json!("🌿");
    rich["statusLabel"] = json!("Away");
    rich["guest"] = json!(true);
    rich["isAssignable"] = json!(false);
    rich["admin"] = json!(true);
    rich["owner"] = json!(true);
    rich["isMe"] = json!(true);
    let output = run_with(
        |_| ready(Ok(page(vec![rich.clone()], false, None))),
        false,
        false,
    )
    .await
    .expect("rich member");
    assert_eq!(
        output,
        b"Workspace Members (1):\n\nFallback (Fallback) [XX] (guest) (not assignable) (admin) (owner) (you)\n  Email: rich@example.invalid\n  Role: Engineer\n  Timezone: America/Los_Angeles\n  Status: \xf0\x9f\x8c\xbf Away\n\n"
    );
}

#[tokio::test]
async fn pagination_failure_discards_partial_output_and_preserves_command_context() {
    let calls = Rc::new(RefCell::new(0));
    let result = run_with(
        {
            let calls = Rc::clone(&calls);
            move |_| {
                *calls.borrow_mut() += 1;
                ready(Ok(page(vec![member("a", "A", true)], true, Some("same"))))
            }
        },
        false,
        true,
    )
    .await
    .expect_err("repeated cursor");
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(
        result.to_string(),
        "Failed to fetch workspace members: Linear reported more workspace members but did not advance the page cursor"
    );
}
