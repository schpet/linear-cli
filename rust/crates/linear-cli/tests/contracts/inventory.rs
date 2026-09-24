use std::collections::BTreeSet;
use std::process::Command;

use linear_cli::cli::{OptionDefault, OptionScope, ParentAction, ROUTES, RouteKind, TypeHandler};
use serde_json::{Value, json};

#[test]
fn generated_inventory_matches_frozen_manifest_in_order() {
    let source = include_str!("../../../../parity/manifest.json");
    let manifest: Value = serde_json::from_str(source).expect("valid frozen manifest");
    let routes = manifest["routes"].as_array().expect("routes array");
    assert_eq!(routes.len(), 110);
    assert_eq!(ROUTES.len(), routes.len());
    let mut paths = BTreeSet::new();
    let mut alias_count = 0;
    let mut option_count = 0;
    let mut parent_count = 0;
    let mut leaf_count = 0;
    let mut completion_count = 0;
    for (generated, source) in ROUTES.iter().zip(routes) {
        assert!(
            paths.insert(generated.path),
            "duplicate route: {}",
            generated.path
        );
        assert_eq!(json!(generated.path), source["path"]);
        assert_eq!(json!(generated.name), source["name"]);
        assert_eq!(json!(generated.aliases), source["aliases"]);
        assert_eq!(json!(generated.hidden), source["hidden"]);
        assert_eq!(json!(generated.description), source["description"]);
        assert_eq!(json!(generated.usage), source["usage"]);
        assert_eq!(json!(generated.args_definition), source["argsDefinition"]);
        let kind = match generated.kind {
            RouteKind::ParentRoute => "parent_route",
            RouteKind::SourceLeaf => "source_leaf",
            RouteKind::GeneratedCompletionChild => "generated_completion_child",
        };
        assert_eq!(json!(kind), source["kind"]);
        match generated.kind {
            RouteKind::ParentRoute => parent_count += 1,
            RouteKind::SourceLeaf => leaf_count += 1,
            RouteKind::GeneratedCompletionChild => completion_count += 1,
        }
        let parent_action = match generated.parent_action {
            ParentAction::NotApplicable => "not_applicable",
            ParentAction::PendingSafeFixture => "pending_safe_fixture",
        };
        assert_eq!(json!(parent_action), source["parentAction"]);
        assert_eq!(json!(generated.children), source["children"]);
        assert_eq!(
            source["aliasResolution"],
            json!(
                generated
                    .aliases
                    .iter()
                    .map(|alias| json!({"alias":alias,"resolves":true}))
                    .collect::<Vec<_>>()
            )
        );
        let source_examples = source["examples"].as_array().expect("examples array");
        assert_eq!(generated.examples.len(), source_examples.len());
        for (example, original) in generated.examples.iter().zip(source_examples) {
            assert_eq!(json!(example.name), original["name"]);
            assert_eq!(json!(example.description), original["description"]);
        }
        let generated_arguments = generated
            .arguments
            .iter()
            .map(|argument| {
                json!({
                    "name": argument.name,
                    "type": argument.type_name,
                    "action": argument.action,
                    "optional": argument.optional,
                    "variadic": argument.variadic,
                    "list": argument.list,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(generated_arguments), source["arguments"]);
        let generated_types = generated
            .local_types
            .iter()
            .map(|definition| {
                let (kind, values) = match definition.handler {
                    TypeHandler::Enum(values) => ("EnumType", json!(values)),
                    TypeHandler::Variable => ("VariableType", json!("not_available")),
                };
                json!({
                    "name": definition.name,
                    "global": definition.global,
                    "override": definition.override_existing,
                    "handlerKind": kind,
                    "values": values,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(generated_types), source["localTypes"]);
        alias_count += generated.aliases.len();
        for (field, options) in [
            ("localOptions", generated.local_options),
            ("inheritedGlobalOptions", generated.inherited_global_options),
        ] {
            let source_options = source[field].as_array().expect("option array");
            assert_eq!(
                options.len(),
                source_options.len(),
                "{} {field}",
                generated.path
            );
            option_count += options.len();
            for (option, original) in options.iter().zip(source_options) {
                let scope = match option.scope {
                    OptionScope::Local => "local",
                    OptionScope::InheritedGlobal => "inherited_global",
                };
                assert_eq!(json!(scope), original["scope"]);
                assert_eq!(json!(option.name), original["name"]);
                assert_eq!(json!(option.flags), original["flags"]);
                assert_eq!(json!(option.description), original["description"]);
                assert_eq!(json!(option.type_definition), original["typeDefinition"]);
                let args = option
                    .args
                    .iter()
                    .map(|argument| {
                        json!({
                            "name": argument.name,
                            "type": argument.type_name,
                            "action": argument.action,
                            "optional": argument.optional,
                            "variadic": argument.variadic,
                            "list": argument.list,
                        })
                    })
                    .collect::<Vec<_>>();
                assert_eq!(json!(args), original["args"]);
                let default = match option.default {
                    OptionDefault::Absent => {
                        assert!(original.get("default").is_none());
                        Value::Null
                    }
                    OptionDefault::Null => json!(null),
                    OptionDefault::Integer(number) => json!(number),
                    OptionDefault::Strings(values) => json!(values),
                };
                assert_eq!(default, original["default"]);
                assert_eq!(original["conflicts"], json!([]));
                assert_eq!(original["depends"], json!([]));
                assert_eq!(original["valueHandler"], "none");
                assert_eq!(json!(option.required), original["required"]);
                assert_eq!(json!(option.collect), original["collect"]);
                assert_eq!(json!(option.hidden), original["hidden"]);
                assert_eq!(json!(option.global), original["global"]);
            }
        }
    }
    assert_eq!(alias_count, 36);
    assert_eq!(option_count, 439);
    assert_eq!((parent_count, leaf_count, completion_count), (20, 86, 4));
    let deno: Value =
        serde_json::from_str(include_str!("../../../../../deno.json")).expect("valid Deno config");
    assert_eq!(env!("CARGO_PKG_VERSION"), "3.0.0-alpha.1");
    assert_eq!(deno["version"].as_str().expect("Deno version"), "2.6.0");
}

#[test]
fn generated_source_is_fresh() {
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg("tools/generate_routes.py")
        .arg("--check")
        .current_dir(rust_root)
        .output()
        .expect("Python 3 is required by the checked-in metadata generator");
    assert!(
        output.status.success(),
        "generated Rust metadata is stale: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
