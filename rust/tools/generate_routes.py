#!/usr/bin/env python3
"""Generate static Rust route metadata from the frozen P01B inventory.

Run from any directory: python3 rust/tools/generate_routes.py [--check]
The runtime never opens manifest.json. This script performs strict shape checks so
an inventory schema change requires an explicit generator change.
"""

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "parity" / "manifest.json"
TARGET = ROOT / "crates" / "linear-cli" / "src" / "cli" / "generated.rs"


def fail(message):
    raise SystemExit(message)


def string(value):
    if not isinstance(value, str):
        fail(f"expected string, got {type(value).__name__}")
    hashes = "#"
    while f'"{hashes}' in value:
        hashes += "#"
    return f'r{hashes}"{value}"{hashes}'


def boolean(value):
    if not isinstance(value, bool):
        fail(f"expected bool, got {type(value).__name__}")
    return "true" if value else "false"


def strings(values):
    if not isinstance(values, list) or any(not isinstance(v, str) for v in values):
        fail("expected string array")
    return "&[" + ", ".join(map(string, values)) + "]"


ARG_TOKEN = re.compile(r"([<\[])([A-Za-z][A-Za-z0-9-]*)(\.\.\.)?(?::([A-Za-z][A-Za-z0-9-]*)(\[\])?)?([>\]])")
FLAG_TOKEN = re.compile(r"--[A-Za-z][A-Za-z0-9-]*")


def argument_definition(value):
    if not isinstance(value, str):
        fail("argument definition must be string")
    if not value:
        return
    if value.strip() != value or "  " in value or any(char in value for char in "\n\r\t"):
        fail(f"unsupported argument spacing: {value!r}")
    for token in value.split(" "):
        if FLAG_TOKEN.fullmatch(token):
            continue
        match = ARG_TOKEN.fullmatch(token)
        if not match or (match.group(1), match.group(6)) not in (("<", ">"), ("[", "]")):
            fail(f"unsupported argument token: {token!r}")


def variant(path, index):
    parts = path.split()[1:]
    name = "".join(part.title().replace("-", "") for part in parts)
    return f"{name or 'Root'}R{index}"


def argument(value):
    if not isinstance(value, dict) or set(value) != {"name", "type", "action", "optional", "variadic", "list"}:
        fail("argument schema changed")
    return ("ArgumentMeta { name: " + string(value["name"])
            + ", type_name: " + string(value["type"])
            + ", action: " + string(value["action"])
            + ", optional: " + boolean(value["optional"])
            + ", variadic: " + boolean(value["variadic"])
            + ", list: " + boolean(value["list"])
            + " }")


def arguments(values):
    if not isinstance(values, list):
        fail("arguments must be array")
    return "&[" + ", ".join(argument(value) for value in values) + "]"


def option_default(value, present):
    if not present:
        return "OptionDefault::Absent"
    if value is None:
        return "OptionDefault::Null"
    if isinstance(value, int) and not isinstance(value, bool):
        return f"OptionDefault::Integer({value})"
    if isinstance(value, list) and len(value) == 1 and all(isinstance(item, str) and re.fullmatch(r"[A-Za-z0-9_-]+", item) for item in value):
        return "OptionDefault::Strings(" + strings(value) + ")"
    fail(f"unsupported option default: {value!r}")


def local_type(value):
    if not isinstance(value, dict) or set(value) != {"name", "global", "override", "handlerKind", "values"}:
        fail("local type schema changed")
    if value["global"] is not False or value["override"] is not False:
        fail("global or overriding custom type needs inherited renderer metadata")
    if value["handlerKind"] == "EnumType":
        if not isinstance(value["values"], list) or any(not isinstance(item, str) or not re.fullmatch(r"[A-Za-z0-9_-]+", item) for item in value["values"]):
            fail("enum inspect formatting needs implementation for non-simple values")
        handler = "TypeHandler::Enum(" + strings(value["values"]) + ")"
    elif value["handlerKind"] == "VariableType" and value["values"] == "not_available":
        handler = "TypeHandler::Variable"
    else:
        fail(f"unsupported local type: {value!r}")
    return ("TypeMeta { name: " + string(value["name"])
            + ", global: " + boolean(value["global"])
            + ", override_existing: " + boolean(value["override"])
            + ", handler: " + handler + " }")


