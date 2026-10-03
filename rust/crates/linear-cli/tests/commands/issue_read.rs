use linear_cli::{
    commands::{issue::read, issue::view},
    graphql::{
        envelope::parse_response,
        operations::{
            issue_read::*,
            number::{Float, WholeNumber},
        },
    },
};
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};
const QUERY: &str = include_str!(
    "../../../../parity/runner/c060-c062-frozen-cases/c061-allteams-filter-pages.json"
);
const VIEW: &str =
    include_str!("../../../../parity/runner/c060-c062-frozen-cases/c062-markdown-all-threads.json");
fn data(case: &str) -> Value {
    serde_json::from_str::<Value>(case).unwrap()["graphql"]["groups"][0]["steps"][0]["response"]["data"].clone()
}
fn issue() -> view::Issue {
    parse_response::<GetIssueDetailsWithComments>(json!({"data":data(VIEW)}).to_string().as_bytes())
        .unwrap()
        .issue
        .unwrap()
}
#[test]
fn issue_json_flattens_connections_and_keeps_metadata() {
    let query: GetIssuesForQuery = serde_json::from_value(data(QUERY)).unwrap();
    let json = serde_json::to_value(&query.issues.nodes).unwrap();
    assert!(json[0]["labels"].is_array(), "{json}");
    let mut i = issue();
    i.attachments.nodes[0]
        .metadata
        .0
        .insert("integer".to_owned(), json!(9007199254740993_u64));
    i.attachments.nodes[0]
        .metadata
        .0
        .insert("zero".to_owned(), json!(-0.0));
    let out = String::from_utf8(view::Fetched::With(i).json()).unwrap();
    assert!(out.contains("\"comments\": ["));
    assert!(out.contains("\"integer\": 9007199254740993"));
    assert!(out.contains("\"zero\": -0.0"));
    assert!(out.contains("\"quotedText\": null"));
    for mutation in ["priority", "identifier", "comments"] {
        let mut wrong = data(VIEW);
        wrong["issue"].as_object_mut().unwrap().remove(mutation);
        assert!(serde_json::from_value::<GetIssueDetailsWithComments>(wrong).is_err());
    }
}
#[test]
fn workflow_sort_uses_actual_returned_teams_and_stable_position_desc() {
    let source: GetIssuesForQuery = serde_json::from_value(data(QUERY)).unwrap();
    let seed = source.issues.nodes[0].clone();
    let mut rows = vec![];
    for (id, team, kind, position) in [
        ("low", "ENG", "unstarted", 1.0),
        ("high", "ENG", "unstarted", 9.0),
        ("tie", "ENG", "unstarted", 9.0),
        ("started", "ENG", "started", 0.0),
    ] {
        let mut r = seed.clone();
        r.identifier = id.to_owned();
        r.team.key = team.to_owned();
        r.state.r#type = kind.to_owned();
        r.state.position = Float(serde_json::Number::from_f64(position).unwrap());
        rows.push(r);
    }
    read::sort(&mut rows);
    assert_eq!(
        rows.iter()
            .map(|r| r.identifier.as_str())
            .collect::<Vec<_>>(),
        ["started", "high", "tie", "low"]
    );
    rows[0].team.key = "OTHER".to_owned();
    rows.rotate_right(1);
    read::sort(&mut rows);
    assert_eq!(
        rows.iter()
            .map(|r| r.identifier.as_str())
            .collect::<Vec<_>>(),
        ["started", "low", "high", "tie"]
    );
}
#[test]
fn table_rows_show_updated_time_and_estimate() {
    let source: GetIssuesForQuery = serde_json::from_value(data(QUERY)).unwrap();
    let rows = source.issues.nodes;
    let now = UNIX_EPOCH + Duration::from_secs(86_400 * 50000);
    let mut row = rows[0].clone();
    row.updated_at.0 = chrono::DateTime::<chrono::Utc>::from(now)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    row.estimate = Some(Float(0.into()));
    let mut short = read::table(&[row.clone()], true, true, now).render(None, false);
    assert!(short.contains("just now"), "{short}");
    assert!(short.contains(" 0 "), "{short}");
    row.updated_at.0 = chrono::DateTime::<chrono::Utc>::from(now - Duration::from_secs(3600))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    short = read::table(&[row], false, false, now).render(None, false);
    assert!(short.contains("1 hour ago"), "{short}");
    assert_eq!(read::priority(WholeNumber(4)), "▄  ");
    assert_eq!(read::priority(WholeNumber(9)), "9");
}
#[test]
fn thread_roots_resolution_hidden_count_orphans_duplicates_and_cycles_are_distinct() {
    let mut i = issue();
    let original = view::threads(&i.comments.nodes, false).unwrap();
    assert_eq!(original.hidden, 1);
    assert!(original.roots.iter().all(|c| c.resolved_at.is_none()));
    let roots = original.roots.len();
    let mut orphan = i.comments.nodes[0].clone();
    orphan.id = cynic::Id::new("orphan");
    orphan.parent = Some(GetIssueDetailsWithCommentsIssueCommentsNodesParent {
        id: cynic::Id::new("missing"),
    });
    i.comments.nodes.push(orphan);
    let visible = view::threads(&i.comments.nodes, false).unwrap();
    assert_eq!(visible.roots.len(), roots);
    assert_eq!(visible.replies["missing"].len(), 1);
    let mut duplicate = i.comments.nodes[0].clone();
    duplicate.parent = None;
    i.comments.nodes.push(duplicate);
    assert_eq!(
        view::threads(&i.comments.nodes, true).unwrap().roots.len(),
        roots + 2
    );
    // Start a separate cycle vector so last-wins duplicate lookup cannot mask it.
    let mut i = issue();
    let first = &mut i.comments.nodes[0];
    first.parent = Some(GetIssueDetailsWithCommentsIssueCommentsNodesParent {
        id: first.id.clone(),
    });
    assert!(
        view::threads(&i.comments.nodes, true)
            .unwrap_err()
            .message()
            .contains("cycle")
    );
}
// These fixtures spell comment dates as UTC month/day/year. Keep every other
// byte exact while expecting the local YYYY-MM-DD date the command prints.
fn local_comment_calendar(expected: &str) -> String {
    let dates = issue()
        .comments
        .nodes
        .into_iter()
        .map(|comment| {
            let parsed = chrono::DateTime::parse_from_rfc3339(&comment.created_at.0).unwrap();
            let utc = parsed.with_timezone(&chrono::Utc);
            let local = parsed.with_timezone(&chrono::Local);
            (
                utc.format("%-m/%-d/%Y").to_string(),
                local.format("%Y-%m-%d").to_string(),
            )
        })
        .collect::<Vec<_>>();
    expected
        .split_inclusive('\n')
        .map(|line| {
            if let Some((utc, local)) = dates.iter().find(|(utc, _)| {
                line.contains(&format!("commented {utc}")) || line.contains(&format!("*{utc}*"))
            }) {
                line.replace(&format!("commented {utc}"), &format!("commented {local}"))
                    .replace(&format!("*{utc}*"), &format!("*{local}*"))
            } else {
                line.to_owned()
            }
        })
        .collect()
}
#[test]
fn pipe_markdown_hierarchy_comments_and_resolved_summary_match_source() {
    let i = issue();
    let source: Value = serde_json::from_str(VIEW).unwrap();
    let want = local_comment_calendar(source["expected"]["stdout"]["utf8"].as_str().unwrap());
    assert_eq!(
        format!(
            "{}\n",
            view::markdown(&i, &Default::default(), true, chrono::Utc::now()).unwrap()
        ),
        want
    );
    let hidden = view::markdown(&i, &Default::default(), false, chrono::Utc::now()).unwrap();
    assert!(hidden.contains("Resolved thread hidden: 1."));
    assert!(
        hidden.contains("## Parent")
            && hidden.contains("## Sub-issues")
            && hidden.contains("## Attachments")
            && hidden.contains("## Documents")
    );
}

