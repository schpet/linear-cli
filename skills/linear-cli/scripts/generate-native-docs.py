#!/usr/bin/env python3
"""Render native skill references from the typed manifest and pinned binary.

Development only. Never finds linear on PATH, runs CLI actions, or reads config.
The writer compares representative manifest help with the immutable CLI first.
"""
import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re


@dataclass(frozen=True)
class Command:
    name: str
    description: str
    help: str
    subcommands: tuple["Command", ...]


def command(raw: object, parent: str | None, seen: set[str]) -> Command:
    if not isinstance(raw, dict) or set(raw) != {"name", "description", "help", "subcommands"}:
        raise ValueError("unexpected native command schema")
    name, description, help_text, children = (raw[k] for k in ("name", "description", "help", "subcommands"))
    if not all(isinstance(v, str) for v in (name, description, help_text)):
        raise ValueError("command text fields must be strings")
    if not isinstance(name, str) or not isinstance(description, str) or not isinstance(help_text, str):
        raise ValueError("invalid command text")
    if not re.fullmatch(r"[a-z][a-z0-9-]*( [a-z][a-z0-9-]*)*", name):
        raise ValueError("unexpected native command path")
    if name in seen or (parent is not None and name.rsplit(" ", 1)[0] != parent):
        raise ValueError("duplicate or misplaced native command")
    seen.add(name)
    if not isinstance(children, list) or not help_text.strip():
        raise ValueError("command children/help invalid")
    return Command(name, description, help_text.rstrip(),
                   tuple(command(child, name, seen) for child in children))


def paths(node: Command) -> list[str]:
    return [f"linear {node.name}"] + [p for child in node.subcommands for p in paths(child)]


def section(node: Command, depth: int) -> str:
    heading = "#" * min(depth, 6)
    text = f"{heading} {node.name.rsplit(' ', 1)[-1]}\n\n> {node.description}\n\n```\n{node.help}\n```\n"
    return text + "".join("\n" + section(child, depth + 1) for child in node.subcommands)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--skill-dir", type=Path, required=True)
    args = parser.parse_args()
    if not args.binary.is_absolute() or not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256):
        raise ValueError("absolute binary and lowercase SHA256 required")
    if hashlib.sha256(args.binary.read_bytes()).hexdigest() != args.binary_sha256:
        raise ValueError("native documentation binary SHA mismatch")
    raw = json.loads(args.manifest.read_text())
    if not isinstance(raw, dict) or set(raw) != {"version", "binarySha256", "rootHelp", "commands"}:
        raise ValueError("unexpected native manifest schema")
    if raw["version"] != "3.0.0-alpha.1" or raw["binarySha256"] != args.binary_sha256:
        raise ValueError("manifest version/SHA does not bind the candidate")
    if (not isinstance(raw["rootHelp"], str) or not raw["rootHelp"].strip()
            or not isinstance(raw["commands"], list)):
        raise ValueError("native root fields invalid")
    seen: set[str] = set()
    commands = sorted((command(value, None, seen) for value in raw["commands"]), key=lambda c: c.name)
    if not commands or any(" " in node.name for node in commands):
        raise ValueError("native root command set invalid")
    # Render and validate everything before changing the destination.
    template = (args.skill_dir / "SKILL.native.template.md").read_text()
    if "{{COMMANDS}}" not in template or "{{REFERENCE_TOC}}" not in template:
        raise ValueError("skill template placeholders missing")
    generated = {f"{node.name}.md": section(node, 1) for node in commands}
    generated["commands.md"] = "# Linear CLI Command Reference\n\n" + "\n".join(
        f"- [linear {node.name}]({node.name}.md): {node.description}" for node in commands) + "\n"
    skill = template.replace("{{COMMANDS}}", "\n".join(f"- `{p}`" for node in commands for p in paths(node)))
    skill = skill.replace("{{REFERENCE_TOC}}", "\n".join(
        f"- [{node.name}](references/{node.name}.md): {node.description}" for node in commands))
    references = args.skill_dir / "references"
    references.mkdir(parents=True, exist_ok=True)
    for name, content in generated.items():
        (references / name).write_text(content)
    preserved = {"organization-features.md"}
    for file in references.glob("*.md"):
        if file.name not in generated and file.name not in preserved:
            file.unlink()
    (args.skill_dir / "SKILL.md").write_text(skill)


if __name__ == "__main__":
    main()
