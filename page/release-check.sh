#!/bin/bash
# The release check for the public page (gameprinter's docs/web.md,
# "Deploying the public page"): everything that must hold before the page
# is uploaded. It lives in the games repo, which has the ROMs' facts; the
# tools keep only what builds and checks a page.
#   page/release-check.sh [--full] [--no-roms] [--out DIR] [--game GAME_DIR]
#
# GP names a built or installed gp (default ../gameprinter/target/debug/gp,
# beside this checkout). The page's files and checks (build-page.sh,
# check-page.mjs, the Node tests, page-digest.py) are read from that gp's
# gameprinter checkout: GP_TOOLS, else the checkout a built gp sits in
# (TOOLS/target/PROFILE/gp), else ../gameprinter. An installed gp with no
# checkout can't build a page: the script says so and stops (exit 2).
#
# The games checkout is this one ($GP_GAMES_DIR when set). A --game GAME_DIR
# that isn't a path is a folder of the checkout. With no --game the page's
# game is the one the checkout's page.toml names (`game = "FOLDER"`). The
# page check reads what it holds each game to from the checkout's game
# folders (check-page.mjs --games; docs/web.md, "The games' check files").
# Before it builds, page/missing-roms.py says in a line each ROM a game's
# page-check.toml routes play that isn't in $GP_ROM_DIR, so a run without
# the owner's ROMs says plainly which sections it skips.
#
# --no-roms is for someone with no ROMs, deploying a copy of the page
# (docs/host-your-own.md): GP_ROM_DIR is an empty folder of its own, so
# every check that needs a ROM is skipped and says so (the page check's ROM
# sections, the lift's ROM tests, Moss Hop's sections), and the two steps
# that can't pass without one are skipped with a line each instead of
# failing: the build's comparison of the page with the game's ROMs (gp
# template served's NOT COMPARED) and gp package scan. A pass is recorded
# as one made without ROMs ("roms none"), and deploy.sh says so before it
# uploads; the upload is still held to the page's digest.
#
# 0. Takes a snapshot of the page's source: a shallow clone of its repo
#    (lex00/gp-registry for the deployed page) at the commit its branch is
#    on now, into DIR.registry, and prints that commit. The page check reads
#    the clone for the whole run (--registry-folder), the page's installs and
#    the check's expectations alike, so a publish during the run changes
#    nothing. A clone that fails ends the run (exit 2): the check never falls
#    back to the live registry. A page whose source is its own address
#    (GP_PAGE_SOURCE=local) has nothing to clone.
# 1. Builds the page from GAME_DIR (above) at this checkout's tip into a
#    folder of its own,
#    $GP_HOME/work/page-release/<commit>-games-<games commit>, each commit
#    with -dirty-<digest of the changes> when that repo has uncommitted
#    changes (a new folder for each, so no file is left from an older
#    build). build-page.sh's last step, `gp template served`, checks that
#    no file of the page or of its play files holds the ROM's bytes,
#    lifted files, pictures made from them, or a game image.
# 2. The page's headless check (check-page.mjs) under the page's _headers,
#    with Moss Hop and three made-up themes in DIR.mosshop for its
#    theme-hud-over and theme-in-region sections (scripts/web/mosshop-hud.sh;
#    with no Moss Hop ROM they are skipped with a note, and a run of the
#    script that fails fails the release check).
#    Moss Hop with a made-up sound role in DIR.mosshop-takes for its
#    theme-sound-takes section (scripts/web/mosshop-takes.sh, the same way).
# 3. The lift under Node (lift.test.mjs) on the page's own lift files, and
#    the page check's theme line rule (theme-line.test.mjs), and the
#    page's address and share link rules (share.test.mjs), and the page's
#    What is this? answers (explain.test.mjs), and the page's key legend
#    on both layouts (keys.test.mjs).
# 4. `gp package scan` on the game's package: it must have no needs-rewrite
#    item, and must have been compared with the ROMs.
# 5. The page's size, its files and their SHA-256, also written to
#    DIR.manifest.txt (beside the page, so it isn't uploaded).
# It runs every step, prints each result and its time, and ends with PASS
# or FAIL.
#
# On PASS it records the page: $GP_HOME/work/page-release/passed/DIGEST,
# where DIGEST is scripts/web/page-digest.py's over the page's folder.
# page/deploy.sh uploads a folder only when its digest has such a record,
# so what goes up is what passed, file for file.
#
# The modules and gp-web-prep's output are reused when none of the files
# they were made from changed (build-preview.sh and build-lift.sh,
# GP_WEB_REUSE=1), and so is a package scan with nothing to rewrite.
# Reused results are printed with "kept". --full reuses nothing: it makes
# and checks everything again, and keeps the results for the next run.
# deploy.sh always runs --full. A page that already passed, file for file,
# is not checked again (GP_RECHECK=1 checks it). The page check runs every section each
# time either way (check-page.mjs says why).
#
# WEB_TOOLS, GP_PAGE_SOURCE and GP_PAGE_CONTENT are as for build-page.sh, and
# GP_PAGE_CONTENT is lex00/turnips when it isn't set, as on the deployed
# page. Exit codes: 0 passed, 1 a check failed, 2 couldn't run.
# The last line on success or failure is "bundle: DIR".

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
checkout="$(cd "$here/.." && pwd)"
home="${GP_HOME:-$HOME/.gameprinter}"
# The gp and the gameprinter checkout it was built from (page/tools.sh).
. "$here/tools.sh" || exit 2
# What the tools' scripts below read as "the repo" is the tools checkout.
repo="$tools"

