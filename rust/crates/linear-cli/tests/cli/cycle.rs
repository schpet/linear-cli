//! The `cycle` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json};
use crate::team::{resolve_vars, resolved};

const ENG_ID: &str = "team-eng-id";

fn cycle(id: &str, number: u32, name: Value, active: bool) -> Value {
    json!({
        "id": id, "number": number, "name": name,
        "startsAt": "2026-01-01T00:00:00.000Z", "endsAt": "2026-01-15T00:00:00.000Z",
        "completedAt": null, "isActive": active, "isFuture": false, "isPast": !active
    })
}

fn cycles_page(nodes: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({ "team": {
        "id": ENG_ID, "name": "Engineering",
        "cycles": { "nodes": nodes, "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor } }
    } })
}

#[test]
fn list_json_follows_pages_for_the_resolved_team() {
    let api = MockLinear::start();
    let old = cycle("old", 7, Value::Null, false);
    let new = cycle("new", 8, json!("Sprint 8"), true);
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCycles",
            cycles_page(vec![old.clone()], json!("cursor-1"), true),
        )
        .on(
            "GetTeamCycles",
            cycles_page(vec![new.clone()], json!("cursor-2"), false),
        );
    let mut nodes = Cli::for_api(&api)
        .run(&["cycle", "list", "--team", "ENG", "--json"])
        .success()
        .json_nodes();
    nodes.sort_by_key(|node| node["number"].as_u64());
    assert_eq!(nodes, [old, new]);
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    let pages: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("GetTeamCycles"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        pages,
        [
            json!({ "teamId": ENG_ID, "first": 100 }),
            json!({ "teamId": ENG_ID, "first": 100, "after": "cursor-1" }),
        ]
    );
}

#[test]
fn list_text_uses_the_configured_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCycles",
            cycles_page(
                vec![cycle("c1", 3, json!("Sprint Three"), true)],
                Value::Null,
                false,
            ),
        );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["cycle", "list"])
        .success()
        .stdout_has("Sprint Three");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
}

#[test]
fn list_without_a_team_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api).run(&["cycle", "list"]).failure();
    assert!(api.requests().is_empty());
}

fn lookup(active: Value) -> Value {
    json!({ "team": {
        "key": "ENG", "cyclesEnabled": true,
        "cycles": {
            "nodes": [{
                "id": "cycle-5", "number": 5, "name": "Sprint 5",
                "startsAt": "2026-02-10T00:00:00.000Z", "isNext": false, "isPrevious": false
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        },
        "activeCycle": active
    } })
}

fn details() -> Value {
    json!({ "cycle": {
        "id": "cycle-5", "number": 5, "name": "Sprint 5", "description": "Ship the parser.",
        "startsAt": "2026-02-10T00:00:00.000Z", "endsAt": "2026-02-24T00:00:00.000Z",
        "completedAt": null, "isActive": true, "isFuture": false, "isPast": false,
        "createdAt": "2026-01-01T00:00:00.000Z", "updatedAt": "2026-01-02T00:00:00.000Z",
        "team": { "id": ENG_ID, "key": "ENG", "name": "Engineering" },
        "issues": {
            "nodes": [{
                "id": "issue-1", "identifier": "ENG-1", "title": "Write the parser",
                "state": { "name": "Done", "type": "completed" }
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        }
    } })
}

#[test]
fn view_json_by_number_prints_the_cycle() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetTeamCyclesForLookup", lookup(Value::Null))
        .on("GetCycleDetails", details());
    let json = Cli::for_api(&api)
        .run(&["cycle", "view", "5", "--team", "ENG", "--json"])
        .success()
        .json();
    assert_json(&json, &details()["cycle"]);
    assert_eq!(
        api.variables("GetTeamCyclesForLookup"),
        json!({ "teamId": ENG_ID, "after": null })
    );
    assert_eq!(api.variables("GetCycleDetails"), json!({ "id": "cycle-5" }));
}

#[test]
fn view_active_uses_the_teams_active_cycle() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            lookup(json!({ "id": "cycle-5", "number": 5, "name": "Sprint 5" })),
        )
        .on("GetCycleDetails", details());
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["cycle", "view", "active"])
        .success()
        .stdout_has("Sprint 5")
        .stdout_has("ENG-1");
    assert_eq!(api.variables("GetCycleDetails"), json!({ "id": "cycle-5" }));
}

