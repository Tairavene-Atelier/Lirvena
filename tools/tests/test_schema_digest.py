from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

MODULE_PATH = Path(__file__).parents[1] / "schema_digest.py"
SPEC = importlib.util.spec_from_file_location("schema_digest", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load schema digest verifier")
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


class SchemaDigestTests(unittest.TestCase):
    def test_accepts_matching_sibling_schema(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            schema = root / "contract.proto"
            schema.write_bytes(b"public contract\n")
            import hashlib

            digest = hashlib.sha256(schema.read_bytes()).hexdigest()
            record = root / "contract.proto.sha256"
            record.write_text(f"{digest}  contract.proto\n", encoding="ascii")
            self.assertTrue(VERIFIER.verify_digest(record))

    def test_rejects_changed_schema(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            schema = root / "contract.proto"
            schema.write_bytes(b"changed\n")
            record = root / "contract.proto.sha256"
            record.write_text(f"{'0' * 64}  contract.proto\n", encoding="ascii")
            self.assertFalse(VERIFIER.verify_digest(record))


if __name__ == "__main__":
    unittest.main()