full=0
no_roms=0
out=""
game=""
while [ $# -gt 0 ]; do
  case "$1" in
    --full) full=1; shift ;;
    --no-roms) no_roms=1; shift ;;
    --out) [ -n "${2:-}" ] || { echo "release-check: --out needs a folder" >&2; exit 2; }; out="$2"; shift 2 ;;
    --game) [ -n "${2:-}" ] || { echo "release-check: --game needs a folder" >&2; exit 2; }; game="$2"; shift 2 ;;
    *) echo "usage: page/release-check.sh [--full] [--no-roms] [--out DIR] [--game GAME_DIR]" >&2; exit 2 ;;
  esac
done
if [ -n "$game" ]; then
  games_dir="${GP_GAMES_DIR:-$checkout}"
  [ -d "$game" ] || game="$games_dir/$game"
else
  games_dir="${GP_GAMES_DIR:-$checkout}"
  [ -n "$games_dir" ] && [ -f "$games_dir/page.toml" ] || { echo "release-check: give --game GAME_DIR, or GP_GAMES_DIR, a games checkout whose page.toml names the page's game" >&2; exit 2; }
  game_folder="$(python3 -I -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))["game"])' "$games_dir/page.toml")" \
    || { echo "release-check: $games_dir/page.toml names no game (game = \"FOLDER\")" >&2; exit 2; }
  game="$games_dir/$game_folder"
fi
[ -f "$game/package.toml" ] || { echo "release-check: $game is no game package (no package.toml)" >&2; exit 2; }
game="$(cd "$game" && pwd)"
games_dir="$(cd "$games_dir" && pwd)" || exit 2
# A repo's commit, and the digest of its uncommitted changes (tracked and
# new, by content) when it has any.
version() {
  local id changes
  id="$(git -C "$1" rev-parse --short=8 HEAD)" || return 1
  if [ -n "$(git -C "$1" status --porcelain)" ]; then
    changes="$(cd "$1" && { git diff HEAD --binary; git ls-files -o --exclude-standard -z | xargs -0 shasum -a 256; } | shasum -a 256 | cut -c1-8)"
    id="$id-dirty-$changes"
  fi
  echo "$id"
}
games_repo="$(git -C "$game" rev-parse --show-toplevel)" || { echo "release-check: $game isn't in a git checkout" >&2; exit 2; }
id="$(version "$repo")-games-$(version "$games_repo")" || exit 2
if [ $no_roms = 1 ]; then
  id="$id-no-roms"
  export GP_ROM_DIR="$home/work/page-release/no-roms"
  mkdir -p "$GP_ROM_DIR" || exit 2
  if [ -n "$(ls -A "$GP_ROM_DIR")" ]; then
    echo "release-check: --no-roms reads $GP_ROM_DIR as an empty ROM folder, but it holds files; move them out first" >&2
    exit 2
  fi
  echo "note: --no-roms: GP_ROM_DIR is $GP_ROM_DIR, empty, so every check that needs a ROM is skipped and says so"
