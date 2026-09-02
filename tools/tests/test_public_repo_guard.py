from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

MODULE_PATH = Path(__file__).parents[1] / "public_repo_guard.py"
SPEC = importlib.util.spec_from_file_location("public_repo_guard", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load public repository guard")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class PublicRepoGuardTests(unittest.TestCase):
    def scan(self, relative: str, data: bytes, literals: tuple[bytes, ...] = ()):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = Path(relative)
            absolute = root / path
            absolute.parent.mkdir(parents=True, exist_ok=True)
            absolute.write_bytes(data)
            return GUARD.scan_paths(root, (path,), literals)

    def test_accepts_bounded_public_text(self):
        self.assertEqual(self.scan("crates/example/src/lib.rs", b"//! public\n"), [])

    def test_rejects_private_path(self):
        findings = self.scan("docs/private/plan.md", b"redacted\n")
        self.assertEqual([finding.code for finding in findings], ["P001"])

    def test_rejects_archive(self):
        findings = self.scan("fixture.zip", b"not an archive")
        self.assertEqual([finding.code for finding in findings], ["P002"])

    def test_rejects_missing_production_path(self):
        macro = b"to" + b"do!()"
        findings = self.scan("crates/example/src/lib.rs", macro)
        self.assertEqual([finding.code for finding in findings], ["R001"])

    def test_redacts_private_literal(self):
        findings = self.scan("README.md", b"prefix hidden-value suffix", (b"hidden-value",))
        self.assertEqual(findings, [GUARD.Finding("S002", "README.md")])

if __name__ == "__main__":
    unittest.main()
