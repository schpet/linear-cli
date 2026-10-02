use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::auth::ApiKeyInput;
use linear_cli::commands::label_list::{Options, Selection, render_text, run_with, select};
use linear_cli::error::Error;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::issue_labels::{GetIssueLabels, IssueLabel, Team};
use linear_cli::refs::{ResolvedTeam, WorkspaceScope};
use serde_json::{Value, json};

fn scope<'a>(api_key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: api_key.clone(),
    }
}

fn label(id: &str, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": null,
        "color": "#112233",
        "team": null
    })
}

fn page(nodes: Vec<Value>, has_next_page: bool, end_cursor: Option<&str>) -> GetIssueLabels {
    let body = json!({"data":{"issueLabels":{
        "nodes": nodes,
        "pageInfo":{"hasNextPage":has_next_page,"endCursor":end_cursor}
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed label page")
}

fn resolved_team() -> ResolvedTeam {
    ResolvedTeam {
        id: "team-id".to_owned(),
        key: "ENG".to_owned(),
        name: "Engineering".to_owned(),
    }
}

#[tokio::test]
async fn workspace_and_configured_team_send_exact_filter_variables() {
    for (selection, expected) in [
        (
            Selection::WorkspaceOnly,
            json!({"first":100,"filter":{"team":{"null":true}}}),
        ),
        (
            Selection::ConfiguredTeam("ENG".to_owned()),
            json!({"first":100,"filter":{"or":[
                {"team":{"key":{"eq":"ENG"}}},
                {"team":{"null":true}}
            ]}}),
        ),
    ] {
        let requests = Rc::new(RefCell::new(Vec::new()));
        let output = run_with(
            selection,
            |_| ready(Ok(resolved_team())),
            {
                let requests = Rc::clone(&requests);
                move |request| {
                    requests
                        .borrow_mut()
                        .push(serde_json::to_value(&request).unwrap());
                    ready(Ok(page(vec![], false, None)))
                }
            },
            true,
            120,
        )
        .await
        .expect("empty connection");
        assert_eq!(requests.borrow().len(), 1);
        assert_eq!(requests.borrow()[0]["variables"], expected);
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "{\n  \"nodes\": [],\n  \"pageInfo\": {\n    \"hasNextPage\": false,\n    \"endCursor\": null\n  }\n}\n"
        );
    }
}

#[test]
fn explicit_blank_team_is_rejected_before_workspace_only_and_all() {
    let absent = ApiKeyInput::Absent;
    let scope = scope(&absent);
    for value in ["", "  ", "\u{00a0}"] {
        for (workspace_only, all) in [(false, false), (true, false), (false, true)] {
            let result = select(
                &Options {
                    team: Some(value.to_owned()),
                    workspace_only,
                    all,
                    json: true,
                },
                Some("DEFAULT"),
                &scope,
            );
            let error = result.expect_err("explicit blank team must not fall through");
            assert_eq!(
                error.to_string(),
                "Failed to fetch labels: Team reference is empty"
            );
        }
    }
    assert_eq!(
        select(
            &Options {
                team: Some("ENG".to_owned()),
                workspace_only: true,
                ..Options::default()
            },
            None,
            &scope,
        )
        .unwrap(),
        Selection::WorkspaceOnly
    );
    assert!(matches!(
        select(
            &Options {
                team: Some("ENG".to_owned()),
                all: true,
                ..Options::default()
            },
            None,
            &scope,
        )
        .unwrap(),
        Selection::Team(_)
    ));
    assert_eq!(
        select(&Options::default(), Some("ENG"), &scope).unwrap(),
        Selection::ConfiguredTeam("ENG".to_owned())
    );
    assert_eq!(
        select(
            &Options {
                all: true,
                ..Options::default()
            },
            Some("ENG"),
            &scope,
        )
        .unwrap(),
        Selection::Unfiltered
    );
}

#[tokio::test]
async fn json_concatenates_all_pages_then_stably_sorts_and_keeps_last_page_info() {
    let mut described = label("z", "zebra");
    described["description"] = json!("a description");
    described["team"] = json!({"key":"ENG","name":"Engineering"});
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(vec![described, label("tie-1", "Alpha")], true, Some("next")),
        page(
            vec![
                label("tie-2", "alpha"),
                label("a", "Álpha"),
                label("tie-duplicate", "Alpha"),
            ],
            false,
            Some("final"),
        ),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        Selection::Unfiltered,
        |_| ready(Ok(resolved_team())),
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        true,
        120,
    )
    .await
    .expect("all labels");
    let output = String::from_utf8(output).expect("UTF-8 JSON");
    let parsed: Value = serde_json::from_str(&output).expect("connection JSON");
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 5);
    assert_eq!(parsed["nodes"][0]["id"], "tie-1");
    assert_eq!(parsed["nodes"][1]["id"], "tie-2");
    assert_eq!(parsed["nodes"][2]["id"], "tie-duplicate");
    assert_eq!(parsed["nodes"][4]["id"], "z");
    assert_eq!(parsed["nodes"][4]["description"], "a description");
    assert_eq!(
        parsed["nodes"][4]["team"],
        json!({"key":"ENG","name":"Engineering"})
    );
    assert_eq!(
        parsed["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"final"})
    );
    assert!(output.starts_with("{\n  \"nodes\": ["));
    assert!(output.ends_with('\n'));
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["variables"], json!({"first":100}));
    assert_eq!(
        requests[1]["variables"],
        json!({"first":100,"after":"next"})
    );
}

#[tokio::test]
async fn explicit_team_resolves_before_filtered_label_request() {
    let absent = ApiKeyInput::Absent;
    let scope = scope(&absent);
    let selection = select(
        &Options {
            team: Some("eng".to_owned()),
            all: true,
            ..Options::default()
        },
        None,
        &scope,
    )
    .expect("prepared team");
    let order = Rc::new(RefCell::new(Vec::new()));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        selection,
        {
            let order = Rc::clone(&order);
            move |_| {
                order.borrow_mut().push("resolve");
                ready(Ok(resolved_team()))
            }
        },
        {
            let order = Rc::clone(&order);
            let requests = Rc::clone(&requests);
            move |request| {
                order.borrow_mut().push("labels");
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                ready(Ok(page(vec![], false, None)))
            }
        },
        false,
        120,
    )
    .await
    .expect("empty labels");
    assert_eq!(output, b"No labels found.\n");
    assert_eq!(&*order.borrow(), &["resolve", "labels"]);
    assert_eq!(
        requests.borrow()[0]["variables"],
        json!({"first":100,"filter":{"or":[
            {"team":{"key":{"eq":"ENG"}}},
            {"team":{"null":true}}
        ]}})
    );
}

