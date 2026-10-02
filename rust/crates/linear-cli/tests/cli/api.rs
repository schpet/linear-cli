//! The raw `api` command and `schema`.
use serde_json::{Value, json};

use crate::support::{API_KEY, Cli, MockLinear};

const QUERY: &str = "query Probe($n: Int) { viewer { id } }";

fn viewer() -> Value {
    json!({ "viewer": { "id": "user-1" } })
}

#[test]
fn prints_the_response_and_sends_the_document() {
    let api = MockLinear::start();
    api.on("Probe", viewer());
    let run = Cli::for_api(&api).run(&["api", QUERY]);
    assert_eq!(run.success().json(), json!({ "data": viewer() }));
    let request = api.request("Probe");
    assert_eq!(request.query, QUERY);
    assert_eq!(request.header("authorization"), Some(API_KEY));
}

#[test]
fn variables_are_coerced_and_merged() {
    let api = MockLinear::start();
    api.on("Probe", viewer());
    Cli::for_api(&api)
        .file("cwd/body.md", "From a file")
        .run(&[
            "api",
            QUERY,
            "--variables-json",
            r#"{"n": 1, "kept": [1, 2], "over": "json"}"#,
            "--variable",
            "n=3",
            "--variable",
            "flag=true",
            "--variable",
            "nothing=null",
            "--variable",
            "text=hello world",
            "--variable",
            "over=flag",
            "--variable",
            "body=@body.md",
        ])
        .success();
    assert_eq!(
        api.variables("Probe"),
        json!({
            "n": 3, "kept": [1, 2], "over": "flag", "flag": true, "nothing": null,
            "text": "hello world", "body": "From a file"
        })
    );
}

#[test]
fn reads_the_document_from_stdin() {
    let api = MockLinear::start();
    api.on("Probe", viewer());
    Cli::for_api(&api)
        .stdin(QUERY.as_bytes())
        .run(&["api", "--variable", "n=1"])
        .success()
        .stdout_has("user-1");
    assert_eq!(api.variables("Probe"), json!({ "n": 1 }));
}

#[test]
fn missing_document_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["api"])
        .failure()
        .stderr_has("No query");
    assert!(api.requests().is_empty());
}

#[test]
fn invalid_variables_json_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["api", QUERY, "--variables-json", "{nope"])
        .failure()
        .stderr_has("--variables-json");
    cli.run(&["api", QUERY, "--variables-json", "[1]"])
        .failure()
        .stderr_has("object");
    cli.run(&["api", QUERY, "--variable", "body=@missing.md"])
        .failure()
        .stderr_has("missing.md");
    assert!(api.requests().is_empty());
}

#[test]
fn graphql_errors_print_the_response_and_fail() {
    let api = MockLinear::start();
    api.on_error("Probe", "Field 'nope' doesn't exist");
    let run = Cli::for_api(&api).run(&["api", QUERY]);
    run.failure();
    assert_eq!(
        run.json()["errors"][0]["message"],
        "Field 'nope' doesn't exist"
    );
}

#[test]
fn http_errors_fail_with_the_body_on_stderr() {
    let api = MockLinear::start();
    api.on_raw("Probe", 400, r#"{"errors":[{"message":"Bad request"}]}"#);
    let run = Cli::for_api(&api).run(&["api", QUERY]);
    run.failure().stderr_has("Bad request");
    assert_eq!(run.stdout, "");
}

#[test]
fn silent_suppresses_output_but_keeps_the_exit_status() {
    let api = MockLinear::start();
    api.on("Probe", viewer()).on_error("Probe", "Denied");
    let cli = Cli::for_api(&api);
    let run = cli.run(&["api", QUERY, "--silent"]);
    run.success();
    assert_eq!(run.stdout, "");
    let run = cli.run(&["api", QUERY, "--silent"]);
    run.failure();
    assert_eq!(run.stdout, "");
}

#[test]
fn paginate_follows_cursors_and_prints_every_node() {
    const ISSUES: &str = "query Issues($after: String) { issues(first: 2, after: $after) { nodes { id } pageInfo { hasNextPage endCursor } } }";
    let page = |ids: &[&str], next: bool, cursor: Value| {
        let nodes: Vec<Value> = ids.iter().map(|id| json!({ "id": id })).collect();
        json!({ "issues": { "nodes": nodes, "pageInfo": { "hasNextPage": next, "endCursor": cursor } } })
    };
    let api = MockLinear::start();
    api.on("Issues", page(&["a", "b"], true, json!("cursor-1")))
        .on("Issues", page(&["c"], false, Value::Null));
    let run = Cli::for_api(&api).run(&["api", ISSUES, "--paginate"]);
    assert_eq!(
        run.success().json(),
        json!([{ "id": "a" }, { "id": "b" }, { "id": "c" }])
    );
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [json!({ "after": null }), json!({ "after": "cursor-1" })]
    );
}