fi
[ -n "$out" ] || out="$home/work/page-release/$id"
if [ $full = 1 ]; then export GP_WEB_REUSE=save; else export GP_WEB_REUSE=1; fi
# The theme the page installs with the game: the deployed page's, Turnips,
# unless GP_PAGE_CONTENT says otherwise (set and empty for none).
export GP_PAGE_CONTENT="${GP_PAGE_CONTENT-lex00/turnips}"
reuse="$repo/scripts/web/reuse.py"

fail=()
now() { perl -MTime::HiRes=time -e 'printf "%.1f", time'; }
start=$(now)
t0=$start
took() { perl -e "printf '%.1f', $(now) - $1"; }
step() {
  [ -n "${current:-}" ] && echo "time  $current: $(took "$t0") s"
  t0=$(now)
  current="$1"
  echo; echo "== $1"
}

step "the ROMs the games' routes play"
# Informational: each route's ROM that isn't here is named, and the page
# check skips its sections with a line of its own. --no-roms has none.
python3 -I "$here/missing-roms.py" "$games_dir" || exit 2

step "a snapshot of the page's source"
source_name="${GP_PAGE_SOURCE:-registry}"
if [ -n "${GP_PAGE_SOURCE_URL:-}" ]; then
  source_url="${GP_PAGE_SOURCE_URL%/}"
  source_name="$source_url"
else
  source_url="$(python3 -I -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))[sys.argv[2]]["url"])' "$repo/scripts/web/sources.toml" "$source_name" 2>/dev/null)" \
    || { echo "release-check: scripts/web/sources.toml has no source named $source_name" >&2; exit 2; }
fi
registry_folder=()
case "$source_url" in
  .) echo "ok: the page's source is its own address ($source_name), so there is no registry to clone" ;;
  https://raw.githubusercontent.com/*/*/*)
    rest="${source_url#https://raw.githubusercontent.com/}"
    source_repo="$(echo "$rest" | cut -d/ -f1-2)"
    source_branch="${rest#*/*/}"
    mkdir -p "$(dirname "$out")" || exit 2
    snapshot="$(cd "$(dirname "$out")" && pwd)/$(basename "$out").registry"
    # An earlier run's snapshot for this build is cleared first, as
    # build-page.sh clears the folders it makes.
    rm -rf "$snapshot"
    if ! GIT_TERMINAL_PROMPT=0 git clone --quiet --depth 1 --single-branch --branch "$source_branch" "https://github.com/$source_repo.git" "$snapshot"; then
      echo "FAIL: couldn't clone $source_repo to take a snapshot of the page's source (is the network down?), so the page can't be checked. The check doesn't read the live registry instead."
      echo "bundle: $out"
      exit 2
    fi
    echo "ok: $source_repo is at commit $(git -C "$snapshot" rev-parse HEAD) on $source_branch; the page and the check read this copy for the whole run ($snapshot)"
    # A copy of it with a made-up second game (test/made-up) and its theme,
    # for the sections about a game named in the address (game-*,
    # share-link), and two made-up themes for the page's own game
    # (acme/harbor-lights, test/paper-moon), so the theme sections never
    # hang on what people publish. The page's own game reads the same files
    # in both. deploy.sh's check of the uploaded page reads this copy too.
    if any="$(GP="$gp" "$repo/scripts/web/test-registry.sh" "$snapshot" "$snapshot.any" "$game" "$games_dir" | tail -1)" && [ -d "$any/index/test/made-up" ]; then
      echo "ok: the check reads a copy with a made-up second game added ($any)"
      snapshot="$any"
    else
      echo "FAIL: scripts/web/test-registry.sh couldn't add the made-up second game, so the page can't be checked with a game named in its address"
      echo "bundle: $out"
      exit 2
    fi
    registry_folder=(--registry-folder "$snapshot") ;;
  *)
    echo "FAIL: the page's source $source_url isn't a GitHub repo's raw address, so the check can't take a snapshot of it. The check doesn't read the live registry instead."
    echo "bundle: $out"
    exit 2 ;;
