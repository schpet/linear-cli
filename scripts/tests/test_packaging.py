"""Public development-tool fixtures; no CLI services, network, or package builds."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Packaging(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="linear-package-fixture-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def run_script(self, path, *arguments):
        return subprocess.run([sys.executable, str(ROOT / path), *map(str, arguments)],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
                              env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})

    def docs(self, change=None, expected=0):
        binary = self.root / "candidate"
        binary.write_bytes(b"fixture; deliberately not executable")
        sha = hashlib.sha256(binary.read_bytes()).hexdigest()
        data = {"version": "3.0.0-alpha.1", "binarySha256": sha, "rootHelp": "root help",
                "commands": [{"name": "issue", "description": "Issues", "help": "issue help",
                              "subcommands": [{"name": "issue view", "description": "View", "help": "view help", "subcommands": []}]}]}
        if change:
            change(data)
        manifest = self.root / "manifest.json"
        manifest.write_text(json.dumps(data))
        skill = self.root / "skill"
        (skill / "references").mkdir(parents=True)
        (skill / "references/organization-features.md").write_text("preserved organization")
        (skill / "references/obsolete.md").write_text("stale")
        (skill / "SKILL.native.template.md").write_text("Commands\n{{COMMANDS}}\nReferences\n{{REFERENCE_TOC}}")
        output = self.run_script("skills/linear-cli/scripts/generate-native-docs.py", "--manifest", manifest,
                                 "--binary", binary, "--binary-sha256", sha, "--skill-dir", skill)
        self.assertEqual(output.returncode, expected, output.stderr.decode())
        self.assertEqual((skill / "references/organization-features.md").read_text(), "preserved organization")
        if expected:
            self.assertFalse((skill / "SKILL.md").exists())
            self.assertEqual((skill / "references/obsolete.md").read_text(), "stale")
        return skill

    def test_native_docs_render_nested_paths_and_preserve_organization(self):
        skill = self.docs()
        self.assertIn("linear issue view", (skill / "SKILL.md").read_text())
        self.assertIn("view help", (skill / "references/issue.md").read_text())
        self.assertFalse((skill / "references/obsolete.md").exists())

    def test_native_docs_sha_failure_has_no_output_effects(self):
        self.docs(lambda data: data.update(binarySha256="0" * 64), 1)

    def test_native_docs_misplaced_path_has_no_output_effects(self):
        self.docs(lambda data: data["commands"][0]["subcommands"][0].update(name="team view"), 1)

    def test_native_docs_blank_root_help_has_no_output_effects(self):
        self.docs(lambda data: data.update(rootHelp="  "), 1)

    def test_native_docs_duplicate_path_has_no_output_effects(self):
        self.docs(lambda data: data["commands"].append(data["commands"][0]), 1)

    def archive(self, bad=None, expected=0):
        names = ["LICENSE", "graphql/schema.graphql", "src/utils/linear.ts", "rust/Cargo.toml", "rust/Cargo.lock",
                 "rust/rust-toolchain.toml", "rust/dist-workspace.toml", "docs/rust-port.md",
                 "skills/linear-cli/SKILL.native.template.md", "skills/linear-cli/scripts/generate-native-docs.py",
                 "skills/linear-cli/references/organization-features.md", "rust/crates/demo/src/lib.rs"]
        repository = self.root / "repository"
        for name in names:
            path = repository / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture " + name)
        if bad:
            bad(repository, names)
        manifest = self.root / "inputs.txt"
        manifest.write_text("\n".join(names) + "\n")
        output = self.root / "out/source.tar.xz"
        process = self.run_script("rust/tools/make-source-archive.py", "--repository", repository,
                                  "--input-manifest", manifest, "--output", output)
        self.assertEqual(process.returncode, expected, process.stderr.decode())
        if expected:
            self.assertFalse(output.parent.exists())
        return output, names

    def test_source_archive_explicit_layout_and_checksum(self):
        output, names = self.archive()
        with tarfile.open(output) as archive:
            self.assertEqual(set(archive.getnames()), {"linear-cli-3.0.0-alpha.1/" + name for name in names})
            self.assertTrue(all(member.mtime == 0 for member in archive.getmembers()))
        self.assertEqual(output.with_suffix(".xz.sha256").read_text().split()[0], hashlib.sha256(output.read_bytes()).hexdigest())

    def test_source_archive_rejects_unlisted_private_tree(self):
        self.archive(lambda repository, names: names.append("untracked/private.txt"), 1)

    def test_source_archive_rejects_symlink_before_output(self):
        def change(repository, names):
            path = repository / "LICENSE"
            path.unlink()
            path.symlink_to(repository / "rust/Cargo.toml")
        self.archive(change, 1)

    def licenses(self, change=None, expected=0):
        repository = self.root / "repository"
        (repository / "licenses/supplemental").mkdir(parents=True)
        (repository / "licenses/supplemental/sources.json").write_text('{"records": []}')
        source = "registry+https://github.com/rust-lang/crates.io-index"
        (repository / "Cargo.lock").write_text(f'version = 4\n[[package]]\nname="example"\nversion="1.0.0"\nsource="{source}"\nchecksum="' + "a" * 64 + '"\n')
        package = self.root / "package"
        package.mkdir()
        (package / "Cargo.toml").write_text('[package]\nname="example"\nversion="1.0.0"\n')
        (package / "LICENSE").write_text("example permission and copyright")
        data = {"packages": [{"name": "example", "version": "1.0.0", "source": source,
                              "license": "MIT", "manifest_path": str(package / "Cargo.toml"), "license_file": None}]}
        if change:
            change(repository, package, data)
        metadata = self.root / "metadata.json"
        metadata.write_text(json.dumps(data))
        output = self.root / "notices"
        process = self.run_script("scripts/generate-license-inventory.py", "--repository", repository,
                                  "--metadata", metadata, "--output", output)
        self.assertEqual(process.returncode, expected, process.stderr.decode())
        if expected:
            self.assertFalse(output.exists())
        return output

    def test_license_inventory_derives_identity_count_and_notice_hash(self):
        output = self.licenses()
        inventory = json.loads((output / "inventory.json").read_text())
        self.assertEqual(inventory["lockedPackageCount"], 1)
        self.assertEqual(inventory["noticeFileCount"], 1)
        notice = inventory["packages"][0]["files"][0]
        self.assertEqual(notice["sha256"], hashlib.sha256((output / notice["file"]).read_bytes()).hexdigest())
        self.assertNotIn(str(self.root), json.dumps(inventory))

    def test_license_inventory_missing_text_has_no_output_effects(self):
        self.licenses(lambda repository, package, data: (package / "LICENSE").unlink(), 1)

    def test_license_inventory_duplicate_identity_has_no_output_effects(self):
        self.licenses(lambda repository, package, data: data["packages"].append(data["packages"][0]), 1)

    def test_license_inventory_bad_supplement_sha_has_no_output_effects(self):
        def change(repository, package, data):
            directory = repository / "licenses/supplemental"
            (directory / "NOTICE").write_text("supplement")
            (directory / "sources.json").write_text(json.dumps({"records": [{"file": "supplemental/NOTICE", "sha256": "0" * 64,
                "sourceUrl": "https://example.test/LICENSE", "packages": ["example@1.0.0"], "qualification": "fixture"}]}))
        self.licenses(change, 1)


if __name__ == "__main__":
    unittest.main()