#[test]
fn paginate_rejects_several_connections() {
    const BOTH: &str = "query Both($after: String) { issues(after: $after) { nodes { id } pageInfo { hasNextPage endCursor } } teams { nodes { id } pageInfo { hasNextPage endCursor } } }";
    let connection = json!({ "nodes": [], "pageInfo": { "hasNextPage": true, "endCursor": "c" } });
    let api = MockLinear::start();
    api.on(
        "Both",
        json!({ "issues": connection.clone(), "teams": connection }),
    );
    Cli::for_api(&api)
        .run(&["api", BOTH, "--paginate"])
        .failure()
        .stderr_has("--paginate");
}

fn introspection() -> Value {
    let named = |kind: &str, name: &str| json!({ "kind": kind, "name": name, "ofType": null });
    let field = |name: &str, ty: Value| {
        json!({
            "name": name, "description": null, "args": [], "type": ty,
            "isDeprecated": false, "deprecationReason": null
        })
    };
    let object = |name: &str, fields: Value| {
        json!({
            "kind": "OBJECT", "name": name, "description": null, "specifiedByURL": null,
            "fields": fields, "inputFields": null, "interfaces": [], "enumValues": null,
            "possibleTypes": null, "isOneOf": null
        })
    };
    let scalar = |name: &str| {
        json!({
            "kind": "SCALAR", "name": name, "description": null, "specifiedByURL": null,
            "fields": null, "inputFields": null, "interfaces": null, "enumValues": null,
            "possibleTypes": null, "isOneOf": null
        })
    };
    let non_null = |ty: Value| json!({ "kind": "NON_NULL", "name": null, "ofType": ty });
    json!({
        "__schema": {
            "description": null,
            "queryType": { "name": "Query" },
            "mutationType": null,
            "subscriptionType": null,
            "types": [
                object("Query", json!([field("viewer", non_null(named("OBJECT", "User")))])),
                object("User", json!([
                    field("id", non_null(named("SCALAR", "ID"))),
                    field("name", non_null(named("SCALAR", "String")))
                ])),
                scalar("ID"),
                scalar("String"),
                scalar("Boolean")
            ],
            "directives": []
        }
    })
}

#[test]
fn schema_prints_sdl() {
    let api = MockLinear::start();
    api.on("IntrospectionQuery", introspection());
    let run = Cli::for_api(&api).run(&["schema"]);
    run.success()
        .stdout_has("type User")
        .stdout_has("viewer: User!");
}

#[test]
fn schema_json_prints_the_introspection_result() {
    let api = MockLinear::start();
    api.on("IntrospectionQuery", introspection());
    let json = Cli::for_api(&api)
        .run(&["schema", "--json"])
        .success()
        .json();
    let schema = json.get("data").unwrap_or(&json);
    assert_eq!(schema["__schema"]["queryType"]["name"], "Query");
    let names: Vec<&str> = schema["__schema"]["types"]
        .as_array()
        .expect("types")
        .iter()
        .filter_map(|ty| ty["name"].as_str())
        .collect();
    assert!(names.contains(&"User"), "{names:?}");
}

#[test]
fn schema_writes_to_a_file() {
    let api = MockLinear::start();
    api.on("IntrospectionQuery", introspection());
    let cli = Cli::for_api(&api);
    cli.run(&["schema", "--output", "schema.graphql"]).success();
    assert!(cli.read("cwd/schema.graphql").contains("type User"));
}

#[test]
fn schema_reports_graphql_errors() {
    let api = MockLinear::start();
    api.on_error("IntrospectionQuery", "Introspection is disabled");
    Cli::for_api(&api)
        .run(&["schema"])
        .failure()
        .stderr_has("Introspection is disabled");
}
