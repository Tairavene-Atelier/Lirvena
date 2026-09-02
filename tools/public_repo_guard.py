#!/usr/bin/env python3
"""Fail closed when public-repository inputs contain unsafe artifact classes."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
from typing import Iterable, NamedTuple

MAX_FILE_BYTES = 1_048_576
MAX_PATHS = 10_000
MAX_DENYLIST_ENTRIES = 4_096
MAX_DENYLIST_ENTRY_BYTES = 256

BLOCKED_PATH_PARTS = frozenset(
    {
        "captures",
        "evidence",
        "private",
        "private-plan",
        "private-plans",
        "private-spec",
        "private-specs",
        "reverse-engineering",
        "reversing",
    }
)
BLOCKED_SUFFIXES = frozenset(
    {
        ".7z",
        ".bin",
        ".dll",
        ".dylib",
        ".exe",
        ".har",
        ".jks",
        ".key",
        ".p12",
        ".pcap",
        ".pcapng",
        ".pfx",
        ".rar",
        ".so",
        ".zip",
    }
)
ALLOWED_BINARY_SUFFIXES = frozenset(
    {".gif", ".ico", ".jpeg", ".jpg", ".png", ".webp", ".woff2"}
)
PRODUCTION_RUST_PREFIXES = ("bins/", "crates/")
PRODUCTION_RUST_SEGMENT = "/src/"


class Finding(NamedTuple):
    """A redacted repository-guard finding."""

    code: str
    path: str


def repository_paths(root: Path) -> tuple[Path, ...]:
    """Return tracked and non-ignored untracked repository paths."""

    command = [
        "git",
        "-C",
        str(root),
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]
    result = subprocess.run(command, check=True, capture_output=True)
    raw_paths = tuple(item for item in result.stdout.split(b"\0") if item)
    if len(raw_paths) > MAX_PATHS:
        raise ValueError("repository path count exceeds the public guard limit")

    paths: list[Path] = []
    for raw_path in raw_paths:
        paths.append(Path(os.fsdecode(raw_path)))
    return tuple(paths)


def load_private_literals(root: Path, environment: dict[str, str]) -> tuple[bytes, ...]:
    """Load an optional denylist that must remain outside the public checkout."""

    configured = environment.get("LIRVENA_PUBLIC_GUARD_DENYLIST")
    if configured is None:
        return ()

    denylist_path = Path(configured).expanduser().resolve(strict=True)
    root_resolved = root.resolve(strict=True)
    if denylist_path.is_relative_to(root_resolved):
        raise ValueError("the private denylist must stay outside the public checkout")

    entries: list[bytes] = []
    for raw_line in denylist_path.read_bytes().splitlines():
        entry = raw_line.strip()
        if not entry or entry.startswith(b"#"):
            continue
        if len(entry) > MAX_DENYLIST_ENTRY_BYTES:
            raise ValueError("a private denylist entry exceeds the size limit")
        entries.append(entry)
        if len(entries) > MAX_DENYLIST_ENTRIES:
            raise ValueError("private denylist entry count exceeds the limit")
    return tuple(entries)


def scan_paths(root: Path, paths: Iterable[Path], private_literals: tuple[bytes, ...]) -> list[Finding]:
    """Scan bounded public inputs without printing matched content."""

    findings: list[Finding] = []
    secret_markers = (
        b"-----BEGIN " + b"PRIVATE" + b" KEY-----",
        b"-----BEGIN OPENSSH " + b"PRIVATE" + b" KEY-----",
        b"gh" + b"p_",
        b"github" + b"_pat_",
    )
    forbidden_macros = (
        b"to" + b"do!",
        b"un" + b"implemented!",
        b"db" + b"g!",
        b"pa" + b"nic!",
    )
    for relative in paths:
        normalized = relative.as_posix()
        absolute = root / relative

        if any(part.casefold() in BLOCKED_PATH_PARTS for part in relative.parts):
            findings.append(Finding("P001", normalized))
            continue
        if relative.suffix.casefold() in BLOCKED_SUFFIXES:
            findings.append(Finding("P002", normalized))
            continue
        if absolute.is_symlink():
            findings.append(Finding("P003", normalized))
            continue
        if not absolute.is_file():
            continue

        size = absolute.stat().st_size
        if size > MAX_FILE_BYTES:
            findings.append(Finding("B001", normalized))
            continue
        data = absolute.read_bytes()

        if b"\0" in data and relative.suffix.casefold() not in ALLOWED_BINARY_SUFFIXES:
            findings.append(Finding("B002", normalized))
            continue
        if any(marker in data for marker in secret_markers):
            findings.append(Finding("S001", normalized))
        if any(literal in data for literal in private_literals):
            findings.append(Finding("S002", normalized))
        is_production_rust = (
            normalized.endswith(".rs")
            and normalized.startswith(PRODUCTION_RUST_PREFIXES)
            and PRODUCTION_RUST_SEGMENT in normalized
        )
        if is_production_rust and any(macro in data for macro in forbidden_macros):
            findings.append(Finding("R001", normalized))

        if b"\0" not in data:
            try:
                data.decode("utf-8")
            except UnicodeDecodeError:
                findings.append(Finding("T001", normalized))

    return findings


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    return parser.parse_args()


def main() -> int:
    """Run the public repository guard."""

    args = parse_args()
    root = args.root.resolve(strict=True)
    try:
        paths = repository_paths(root)
        private_literals = load_private_literals(root, dict(os.environ))
        findings = scan_paths(root, paths, private_literals)
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f"public-repo-guard error: {error}", file=sys.stderr)
        return 2

    if findings:
        for finding in sorted(set(findings)):
            print(f"public-repo-guard {finding.code}: {finding.path}", file=sys.stderr)
        return 1

    print(f"public-repo-guard PASS: {len(paths)} bounded paths")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
