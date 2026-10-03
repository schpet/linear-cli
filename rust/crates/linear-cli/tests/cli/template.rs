//! The `template` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json, nodes};
use crate::team::{resolve_vars, resolved};

const BUG_ID: &str = "11111111-1111-4111-8111-111111111111";

fn template(id: &str, name: &str, kind: &str, team: Option<(&str, &str)>) -> Value {
    json!({
        "id": id, "name": name, "description": format!("{name} description"),
        "type": kind, "icon": null, "color": null, "hasFormFields": false,
        "lastAppliedAt": null, "sortOrder": 0,
        "createdAt": "2024-01-01T12:00:00.000Z", "updatedAt": "2024-01-02T12:00:00.000Z",
        "team": team.map(|(id, key)| json!({ "id": id, "key": key, "name": format!("Team {key}") })),
        "inheritedFrom": null,
        "creator": { "id": "user-1", "name": "Sam" },
        "templateData": "{\"title\":\"Bug: \",\"priority\":2}"
    })
}

fn bug_report() -> Value {
    template(BUG_ID, "Bug report", "issue", Some(("team-eng-id", "ENG")))
}

fn all_templates() -> Value {
    json!({ "templates": [
        bug_report(),
        template("tpl-ops-bug", "Ops bug", "issue", Some(("team-ops-id", "OPS"))),
        template("tpl-kickoff", "Kickoff", "project", None),
        template("tpl-design", "Design doc", "document", Some(("team-eng-id", "ENG"))),
    ] })
}

fn ids(json: &Value) -> Vec<String> {
    let mut ids: Vec<String> = nodes(json)
        .iter()
        .map(|template| template["id"].as_str().expect("template id").to_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn list_json_contains_every_template() {
    let api = MockLinear::start();
    api.on("GetTemplates", all_templates());
    let json = Cli::for_api(&api)
        .run(&["template", "list", "--json"])
        .success()
        .json();
    assert_eq!(
        ids(&json),
        [BUG_ID, "tpl-design", "tpl-kickoff", "tpl-ops-bug"]
    );
    let listed = nodes(&json);
    let bug = listed
        .iter()
        .find(|t| t["id"] == BUG_ID)
        .expect("bug report listed");
    assert_json(bug, &bug_report());
    assert_eq!(api.variables("GetTemplates"), Value::Null);
}

#[test]
fn list_filters_by_type() {
    let api = MockLinear::start();
    api.on("GetTemplates", all_templates());
    let json = Cli::for_api(&api)
        .run(&["template", "list", "--type", "issue", "--json"])
        .success()
        .json();
    assert_eq!(ids(&json), [BUG_ID, "tpl-ops-bug"]);
}

#[test]
fn list_for_a_team_includes_workspace_templates() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng-id", "ENG", "Engineering"))
        .on("GetTemplates", all_templates());
    let json = Cli::for_api(&api)
        .run(&["template", "list", "--team", "ENG", "--json"])
        .success()
        .json();
    assert_eq!(ids(&json), [BUG_ID, "tpl-design", "tpl-kickoff"]);
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
}

#[test]
fn list_text_shows_names() {
    let api = MockLinear::start();
    api.on("GetTemplates", all_templates());
    Cli::for_api(&api)
        .run(&["template", "list"])
        .success()
        .stdout_has("Bug report")
        .stdout_has("Kickoff");
}

#[test]
fn list_rejects_an_unknown_type_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["template", "list", "--type", "initiative"])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn view_by_id_prints_the_template_json() {
    let api = MockLinear::start();
    api.on("GetTemplate", json!({ "template": bug_report() }));
    let json = Cli::for_api(&api)
        .run(&["template", "view", BUG_ID, "--json"])
        .success()
        .json();
    assert_json(&json, &bug_report());
    assert_eq!(api.variables("GetTemplate"), json!({ "id": BUG_ID }));
}

#[test]
fn view_by_name_searches_all_templates() {
    let api = MockLinear::start();
    api.on("GetTemplates", all_templates());
    let json = Cli::for_api(&api)
        .run(&["template", "v", "Kickoff", "-j"])
        .success()
        .json();
    assert_eq!(json["id"], "tpl-kickoff");
}