#[test]
fn final_query_team_scope_uses_eq_and_or_while_state_lookup_retains_in() {
    let one = vec!["ENG".to_owned()];
    let two = vec!["ENG".to_owned(), "OPS".to_owned()];
    assert_eq!(
        serde_json::to_value(read::query_team_filter(&one)).unwrap(),
        serde_json::json!({"key":{"eq":"ENG"}})
    );
    assert_eq!(
        serde_json::to_value(read::query_team_filter(&two)).unwrap(),
        serde_json::json!({"or":[{"key":{"eq":"ENG"}},{"key":{"eq":"OPS"}}]})
    );
    assert_eq!(
        serde_json::to_value(read::team_filter(&one, false)).unwrap(),
        serde_json::json!({"key":{"in":["ENG"]}})
    );
}

#[test]
fn terminal_comment_roots_with_replies_preserve_exact_source_separator() {
    let options = linear_cli::platform::markdown_terminal::RenderOptions::for_terminal(
        std::num::NonZeroU16::new(80).unwrap(),
        false,
        None,
    );
    let rendered = format!(
        "{}\n",
        view::terminal(
            &issue(),
            &Default::default(),
            true,
            chrono::Utc::now(),
            &options,
            false
        )
        .unwrap()
    );
    let (_, tail) = rendered.split_once("## Comments").unwrap();
    assert_eq!(
        tail,
        local_comment_calendar(
            "\n\n@Dummy Person commented 1/1/2000 [thread: root]\nRoot body\n\n\n  @Dummy Person commented 1/3/2000\n  Reply\n  secondline\n  \n  @Dummy Person commented 1/4/2000\n  Grandchild\n  \n\n@Dummy Person commented 1/2/2000 [thread: resolved] [resolved]\nHidden thread\n\n"
        )
    );
}
