#!/usr/bin/env python3
"""Verify checked-in public schema digests without rewriting source files."""

from __future__ import annotations

import hashlib
from pathlib import Path
import sys


def verify_digest(digest_file: Path) -> bool:
    """Return whether one strict SHA-256 record matches its sibling schema."""

    fields = digest_file.read_text(encoding="ascii").strip().split()
    if len(fields) != 2 or len(fields[0]) != 64:
        return False
    try:
        expected = bytes.fromhex(fields[0])
    except ValueError:
        return False
    schema = digest_file.parent / fields[1]
    if not schema.is_file() or schema.parent != digest_file.parent:
        return False
    actual = hashlib.sha256(schema.read_bytes()).digest()
    return actual == expected


def main() -> int:
    """Verify the canonical Ceylith v2 schema digest."""

    root = Path(__file__).resolve().parents[1]
    digest_file = root / "schemas" / "ceylith" / "v2.proto.sha256"
    if not verify_digest(digest_file):
        print("schema-digest FAIL: schemas/ceylith/v2.proto", file=sys.stderr)
        return 1
    print("schema-digest PASS: schemas/ceylith/v2.proto")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
