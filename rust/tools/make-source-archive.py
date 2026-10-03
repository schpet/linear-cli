#!/usr/bin/env python3
"""Create the native source artifact subset, not a VCS workspace/checkout.

Preserves authoritative schema/build relative layout.
No builds, downloads, user configuration or credential files.
"""
import argparse
import hashlib
from pathlib import Path
import tarfile


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--input-manifest", type=Path, required=True)
    args = parser.parse_args()
    root = args.repository.resolve()
    version = "3.0.0-alpha.1"
    required = ["LICENSE", "graphql/schema.graphql", "src/utils/linear.ts", "rust/Cargo.toml", "rust/Cargo.lock",
                "rust/rust-toolchain.toml", "rust/dist-workspace.toml", "docs/rust-port.md", "skills/linear-cli/SKILL.native.template.md",
                "skills/linear-cli/scripts/generate-native-docs.py",
                "skills/linear-cli/references/organization-features.md"]
    directories = ["rust/crates", "rust/parity", "rust/licenses", "rust/scripts"]
    # Root supplies a snapshot-bound list of tracked inputs plus qualified generated
    # notices. Never glob ignored caches/private captures into a source artifact.
    names = args.input_manifest.read_text().splitlines()
    if len(names) != len(set(names)) or not set(required).issubset(names):
        raise ValueError("source manifest duplicates or missing required inputs")
    files = []
    for name in names:
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts or not name:
            raise ValueError("source manifest needs portable relative paths")
        if name not in required and not any(name.startswith(directory + "/") for directory in directories):
            raise ValueError(f"source input outside explicit artifact scope: {name}")
        files.append(root / relative)
    for file in files:
        if not file.is_file() or file.is_symlink() or not file.resolve().is_relative_to(root):
            raise ValueError(f"unsupported source archive input: {file}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(args.output, "w:xz") as archive:
        for file in sorted(set(files)):
            name = f"linear-cli-{version}/{file.relative_to(root).as_posix()}"
            info = archive.gettarinfo(file, name)
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = 0
            with file.open("rb") as stream:
                archive.addfile(info, stream)
    args.output.with_suffix(args.output.suffix + ".sha256").write_text(
        hashlib.sha256(args.output.read_bytes()).hexdigest() + "  " + args.output.name + "\n")


if __name__ == "__main__":
    main()