def option(option_value):
    expected = {
        "scope", "name", "flags", "description", "typeDefinition", "args",
        "required", "collect", "conflicts", "depends", "hidden",
        "global", "valueHandler",
    }
    if not isinstance(option_value, dict) or not expected <= set(option_value) or set(option_value) - expected - {"default"}:
        fail(f"option schema changed: {set(option_value) if isinstance(option_value, dict) else option_value}")
    if option_value["scope"] not in ("local", "inherited_global"):
        fail("unknown option scope")
    if not isinstance(option_value["args"], list):
        fail("option args must be array")
    argument_definition(option_value["typeDefinition"])
    if option_value["conflicts"] != [] or option_value["depends"] != [] or option_value["valueHandler"] != "none":
        fail(f"unsupported option behavior: {option_value['name']}")
    scope = {"local": "OptionScope::Local", "inherited_global": "OptionScope::InheritedGlobal"}[option_value["scope"]]
    return ("OptionMeta { scope: " + scope
            + ", name: " + string(option_value["name"])
            + ", flags: " + strings(option_value["flags"])
            + ", description: " + string(option_value["description"])
            + ", type_definition: " + string(option_value["typeDefinition"])
            + ", args: " + arguments(option_value["args"])
            + ", default: " + option_default(option_value.get("default"), "default" in option_value)
            + ", required: " + boolean(option_value["required"])
            + ", collect: " + boolean(option_value["collect"])
            + ", hidden: " + boolean(option_value["hidden"])
            + ", global: " + boolean(option_value["global"])
            + " }")


