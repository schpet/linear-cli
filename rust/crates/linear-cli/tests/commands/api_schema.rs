use clap::Parser;
use linear_cli::{
    cli::{Cli, RootCommand},
    commands::{api, schema},
    graphql::{
        schema_defaults,
        schema_introspection::{Model, natural},
    },
    js_value::{JsObject, JsValue, js_stringify},
};
fn action(args: &[&str]) -> linear_cli::cli::api::Api {
    let cli = Cli::try_parse_from(std::iter::once("linear").chain(args.iter().copied())).unwrap();
    match cli.command.unwrap() {
        RootCommand::Api(v) => v,
        _ => panic!("not api"),
    }
}
#[test]
fn api_variables_preserve_js_numbers_property_order_and_assignment_semantics() {
    let cli = action(&[
        "api",
        " verBatim ",
        "--variables-json",
        r#"{"z":1,"3":"old","__proto__":true,"constructor":"yes","1":1,"z":2}"#,
        "--variable",
        "z=3",
        "--variable",
        "3=4",
        "--variable",
        "2=Infinity",
        "--variable",
        "=a=b",
        "--variable",
        "keep=-0",
        "--variable",
        "big=9007199254740992",
        "--variable",
        "__proto__=ignored",
    ]);
    let vars = api::variables(&cli).unwrap();
    assert_eq!(
        api::request(" verBatim ", &vars),
        r#"{"query":" verBatim ","variables":{"1":1,"2":null,"3":4,"z":3,"constructor":"yes","":"a=b","keep":"-0","big":9007199254740992}}"#
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
        ("-Infinity", "null"),
        ("NaN", r#""NaN""#),
        ("true", "true"),
        ("null", "null"),
        ("-2", "-2"),
    ] {
        let v = api::variables(&action(&["api", "q", "--variable", &format!("x={raw}")])).unwrap();
        assert_eq!(js_stringify(v.get("x").unwrap()), expected);
    }
}
#[test]
fn whole_input_json_classifier_keeps_malformed_fallback_and_refuses_only_typed_domain() {
    for text in [
        r#""\ud800""#,
        "1e400",
        &format!("{}0{}", "[".repeat(128), "]".repeat(128)),
    ] {
        assert!(
            api::decode(text, false)
                .unwrap_err()
                .message
                .contains("request not sent")
        );
        assert!(
            api::decode(text, true)
                .unwrap_err()
                .message
                .contains("request sent, any effects unknown")
        );
    }
    for text in [
        r#"{"a":"\ud800","#,
        r#"{"a":1e400,"#,
        "1e+",
        "1 2",
        r#""\u""#,
        &format!("{}0{}", "[".repeat(128), "]".repeat(127)),
    ] {
        assert!(matches!(
            api::decode(text, false).unwrap(),
            api::JsonDecode::Malformed
        ));
    }
    assert!(matches!(
        api::decode(&format!("{}0{}", "[".repeat(127), "]".repeat(127)), false).unwrap(),
        api::JsonDecode::Value(_)
    ));
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
        let value: JsValue = serde_json::from_str(data).unwrap();
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
    let value: JsValue = serde_json::from_str(&value.to_string()).unwrap();
    assert_eq!(
        format!("{}\n", schema::content(&value, false).unwrap()),
        include_str!("fixtures/api-schema/synthetic.graphql")
    );
}
#[test]
fn schema_json_accepts_non_sdl_data_and_prints_js_order_numbers() {
    let value: JsValue =
        serde_json::from_str(r#"{"future":2.0,"2":9007199254740993,"1":true}"#).unwrap();
    assert_eq!(
        schema::content(&value, true).unwrap(),
        "{\n  \"1\": true,\n  \"2\": 9007199254740992,\n  \"future\": 2\n}"
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
    let value: JsValue =
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
    let value: JsValue = serde_json::from_str(&value.to_string()).unwrap();
    let error = schema::content(&value, false).unwrap_err();
    assert_eq!(error.kind, linear_cli::error::AppErrorKind::Validation);
    assert!(error.message.contains("Cyclic input-object default: A.x"));
}

#[test]
fn ordered_objects_keep_duplicate_positions_and_incremental_numeric_insertions() {
    let mut object = JsObject::from_created([
        ("z".into(), JsValue::Number(1.0)),
        ("3".into(), JsValue::Number(3.0)),
        ("a".into(), JsValue::Number(2.0)),
        ("z".into(), JsValue::Number(4.0)),
        ("1".into(), JsValue::Number(1.0)),
    ]);
    object.assign("a".into(), JsValue::Bool(true));
    object.assign("2".into(), JsValue::Bool(false));
    object.assign("0".into(), JsValue::Null);
    object.assign("__proto__".into(), JsValue::Null);
    object.assign("constructor".into(), JsValue::String("own".into()));
    assert_eq!(object.get("2"), Some(&JsValue::Bool(false)));
    assert_eq!(object.get("z"), Some(&JsValue::Number(4.0)));
    assert_eq!(
        js_stringify(&JsValue::Object(object)),
        r#"{"0":null,"1":1,"2":false,"3":3,"z":4,"a":true,"constructor":"own"}"#
    );
    let parsed: JsValue =
        serde_json::from_str(r#"{"__proto__":1,"z":0,"__proto__":2,"z":3}"#).unwrap();
    assert_eq!(js_stringify(&parsed), r#"{"__proto__":2,"z":3}"#);
}
