use linear_cli::commands::template_data::{
    JsObject, JsValue, js_number, js_stringify, parse_template_data,
};
use linear_cli::error::AppErrorKind;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::templates::{GetTemplates, Template};
use serde_json::json;

fn parse(text: &str) -> JsValue {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn keys(value: &JsValue) -> Vec<&str> {
    let JsValue::Object(object) = value else {
        panic!("not an object: {value:?}");
    };
    object
        .entries()
        .iter()
        .map(|(key, _)| key.as_str())
        .collect()
}

fn template(template_data: &str) -> Template {
    let body = json!({"data": {"templates": [{
        "id": "tpl",
        "name": "Bug report",
        "description": null,
        "type": "issue",
        "icon": null,
        "color": null,
        "hasFormFields": false,
        "lastAppliedAt": null,
        "sortOrder": 0,
        "createdAt": "2024-01-01T00:00:00.000Z",
        "updatedAt": "2024-01-01T00:00:00.000Z",
        "team": null,
        "inheritedFrom": null,
        "creator": null,
        "templateData": template_data
    }]}})
    .to_string();
    let response: GetTemplates = parse_response(body.as_bytes()).expect("typed templates");
    response.templates.into_iter().next().expect("one template")
}

#[test]
fn object_keys_follow_object_entries_order() {
    assert_eq!(
        keys(&parse(
            r#"{"b":1,"10":1,"2":1,"b":2,"01":1,"-1":1,"4294967295":1,"4294967294":1,"0":1,"1.0":1," 3":1}"#
        )),
        [
            "0",
            "2",
            "10",
            "4294967294",
            "b",
            "01",
            "-1",
            "4294967295",
            "1.0",
            " 3"
        ]
    );
    // A duplicate keeps its first position and its last value.
    let JsValue::Object(object) = parse(r#"{"a":1,"b":2,"a":{"x":true}}"#) else {
        panic!("object");
    };
    assert_eq!(
        object.entries(),
        [
            (
                "a".to_owned(),
                JsValue::Object(JsObject::from_created([(
                    "x".to_owned(),
                    JsValue::Bool(true)
                )]))
            ),
            ("b".to_owned(), JsValue::Number(2.0)),
        ]
    );
    assert_eq!(object.get("b"), Some(&JsValue::Number(2.0)));
    assert_eq!(object.get("c"), None);
    // Nested objects are ordered too; arrays keep their order.
    let JsValue::Object(outer) = parse(r#"{"x":{"z":1,"1":2},"y":[3,1,2]}"#) else {
        panic!("object");
    };
    assert_eq!(keys(outer.get("x").expect("x")), ["1", "z"]);
    assert_eq!(
        outer.get("y"),
        Some(&JsValue::Array(vec![
            JsValue::Number(3.0),
            JsValue::Number(1.0),
            JsValue::Number(2.0)
        ]))
    );
}

#[test]
fn numbers_round_like_json_parse_and_print_like_string() {
    for (literal, printed) in [
        ("0", "0"),
        ("-0", "0"),
        ("-0.0", "0"),
        ("1", "1"),
        ("-7", "-7"),
        ("2.50", "2.5"),
        ("1e21", "1e+21"),
        ("123456789012345678901", "123456789012345680000"),
        ("1e-7", "1e-7"),
        ("0.000001", "0.000001"),
        ("9007199254740993", "9007199254740992"),
        ("18446744073709551616", "18446744073709552000"),
        ("-9223372036854775809", "-9223372036854776000"),
        ("5e-324", "5e-324"),
        ("1.7976931348623157e308", "1.7976931348623157e+308"),
    ] {
        let JsValue::Number(value) = parse(literal) else {
            panic!("{literal} is not a number");
        };
        assert_eq!(js_number(value), printed, "{literal}");
    }
}

#[test]
fn stringify_matches_json_stringify_without_indentation() {
    assert_eq!(
        js_stringify(&parse(
            r#"[null,true,false,-0,1e21,"q\"b\\s\/\b\f\n\r\t\u0000\u001f\u007f\u2028é",[],{},{"2":[1],"a":{"1":null}}]"#
        )),
        "[null,true,false,0,1e+21,\"q\\\"b\\\\s/\\b\\f\\n\\r\\t\\u0000\\u001f\u{7f}\u{2028}é\",[],{},{\"2\":[1],\"a\":{\"1\":null}}]"
    );
}

#[test]
fn template_data_must_decode_to_an_object() {
    let decoded =
        parse_template_data(&template(r#"{"title":"Bug: ","priority":2}"#)).expect("object data");
    assert_eq!(
        decoded.entries(),
        [
            ("title".to_owned(), JsValue::String("Bug: ".to_owned())),
            ("priority".to_owned(), JsValue::Number(2.0)),
        ]
    );
    assert!(
        parse_template_data(&template(" \n{}\t"))
            .expect("JSON whitespace")
            .is_empty()
    );
    for (data, suffix) in [
        ("not json", "is not valid JSON"),
        ("{\"a\":1,}", "is not valid JSON"),
        ("{'a':1}", "is not valid JSON"),
        ("{\"a\":NaN}", "is not valid JSON"),
        ("{\"a\":1e400}", "is not valid JSON"),
        ("{\"\\udc00\":1}", "is not valid JSON"),
        ("[1,2]", "is not a JSON object"),
        ("42", "is not a JSON object"),
        ("\"{}\"", "is not a JSON object"),
    ] {
        let error = parse_template_data(&template(data)).expect_err(data);
        assert_eq!(error.kind, AppErrorKind::Validation, "{data}");
        assert_eq!(
            error.message,
            format!("Template data for \"Bug report\" (tpl) {suffix}"),
            "{data}"
        );
        assert_eq!(error.context, None);
    }
}