esac

step "build the page and check it for game data ($out)"
mkdir -p "$out" || exit 2
rm -f "$out.served.txt"
GP="$gp" "$repo/scripts/web/build-page.sh" "$game" "$out"
s=$?
# Without ROMs, gp template served can't compare (its exit 2, with a NOT
# COMPARED line and its verdict "not checked against every ROM", which it
# gives only when nothing was found): said, and not a failure.
if [ $s = 2 ] && [ $no_roms = 1 ] && [ -f "$out/index.html" ] && grep -q '^NOT COMPARED' "$out.served.txt" 2>/dev/null && grep -q ': not checked against every ROM$' "$out.served.txt"; then
  echo "skip: built; with no ROMs (--no-roms) the page and its play files weren't compared with the game's ROMs (gp template served: NOT COMPARED)"
  s=0
fi
case $s in
  0) [ $no_roms = 1 ] || echo "ok: built; no file holds a ROM's bytes, lifted files or pictures made from them" ;;
  1) fail+=("the page holds game data (gp template served)") ;;
  *) fail+=("the page could not be built or checked (build-page.sh exit $s; gp template served needs the ROMs in GP_ROM_DIR)") ;;
esac
if [ ! -f "$out/index.html" ]; then
  echo "FAIL: no page"; echo "bundle: $out"; exit 2
fi
out="$(cd "$out" && pwd)"
built="$(python3 -I "$repo/scripts/web/page-digest.py" "$out")" || exit 2

# A page whose every file is as it was when the release check last passed
# on it (the same digest, and with ROMs if this run has them) is not
# checked again: it is that page. GP_RECHECK=1 checks it anyway.
prior="$home/work/page-release/passed/$built"
if [ ${#fail[@]} -eq 0 ] && [ "${GP_RECHECK:-}" != 1 ] && [ -f "$prior" ] \
  && { [ $no_roms = 1 ] || grep -qx 'roms some' "$prior"; }; then
  echo
  echo "kept  this page passed already, file for file ($(sed -n 's/^when //p' "$prior")): not checked again (GP_RECHECK=1 checks it)"
  echo "PASS: the page can be deployed (digest $built, recorded in $home/work/page-release/passed)"
  echo "bundle: $out"
  exit 0
fi

step "Moss Hop with two made-up themes, for the page check's theme-hud-over section"
# The page's own player draws them (check-page.mjs serves DIR.mosshop beside
# its files). Moss Hop's art is its own ROM's (`just mosshop`); without it
# (mosshop-hud.sh's exit 3) the sections are skipped and say so. A script
# that runs and fails fails the release check: its sections would be
# skipped and the check pass with them unrun.
GP="$gp" "$repo/scripts/web/mosshop-hud.sh" "$out.mosshop" >/dev/null
s=$?
case $s in
  0) echo "ok: $out.mosshop" ;;
  3) echo "note: no Moss Hop ROM to lift its art from (just mosshop builds it), so the page check skips theme-hud-over and theme-in-region" ;;
  *) fail+=("scripts/web/mosshop-hud.sh failed (exit $s), so theme-hud-over and theme-in-region couldn't run")
     echo "FAIL: scripts/web/mosshop-hud.sh exited $s making $out.mosshop" ;;
