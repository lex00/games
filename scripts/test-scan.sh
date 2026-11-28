#!/bin/bash
# The scan on made-up files in a throwaway repo: a clean one passes, and each
# kind of file it must refuse fails it. No game data.
set -euo pipefail
scan="$(cd "$(dirname "$0")" && pwd)/scan.py"
repo="$(mktemp -d)"
trap 'rm -rf "$repo"' EXIT
git -C "$repo" init -q
expect() {
  local want="$1" label="$2"
  git -C "$repo" add -A -f
  if python3 -I -B "$scan" "$repo" >/dev/null; then got=pass; else got=refuse; fi
  git -C "$repo" rm -rqf --cached . ; find "$repo" -mindepth 1 -maxdepth 1 -not -name .git -exec rm -rf {} +
  if [ "$got" = "$want" ]; then echo "ok    $label ($got)"; else echo "WRONG $label ($got, wanted $want)"; exit 1; fi
}
echo 'model = 1' > "$repo/model.toml";                 expect pass "a text file"
printf 'NES\032\002\001' > "$repo/x.dat";              expect refuse "an iNES header in a .dat file"
echo text > "$repo/game.nes";                          expect refuse "a .nes name"
printf 'a\0b' > "$repo/raw.dat";                       expect refuse "a headerless binary"
echo text > "$repo/module.wasm";                       expect refuse "a built module"
head -c 1048577 /dev/zero | tr '\0' 'a' > "$repo/big.toml"; expect refuse "a file over 1 MB"
mkdir -p "$repo/hilltop/themes/dusk/art"; printf '\211PNG\r\n\032\n\0\0' > "$repo/hilltop/themes/dusk/art/sky.png"; expect pass "a theme's picture"
mkdir -p "$repo/hilltop/themes/dusk"; printf 'RIFF\0\0\0\0WAVEfmt ' > "$repo/hilltop/themes/dusk/hop.wav"; expect pass "a theme's sound"
mkdir -p "$repo/hilltop/themes/dusk"; printf 'NES\032\002\001' > "$repo/hilltop/themes/dusk/sky.png"; expect refuse "a game image named as a theme's picture"
mkdir -p "$repo/hilltop/art"; printf '\211PNG\r\n\032\n\0\0' > "$repo/hilltop/art/sky.png"; expect refuse "a picture outside a theme"
echo "scan self-test: passed"