#[test]
fn view_unknown_cycle_fails_without_fetching_details() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetTeamCyclesForLookup", lookup(Value::Null));
    Cli::for_api(&api)
        .run(&["cycle", "view", "42", "--team", "ENG"])
        .failure()
        .stderr_has("42");
}

#[test]
fn view_rejects_a_cycle_url_number_outside_u32_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "cycle",
            "view",
            "https://linear.app/acme/team/eng/cycle/4294967296",
        ])
        .failure()
        .stderr_has("the largest cycle number is 4294967295");
    assert!(api.requests().is_empty());
}

#[test]
fn view_explains_numbers_that_cannot_be_cycle_numbers() {
    for (reference, reason) in [
        ("0", "cycle numbers start at 1"),
        ("4294967296", "the largest cycle number is 4294967295"),
    ] {
        let api = MockLinear::start();
        api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
            .on("GetTeamCyclesForLookup", lookup(Value::Null));
        Cli::for_api(&api)
            .run(&["cycle", "view", reference, "--team", "ENG"])
            .failure()
            .stderr_has(&format!("\"{reference}\" is not a cycle number: {reason}"));
        assert!(
            api.requests()
                .iter()
                .all(|request| request.operation.as_deref() != Some("GetCycleDetails"))
        );
    }
}

fn lookup_page(key: &str, enabled: bool, nodes: Value, end_cursor: Value) -> Value {
    json!({ "team": {
        "key": key, "cyclesEnabled": enabled,
        "cycles": {
            "nodes": nodes,
            "pageInfo": { "hasNextPage": !end_cursor.is_null(), "endCursor": end_cursor }
        },
        "activeCycle": null
    } })
}

fn lookup_cycle(id: &str, number: u32, name: &str) -> Value {
    json!({
        "id": id, "number": number, "name": name,
        "startsAt": "2026-02-10T00:00:00.000Z", "isNext": false, "isPrevious": false
    })
}

#[test]
fn view_reads_every_cycle_page_and_prefers_a_number_over_a_later_name() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            lookup_page(
                "ENG",
                true,
                json!([lookup_cycle("cycle-5", 5, "Other")]),
                json!("next"),
            ),
        )
        .on(
            "GetTeamCyclesForLookup",
            lookup_page(
                "ENG",
                true,
                json!([lookup_cycle("named-5", 6, "5")]),
                Value::Null,
            ),
        )
        .on("GetCycleDetails", details());
    Cli::for_api(&api)
        .run(&["cycle", "view", "5", "--team", "ENG", "--json"])
        .success();
    let pages: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("GetTeamCyclesForLookup"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        pages,
        [
            json!({ "teamId": ENG_ID, "after": null }),
            json!({ "teamId": ENG_ID, "after": "next" }),
        ]
    );
    assert_eq!(api.variables("GetCycleDetails"), json!({ "id": "cycle-5" }));
}

#[test]
fn view_fails_on_the_first_page_when_cycles_are_disabled() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            lookup_page("ENG", false, json!([]), json!("more")),
        );
    Cli::for_api(&api)
        .run(&["cycle", "view", "5", "--team", "ENG"])
        .failure()
        .stderr_has("Cycles are not enabled for team ENG");
}

#[test]
fn view_checks_a_cycle_url_team_against_the_working_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            lookup_page("ENG", true, json!([]), Value::Null),
        );
    Cli::for_api(&api)
        .run(&[
            "cycle",
            "view",
            "https://linear.app/acme/team/OPS/cycle/5",
            "--team",
            "ENG",
        ])
        .failure()
        .stderr_has("That cycle URL is for team OPS, but this command is working in team ENG.");
}

#[test]
fn view_takes_the_team_from_a_cycle_url() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            lookup_page(
                "eng",
                true,
                json!([lookup_cycle("cycle-5", 5, "Sprint 5")]),
                Value::Null,
            ),
        )
        .on("GetCycleDetails", details());
    Cli::for_api(&api)
        .run(&[
            "cycle",
            "view",
            "https://linear.app/acme/team/ENG/cycle/5",
            "--json",
        ])
        .success();
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
}
