#!/usr/bin/env python3
"""Bundle checksum-bound Cargo package notices, with explicit upstream supplements.

Development packaging only. No downloads, service/credential calls, or legal assessment.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tomllib


def text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a nonempty string")
    return value


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate(repository: Path, metadata_file: Path, output: Path) -> dict:
    repository = repository.resolve()
    metadata = json.loads(metadata_file.read_text())
    if not isinstance(metadata, dict) or not isinstance(metadata.get("packages"), list):
        raise ValueError("Cargo metadata packages must be an array")
    lock = tomllib.loads((repository / "rust/Cargo.lock").read_text())
    locked = {}
    for package in lock["package"]:
        key = (text(package.get("name"), "locked name"), text(package.get("version"), "locked version"))
        if key in locked:
            raise ValueError("duplicate locked package identity")
        locked[key] = package
    supplemental_dir = repository / "rust/licenses/supplemental"
    supplemental = json.loads((supplemental_dir / "sources.json").read_text())
    if not isinstance(supplemental, dict) or not isinstance(supplemental.get("records"), list):
        raise ValueError("supplemental notice records must be an array")
    additions = {}
    for record in supplemental["records"]:
        if not isinstance(record, dict) or set(record) != {"file", "sha256", "sourceUrl", "packages", "qualification"}:
            raise ValueError("unexpected supplemental notice schema")
        relative = Path(text(record["file"], "supplemental path"))
        if relative.is_absolute() or ".." in relative.parts or relative.parts[0] != "supplemental":
            raise ValueError("supplemental notice path escapes its directory")
        source = repository / "rust/licenses" / relative
        data = source.read_bytes()
        if source.is_symlink() or digest(data) != record["sha256"]:
            raise ValueError("supplemental notice SHA mismatch")
        url = text(record["sourceUrl"], "source URL")
        if not url.startswith("https://"):
            raise ValueError("supplemental source needs HTTPS")
        identities = record["packages"]
        if not isinstance(identities, list) or not identities:
            raise ValueError("supplemental package identities missing")
        for identity in identities:
            identity = text(identity, "supplemental package identity")
            additions.setdefault(identity, []).append((source.name, data, url, text(record["qualification"], "qualification")))
    rows = []
    writes = {}
    seen = set()
    for package in metadata["packages"]:
        if not isinstance(package, dict):
            raise ValueError("Cargo package must be an object")
        name = text(package.get("name"), "package name")
        version = text(package.get("version"), "package version")
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name) or not re.fullmatch(r"[A-Za-z0-9.+-]+", version):
            raise ValueError("package identity is not portable")
        key = (name, version)
        if key in seen or key not in locked:
            raise ValueError("duplicate or unlocked Cargo package")
        seen.add(key)
        license_text = text(package.get("license"), "SPDX license declaration")
        manifest = Path(text(package.get("manifest_path"), "package manifest")).resolve()
        root = manifest.parent
        source = package.get("source")
        if source != locked[key].get("source"):
            raise ValueError("Cargo source differs from lock")
        identity = f"{name}@{version}"
        files = []
        local = source is None
        if local and name in {"linear-cli", "linear-schema"}:
            files.append((repository / "LICENSE", "LICENSE", "repository:LICENSE", "project MIT text"))
        else:
            for candidate in sorted(root.iterdir()):
                lower = candidate.name.lower()
                notice = lower.startswith(("license", "copying", "notice", "copyright"))
                # This checksum-bound package stores its full MIT permission/copyright in AUTHORS.
                notice = notice or (name == "r-efi" and candidate.name == "AUTHORS")
                if not notice:
                    continue
                candidates = sorted(candidate.rglob("*")) if candidate.is_dir() else [candidate]
                for file in candidates:
                    if file.is_file():
                        if file.is_symlink() or not file.resolve().is_relative_to(root):
                            raise ValueError("notice input symlink/escape")
                        relative = file.relative_to(root).as_posix()
                        url = f"https://static.crates.io/crates/{name}/{name}-{version}.crate" if not local else f"repository:{root.relative_to(repository).as_posix()}/{relative}"
                        files.append((file, relative, url, "checksum-bound package notice" if not local else "vendored package notice"))
            explicit = package.get("license_file")
            if explicit is not None:
                file = Path(text(explicit, "license file"))
                if not file.is_absolute():
                    file = root / file
                file = file.resolve()
                if not file.is_file() or not file.is_relative_to(root):
                    raise ValueError("explicit license file is outside package")
                if not any(file == record[0] for record in files):
                    files.append((file, file.relative_to(root).as_posix(), f"https://static.crates.io/crates/{name}/{name}-{version}.crate", "explicit package license_file"))
        notices = []
        directory = Path(f"{name}-{version}")
        for file, filename, url, qualification in files:
            data = file.read_bytes()
            if not data:
                raise ValueError("empty package notice")
            relative = directory / filename
            if relative in writes:
                raise ValueError("notice path collision")
            writes[relative] = data
            notices.append({"file": relative.as_posix(), "sha256": digest(data), "sourceUrl": url, "qualification": qualification})
        for filename, data, url, qualification in additions.get(identity, []):
            relative = directory / ("UPSTREAM-" + filename)
            if relative in writes:
                raise ValueError("supplemental notice path collision")
            writes[relative] = data
            notices.append({"file": relative.as_posix(), "sha256": digest(data), "sourceUrl": url, "qualification": qualification})
        if not notices:
            raise ValueError(f"notice text unavailable for {identity}; do not label declaration-only complete")
        rows.append({"name": name, "version": version, "source": source or f"repository:{root.relative_to(repository).as_posix()}", "checksum": locked[key].get("checksum"), "license": license_text, "status": "NOTICE-TEXT-BUNDLED", "files": notices})
    if seen != set(locked):
        raise ValueError("metadata does not cover every locked package")
    if set(additions) - {f"{name}@{version}" for name, version in seen}:
        raise ValueError("unused supplemental package identity")
    # Resolve all input identity/hash/path/notice gaps before creating output.
    for relative, data in writes.items():
        path = output / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    result = {"kind": "all-lock package notice provenance; not legal assessment or all-target artifact proof", "lockedPackageCount": len(locked), "metadataPackageCount": len(seen), "noticeFileCount": len(writes), "packages": sorted(rows, key=lambda row: (row["name"], row["version"]))}
    output.mkdir(parents=True, exist_ok=True)
    (output / "inventory.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = generate(args.repository, args.metadata, args.output)
    print(f"Bundled {result['lockedPackageCount']} packages / {result['noticeFileCount']} notice files")


if __name__ == "__main__":
    main()
