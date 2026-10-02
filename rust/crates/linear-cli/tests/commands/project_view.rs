use chrono::Utc;
use linear_cli::commands::project_view::{json, markdown, picker_options};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::project_view::{GetProjectDetails, GetProjectsForPicker};
use serde_json::{Value, json as value};

fn case(id: &str) -> Value {
    let file = format!(
        "{}/../../parity/runner/c024-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(file).expect("frozen case")).expect("case JSON")
}

fn details(id: &str) -> linear_cli::graphql::operations::project_view::ProjectDetails {
    let fixture = case(id);
    let project = fixture["graphql"]["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .flat_map(|group| group["steps"].as_array().expect("steps"))
        .find(|step| step["id"] == "GetProjectDetails")
        .expect("detail step")["response"]["data"]["project"]
        .clone();
    let response: GetProjectDetails =
        parse_response(value!({"data":{"project":project}}).to_string().as_bytes())
            .expect("typed project");
    response.project.expect("project")
}

#[test]
fn frozen_minimal_and_rich_json_preserve_graphql_shape_and_number_bytes() {
    for id in ["c024-minimal-json", "c024-rich-json"] {
        let project = details(id);
        let actual = String::from_utf8(json(&project).expect("JSON output")).expect("UTF-8");
        let expected = case(id)["expected"]["stdout"]["utf8"]
            .as_str()
            .expect("expected output")
            .to_owned();
        assert_eq!(actual, expected, "{id}");
    }
    let rich =
        String::from_utf8(json(&details("c024-rich-json")).expect("rich JSON")).expect("UTF-8");
    assert!(rich.contains("\"scope\": 8,"));
    assert!(rich.contains("\"progress\": 0.3125,"));
    assert!(rich.contains("\"sortOrder\": -3"));
    assert!(rich.contains("\"sourceType\": \"ci\""));
}

#[test]
fn frozen_minimal_rich_and_truncated_markdown_match_source_sections() {
    let now = "2026-09-28T12:00:00Z".parse().expect("clock");
    for id in ["c024-minimal-text", "c024-rich-text", "c024-truncated-text"] {
        let rendered = format!("{}\n", markdown(&details(id), now, &Utc).expect("markdown"));
        let expected = case(id)["expected"]["stdout"]["utf8"]
            .as_str()
            .expect("expected output")
            .to_owned();
        assert_eq!(rendered, expected, "{id}");
    }
}

#[test]
fn empty_attachment_source_type_omits_the_label() {
    let now = "2026-09-28T12:00:00Z".parse().expect("clock");
    let mut project = details("c024-rich-text");
    let with_source = markdown(&project, now, &Utc).expect("rich Markdown");
    assert!(with_source.contains(" _[ci]_"));
    project
        .attachments
        .nodes
        .first_mut()
        .expect("rich attachment")
        .source_type = Some(String::new());
    let without_source = markdown(&project, now, &Utc).expect("empty source Markdown");
    assert_eq!(without_source, with_source.replace(" _[ci]_", ""));
}

#[test]
fn picker_sorts_case_insensitively_and_keeps_uuid_values() {
    let data = value!({"data":{"projects":{"nodes":[
        {"id":"z", "name":"Zeta", "slugId":"z-1", "status":{"name":"Started"}, "teams":{"nodes":[{"key":"ENG"}]}},
        {"id":"a", "name":"alpha", "slugId":"a-1", "status":{"name":"Backlog"}, "teams":{"nodes":[]}}
    ],"pageInfo":{"hasNextPage":false,"endCursor":null}}}});
    let response: GetProjectsForPicker =
        parse_response(data.to_string().as_bytes()).expect("picker response");
    let options = picker_options(&response.projects.nodes);
    assert_eq!(options[0].label, "alpha  ·  Backlog  ·  a-1");
    assert_eq!(options[0].value, "a");
    assert_eq!(options[1].label, "Zeta  ·  Started  ·  ENG  ·  z-1");
    assert_eq!(options[1].value, "z");
}

#[test]
fn schema_invalid_float_fixtures_are_rejected() {
    for id in [
        "c024-one-null-sort-json",
        "c024-one-node-null-sort-text",
        "c024-two-null-sort-text",
        "c024-overflow-json",
        "c024-overflow-text",
    ] {
        let fixture = case(id);
        let step = fixture["graphql"]["groups"]
            .as_array()
            .expect("groups")
            .iter()
            .flat_map(|group| group["steps"].as_array().expect("steps"))
            .find(|step| step["id"] == "GetProjectDetails")
            .expect("details");
        let response = step["response"]["body"]["utf8"]
            .as_str()
            .expect("raw response");
        let error = parse_response::<GetProjectDetails>(response.as_bytes()).expect_err(id);
        let diagnostic = error.to_string();
        if id.starts_with("c024-overflow") {
            assert!(
                diagnostic.contains("number out of range"),
                "{id}: {diagnostic}"
            );
        } else {
            assert!(
                diagnostic.contains("invalid type: null, expected a JSON number"),
                "{id}: {diagnostic}"
            );
        }
    }
}
