use std::future::ready;

use linear_cli::commands::team_states::{render_text, run_with};
use linear_cli::graphql::envelope::{ResponseError, parse_response};
use linear_cli::graphql::operations::workflow_states::GetWorkflowStates;
use serde_json::json;

fn response(nodes: &str) -> GetWorkflowStates {
    let body = format!("{{\"data\":{{\"team\":{{\"states\":{{\"nodes\":{nodes}}}}}}}}}");
    parse_response(body.as_bytes()).expect("typed state response")
}

#[tokio::test]
async fn request_and_json_follow_the_source_connection_contract() {
    let received = response(
        r#"[{"id":"completed","name":"Done","type":"completed","position":-0},{"id":"started","name":"In progress","type":"started","position":900}]"#,
    );
    let output = run_with("ENG".into(), true, false, |wire| {
        assert_eq!(
            serde_json::to_value(&wire).unwrap()["variables"],
            json!({"teamKey":"ENG"})
        );
        let query = wire.query.split_whitespace().collect::<String>();
        assert_eq!(query, "queryGetWorkflowStates($teamKey:String!){team(id:$teamKey){states{nodes{idnametypeposition}}}}");
        ready(Ok(received))
    })
    .await
    .expect("state JSON");
    let json = String::from_utf8(output).unwrap();
    assert_eq!(
        json,
        "{\n  \"nodes\": [\n    {\n      \"id\": \"started\",\n      \"name\": \"In progress\",\n      \"type\": \"started\",\n      \"position\": 900\n    },\n    {\n      \"id\": \"completed\",\n      \"name\": \"Done\",\n      \"type\": \"completed\",\n      \"position\": 0\n    }\n  ]\n}\n"
    );
}

#[test]
fn stable_order_uses_type_rank_collation_then_descending_position() {
    let mut states = response(
        r#"[
      {"id":"future-lower","name":"Future 1","type":"é","position":1},
      {"id":"equal-a","name":"First","type":"started","position":1},
      {"id":"future-upper","name":"Future 2","type":"é","position":2},
      {"id":"equal-b","name":"Second","type":"started","position":1},
      {"id":"triage","name":"Triage","type":"triage","position":-1},
      {"id":"started-high","name":"High","type":"started","position":2}
    ]"#,
    )
    .team
    .states
    .nodes;
    linear_cli::workflow_states::sort(&mut states).unwrap();
    let ids = states
        .iter()
        .map(|state| state.id.inner())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "triage",
            "started-high",
            "equal-a",
            "equal-b",
            "future-upper",
            "future-lower"
        ]
    );
}

#[test]
fn table_padding_and_independent_header_underlines() {
    let states = response(
        r#"[
      {"id":"hexagrams","name":"䷀䷀䷀","type":"started","position":2},
      {"id":"wide","name":"漢字","type":"started","position":1}
    ]"#,
    )
    .team
    .states
    .nodes;
    assert_eq!(
        render_text(&states, false),
        "NAME   TYPE   \n䷀䷀䷀ started\n漢字   started\n"
    );
    assert_eq!(
        render_text(&states, true),
        "\x1b[4mNAME  \x1b[24m \x1b[4mTYPE   \x1b[24m\x1b[0m\n䷀䷀䷀ started\n漢字   started\n"
    );
    assert_eq!(
        render_text(&[], false),
        "No workflow states found for this team.\n"
    );
}

#[test]
fn malformed_position_and_extra_fields_have_typed_boundaries() {
    for position in ["null", "\"1\""] {
        let body = format!(
            r#"{{"data":{{"team":{{"states":{{"nodes":[{{"id":"a","name":"A","type":"started","position":{position}}}]}}}}}}}}"#
        );
        assert!(matches!(
            parse_response::<GetWorkflowStates>(body.as_bytes()),
            Err(ResponseError::UnexpectedShape(_))
        ));
    }
    let body = br#"{"data":{"team":{"states":{"nodes":[{"position":1e400,"type":"started","name":"A","id":"a"}]}}}}"#;
    assert!(matches!(
        parse_response::<GetWorkflowStates>(body),
        Err(ResponseError::MalformedJson(_))
    ));
    let body = r#"{"data":{"team":{"states":{"nodes":[{"position":1,"extra":true,"type":"started","name":"A","id":"a"}]}}}}"#;
    let typed: GetWorkflowStates = parse_response(body.as_bytes()).unwrap();
    assert_eq!(typed.team.states.nodes.len(), 1);
}

#[tokio::test]
async fn positions_keep_their_numbers() {
    let states = response(
        r#"[
      {"id":"a","name":"A","type":"started","position":900},
      {"id":"b","name":"B","type":"started","position":-0},
      {"id":"c","name":"C","type":"started","position":0.000001},
      {"id":"d","name":"D","type":"started","position":1e21},
      {"id":"e","name":"E","type":"started","position":1e-7},
      {"id":"f","name":"F","type":"started","position":0.30000000000000004},
      {"id":"g","name":"G","type":"started","position":5e-324}
    ]"#,
    );
    let bytes = run_with("ENG".into(), true, false, |_| ready(Ok(states)))
        .await
        .unwrap();
    let output = String::from_utf8(bytes).unwrap();
    for literal in [
        "900",
        "0",
        "1e-6",
        "1e+21",
        "1e-7",
        "0.30000000000000004",
        "5e-324",
    ] {
        assert!(
            output.contains(&format!("\"position\": {literal}")),
            "missing {literal}: {output}"
        );
    }
}