def generate():
    inventory = json.loads(SOURCE.read_text())
    if not isinstance(inventory, dict) or not isinstance(inventory.get("routes"), list):
        fail("manifest routes missing")
    routes = inventory["routes"]
    if len(routes) != 110:
        fail(f"expected 110 routes, found {len(routes)}")
    paths = []
    variants = []
    aliases = 0
    descriptions = 0
    for route in routes:
        if not isinstance(route, dict):
            fail("route must be object")
        required = {"path", "name", "aliases", "aliasResolution", "hidden", "description", "usage", "argsDefinition", "arguments", "localTypes", "allTypes", "localEnvVars", "inheritedGlobalEnvVars", "localOptions", "inheritedGlobalOptions", "kind", "parentAction", "children", "examples"}
        if not required <= set(route):
            fail(f"route fields missing: {required - set(route)}")
        if route["kind"] not in ("source_leaf", "parent_route", "generated_completion_child"):
            fail(f"unknown route kind: {route['kind']}")
        if route["path"] != "linear" and route["path"].rsplit(" ", 1)[-1] != route["name"]:
            fail(f"route path/name mismatch: {route['path']}")
        if not re.fullmatch(r"linear(?: [a-z][a-z0-9-]*)*", route["path"]):
            fail(f"invalid route path: {route['path']}")
        if not isinstance(route["description"], str) or not isinstance(route["usage"], str):
            fail("description/usage must be strings")
        if route["path"] == "linear completions":
            shell_snippets = (
                "~/.bashrc",
                "source <(linear completions [shell])",
                "linear completions [shell] --help",
            )
            if any(route["description"].count(snippet) != 1 for snippet in shell_snippets):
                fail("generated completion description styling anchors changed")
        if route["argsDefinition"] is not None and not isinstance(route["argsDefinition"], str):
            fail("argsDefinition must be string or null")
        argument_definition(route["usage"])
        if route["argsDefinition"] is not None:
            argument_definition(route["argsDefinition"])
        if route["localEnvVars"] != [] or route["inheritedGlobalEnvVars"] != []:
            fail("environment-variable help metadata is not implemented")
        if route["allTypes"] != route["localTypes"]:
            fail("inherited custom type help metadata is not implemented")
        arguments(route["arguments"])
        if not isinstance(route["localTypes"], list):
            fail("localTypes must be array")
        for definition in route["localTypes"]:
            local_type(definition)
        resolution = route["aliasResolution"]
        if not isinstance(resolution, list) or len(resolution) != len(route["aliases"]):
            fail("alias resolution count changed")
        for alias, result in zip(route["aliases"], resolution):
            if not isinstance(result, dict) or set(result) != {"alias", "resolves"} or result != {"alias": alias, "resolves": True}:
                fail(f"unresolved alias for {route['path']}: {result!r}")
        if not isinstance(route["examples"], list):
            fail("examples must be array")
        for example in route["examples"]:
            if not isinstance(example, dict) or set(example) != {"name", "description"}:
                fail("example schema changed")
            if not isinstance(example["name"], str) or not isinstance(example["description"], str):
                fail("example fields must be strings")
        aliases += len(route["aliases"])
        descriptions += len(route["localOptions"]) + len(route["inheritedGlobalOptions"])
        paths.append(route["path"])
        variants.append(variant(route["path"], len(variants)))
    if len(set(paths)) != 110 or len(set(variants)) != 110:
        fail("duplicate route path or variant")
    if aliases != 36 or descriptions != 439:
        fail(f"inventory cardinality changed: {aliases} aliases, {descriptions} option descriptions")
    for route in routes:
        path = route["path"]
        for child in route["children"]:
            if f"{path} {child}" not in paths:
                fail(f"unresolved child {path} {child}")
        for alias in route["aliases"]:
            if not isinstance(alias, str) or not alias:
                fail(f"invalid alias for {path}")

    lines = [
        "// @generated by rust/tools/generate_routes.py; do not edit by hand.",
        "use super::{ArgumentMeta, ExampleMeta, OptionDefault, OptionMeta, OptionScope, ParentAction, RouteKind, RouteMeta, TypeHandler, TypeMeta};",
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]",
        "pub enum Route {",
    ]
    lines.extend(f"    {v}," for v in variants)
    lines += ["}", "", "pub static ROUTES: &[RouteMeta] = &["]
    for route, v in zip(routes, variants):
        local = ",\n            ".join(option(o) for o in route["localOptions"])
        inherited = ",\n            ".join(option(o) for o in route["inheritedGlobalOptions"])
        examples = ",\n            ".join("ExampleMeta { name: " + string(example["name"]) + ", description: " + string(example["description"]) + " }" for example in route["examples"])
        local_types = ",\n            ".join(local_type(definition) for definition in route["localTypes"])
        kind = {"source_leaf": "RouteKind::SourceLeaf", "parent_route": "RouteKind::ParentRoute", "generated_completion_child": "RouteKind::GeneratedCompletionChild"}[route["kind"]]
        parent_action = {"not_applicable": "ParentAction::NotApplicable", "pending_safe_fixture": "ParentAction::PendingSafeFixture"}.get(route["parentAction"])
        if parent_action is None:
            fail(f"unknown parent action: {route['parentAction']}")
        lines += [
            "    RouteMeta {",
            f"        route: Route::{v}, path: {string(route['path'])}, name: {string(route['name'])},",
            f"        aliases: {strings(route['aliases'])}, hidden: {boolean(route['hidden'])},",
            f"        description: {string(route['description'])}, usage: {string(route['usage'])},",
            f"        args_definition: {('None' if route['argsDefinition'] is None else 'Some(' + string(route['argsDefinition']) + ')')},",
            f"        kind: {kind}, parent_action: {parent_action},",
            f"        children: {strings(route['children'])},",
            "        examples: &[" + examples + "],",
            "        arguments: " + arguments(route["arguments"]) + ",",
            "        local_types: &[" + local_types + "],",
            "        local_options: &[" + local + "],",
            "        inherited_global_options: &[" + inherited + "],",
            "    },",
        ]
    lines += ["];"]
    lines += ["", "impl Route {", "    pub fn action(self) -> super::DispatchAction {", "        match self {"]
    for route, v in zip(routes, variants):
        action = {
            "linear": "Root",
            "linear auth list": "AuthList",
            "linear auth whoami": "AuthWhoami",
            "linear team list": "TeamList",
            "linear project list": "ProjectList",
            "linear project view": "ProjectView",
            "linear team members": "TeamMembers",
            "linear team states": "TeamStates",
            "linear template list": "TemplateList",
            "linear template view": "TemplateView",
            "linear user list": "UserList",
            "linear cycle list": "CycleList",
            "linear cycle view": "CycleView",
            "linear milestone list": "MilestoneList",
            "linear milestone view": "MilestoneView",
            "linear label list": "LabelList",
            "linear document": "Document",
            "linear markdown": "Markdown",
            "linear team id": "TeamId",
            "linear completions": "Completions",
            "linear completions bash": "CompletionsBash",
            "linear completions fish": "CompletionsFish",
            "linear completions zsh": "CompletionsZsh",
            "linear completions complete": "CompletionsComplete",
        }.get(route["path"], "ParentPending" if route["kind"] == "parent_route" else "Unimplemented")
        lines.append(f"            Self::{v} => super::DispatchAction::{action},")
    lines += ["        }", "    }", "}"]
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    if sys.argv[1:] not in ([], ["--check"]):
        fail("usage: generate_routes.py [--check]")
    result = generate()
    with tempfile.TemporaryDirectory() as temporary:
        output = Path(temporary) / "generated.rs"
        output.write_text(result)
        subprocess.run(["rustfmt", "+1.93.0", "--edition", "2024", str(output)], check=True)
        result = output.read_text()
    if sys.argv[1:] == ["--check"]:
        if not TARGET.exists() or TARGET.read_text() != result:
            fail(f"generated metadata is stale: run python3 {Path(__file__).as_posix()}")
    else:
        TARGET.parent.mkdir(parents=True, exist_ok=True)
        TARGET.write_text(result)
