#!/usr/bin/env python3
"""The scan every commit in this repo must pass, hosted or on the user's
machine. It reads the tracked files of a work tree and refuses what the
ROM-blocking hook refuses, so a commit that reached a branch without the hook
is still caught. It needs no ROM; the runner's job adds the check of ROM
bytes written as text.

    python3 -I -B scripts/scan.py [ROOT]

Exit codes: 0 clean, 1 something refused.
"""
import subprocess
import sys
from pathlib import Path

MAX_BYTES = 1048576
BAD_EXT = {".nes", ".fds", ".unf", ".unif", ".prg", ".chr", ".nsf", ".nsfe",
           ".fm2", ".bk2", ".mmo", ".state", ".mss", ".mst", ".sav", ".srm",
           ".ips", ".bps", ".ups", ".xdelta", ".wasm", ".png", ".wav"}
MAGIC = [b"NES\x1a", b"FDS\x1a", b"UNIF", b"\x01*NINTENDO-HVC*", b"NESM\x1a",
         b"NSFE", b"PATCH", b"BPS1", b"UPS1"]


def refusal(rel: str, data: bytes) -> str | None:
    if Path(rel).suffix.lower() in BAD_EXT:
        return "a game image, patch or built file by its name"
    if any(data.startswith(m) for m in MAGIC):
        return "a game image or patch by its contents"
    if b"\0" in data[:8192]:
        return "a binary file; this repo holds text only"
    if len(data) > MAX_BYTES:
        return f"{len(data)} bytes, over the 1 MB limit"
    return None


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        check=True, capture_output=True,
    ).stdout.split(b"\0")
    bad = 0
    count = 0
    for raw in filter(None, tracked):
        rel = raw.decode()
        count += 1
        why = refusal(rel, (root / rel).read_bytes())
        if why:
            print(f"refused  {rel}: {why}")
            bad += 1
    print(f"scan: {count} files, {bad} refused")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
