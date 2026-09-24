"""Narrow contracts for the F01D3 local candidate materializer."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from materialize_f01d3 import (  # noqa: E402
    MANIFEST,
    OUTPUT,
    RUST_ROOT,
    TEMPLATE,
    MaterializeError,
    descriptor_bytes,
    resolve_binary,
    write_or_check,
)


class MaterializeF01D3Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        (RUST_ROOT / "target").mkdir(parents=True, exist_ok=True)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="f01d3-test-", dir=RUST_ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "linear"
        self.binary.write_text("#!/bin/sh\nexit 0\n")
        self.binary.chmod(0o700)
        self.template = json.loads(TEMPLATE.read_text())
        self.manifest = json.loads(MANIFEST.read_text())

    def test_output_is_script_relative(self):
        self.assertEqual(OUTPUT, RUST_ROOT / "target" / "f01d3-candidate.json")
        self.assertEqual(OUTPUT.parent, RUST_ROOT / "target")

    def test_exact_manifest_parent_list_and_deterministic_output(self):
        expected = [route["path"] for route in self.manifest["routes"] if route["kind"] == "parent_route"]
        self.assertEqual(len(expected), 20)
        self.assertEqual(self.template["implementedRoutes"], expected)
        first = descriptor_bytes(self.template, self.manifest, self.binary)
        self.assertEqual(first, descriptor_bytes(self.template, self.manifest, self.binary))
        output = self.root / "candidate.json"
        write_or_check(output, first, check=False)
        write_or_check(output, first, check=True)
        self.assertEqual(json.loads(output.read_text())["program"],
                         {"kind": "executable", "path": str(self.binary.resolve())})
        output.write_text("stale\n")
        with self.assertRaisesRegex(MaterializeError, "stale"):
            write_or_check(output, first, check=True)
        self.assertEqual(output.read_text(), "stale\n")

    def test_route_template_rejects_leaf_completion_missing_reorder_and_duplicate(self):
        parents = self.template["implementedRoutes"]
        for variant in (
            parents[:-1],
            [*parents[:-1], "linear api"],
            [*parents[:-1], "linear completions bash"],
            [parents[1], parents[0], *parents[2:]],
            [*parents, parents[0]],
        ):
            with self.subTest(variant=variant[-2:]):
                template = {"name": self.template["name"], "implementedRoutes": variant}
                with self.assertRaises(MaterializeError):
                    descriptor_bytes(template, self.manifest, self.binary)
        with self.assertRaisesRegex(MaterializeError, "only name"):
            descriptor_bytes({**self.template, "program": {}}, self.manifest, self.binary)

    def test_binary_path_is_absolute_regular_executable_and_allowed(self):
        with self.assertRaisesRegex(MaterializeError, "absolute"):
            resolve_binary(Path("relative/linear"))
        with self.assertRaisesRegex(MaterializeError, "does not exist"):
            resolve_binary(self.root / "missing")
        self.binary.chmod(0o600)
        with self.assertRaisesRegex(MaterializeError, "not executable"):
            resolve_binary(self.binary)
        self.binary.chmod(0o700)
        with self.assertRaisesRegex(MaterializeError, "forbidden bind prefix"):
            resolve_binary(Path("/tmp/not-a-candidate"))
        with self.assertRaisesRegex(MaterializeError, "not a regular file"):
            resolve_binary(self.root)

    def test_symlink_emits_real_binary_path(self):
        alias = self.root / "alias"
        alias.symlink_to(self.binary)
        result = json.loads(descriptor_bytes(self.template, self.manifest, alias))
        self.assertEqual(result["program"]["path"], str(self.binary.resolve()))


if __name__ == "__main__":
    unittest.main()
