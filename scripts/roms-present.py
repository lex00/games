#!/usr/bin/env python3
"""On the machine holding the ROMs: says whether each guarded ROM is there, by
headerless SHA-256. It prints only ids and yes or no, never a byte or a path.

Exit codes: 0 every ROM found, 1 one is missing.
"""
import hashlib
import os
import sys
import tomllib
from pathlib import Path


def headerless(b: bytes) -> str | None:
    if b[:4] != b"NES\x1a":
        return None
    return hashlib.sha256(b[16 + (512 if b[6] & 4 else 0):]).hexdigest()


def main() -> int:
    guarded = tomllib.loads(Path(".githooks/guarded-roms.toml").read_text())["source"]
    root = Path(os.environ["GP_ROM_DIR"])
    have = set()
    for p in root.rglob("*.nes"):
        have.add(headerless(p.read_bytes()))
    missing = 0
    for e in guarded:
        found = e["sha256"] in have
        print(f"{e['id']}: {'found' if found else 'MISSING'}")
        missing += not found
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
