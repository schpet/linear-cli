use linear_cli::commands::template_json::{render_list, render_one};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::templates::{GetTemplates, Template};
use serde_json::{Value, json};

fn template(id: &str, name: &str, template_type: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": null,
        "type": template_type,
        "icon": null,
        "color": null,
        "hasFormFields": false,
        "lastAppliedAt": null,
        "sortOrder": 1,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-02T00:00:00.000Z",
        "team": null,
        "inheritedFrom": null,
        "creator": null,
        "templateData": "{}"
    })
}

fn typed(value: Value) -> Template {
    let body = json!({"data": {"templates": [value]}}).to_string();
    let response: GetTemplates = parse_response(body.as_bytes()).expect("typed template response");
    response.templates.into_iter().next().expect("one template")
}

#[test]
fn one_template_keeps_exact_fields_nulls_and_stringified_template_data() {
    let mut value = template("t1", "Name", "issue");
    value["templateData"] = json!("{\"content\": [1]}");
    let output = render_one(&typed(value)).expect("render one");
    assert_eq!(
        String::from_utf8(output).expect("utf8"),
        concat!(
            "{\n",
            "  \"id\": \"t1\",\n",
            "  \"name\": \"Name\",\n",
            "  \"description\": null,\n",
            "  \"type\": \"issue\",\n",
            "  \"icon\": null,\n",
            "  \"color\": null,\n",
            "  \"hasFormFields\": false,\n",
            "  \"lastAppliedAt\": null,\n",
            "  \"sortOrder\": 1,\n",
            "  \"createdAt\": \"2026-01-01T00:00:00.000Z\",\n",
            "  \"updatedAt\": \"2026-01-02T00:00:00.000Z\",\n",
            "  \"team\": null,\n",
            "  \"inheritedFrom\": null,\n",
            "  \"creator\": null,\n",
            "  \"templateData\": \"{\\\"content\\\": [1]}\"\n",
            "}\n"
        )
    );
}

#[test]
fn one_template_keeps_populated_fields_and_nested_field_order() {
    let mut value = template("t2", "Full", "project");
    value["description"] = json!("Description");
    value["icon"] = json!("star");
    value["color"] = json!("#123456");
    value["hasFormFields"] = json!(true);
    value["lastAppliedAt"] = json!("2026-01-03T00:00:00.000Z");
    value["team"] = json!({"id": "team-id", "key": "ENG", "name": "Engineering"});
    value["inheritedFrom"] = json!({"id": "parent-id", "name": "Parent"});
    value["creator"] = json!({"id": "user-id", "name": "Ada"});
    let output = render_one(&typed(value)).expect("render populated template");
    assert_eq!(
        String::from_utf8(output).expect("utf8"),
        concat!(
            "{\n",
            "  \"id\": \"t2\",\n",
            "  \"name\": \"Full\",\n",
            "  \"description\": \"Description\",\n",
            "  \"type\": \"project\",\n",
            "  \"icon\": \"star\",\n",
            "  \"color\": \"#123456\",\n",
            "  \"hasFormFields\": true,\n",
            "  \"lastAppliedAt\": \"2026-01-03T00:00:00.000Z\",\n",
            "  \"sortOrder\": 1,\n",
            "  \"createdAt\": \"2026-01-01T00:00:00.000Z\",\n",
            "  \"updatedAt\": \"2026-01-02T00:00:00.000Z\",\n",
            "  \"team\": {\n",
            "    \"id\": \"team-id\",\n",
            "    \"key\": \"ENG\",\n",
            "    \"name\": \"Engineering\"\n",
            "  },\n",
            "  \"inheritedFrom\": {\n",
            "    \"id\": \"parent-id\",\n",
            "    \"name\": \"Parent\"\n",
            "  },\n",
            "  \"creator\": {\n",
            "    \"id\": \"user-id\",\n",
            "    \"name\": \"Ada\"\n",
            "  },\n",
            "  \"templateData\": \"{}\"\n",
            "}\n"
        )
    );
}

#[test]
fn number_spelling_and_empty_list_are_stable() {
    assert_eq!(render_list(&[]).expect("empty"), b"[]\n");
    for (number, expected) in [(0.5, "0.5"), (1e21, "1e+21"), (-0.0, "0")] {
        let mut value = template("id", "N", "issue");
        value["sortOrder"] = json!(number);
        let output = String::from_utf8(render_one(&typed(value)).expect("one")).expect("utf8");
        assert!(
            output.contains(&format!("\"sortOrder\": {expected},")),
            "{output}"
        );
    }
}