#[test]
fn view_text_shows_what_the_template_fills_in() {
    let api = MockLinear::start();
    api.on("GetTemplate", json!({ "template": bug_report() }));
    Cli::for_api(&api)
        .run(&["template", "view", BUG_ID])
        .success()
        .stdout_has("Bug report")
        .stdout_has("Bug: ");
}

#[test]
fn view_unknown_name_fails() {
    let api = MockLinear::start();
    api.on("GetTemplates", all_templates());
    Cli::for_api(&api)
        .run(&["template", "view", "Nonexistent"])
        .failure()
        .stderr_has("Nonexistent");
}

#[test]
fn list_text_is_a_table() {
    let api = MockLinear::start();
    api.on(
        "GetTemplates",
        json!({ "templates": [template("tpl-kickoff", "Kickoff", "project", None)] }),
    );
    let run = Cli::for_api(&api).run(&["template", "list"]);
    run.success();
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert!(lines[0].starts_with("ID "), "{run}");
    assert!(lines[1].starts_with("tpl-kickoff "), "{run}");
    assert!(lines[1].contains("Kickoff"), "{run}");
    assert!(lines[1].contains("Workspace"), "{run}");
    assert_eq!(lines.len(), 2, "{run}");
}

#[test]
fn view_text_lists_pre_fills_with_bodies_last() {
    let api = MockLinear::start();
    let mut bug = bug_report();
    let body = json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "Steps to reproduce" }] }
    ] });
    bug["templateData"] = json!(
        json!({
            "descriptionData": body,
            "title": "Bug: ",
            "priority": 2,
            "labelIds": ["label-1", "label-2"],
            "subIssueData": [{ "title": "Triage", "priority": 1 }]
        })
        .to_string()
    );
    api.on("GetTemplate", json!({ "template": bug }));
    let run = Cli::for_api(&api).run(&["template", "view", BUG_ID]);
    run.success();
    assert!(
        run.stdout.starts_with(&format!(
            "Bug report\nIssue template · Team ENG (Team ENG)\nID: {BUG_ID}\nDescription: Bug report description\nCreated by: Sam\n"
        )),
        "{run}"
    );
    let pre_fills = run
        .stdout
        .split_once("Pre-fills:\n")
        .expect("pre-fills heading")
        .1;
    assert!(
        pre_fills.starts_with(
            "  title: Bug: \n  priority: 2 (high)\n  labelIds: label-1, label-2\n  subIssueData: 1 item\n    - Triage\n        priority: 1 (urgent)\n  descriptionData:\n    Steps to reproduce\n"
        ),
        "{run}"
    );
}

#[test]
fn view_unknown_id_is_not_found() {
    let api = MockLinear::start();
    api.on_error("GetTemplate", "Entity not found")
        .on("GetTemplates", json!({ "templates": [] }));
    Cli::for_api(&api)
        .run(&["template", "view", BUG_ID])
        .failure()
        .stderr_has(&format!("Template not found: {BUG_ID}"))
        .stderr_has("linear template list");
}

#[test]
fn view_reports_the_error_for_a_template_that_exists() {
    let api = MockLinear::start();
    api.on_error("GetTemplate", "Rate limited")
        .on("GetTemplates", json!({ "templates": [bug_report()] }));
    Cli::for_api(&api)
        .run(&["template", "view", BUG_ID])
        .failure()
        .stderr_has("Rate limited");
}

#[test]
fn view_ambiguous_name_lists_the_ids() {
    let api = MockLinear::start();
    api.on(
        "GetTemplates",
        json!({ "templates": [
            bug_report(),
            template("tpl-ops-bug", "bug report", "issue", Some(("team-ops-id", "OPS"))),
        ] }),
    );
    Cli::for_api(&api)
        .run(&["template", "view", "Bug Report"])
        .failure()
        .stderr_has("ambiguous")
        .stderr_has(BUG_ID)
        .stderr_has("tpl-ops-bug");
}

#[test]
fn view_rejects_a_linear_url_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["template", "view", "https://linear.app/acme/issue/ENG-1"])
        .failure();
    assert!(api.requests().is_empty());
}
