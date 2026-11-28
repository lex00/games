#!/usr/bin/env python3
"""A game folder's facts, for this repo's justfile (its ROM checks), which
runs from a game's folder. Moved from gameprinter's scripts/check, where
the tool's own checks read every game's files; the tool knows no game now.

One fact a call, printed on stdout; a fact the game doesn't have prints
nothing and exits 1.

  game-facts.py list GAMES FILE...    the game folders under GAMES holding
                                      every FILE, one a line, sorted
  game-facts.py arcade GAMES          the game folders under GAMES whose
                                      roms.toml has an arcade source
  game-facts.py GAME rom              corpus.toml's [target] rom (sha256)
  game-facts.py GAME platform         the platform of the source that is
                                      the corpus's ROM (else the first's)
  game-facts.py GAME system [ID]      the MAME system of source ID, else of
                                      the corpus's ROM, else of the
                                      package's scored set
  game-facts.py GAME source SYSTEM    the source id whose system is SYSTEM
  game-facts.py GAME sha256 ID        source ID's sha256
  game-facts.py GAME role ROLE        package.toml's [[rom.entry]] id with
                                      that role (scored, play)
  game-facts.py GAME asset-map        the scored entry's asset_map
  game-facts.py GAME label NAME       labels.toml's address of NAME, in hex
  game-facts.py GAME check KEY.KEY    checks.toml's value (a list is
                                      printed one item a line; a table as
                                      JSON)
  game-facts.py GAME captures         oracle/mame-captures.toml's entries,
                                      one a line: name system input
                                      input_sha256 frames sha256

Only Python's standard library: run it with python3 -I -B.
"""

import json
import os
import sys
import tomllib


def load(path):
    try:
        with open(path, "rb") as f:
            return tomllib.load(f)
    except FileNotFoundError:
        return None


def sources(game):
    return (load(os.path.join(game, "roms.toml")) or {}).get("source", [])


def entries(game):
    return ((load(os.path.join(game, "package.toml")) or {}).get("rom") or {}).get("entry", [])


def corpus_rom(game):
    c = load(os.path.join(game, "corpus.toml")) or {}
    return (c.get("target") or {}).get("rom")


def out(v):
    if v is None or v == [] or v == "":
        sys.exit(1)
    if isinstance(v, list):
        for x in v:
            print(x)
    elif isinstance(v, dict):
        print(json.dumps(v))
    elif isinstance(v, bool):
        print("true" if v else "false")
    else:
        print(v)
    sys.exit(0)


def games_in(root):
    try:
        names = sorted(os.listdir(root))
    except FileNotFoundError:
        return []
    return [os.path.join(root, n) for n in names if os.path.isdir(os.path.join(root, n)) and not n.startswith(".")]


def main(a):
    if len(a) < 2:
        print(__doc__.strip(), file=sys.stderr)
        sys.exit(2)
    if a[0] == "list":
        out([g for g in games_in(a[1]) if all(os.path.isfile(os.path.join(g, f)) for f in a[2:])])
    if a[0] == "arcade":
        out([g for g in games_in(a[1]) if any(s.get("platform") == "arcade" for s in sources(g))])
    game, what, rest = a[0], a[1], a[2:]
    src = sources(game)
    rom = corpus_rom(game)
    if what == "rom":
        out(rom)
    if what == "platform":
        hit = [s for s in src if s.get("sha256") == rom] or src[:1]
        out(hit[0].get("platform") if hit else None)
    if what == "system":
        if rest:
            hit = [s for s in src if s.get("id") == rest[0]]
        else:
            hit = [s for s in src if s.get("sha256") == rom]
            if not hit:
                scored = [e.get("id") for e in entries(game) if e.get("role") == "scored"]
                hit = [s for s in src if s.get("id") in scored]
        out(hit[0].get("system") if hit else None)
    if what == "source":
        out(next((s.get("id") for s in src if s.get("system") == rest[0]), None))
    if what == "sha256":
        out(next((s.get("sha256") for s in src if s.get("id") == rest[0]), None))
    if what == "role":
        out(next((e.get("id") for e in entries(game) if e.get("role") == rest[0]), None))
    if what == "asset-map":
        out(next((e.get("asset_map") for e in entries(game) if e.get("role") == "scored"), None))
    if what == "label":
        labels = (load(os.path.join(game, "labels.toml")) or {}).get("label", [])
        out(next((f"{l['addr']:X}" for l in labels if l.get("name") == rest[0]), None))
    if what == "check":
        v = load(os.path.join(game, "checks.toml")) or {}
        for k in rest[0].split("."):
            v = v.get(k) if isinstance(v, dict) else None
        out(v)
    if what == "captures":
        c = load(os.path.join(game, "oracle", "mame-captures.toml")) or {}
        out([" ".join(str(e[k]) for k in ("name", "system", "input", "input_sha256", "frames", "sha256")) for e in c.get("capture", [])])
    print(f"game-facts.py: no fact {what}", file=sys.stderr)
    sys.exit(2)


main(sys.argv[1:])