esac
step "Moss Hop with a made-up sound role and a theme of three takes, for the page check's theme-sound-takes section"
GP="$gp" "$repo/scripts/web/mosshop-takes.sh" "$out.mosshop-takes" >/dev/null
s=$?
case $s in
  0) echo "ok: $out.mosshop-takes" ;;
  3) echo "note: no Moss Hop ROM to lift its art from (just mosshop builds it), so the page check skips theme-sound-takes" ;;
  *) fail+=("scripts/web/mosshop-takes.sh failed (exit $s), so theme-sound-takes couldn't run")
     echo "FAIL: scripts/web/mosshop-takes.sh exited $s making $out.mosshop-takes" ;;
esac

step "the page in a headless browser, under the page's _headers"
browser="$("$repo/scripts/inspector/fetch-browser.sh" "$home")" || { echo "no headless browser"; exit 2; }
if [ -z "${PLAYWRIGHT_FROM:-}" ] && [ ! -d "$repo/scripts/web/node_modules/playwright-core" ]; then
  npm ci --prefix "$repo/scripts/web" --silent --no-audit --no-fund || exit 2
fi
page_full=""
[ $full = 1 ] && page_full="--full"
node "$repo/scripts/web/public/check-page.mjs" --dist "$out" --browser "$browser" --games "$games_dir" --shots "$out.shots" $page_full ${registry_folder[@]+"${registry_folder[@]}"}
s=$?
[ $s -eq 0 ] || fail+=("the page's headless check (exit $s; shots in $out.shots)")

step "the lift under Node, on the page's lift files, and the page check's theme line rule"
# build-page.sh splits the lift: the module stays with the page, the
# folder it reads goes to the play files. The test reads them together.
lift="$out.lift-test"
mkdir -p "$lift/lift" "$lift/work" || exit 2
cp -R "$out/game/lift/gp-web-lift.wasm" "$out.play/lift/"* "$lift/lift/" || exit 2
GP_WEB_LIFT_OUT="$lift" GP_WORKER_OUT="$lift/work" node --test "$repo/scripts/web/public/lift.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the lift's tests (lift.test.mjs, exit $s)")
node --test "$repo/scripts/web/public/theme-line.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the page check's theme line rule (theme-line.test.mjs, exit $s)")
node --test "$repo/scripts/web/public/share.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the page's address and share link rules (share.test.mjs, exit $s)")
node --test "$repo/scripts/web/public/explain.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the page's What is this? answers (explain.test.mjs, exit $s)")
node --test "$repo/scripts/web/public/keys.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the page's key legend (keys.test.mjs, exit $s)")
# The page's controls (gp_controls.wasm, as built for this page) through
# fixed cases of the rules the owner tuned on the screen page, which reads
# its controls through the same module (docs/controls.md).
GP_CONTROLS_WASM="$out/game/gp_controls.wasm" node --test "$repo/scripts/web/page/controls.test.mjs"
s=$?
[ $s -eq 0 ] || fail+=("the page's controls (controls.test.mjs, exit $s)")

step "gp package scan on the game's package"
# A scan reads the package's folder, the ROMs in $GP_ROM_DIR and gp. Only
# a complete scan with nothing to rewrite is kept.
gp_bin="$(cd "$(dirname "$gp")" && pwd)/$(basename "$gp")"
name="$(basename "$game")"
scan_in=(--file "$gp_bin" --listing "${GP_ROM_DIR:-$home/roms}" --tree "$game")
if [ $no_roms = 1 ]; then
  echo "skip  $name: gp package scan compares the package with its ROMs, and --no-roms has none"
