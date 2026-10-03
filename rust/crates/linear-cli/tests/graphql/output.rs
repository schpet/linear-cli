use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::teams::GetTeams;

const TEAMS_BODY: &str = r##"{
  "data": {
    "teams": {
      "nodes": [
        {
          "id": "team-1",
          "name": "Engineering",
          "key": "ENG",
          "description": null,
          "icon": "Code",
          "color": "#0f7488",
          "cyclesEnabled": true,
          "createdAt": "2024-01-02T03:04:05.678Z",
          "updatedAt": "2025-06-07T08:09:10.111Z",
          "archivedAt": null,
          "organization": { "id": "org-1", "name": "Acme" }
        },
        {
          "id": "team-2",
          "name": "Design",
          "key": "DES",
          "description": "Design system",
          "icon": null,
          "color": null,
          "cyclesEnabled": false,
          "createdAt": "2024-01-02T03:04:05.678Z",
          "updatedAt": "2025-06-07T08:09:10.111Z",
          "archivedAt": "2025-08-01T00:00:00.000Z",
          "organization": { "id": "org-1", "name": "Acme" }
        }
      ],
      "pageInfo": { "hasNextPage": false, "endCursor": null }
    }
  }
}"##;

const GOLDEN_TEAMS_JSON: &str = r##"[
  {
    "id": "team-1",
    "name": "Engineering",
    "key": "ENG",
    "description": null,
    "icon": "Code",
    "color": "#0f7488",
    "cyclesEnabled": true,
    "createdAt": "2024-01-02T03:04:05.678Z",
    "updatedAt": "2025-06-07T08:09:10.111Z",
    "archivedAt": null,
    "organization": {
      "id": "org-1",
      "name": "Acme"
    }
  },
  {
    "id": "team-2",
    "name": "Design",
    "key": "DES",
    "description": "Design system",
    "icon": null,
    "color": null,
    "cyclesEnabled": false,
    "createdAt": "2024-01-02T03:04:05.678Z",
    "updatedAt": "2025-06-07T08:09:10.111Z",
    "archivedAt": "2025-08-01T00:00:00.000Z",
    "organization": {
      "id": "org-1",
      "name": "Acme"
    }
  }
]"##;

#[test]
fn entity_json_output_keeps_graphql_names_document_order_and_nulls() {
    let data: GetTeams = parse_response(TEAMS_BODY.as_bytes()).expect("teams");
    assert_eq!(data.teams.nodes.len(), 2);
    assert!(data.teams.nodes[0].archived_at.is_none());
    assert_eq!(
        data.teams.nodes[1]
            .archived_at
            .as_ref()
            .map(|at| at.0.as_str()),
        Some("2025-08-01T00:00:00.000Z")
    );
    let rendered = serde_json::to_string_pretty(&data.teams.nodes).expect("pretty");
    assert_eq!(rendered, GOLDEN_TEAMS_JSON);
}

#[test]
fn connection_with_cursor_and_empty_nodes_parses() {
    let body =
        r#"{"data":{"teams":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"abc"}}}}"#;
    let data: GetTeams = parse_response(body.as_bytes()).expect("teams");
    assert_eq!(data.teams.page_info.end_cursor.as_deref(), Some("abc"));
    assert!(data.teams.page_info.has_next_page);
    assert!(data.teams.nodes.is_empty());
}

#[test]
fn missing_non_null_field_is_rejected_rather_than_defaulted() {
    let body = r#"{"data":{"teams":{"nodes":[{"id":"t","name":"N","key":"K","description":null,"icon":null,"color":null,"cyclesEnabled":true,"createdAt":"2024-01-01T00:00:00.000Z","updatedAt":"2024-01-01T00:00:00.000Z","archivedAt":null}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#;
    let error = parse_response::<GetTeams>(body.as_bytes()).expect_err("organization missing");
    assert!(error.to_string().contains("organization"), "{error}");
}
