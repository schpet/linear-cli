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


def variant(path, index):
    parts = path.split()[1:]
    name = "".join(part.title().replace("-", "") for part in parts)
    return f"{name or 'Root'}R{index}"


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
    for arg in option_value["args"]:
        if not isinstance(arg, dict) or set(arg) != {"name", "type", "action", "optional", "variadic", "list"}:
            fail("option arg schema changed")
    default = json.dumps(option_value.get("default"), ensure_ascii=False, separators=(",", ":"))
    args = json.dumps(option_value["args"], ensure_ascii=False, separators=(",", ":"))
    return ("OptionMeta { scope: " + string(option_value["scope"])
            + ", name: " + string(option_value["name"])
            + ", flags: " + strings(option_value["flags"])
            + ", description: " + string(option_value["description"])
            + ", type_definition: " + string(option_value["typeDefinition"])
            + ", args_json: " + string(args)
            + ", default_json: " + string(default)
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
        required = {"path", "name", "aliases", "hidden", "description", "usage", "argsDefinition", "localOptions", "inheritedGlobalOptions", "kind", "parentAction", "children", "examples"}
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
        if route["argsDefinition"] is not None and not isinstance(route["argsDefinition"], str):
            fail("argsDefinition must be string or null")
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
        "use super::{ExampleMeta, OptionMeta, RouteMeta};",
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]",
        "pub enum Route {",
    ]
    lines.extend(f"    {v}," for v in variants)
    lines += ["}", "", "pub static ROUTES: &[RouteMeta] = &["]
    for route, v in zip(routes, variants):
        local = ",\n            ".join(option(o) for o in route["localOptions"])
        inherited = ",\n            ".join(option(o) for o in route["inheritedGlobalOptions"])
        examples = ",\n            ".join("ExampleMeta { name: " + string(example["name"]) + ", description: " + string(example["description"]) + " }" for example in route["examples"])
        lines += [
            "    RouteMeta {",
            f"        route: Route::{v}, path: {string(route['path'])}, name: {string(route['name'])},",
            f"        aliases: {strings(route['aliases'])}, hidden: {boolean(route['hidden'])},",
            f"        description: {string(route['description'])}, usage: {string(route['usage'])},",
            f"        args_definition: {('None' if route['argsDefinition'] is None else 'Some(' + string(route['argsDefinition']) + ')')},",
            f"        kind: {string(route['kind'])}, parent_action: {string(route['parentAction'])},",
            f"        children: {strings(route['children'])},",
            "        examples: &[" + examples + "],",
            "        local_options: &[" + local + "],",
            "        inherited_global_options: &[" + inherited + "],",
            "    },",
        ]
    lines += ["];"]
    lines += ["", "impl Route {", "    pub fn action(self) -> super::DispatchAction {", "        match self {"]
    for route, v in zip(routes, variants):
        action = "Root" if route["path"] == "linear" else ("Document" if route["path"] == "linear document" else ("ParentPending" if route["kind"] == "parent_route" else "Unimplemented"))
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
