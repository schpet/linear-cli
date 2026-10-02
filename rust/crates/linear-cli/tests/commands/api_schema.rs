use clap::Parser;
use linear_cli::{
    cli::{Cli, RootCommand},
    commands::{api, schema},
    graphql::{
        schema_defaults,
        schema_introspection::{Model, natural},
    },
};
use serde_json::Value;
fn action(args: &[&str]) -> linear_cli::cli::api::Api {
    let cli = Cli::try_parse_from(std::iter::once("linear").chain(args.iter().copied())).unwrap();
    match cli.command.unwrap() {
        RootCommand::Api(v) => v,
        _ => panic!("not api"),
    }
}
#[test]
fn api_variables_keep_json_order_and_let_flags_override() {
    let cli = action(&[
        "api",
        " verBatim ",
        "--variables-json",
        r#"{"z":1,"3":"old","constructor":"yes","1":1.5,"z":2}"#,
        "--variable",
        "z=3",
        "--variable",
        "3=4",
        "--variable",
        "=a=b",
        "--variable",
        "keep=-0",
        "--variable",
        "big=9007199254740993",
    ]);
    let vars = api::variables(&cli).unwrap();
    assert_eq!(
        api::request(" verBatim ", &vars),
        r#"{"query":" verBatim ","variables":{"z":3,"3":4,"constructor":"yes","1":1.5,"":"a=b","keep":"-0","big":9007199254740993}}"#
    );
    assert_eq!(
        api::request(
            " query ",
            &api::variables(&action(&["api", " query "])).unwrap()
        ),
        r#"{"query":" query "}"#
    );
    for (raw, expected) in [
        ("01", r#""01""#),
        ("1e3", r#""1e3""#),
        ("1.5", "1.5"),
        ("true", "true"),
        ("null", "null"),
        ("-2", "-2"),
    ] {
        let v = api::variables(&action(&["api", "q", "--variable", &format!("x={raw}")])).unwrap();
        assert_eq!(v.get("x").unwrap().to_string(), expected);
    }
    for raw in ["Infinity", "-Infinity", "NaN", "1e400"] {
        let error =
            api::variables(&action(&["api", "q", "--variable", &format!("x={raw}")])).unwrap_err();
        assert!(error.message.contains("is not a finite number"), "{raw}");
    }
}
#[test]
fn json_decode_accepts_any_valid_json_and_rejects_malformed_text() {
    for text in [
        r#"{"a":"\ud800","#,
        "1e400",
        "1e+",
        "1 2",
        r#""\u""#,
        &format!("{}0{}", "[".repeat(128), "]".repeat(128)),
    ] {
        assert!(api::decode(text).is_none(), "{text}");
    }
    assert_eq!(
        api::decode(r#"{"b":1,"a":2.5,"n":123456789012345678}"#)
            .unwrap()
            .to_string(),
        r#"{"b":1,"a":2.5,"n":123456789012345678}"#
    );
    assert!(api::decode(&format!("{}0{}", "[".repeat(127), "]".repeat(127))).is_some());
}
#[test]
fn schema_runtime_full_synthetic_and_graphql_js_corner_bytes_are_exact() {
    for (data, expected) in [
        (
            include_str!("fixtures/api-schema/full.json"),
            include_str!("fixtures/api-schema/full.graphql"),
        ),
        (
            include_str!("fixtures/api-schema/synthetic.json"),
            include_str!("fixtures/api-schema/synthetic.graphql"),
        ),
        (
            include_str!("fixtures/api-schema/corners.json"),
            include_str!("fixtures/api-schema/corners.graphql"),
        ),
    ] {
        let value: Value = serde_json::from_str(data).unwrap();
        assert_eq!(
            format!("{}\n", schema::content(&value, false).unwrap()),
            expected
        );
    }
}
#[test]
fn supplied_standard_scalar_metadata_is_replaced_by_graphql_canonical_builtin() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/api-schema/synthetic.json")).unwrap();
    for t in value["__schema"]["types"].as_array_mut().unwrap() {
        if t["name"] == "String" {
            t["kind"] = "OBJECT".into();
            t["fields"] = serde_json::Value::Null;
            t["interfaces"] = serde_json::Value::Null;
        }
    }
    let value: Value = serde_json::from_str(&value.to_string()).unwrap();
    assert_eq!(
        format!("{}\n", schema::content(&value, false).unwrap()),
        include_str!("fixtures/api-schema/synthetic.graphql")
    );
}
#[test]
fn schema_json_accepts_non_sdl_data_and_keeps_order_and_numbers() {
    let value: Value =
        serde_json::from_str(r#"{"future":2.5,"2":9007199254740993,"1":true}"#).unwrap();
    assert_eq!(
        schema::content(&value, true).unwrap(),
        "{\n  \"future\": 2.5,\n  \"2\": 9007199254740993,\n  \"1\": true\n}"
    );
    assert!(schema::content(&value, false).is_err());
}
#[test]
fn default_value_grammar_supports_graphql_escapes_and_rejects_bad_syntax() {
    for raw in [
        r#""\b\u{1F600}\uD83D\uDE00""#,
        r#""""line\"""end""""#,
        "{x: $v, y: [1 2.0], z: ENUM}",
    ] {
        schema_defaults::parse(raw).unwrap();
    }
    for raw in [
        "01",
        "1e",
        "1.",
        "1x",
        "{x:}",
        "[1",
        r#""\ud800""#,
        r#""\u{110000}""#,
        "null false",
    ] {
        assert!(schema_defaults::parse(raw).is_err(), "{raw}");
    }
    assert_eq!(natural("T2", "T10"), std::cmp::Ordering::Less);
    let value: Value =
        serde_json::from_str(include_str!("fixtures/api-schema/synthetic.json")).unwrap();
    Model::parse(&value).unwrap();
}

#[test]
fn default_nesting_and_recursive_input_defaults_return_typed_shape_errors() {
    schema_defaults::parse(&format!("{}0{}", "[".repeat(128), "]".repeat(128))).unwrap();
    let error =
        schema_defaults::parse(&format!("{}0{}", "[".repeat(129), "]".repeat(129))).unwrap_err();
    assert!(error.message.contains("nesting exceeds 128 levels"));
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/api-schema/synthetic.json")).unwrap();
    value["__schema"]["types"].as_array_mut().unwrap().push(serde_json::json!({
        "kind":"INPUT_OBJECT", "name":"A", "inputFields":[{
            "name":"x", "type":{"kind":"INPUT_OBJECT", "name":"A", "ofType":null}, "defaultValue":"{}"
        }]
    }));
    let value: Value = serde_json::from_str(&value.to_string()).unwrap();
    let error = schema::content(&value, false).unwrap_err();
    assert_eq!(error.kind, linear_cli::error::AppErrorKind::Validation);
    assert!(error.message.contains("Cyclic input-object default: A.x"));
}
