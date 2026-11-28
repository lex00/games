"""The ROMs the games' page-check routes play, and which aren't here.

    python3 -I page/missing-roms.py GAMES_DIR

For each game folder of GAMES_DIR with a page-check.toml, each `rom` its
[[route]] and [[load]] entries name (a roms.toml source id) is looked for in
$GP_ROM_DIR (default ~/.gameprinter/roms): an NES ROM by the source's
sha256 over the file less its 16-byte header, an arcade set as
arcade/ID.zip. Prints "ok" or "missing ROM" a line each and exits 0: the
page check skips a missing ROM's sections with a line of its own, and this
says so before the build, plainly. Exit 2 when a file can't be read.
"""
import hashlib
import os
import sys
import tomllib

games = sys.argv[1]
root = os.environ.get("GP_ROM_DIR") or os.path.join(os.path.expanduser("~"), ".gameprinter", "roms")


def nes_hashes(d):
    out = set()
    for dirpath, _, names in os.walk(d):
        for n in names:
            p = os.path.join(dirpath, n)
            try:
                size = os.path.getsize(p)
                if not 16 < size < 4 << 20:
                    continue
                with open(p, "rb") as f:
                    b = f.read()
            except OSError:
                continue
            if b[:4] == b"NES\x1a":
                out.add(hashlib.sha256(b[16:]).hexdigest())
    return out


have = None
missing = 0
for name in sorted(os.listdir(games)):
    folder = os.path.join(games, name)
    check = os.path.join(folder, "page-check.toml")
    if not os.path.isfile(check):
        continue
    try:
        with open(check, "rb") as f:
            facts = tomllib.load(f)
        roms_toml = os.path.join(folder, "roms.toml")
        sources = {}
        if os.path.isfile(roms_toml):
            with open(roms_toml, "rb") as f:
                sources = {s.get("id"): s for s in tomllib.load(f).get("source", [])}
    except (OSError, tomllib.TOMLDecodeError) as e:
        sys.exit(f"missing-roms: {e}")
    ids = []
    for entry in facts.get("route", []) + facts.get("load", []):
        rom = entry.get("rom")
        if rom and rom not in ids:
            ids.append(rom)
    for rom in ids:
        s = sources.get(rom, {})
        title = s.get("title", rom)
        zipped = os.path.join(root, "arcade", f"{rom}.zip")
        sha = s.get("sha256")
        if os.path.isfile(zipped):
            found = True
        elif sha:
            if have is None:
                have = nes_hashes(root)
            found = sha in have
        else:
            found = False
        if found:
            print(f"ok    {name}'s {rom} ({title}) is in GP_ROM_DIR")
        else:
            missing += 1
            print(f"missing ROM: {name}'s {rom} ({title}) isn't in GP_ROM_DIR ({root}), so its route sections are skipped")
if missing:
    print(f"note: {missing} ROM(s) the routes play are missing; the check runs without them and says which sections it skipped")