#[tokio::test]
async fn resolver_and_later_page_failures_return_context_without_partial_output() {
    let absent = ApiKeyInput::Absent;
    let scope = scope(&absent);
    let selection = select(
        &Options {
            team: Some("ENG".to_owned()),
            ..Options::default()
        },
        None,
        &scope,
    )
    .unwrap();
    let resolver_failure = run_with(
        selection,
        |_| ready(Err(Error::new("team vanished"))),
        |_| ready(Ok(page(vec![], false, None))),
        true,
        120,
    )
    .await
    .expect_err("resolver failure");
    assert_eq!(
        resolver_failure.to_string(),
        "Failed to fetch labels: team vanished"
    );

    let calls = Rc::new(RefCell::new(0));
    let later_page_failure = run_with(
        Selection::Unfiltered,
        |_| ready(Ok(resolved_team())),
        {
            let calls = Rc::clone(&calls);
            move |_| {
                *calls.borrow_mut() += 1;
                if *calls.borrow() == 1 {
                    ready(Ok(page(vec![label("first", "Alpha")], true, Some("next"))))
                } else {
                    ready(Err(Error::new("page failed")))
                }
            }
        },
        true,
        120,
    )
    .await
    .expect_err("later page failure");
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(
        later_page_failure.to_string(),
        "Failed to fetch labels: page failed"
    );
}

#[tokio::test]
async fn missing_or_repeated_cursor_aborts_without_third_request() {
    for cursor in [None, Some(""), Some("same")] {
        let calls = Rc::new(RefCell::new(0));
        let result = run_with(
            Selection::Unfiltered,
            |_| ready(Ok(resolved_team())),
            {
                let calls = Rc::clone(&calls);
                move |_| {
                    *calls.borrow_mut() += 1;
                    ready(Ok(page(vec![label("id", "Alpha")], true, cursor)))
                }
            },
            true,
            120,
        )
        .await
        .expect_err("invalid cursor");
        let expected_calls = if cursor == Some("same") { 2 } else { 1 };
        assert_eq!(*calls.borrow(), expected_calls);
        let expected = if cursor == Some("same") {
            "Failed to fetch labels: Linear repeated a label pagination cursor on page 2"
        } else {
            "Failed to fetch labels: Linear reported more labels but returned no pagination cursor"
        };
        assert_eq!(result.to_string(), expected);
    }
}

#[test]
fn text_truncates_by_display_width_and_underlines_header_cells() {
    let labels = [
        IssueLabel {
            id: cynic::Id::new("id"),
            name: "abcdefghijklmnop😀tailtail".to_owned(),
            description: None,
            color: "#112233".to_owned(),
            team: None,
        },
        IssueLabel {
            id: cynic::Id::new("id-2"),
            name: "漢".repeat(11),
            description: None,
            color: "#445566".to_owned(),
            team: None,
        },
    ];
    let plain = render_text(&labels, 0, false);
    assert!(plain.contains("abcdefghijklmnop..."));
    assert!(plain.contains(&format!("{}...", "漢".repeat(8))));
    assert!(plain.ends_with("\n2 labels found.\n"));
    assert!(!plain.contains("\x1b["));
    let styled = render_text(&labels, 0, true);
    assert!(styled.starts_with("\x1b[4mID"));
    assert!(styled.contains("\x1b[24m \x1b[4mNAME"));
    assert!(styled.contains("\x1b[24m \x1b[4mCOLOR"));
    assert!(styled.contains("\x1b[24m \x1b[4mTEAM"));
    assert!(styled.contains("\x1b[0m\n"));
}

#[test]
fn text_table_preserves_header_widths_workspace_fallback_and_long_team_key() {
    let workspace = IssueLabel {
        id: cynic::Id::new("id"),
        name: "Bug".to_owned(),
        description: None,
        color: "#112233".to_owned(),
        team: Some(Team {
            key: String::new(),
            name: "Empty key".to_owned(),
        }),
    };
    let one = render_text(std::slice::from_ref(&workspace), 120, false);
    assert_eq!(
        one,
        format!(
            "{:<36} {} {:<7} {:<9}\n{:<36} {} {:<7} {:<9}\n\n1 labels found.\n",
            "ID", "NAME", "COLOR", "TEAM", "id", "Bug", "#112233", "Workspace"
        )
    );
    let mut long_team = workspace;
    long_team.team = Some(Team {
        key: "LONGTEAMKEYLONGERTHAN15".to_owned(),
        name: "Long key".to_owned(),
    });
    let long = render_text(&[long_team], 120, false);
    assert!(long.contains("LONGTEAMKEYLONGERTHAN15\n"));
}
