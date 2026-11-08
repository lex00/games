#!/bin/bash
# Self-test for .githooks/pre-commit, run in a throwaway repo so the real
# index is never touched. Every file here is synthetic; no game data.
set -euo pipefail

hook="$(cd "$(dirname "$0")/.." && pwd)/.githooks/pre-commit"
repo="$(mktemp -d)"
data="$(mktemp -d)"
trap 'rm -rf "$repo" "$data"' EXIT
# A made-up ROM (seeded random bytes in an iNES header), registered in this
# repo's roms/, so the ROM-bytes-in-text check has something to find.
export GP_HOME="$data/home" GP_ROM_DIR="$data/roms"
mkdir -p "$GP_ROM_DIR"
python3 -I -B - "$GP_ROM_DIR/made up.nes" "$data/bytes16.txt" "$data/bytes15.txt" "$data/sha256" <<'PY'
import hashlib, random, sys
body = random.Random(7).randbytes(16384 + 8192)
open(sys.argv[1], "wb").write(b"NES\x1a\x01\x01\x01" + bytes(9) + body)
open(sys.argv[2], "w").write("const T: [u8; 16] = [" + ", ".join(f"0x{b:02X}" for b in body[100:116]) + "];\n")
open(sys.argv[3], "w").write("const T: [u8; 15] = [" + ", ".join(f"0x{b:02X}" for b in body[100:115]) + "];\n")
open(sys.argv[4], "w").write(hashlib.sha256(body).hexdigest())
PY
cd "$repo"
mkdir roms
printf '[[source]]\nid = "madeup"\nsha256 = "%s"\n' "$(cat "$data/sha256")" > roms/nes.toml
git init -q
git config user.email test@example.invalid
git config user.name "hook test"
git config core.hooksPath "$(dirname "$hook")"

failures=0
expect() {
  local want="$1" label="$2" file="$3"
  git add -f "$file"
  if git commit -q -m "$label" >/dev/null 2>&1; then got=accept; else got=reject; fi
  git reset -q --hard >/dev/null 2>&1 || true
  git rm -q --cached -f "$file" >/dev/null 2>&1 || true
  rm -f "$file"
  if [ "$got" = "$want" ]; then
    echo "ok    $label ($got)"
  else
    echo "FAIL  $label: wanted $want, got $got"
    failures=$((failures + 1))
  fi
}

printf 'NES\x1a\x02\x01\x00\x00' > disguised.txt
expect reject "iNES header in a .txt file" disguised.txt

printf 'NESM\x1a\x01' > tune.dat
expect reject "NSF header in a .dat file" tune.dat

printf '\x01*NINTENDO-HVC*' > disk.bin.txt
expect reject "FDS header in a .txt file" disk.bin.txt

printf 'hello' > fake.nes
expect reject ".nes extension" fake.nes

printf 'save' > slot.state
expect reject "save state extension" slot.state

printf 'made up' > hack.ips
expect reject "ROM patch extension" hack.ips

printf 'PATCH\x00\x00\x10\x00\x01\x41EOF' > patch.txt
expect reject "IPS patch contents in a .txt file" patch.txt

printf 'BPS1\x80\x80\x80' > notes.dat
expect reject "BPS patch contents in a .dat file" notes.dat

head -c 2000000 /dev/zero > big.dat
expect reject "file over 1MB" big.dat

head -c 40000 /dev/urandom > prg.dat
expect reject "headerless binary blob (a raw PRG dump has no magic bytes)" prg.dat

head -c 1100000 < <(yes "a line of text") > big.txt
expect reject "text file over 1MB" big.txt

mkdir -p .githooks art
printf 'art/*.png\n' > .githooks/binary-allowlist
printf '\x89PNG\r\n\x1a\n\x00\x00' > art/logo.png
expect accept "binary file on the allowlist" art/logo.png
printf '\x89PNG\r\n\x1a\n\x00\x01' > elsewhere.png
expect reject "binary file not on the allowlist" elsewhere.png
rm -rf .githooks art

printf 'plain notes\n' > notes.md
expect accept "ordinary small text file" notes.md

cp "$data/bytes16.txt" table.rs
expect reject "16 bytes of a registered ROM written as hex in source" table.rs
cp "$data/bytes15.txt" table.rs
expect accept "15 bytes of a registered ROM" table.rs
cp "$data/bytes16.txt" table.rs
GP_ROM_DIR="$data/none" expect accept "16 bytes of a ROM, with no ROM folder" table.rs

[ "$failures" -eq 0 ] || { echo "$failures hook checks failed"; exit 1; }
