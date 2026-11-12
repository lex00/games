#!/usr/bin/env python3
"""On the machine holding the ROMs: says whether each ROM a package here names
([[rom.entry]] in */package.toml) is there, a NES image by headerless SHA-256,
an arcade set by its zip, $GP_ROM_DIR/arcade/<set>.zip. It prints only ids and
yes or no, never a byte or a path. (.githooks/guarded-roms.toml guards more
than the packages use, the old tool's sources among them, so it isn't the list.)

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
    guarded = [
        e
        for p in sorted(Path(".").glob("*/package.toml"))
        for e in tomllib.loads(p.read_text()).get("rom", {}).get("entry", [])
    ]
    root = Path(os.environ["GP_ROM_DIR"])
    have = set()
    for p in root.rglob("*.nes"):
        have.add(headerless(p.read_bytes()))
    missing = 0
    for e in guarded:
        if e.get("platform", "nes") == "arcade":
            found = (root / "arcade" / f"{e.get('system')}.zip").is_file()
        else:
            found = e["sha256"] in have
        print(f"{e['id']}: {'found' if found else 'MISSING'}")
        missing += not found
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
