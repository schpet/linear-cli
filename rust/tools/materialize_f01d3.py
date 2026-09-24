#!/usr/bin/env python3
"""Materialize the reviewed F01D3 route allowlist for one local Rust binary.

The committed route template is portable. The full candidate descriptor is
machine-local, always written under ignored rust/target, and remains subject
to the parity runner's strict schema and confinement checks.
"""

import argparse
import json
import os
import stat
import tempfile
from pathlib import Path

RUST_ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = RUST_ROOT / "parity" / "candidates" / "f01d3-routes.json"
MANIFEST = RUST_ROOT / "parity" / "manifest.json"
OUTPUT = RUST_ROOT / "target" / "f01d3-candidate.json"
FORBIDDEN_PREFIXES = (
    "/proc", "/dev", "/sys", "/tmp", "/run", "/var/run",
    "/etc", "/boot", "/root", "/usr/local",
)
FORBIDDEN_ROOTS = frozenset((
    "/", "/home", "/usr", "/var", "/var/tmp", "/opt", "/srv",
    "/mnt", "/media",
))


class MaterializeError(ValueError):
    """Candidate cannot be represented safely in the confined runner."""


def under(path: Path, prefix: str) -> bool:
    return path == Path(prefix) or Path(prefix) in path.parents


def reject_forbidden(path: Path) -> None:
    if str(path) in FORBIDDEN_ROOTS:
        raise MaterializeError(f"refusing whole-tree binary path: {path}")
    for prefix in FORBIDDEN_PREFIXES:
        if under(path, prefix):
            raise MaterializeError(f"binary path is under forbidden bind prefix {prefix}: {path}")


def resolve_binary(path: Path) -> Path:
    if not path.is_absolute():
        raise MaterializeError(f"binary path must be absolute: {path}")
    reject_forbidden(path)
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise MaterializeError(f"binary path does not exist: {path}") from error
    reject_forbidden(resolved)
    mode = resolved.stat().st_mode
    if not stat.S_ISREG(mode):
        raise MaterializeError(f"binary is not a regular file: {resolved}")
    if mode & 0o111 == 0:
        raise MaterializeError(f"binary is not executable: {resolved}")
    return resolved


def descriptor_bytes(template: object, manifest: object, binary: Path) -> bytes:
    if not isinstance(template, dict) or set(template) != {"name", "implementedRoutes"}:
        raise MaterializeError("route template must contain only name and implementedRoutes")
    name = template["name"]
    routes = template["implementedRoutes"]
    if not isinstance(name, str) or not name:
        raise MaterializeError("candidate name must be a nonempty string")
    if not isinstance(routes, list) or any(not isinstance(route, str) or not route for route in routes):
        raise MaterializeError("implementedRoutes must be a list of nonempty strings")
    if len(routes) != len(set(routes)):
        raise MaterializeError("implementedRoutes contains a duplicate")
    if not isinstance(manifest, dict) or not isinstance(manifest.get("routes"), list):
        raise MaterializeError("manifest routes are missing")
    entries = manifest["routes"]
    if any(not isinstance(route, dict) or not isinstance(route.get("path"), str)
           or not isinstance(route.get("kind"), str) for route in entries):
        raise MaterializeError("manifest route shape changed")
    parents = [route["path"] for route in entries if route["kind"] == "parent_route"]
    if len(parents) != 20 or parents[0] != "linear" or len(set(parents)) != len(parents):
        raise MaterializeError("expected 20 unique parent routes with linear first")
    if routes != parents:
        raise MaterializeError("route template differs from manifest parent routes or order")
    resolved = resolve_binary(binary)
    descriptor = {
        "name": name,
        "program": {"kind": "executable", "path": str(resolved)},
        "implementedRoutes": routes,
    }
    return (json.dumps(descriptor, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def write_or_check(output: Path, content: bytes, check: bool) -> None:
    if check:
        try:
            current = output.read_bytes()
        except OSError as error:
            raise MaterializeError(f"candidate descriptor is missing: {output}") from error
        if current != content:
            raise MaterializeError(f"candidate descriptor is stale: {output}")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    temp_path = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix=".f01d3-", delete=False) as temporary:
            temp_path = Path(temporary.name)
            temporary.write(content)
        os.replace(temp_path, output)
        temp_path = None
    finally:
        if temp_path is not None:
            temp_path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        content = descriptor_bytes(
            json.loads(TEMPLATE.read_text()),
            json.loads(MANIFEST.read_text()),
            args.binary,
        )
        write_or_check(OUTPUT, content, args.check)
    except (MaterializeError, OSError, json.JSONDecodeError) as error:
        parser.exit(1, f"F01D3 candidate: {error}\n")
    print(OUTPUT)


if __name__ == "__main__":
    main()
