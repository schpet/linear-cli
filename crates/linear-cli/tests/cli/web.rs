//! `--web` / `--app`: commands hand a Linear URL to the platform opener (`open` on macOS,
//! `xdg-open` elsewhere), both stubbed here.
use serde_json::json;

use crate::support::{Cli, MockLinear};

pub fn open_stubs(cli: Cli) -> Cli {
    cli.stub_bin("open", "exit 0")
        .stub_bin("xdg-open", "exit 0")
}

/// The URL of every opener invocation.
pub fn opened(cli: &Cli) -> Vec<String> {
    let mut calls = cli.calls("open");
    calls.extend(cli.calls("xdg-open"));
    calls
        .into_iter()
        .map(|argv| argv.last().cloned().expect("opener got a URL"))
        .collect()
}

/// A sandbox for `api` whose workspace is configured, so URLs can be built without a lookup.
fn configured(api: &MockLinear) -> Cli {
    open_stubs(Cli::for_api(api).env("LINEAR_WORKSPACE", "acme"))
}

#[test]
fn issue_view_opens_the_issue_without_a_request() {
    let api = MockLinear::start();
    let cli = configured(&api);
    cli.run(&["issue", "view", "ENG-1", "--web"])
        .success()
        .stdout_has("https://linear.app/acme/issue/ENG-1");
    cli.run(&["issue", "view", "ENG-1", "--app"])
        .success()
        .stdout_has("Linear.app");
    assert_eq!(
        opened(&cli),
        [
            "https://linear.app/acme/issue/ENG-1",
            "https://linear.app/acme/issue/ENG-1"
        ]
    );
}

#[test]
fn issue_list_opens_the_team_issue_view() {
    let api = MockLinear::start();
    let cli = configured(&api).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "list", "--web"]).success();
    let urls = opened(&cli);
    assert_eq!(urls.len(), 1);
    assert!(
        urls[0].starts_with("https://linear.app/acme/team/ENG/"),
        "{urls:?}"
    );
}

#[test]
fn project_view_opens_the_project() {
    const ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";
    let api = MockLinear::start();
    let cli = configured(&api);
    cli.run(&["project", "view", ID, "--web"]).success();
    cli.run(&["project", "view", ID, "--app"])
        .success()
        .stdout_has("Linear.app");
    let url = format!("https://linear.app/acme/project/{ID}");
    assert_eq!(opened(&cli), [url.clone(), url]);
}

#[test]
fn project_list_opens_the_team_projects() {
    let api = MockLinear::start();
    let cli = configured(&api).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["project", "list", "--app"]).success();
    assert_eq!(
        opened(&cli),
        ["https://linear.app/acme/team/ENG/projects/all"]
    );
}

#[test]
fn project_list_looks_up_the_workspace_when_unconfigured() {
    let api = MockLinear::start();
    api.on(
        "GetViewer",
        json!({ "viewer": { "organization": { "urlKey": "acme" } } }),
    );
    let cli = open_stubs(Cli::for_api(&api)).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["project", "list", "--web"]).success();
    assert_eq!(
        opened(&cli),
        ["https://linear.app/acme/team/ENG/projects/all"]
    );
}

#[test]
fn team_list_opens_team_settings() {
    let api = MockLinear::start();
    let cli = configured(&api);
    cli.run(&["team", "list", "--web"]).success();
    assert_eq!(opened(&cli), ["https://linear.app/acme/settings/teams"]);
}

#[test]
fn document_view_opens_the_document_url() {
    let url = "https://linear.app/acme/document/design-notes-d0c5a1b2c3d4";
    let api = MockLinear::start();
    api.on(
        "GetDocument",
        json!({ "document": {
            "id": "doc-1", "title": "Design notes", "slugId": "d0c5a1b2c3d4",
            "content": "# Heading\n", "url": url,
            "createdAt": "2024-01-02T00:00:00.000Z", "updatedAt": "2024-01-03T00:00:00.000Z",
            "creator": { "name": "Ada", "email": "ada@example.com" },
            "project": null, "issue": null, "initiative": null, "team": null, "cycle": null,
            "release": null
        } }),
    );
    let cli = open_stubs(Cli::for_api(&api));
    cli.run(&["document", "view", "d0c5a1b2c3d4", "--web"])
        .success();
    assert_eq!(opened(&cli), [url]);
    assert_eq!(
        api.variables("GetDocument"),
        json!({ "id": "d0c5a1b2c3d4" })
    );
}

#[test]
fn opener_failures_fail_the_command() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api)
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("open", "exit 3")
        .stub_bin("xdg-open", "exit 3");
    cli.run(&["issue", "view", "ENG-1", "--web"]).failure();
}

#[test]
fn project_view_looks_up_the_workspace_for_an_environment_key() {
    const ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";
    let api = MockLinear::start();
    api.on(
        "GetViewer",
        json!({ "viewer": { "organization": { "urlKey": "acme" } } }),
    );
    let cli = open_stubs(Cli::for_api(&api));
    cli.run(&["project", "view", ID, "--web"]).success();
    assert_eq!(
        opened(&cli),
        [format!("https://linear.app/acme/project/{ID}")]
    );
}

#[test]
fn project_list_opens_the_projects_of_a_named_team() {
    let api = MockLinear::start();
    api.on(
        "ResolveTeam",
        json!({ "teams": { "nodes": [{ "id": "team-1", "key": "OPS", "name": "Operations" }] } }),
    );
    let cli = configured(&api);
    cli.run(&["project", "list", "--team", "Operations", "--web"])
        .success();
    assert_eq!(
        opened(&cli),
        ["https://linear.app/acme/team/OPS/projects/all"]
    );
}
