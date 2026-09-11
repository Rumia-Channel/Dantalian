#!/usr/bin/env python3
"""Bump the Dantalian version across every file that records it.

Usage:
    python scripts/bump_version.py 0.1.3

Updates:
    Cargo.toml              package version
    worker/Cargo.toml       package version + dantalian dependency version
    about.hbs               license page header ("Dantalian X.Y.Z")
    Cargo.lock              regenerated via `cargo check`
    worker/Cargo.lock       regenerated via `cargo check`
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+$")


def fail(message: str) -> None:
    raise SystemExit(f"bump_version: {message}")


def replace_once(path: Path, pattern: re.Pattern[str], replacement: str) -> None:
    text = path.read_text(encoding="utf-8")
    new_text, count = pattern.subn(replacement, text, count=1)
    if count != 1:
        fail(f"{path}: pattern {pattern.pattern!r} did not match")
    path.write_text(new_text, encoding="utf-8")
    print(f"updated {path.relative_to(ROOT)}")


def cargo_check(cwd: Path) -> None:
    result = subprocess.run(
        ["cargo", "check", "--offline"],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        fail(f"cargo check failed in {cwd}:\n{result.stderr.strip()}")
    print(f"cargo check ok ({cwd.relative_to(ROOT) or '.'})")


def main() -> None:
    if len(sys.argv) != 2 or not VERSION_PATTERN.fullmatch(sys.argv[1]):
        fail("usage: python scripts/bump_version.py X.Y.Z")
    version = sys.argv[1]

    replace_once(
        ROOT / "Cargo.toml",
        re.compile(r'(?m)^version = "[0-9]+\.[0-9]+\.[0-9]+"'),
        f'version = "{version}"',
    )
    replace_once(
        ROOT / "worker" / "Cargo.toml",
        re.compile(r'(?m)^version = "[0-9]+\.[0-9]+\.[0-9]+"'),
        f'version = "{version}"',
    )
    replace_once(
        ROOT / "worker" / "Cargo.toml",
        re.compile(r'dantalian = \{ version = "[0-9]+\.[0-9]+\.[0-9]+"'),
        f'dantalian = {{ version = "{version}"',
    )
    replace_once(
        ROOT / "about.hbs",
        re.compile(r"Dantalian [0-9]+\.[0-9]+\.[0-9]+"),
        f"Dantalian {version}",
    )

    cargo_check(ROOT)
    cargo_check(ROOT / "worker")

    print(f"bumped to {version}; review with `git diff` and commit")


if __name__ == "__main__":
    main()
