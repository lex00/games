#!/bin/bash
# Tapper's preview with a made-up large-art theme over its board's
# pictures, for the public page check's arcade-theme section (the tools'
# scripts/web/public/check-page.mjs, --arcade-theme OUT; page-check.toml's
# [arcade.theme] names this script):
#   tapper/scripts/page-theme.sh OUT
#
# OUT gets what gp-web-prep writes for Tapper (this folder) from the
# player's own romset, $GP_ROM_DIR/arcade/tapper.zip, lifted into
# $GP_HOME/work/arcade-theme/tapper/assets (never into a repo), and one
# theme in OUT/themes/: `flat`, which draws the bartender (every picture
# code of the entity `player`, sprite-player-NN) as a flat square of one
# made-up colour and names nothing else, so every other sprite and the
# screen stay the board's, and the game's module (`gp game build`). The
# page check serves OUT beside the page's own player files, so the frame is
# drawn by the page's preview.js and gp_board.wasm. OUT holds pictures of
# the romset's game: keep it out of a repo.
#
# The tools are $GP_TOOLS, else the gameprinter checkout beside this repo.
# GP names the gp to run (default the tools' target/debug/gp). Exit codes:
# 0 made, 2 couldn't (no tools, no romset).
set -euo pipefail
trap 'exit 2' ERR
if [ $# -ne 1 ]; then
  echo "usage: tapper/scripts/page-theme.sh OUT" >&2
  exit 2
fi
game="$(cd "$(dirname "$0")/.." && pwd)"
tools="$(cd "${GP_TOOLS:-$game/../../gameprinter}" 2>/dev/null && pwd)" || { echo "page-theme: no tools checkout (set GP_TOOLS)" >&2; exit 2; }
home="${GP_HOME:-$HOME/.gameprinter}"
gp="${GP:-$tools/target/debug/gp}"
target="$(cd "$tools" && mkdir -p "${CARGO_TARGET_DIR:-target}" && cd "${CARGO_TARGET_DIR:-target}" && pwd)"
out="$1"
zip="${GP_ROM_DIR:-$home/roms}/arcade/tapper.zip"
[ -f "$game/model.toml" ] || { echo "page-theme: no model at $game" >&2; exit 2; }
[ -f "$zip" ] || { echo "page-theme: no romset at $zip" >&2; exit 2; }
assets="$home/work/arcade-theme/tapper/assets"
mkdir -p "$assets"
"$gp" assets lift "$zip" --map "$game/assets/tapper.toml" -o "$assets" --force >/dev/null
[ -e "$out" ] && mv "$out" "$out.old-$$"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
(cd "$tools" && cargo build -q --locked -p gp-web --bin gp-web-prep --target-dir "$target")
"$target/debug/gp-web-prep" "$game" "$assets" "$out" >/dev/null
# The game's own module (its hooks): the page plays a game with hooks only
# through it.
"$gp" game kit -o "$target/module-kit" >/dev/null
"$gp" game build "$game" --kit "$target/module-kit" -o "$out/module.wasm" >/dev/null
python3 -I - "$out" <<'PY'
import json, os, struct, sys, zlib
out = sys.argv[1]
def png(path, w, h, rgba):
    raw = b"".join(b"\x00" + bytes(rgba) * w for _ in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
# The bartender at every picture code the board has (a name the lift's
# sprites lack is passed over), drawn where the board draws his sprite, 32
# by 32 of the board's picture's pixels. A made-up colour.
rules = "gp-art 1\n# A made-up theme for the page check: the bartender as a flat square.\n"
rules += "".join(f"pic sprite-player-{c:02x} art/square.png 32 32 0 0\n" for c in range(512))
d = f"{out}/themes/flat"
os.makedirs(f"{d}/art", exist_ok=True)
open(f"{d}/look.txt", "w").write("gp-look 1\nid flat\ntitle Flat\nart art.txt\n")
open(f"{d}/art.txt", "w").write(rules)
png(f"{d}/art/square.png", 32, 32, [200, 40, 160, 255])
ix = f"{out}/themes/index.json"
index = json.load(open(ix)) if os.path.exists(ix) else {"themes": []}
index["themes"] = [t for t in index["themes"] if t.get("id") != "flat"] + [{"id": "flat", "title": "Flat"}]
json.dump(index, open(ix, "w"))
cfg = json.load(open(f"{out}/preview.json"))
cfg["looks"] = True
json.dump(cfg, open(f"{out}/preview.json", "w"))
PY
echo "$out"
