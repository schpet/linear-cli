use linear_cli::{
    commands::{issue_read as read, issue_view as view},
    graphql::{envelope::parse_response, operations::issue_read::*},
    platform::markdown_assets,
};
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};
const MINE: &str =
    include_str!("../../../../parity/runner/c060-c062-frozen-cases/c060-default-table.json");
const QUERY: &str = include_str!(
    "../../../../parity/runner/c060-c062-frozen-cases/c061-allteams-filter-pages.json"
);
const SEARCH: &str =
    include_str!("../../../../parity/runner/c060-c062-frozen-cases/c061-search-pages-json.json");
const VIEW: &str =
    include_str!("../../../../parity/runner/c060-c062-frozen-cases/c062-markdown-all-threads.json");
fn data(case: &str) -> Value {
    serde_json::from_str::<Value>(case).unwrap()["graphql"]["groups"][0]["steps"][0]["response"]["data"].clone()
}
fn issue() -> view::Issue {
    parse_response::<GetIssueDetailsWithComments>(json!({"data":data(VIEW)}).to_string().as_bytes())
        .unwrap()
        .issue
}
#[test]
fn strict_date_accepts_only_complete_ascii_real_calendar_and_utc_milliseconds() {
    for (value, expected) in [
        ("2024-02-29", "2024-02-29T00:00:00.000Z"),
        ("2026-01-02T03:04:05+02:30", "2026-01-02T00:34:05.000Z"),
        ("2026-01-02T03:04:05-05:00", "2026-01-02T08:04:05.000Z"),
        ("2026-01-02T03:04:05.1239Z", "2026-01-02T03:04:05.123Z"),
        ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00.000Z"),
        ("9999-12-31T23:59:59.999Z", "9999-12-31T23:59:59.999Z"),
    ] {
        assert_eq!(
            read::date_filter(value, "--created-after").unwrap().0,
            expected
        );
    }
    for value in [
        "2026-02-30",
        "2025-02-29",
        "2026-01-02T24:00:00Z",
        "2026-13-01",
        "2026-01-02T03:04:60Z",
        "2026-01-32",
        "2026-01-02T24:00:01Z",
    ] {
        let e = read::date_filter(value, "--created-after").unwrap_err();
        assert_eq!(
            e.message,
            format!("Invalid date for --created-after: \"{value}\"")
        );
        assert!(
            e.suggestion
                .unwrap()
                .starts_with("Use YYYY-MM-DD or ISO 8601 format")
        );
    }
    for value in [
        "2026-01-02t03:04:05Z",
        "2026-01-02T03:04:05z",
        "2026-01-02 03:04:05Z",
        "2026-01-02T03:04:05−05:00",
        "2026-01",
        "yesterday",
        "2026-01-02T03:04Z",
        "2026-01-02T03:04:05",
        "2026-01-02T03:04:05+0200",
        "0000-01-01T00:00:00+01:00",
        "9999-12-31T23:00:00-05:00",
        "2026-01-02T03:04:05.Z",
    ] {
        assert_eq!(
            read::date_filter(value, "--updated-after")
                .unwrap_err()
                .message,
            format!("Invalid date format for --updated-after: \"{value}\"")
        );
    }
    let mut filter = IssueFilter::default();
    assert!(
        read::apply_dates(&mut filter, Some("yesterday"), Some("tomorrow"))
            .unwrap_err()
            .message
            .contains("--created-after")
    );
}
#[test]
fn complete_typed_connections_preserve_json_and_binary64_metadata() {
    let query: GetIssuesForQuery = serde_json::from_value(data(QUERY)).unwrap();
    let json = serde_json::to_value(&query.issues).unwrap();
    assert!(json.get("nodes").unwrap().is_array());
    assert!(json.get("pageInfo").unwrap().is_object());
    assert!(json.get("issues").is_none());
    let mut i = issue();
    i.attachments.nodes[0]
        .metadata
        .0
        .insert("integer".to_owned(), json!(9007199254740993_u64));
    i.attachments.nodes[0]
        .metadata
        .0
        .insert("zero".to_owned(), json!(-0.0));
    let out = view::Fetched::With(i).json().unwrap();
    assert!(out.contains("\"integer\": 9007199254740992"));
    assert!(out.contains("\"zero\": 0"));
    assert!(out.contains("\"quotedText\": null"));
    for mutation in ["priority", "identifier", "comments"] {
        let mut wrong = data(VIEW);
        wrong["issue"].as_object_mut().unwrap().remove(mutation);
        assert!(serde_json::from_value::<GetIssueDetailsWithComments>(wrong).is_err());
    }
}
#[test]
fn workflow_sort_uses_actual_returned_teams_and_stable_position_desc() {
    let source: GetIssuesForState = serde_json::from_value(data(MINE)).unwrap();
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
        r.state.position = position;
        rows.push(r);
    }
    read::sort_mine(&mut rows).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.identifier.as_str())
            .collect::<Vec<_>>(),
        ["started", "high", "tie", "low"]
    );
    rows[0].team.key = "OTHER".to_owned();
    rows.rotate_right(1);
    read::sort_mine(&mut rows).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.identifier.as_str())
            .collect::<Vec<_>>(),
        ["started", "low", "high", "tie"]
    );
    rows[0].state.position = f64::NAN;
    assert!(read::sort_mine(&mut rows).is_err());
}
#[test]
fn exact_pipe_table_and_clock_thresholds_match_frozen_mine() {
    let mut source: GetIssuesForState = serde_json::from_value(data(MINE)).unwrap();
    read::sort_mine(&mut source.issues.nodes).unwrap();
    let rows = source
        .issues
        .nodes
        .into_iter()
        .map(read::TableRow::from)
        .collect::<Vec<_>>();
    let expected = serde_json::from_str::<Value>(MINE).unwrap()["expected"]["stdout"]["utf8"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        format!(
            "{}\n",
            read::table(
                &rows,
                true,
                false,
                false,
                120,
                false,
                UNIX_EPOCH + Duration::from_secs(1_000_000)
            )
            .unwrap()
        ),
        expected
    );
    let now = UNIX_EPOCH + Duration::from_secs(86_400 * 50000);
    let mut row = rows[0].clone();
    row.updated = chrono::DateTime::<chrono::Utc>::from(now)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    row.estimate = Some(0.0);
    let mut short = read::table(&[row.clone()], false, true, true, 40, false, now).unwrap();
    assert!(short.contains("just now"));
    assert!(short.contains(" 0 "));
    row.updated = chrono::DateTime::<chrono::Utc>::from(now - Duration::from_secs(3600))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    short = read::table(&[row], false, false, false, 120, false, now).unwrap();
    assert!(short.contains("1 hour ago"));
    assert_eq!(read::priority(4.0).unwrap(), "▄  ");
    assert_eq!(read::priority(9.0).unwrap(), "9");
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
            .message
            .contains("cycle")
    );
}
// These hierarchy/separator fixtures were captured in UTC. Keep every non-date
// byte exact while expecting the host-local calendar used by the command.
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
                local.format("%-m/%-d/%Y").to_string(),
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
#[tokio::test]
async fn source_array_images_then_links_per_body_dedup_first_alt_and_cache_hits() {
    let dir = std::env::temp_dir().join(format!("linear-issue-read-assets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut fetched = vec![];
    let sources = [
        "![first](data:text/plain,one) [link](https://uploads.linear.app/link)",
        "![later](data:text/plain,one) ![second](data:text/plain,two)",
    ];
    let paths = markdown_assets::download_sources_with(
        &sources,
        &dir,
        |url| {
            fetched.push(url.clone());
            std::future::ready(Ok(url.into_bytes()))
        },
        |_| panic!("no failures"),
    )
    .await
    .unwrap()
    .paths;
    assert_eq!(
        fetched,
        [
            "data:text/plain,one",
            "https://uploads.linear.app/link",
            "data:text/plain,two"
        ]
    );
    assert!(paths["data:text/plain,one"].ends_with("/first"));
    fetched.clear();
    markdown_assets::download_sources_with(
        &sources,
        &dir,
        |url| {
            fetched.push(url);
            std::future::ready(Ok(vec![]))
        },
        |_| panic!("no failures"),
    )
    .await
    .unwrap();
    assert!(fetched.is_empty());
    assert_eq!(markdown_assets::sanitized_attachment_filename(""), "");
    assert_eq!(markdown_assets::sanitized_filename(None), "image");
    assert_eq!(
        markdown_assets::sanitized_attachment_filename("report:?.bin"),
        "report.bin"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn source_search_selected_state_has_no_position_and_preserves_total_count() {
    let result: SearchIssues = serde_json::from_value(data(SEARCH)).unwrap();
    let out = serde_json::to_value(result.search_issues).unwrap();
    assert!(out["nodes"][0]["state"].get("position").is_none());
    assert!(out.get("totalCount").is_some());
    assert!(out["nodes"][0].get("metadata").is_some());
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
fn source_valid_project_menu_controls_have_command_validation_before_raw() {
    read::project_menu_text("Similar projects?", &["Planning", "none of the above"]).unwrap();
    for (message, label) in [
        ("Similar projects?", ""),
        ("Similar projects?", "A\nB"),
        ("Plan\r?", "yes"),
    ] {
        let error = read::project_menu_text(message, &[label]).unwrap_err();
        assert_eq!(error.kind, linear_cli::error::AppErrorKind::Validation);
        assert!(error.message.contains("no control characters"));
    }
}

#[test]
fn terminal_comment_roots_with_replies_preserve_exact_source_separator() {
    let options = linear_cli::platform::markdown_terminal::RenderOptions {
        columns: std::num::NonZeroU16::new(80).unwrap(),
        styled: false,
        image_hyperlinks: None,
    };
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