elif [ $full = 0 ] && from="$(python3 -I "$reuse" find "package-scan-$name" --extra "$name")"; then
  json="$(cat "$from"/out/*.json)"
  echo "kept  $name: the scan of $(sed -n 's/.*"made": "\([^"]*\)".*/\1/p' "$from/inputs.json"), none of the files it read changed"
else
  json="$("$gp" --json package scan "$game" 2>/dev/null)" || true
  printf '%s\n' "$json" > "$out.scan-$name.json"
  if python3 -I -c 'import json, sys; d = json.load(open(sys.argv[1])); sys.exit(0 if d["rewrite"] == 0 and d["complete"] else 1)' "$out.scan-$name.json" 2>/dev/null; then
    python3 -I "$reuse" save "package-scan-$name" --extra "$name" "${scan_in[@]}" --out "$out.scan-$name.json" >/dev/null
  fi
fi
[ $no_roms = 1 ] || python3 -I - "$name" "$json" <<'PY' || fail+=("package scan: $name has needs-rewrite items or wasn't compared with its ROMs (above)")
import json, sys
t, raw = sys.argv[1:]
try:
    d = json.loads(raw)
except Exception:
    print(f"{t}: no scan result"); sys.exit(1)
print(f"{t}: needs rewrite {d['rewrite']} items; compared with {', '.join(d['compared']) or 'no ROM'}")
for part in d["parts"]:
    for f in part.get("files", []):
        if f.get("rewrite"):
            print(f"  {f['file']}: {f['rewrite']} {f.get('unit', '')}")
if not d["complete"]:
    print(f"  not compared: {', '.join(d['not_compared'])} (not in GP_ROM_DIR)")
sys.exit(0 if d["rewrite"] == 0 and d["complete"] else 1)
PY

step "page size and files"
python3 -I - "$out" "$out.manifest.txt" <<'PY' || fail+=("page over a Workers static assets limit")
import hashlib, os, sys
root, manifest = sys.argv[1:]
rows, total = [], 0
for d, _, names in os.walk(root):
    for n in names:
        p = os.path.join(d, n)
        b = open(p, "rb").read()
        rows.append((os.path.relpath(p, root), len(b), hashlib.sha256(b).hexdigest()))
        total += len(b)
rows.sort()
with open(manifest, "w") as f:
    for r, n, h in rows:
        f.write(f"{h}  {n:>9}  {r}\n")
for r, n, h in rows:
    print(f"{h[:16]}  {n:>9}  {r}")
big = max(rows, key=lambda r: r[1])
print(f"{len(rows)} files, {total/1048576:.2f} MiB; largest {big[0]} ({big[1]/1048576:.2f} MiB); manifest in {manifest}")
# Workers static assets: 20000 files, 25 MiB each.
ok = len(rows) <= 20000 and big[1] <= 25 * 1048576
print("ok: within the Workers static assets limits (20000 files, 25 MiB a file)" if ok else "FAIL: over a limit")
sys.exit(0 if ok else 1)
PY
# Nothing may have changed the page while it was checked.
checked="$(python3 -I "$repo/scripts/web/page-digest.py" "$out")" || exit 2
[ "$checked" = "$built" ] || fail+=("the page's files changed during the check (digest $built after the build, $checked now)")

echo "time  $current: $(took "$t0") s"
echo "time  the release check: $(took "$start") s"
echo
if [ ${#fail[@]} -eq 0 ]; then
  passed="$home/work/page-release/passed"
  mkdir -p "$passed" || exit 2
  roms=some
  [ $no_roms = 1 ] && roms=none
  printf 'bundle %s\nid %s\ngame %s\ngames %s\nfull %s\nroms %s\nwhen %s\n' "$out" "$id" "$game" "$games_dir" "$full" "$roms" "$(date '+%Y-%m-%dT%H:%M:%S')" > "$passed/$checked"
  if [ $no_roms = 1 ]; then
    echo "PASS without ROMs: every check that needs a ROM was skipped (above); the page can be deployed (digest $checked, recorded in $passed as made without ROMs)"
  else
    echo "PASS: the page can be deployed (digest $checked, recorded in $passed)"
  fi
  echo "bundle: $out"
  exit 0
fi
echo "FAIL: ${#fail[@]} problem(s)"
for f in "${fail[@]}"; do echo "  - $f"; done
echo "bundle: $out"
exit 1
