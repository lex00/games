# The user's own games. `just check` is what every commit must pass.
#
# The checks of a game against its ROM live here, not in gameprinter: the
# tool knows no game. Each game's recipe runs from the game's folder (cd
# tapper && just check-game) and takes the game from that folder; `just
# check-all` at the top runs it in every game's folder. Each calls an
# installed gp (gp on PATH), or the one GP names (GP=path/to/gp). A ROM or
# romset that isn't on this machine is one line saying which and where,
# and that check is skipped. The few that run a gameprinter script or
# example (the arcade player, web parity, the MAME match, the mechanic
# page's arcade run) need a gameprinter checkout: GAMEPRINTER, else
# ../gameprinter beside this repo.

gp := env_var_or_default("GP", "gp")
gp_home := env_var_or_default("GP_HOME", env_var("HOME") / ".gameprinter")
roms := env_var_or_default("GP_ROM_DIR", gp_home / "roms")
tool := env_var_or_default("GAMEPRINTER", justfile_directory() / ".." / "gameprinter")
# The game: the folder `just` was run from.
game := invocation_directory()
facts := "python3 -I -B " + quote(justfile_directory() / "scripts" / "game-facts.py")

setup:
    git config core.hooksPath .githooks

# The gp the recipes run, or a line saying how to get one.
_gp:
    @command -v {{quote(gp)}} > /dev/null || [ -x {{quote(gp)}} ] || { echo "no gp: install it (brew install lex00/tap/gp), put it on PATH, or set GP=path/to/gp" >&2; exit 1; }

# Fails unless `just` was run from a game's folder (one with package.toml).
_in-game:
    @[ -f {{quote(game / "package.toml")}} ] || { echo "run this from a game's folder (one with package.toml), not {{game}}" >&2; exit 1; }

# Every package folder through gp package scan. With no ROM here it says
# nothing was compared; with ROMs it compares them.
_scan-packages:
    #!/usr/bin/env bash
    set -euo pipefail
    git ls-files -- 'package.toml' '*/package.toml' | while IFS= read -r p; do
        {{quote(gp)}} package scan "$(dirname "$p")"
    done

# Every game's model: gp model check and gp model fmt --check (no ROM).
_models:
    #!/usr/bin/env bash
    set -euo pipefail
    git ls-files -- '*/model.toml' | while IFS= read -r m; do
        d="$(dirname "$m")"
        {{quote(gp)}} model check "$d" > /dev/null
        {{quote(gp)}} model fmt --check "$d"
        echo "models: $d checked and in canonical form"
    done

check: _gp check-hosted
    just _models
    just _scan-packages
    @echo "just check: passed"

# What needs no gp: the job on GitHub's machines, which can't build the private tool.
check-hosted:
    @[ "$(git config core.hooksPath)" = ".githooks" ] || { echo "The ROM-blocking hook isn't enabled. Run: just setup" >&2; exit 1; }
    python3 -I -B scripts/scan.py
    scripts/test-scan.sh
    scripts/test-hook.sh
    python3 -I -B -m unittest discover -s scripts/romguard -q

# check-game in every game's folder, then `just check`. A check that can't
# run (no ROM, no capture, no gameprinter checkout) is one line; it fails
# only when a check that ran failed.
check-all: check
    #!/usr/bin/env bash
    set -uo pipefail
    failed=()
    for p in $(git ls-files -- '*/package.toml'); do
        d="$(dirname "$p")"
        (cd "$d" && just check-game) || failed+=("$d")
    done
    [ "${#failed[@]}" -eq 0 ] || { echo "check-all: failed on ${failed[*]}" >&2; exit 1; }
    echo "check-all: passed"

# Every check of this folder's game that this machine can run: its model,
# its corpus scored, its MAME captures, its arcade player and its web
# parity, each skipped in words when what it needs isn't here.
check-game: _gp _in-game
    #!/usr/bin/env bash
    set -uo pipefail
    name="$(basename {{quote(game)}})"
    failed=()
    # A recipe runs in the justfile's folder: each is started from the game's.
    for r in model score mame board play-web-arcade web-parity-arcade; do
        (cd {{quote(game)}} && just "$r") || failed+=("$r")
    done
    [ "${#failed[@]}" -eq 0 ] || { echo "check-game: $name failed ${failed[*]}" >&2; exit 1; }
    echo "check-game: $name passed"

# An arcade game's checks against its own ROM (gp oracle board, gameprinter
# docs/oracle-mame.md): the board against each [[replay]] capture of
# oracle/mame-captures.toml with snapshots and the watchdog, the sound board
# alone against the whole machine, the disassembler against a trace, and the
# static harvest against harvest.toml. gp skips in words a check whose
# romset, capture or file isn't on this machine; a game with no
# oracle/mame-captures.toml is skipped here.
board: _gp _in-game
    #!/usr/bin/env bash
    set -uo pipefail
    g={{quote(game)}} name="$(basename {{quote(game)}})"
    if [ ! -f "$g/oracle/mame-captures.toml" ]; then echo "board: skipped, $name has no oracle/mame-captures.toml"; exit 0; fi
    out=$(cd "$g" && GP_GAMES_DIR={{quote(justfile_directory())}} {{quote(gp)}} oracle board 2>&1); rc=$?
    grep -v '^hint' <<<"$out"
    case "$rc" in
        0) echo "board: $name passed" ;;
        2) echo "board: skipped, nothing $name's checks need is on this machine" ;;
        *) echo "board: $name FAILED" >&2; exit 1 ;;
    esac

# This game's model checked and in canonical form (no ROM).
model: _gp _in-game
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f {{quote(game / "model.toml")}} ]; then
        echo "model: skipped, $(basename {{quote(game)}}) has no model.toml"
        exit 0
    fi
    {{quote(gp)}} model check {{quote(game)}} > /dev/null
    {{quote(gp)}} model fmt --check {{quote(game)}}
    echo "model: $(basename {{quote(game)}}) checked and in canonical form"

# gp corpus score on this game, passing only when every clip is exact,
# none failed to score and it exits 0. Needs the ROM its corpus is for
# (an arcade game's romset, SYSTEM.zip under the ROM folder's arcade/).
score: _gp _in-game
    #!/usr/bin/env bash
    set -uo pipefail
    g={{quote(game)}} name="$(basename {{quote(game)}})" roms={{quote(roms)}}
    if [ ! -f "$g/corpus.toml" ]; then echo "score: skipped, $name has no corpus.toml"; exit 0; fi
    rom=$({{facts}} "$g" rom || true)
    if [ "$({{facts}} "$g" platform || true)" = arcade ]; then
        zip="$roms/arcade/$({{facts}} "$g" system || true).zip"
        [ -f "$zip" ] || { echo "score: skipped, no romset at $zip, the one $name's corpus is for (sha256 $rom)"; exit 0; }
    else
        scan=$(GP_ROM_DIR="$roms" {{quote(gp)}} --json roms scan 2>/dev/null || true)
        if [ -z "$rom" ] || ! grep -q "\"sha256\": \"$rom\"" <<<"$scan"; then
            echo "score: skipped, no ROM with sha256 $rom (the one $name's corpus is for) in $roms"
            exit 0
        fi
    fi
    out=$(GP_ROM_DIR="$roms" {{quote(gp)}} corpus score --run "check-$name" "$g" 2>&1)
    status=$?
    line=$(grep -Eo '[0-9]+ of [0-9]+ clips exact, fidelity [0-9.]+%, [0-9]+ couldn.t be scored' <<<"$out" | head -1)
    if [ -z "$line" ]; then echo "$out"; echo "score: no score line in gp corpus score's output (exit $status)" >&2; exit 1; fi
    read -r exact _ clips _ <<<"$line"
    if [ "$exact" != "$clips" ] || ! grep -q ', 0 couldn.t be scored' <<<"$line"; then echo "$out"; echo "score: not every clip exact: $line" >&2; exit 1; fi
    # Exit 1 with every clip exact: the corpus has problems its check listed.
    if [ "$status" -ne 0 ]; then echo "$out"; echo "score: gp corpus score exited $status with every clip exact: $line" >&2; exit 1; fi
    echo "score: $name: $line"

# Each MAME capture this game's oracle/mame-captures.toml names, played on
# gp-arcade: every frame, AY writes too. Needs the romset (SYSTEM.zip), the
# capture under $GP_HOME/work/oracle/NAME (`just mame-record` in
# gameprinter makes one) and a gameprinter checkout for gp-arcade's
# compare example, built in release.
mame: _in-game
    #!/usr/bin/env bash
    set -uo pipefail
    g={{quote(game)}} name="$(basename {{quote(game)}})" roms={{quote(roms)}} tool={{quote(tool)}}
    if [ ! -f "$g/oracle/mame-captures.toml" ]; then echo "mame: skipped, $name has no oracle/mame-captures.toml"; exit 0; fi
    [ -f "$tool/Cargo.toml" ] || { echo "mame: skipped, no gameprinter checkout at $tool (set GAMEPRINTER)"; exit 0; }
    failed=0
    while read -r cap system input input_sha _frames _sha; do
        [ -n "$cap" ] || continue
        zip="$roms/arcade/$system.zip" dir={{quote(gp_home / "work" / "oracle")}}/"$cap" log="$g/$input"
        if [ ! -f "$zip" ]; then echo "mame: skipped $cap, no romset at $zip"; continue; fi
        if [ ! -d "$dir" ]; then echo "mame: skipped $cap, no MAME capture at $dir (in gameprinter: just mame-record $zip $log $cap 0)"; continue; fi
        if [ "$(shasum -a 256 < "$log" 2>/dev/null | cut -d' ' -f1)" != "$input_sha" ]; then
            echo "mame: $cap FAILED: $log isn't the input oracle/mame-captures.toml names (sha256 $input_sha)" >&2
            failed=1; continue
        fi
        out=$(GP_GAMES_DIR={{quote(justfile_directory())}} cargo run -q --release --manifest-path "$tool/Cargo.toml" -p gp-arcade --example compare -- \
            --rom "$zip" --capture "$dir" --log "$log" --all 2>&1)
        if [ $? -eq 0 ] && grep -Eq 'all [0-9]+ frames match MAME' <<<"$out"; then
            echo "mame: $cap: $(grep -Eo 'all [0-9]+ frames match MAME' <<<"$out" | head -1)"
        else
            echo "$out" | tail -20; echo "mame: $cap FAILED" >&2; failed=1
        fi
    done < <({{facts}} "$g" captures || true)
    exit "$failed"

# The arcade player on this game: a MAME capture's input that its
# oracle/mame-captures.toml names (the first for the set) played in
# gp_arcade_play.wasm under node, drawing every frame, with a save point
# restored on the way, then the same log replayed natively; every frame's
# picture hash must agree, and with that MAME capture on this machine
# ($GP_HOME/work/oracle/NAME), MAME's too. So must every frame's sound and
# the sound board alone's, fed the latch writes the whole machine made.
# SOURCE is one of roms.toml's source ids (default: the first capture's
# set). Needs the romset ($GP_ARCADE_ZIP, else the ROM folder's
# arcade/SYSTEM.zip) and a gameprinter checkout, whose just
# play-arcade-wasm builds the player and the native examples.
play-web-arcade SOURCE="": _in-game
    #!/usr/bin/env bash
    set -euo pipefail
    g={{quote(game)}} name="$(basename {{quote(game)}})" tool={{quote(tool)}} source={{quote(SOURCE)}}
    if [ ! -f "$g/oracle/mame-captures.toml" ]; then echo "play-web-arcade: skipped, $name has no oracle/mame-captures.toml"; exit 0; fi
    [ -f "$tool/justfile" ] || { echo "play-web-arcade: skipped, no gameprinter checkout at $tool (set GAMEPRINTER)"; exit 0; }
    if [ -z "$source" ]; then
        system=$({{facts}} "$g" captures | head -1 | cut -d' ' -f2)
        source=$({{facts}} "$g" source "$system" || true)
    fi
    system=$({{facts}} "$g" system "$source" || true)
    [ -n "$system" ] || { echo "play-web-arcade: $name has no source ${source:-with a capture} with a system in its roms.toml" >&2; exit 1; }
    cap="" input="" sha=""
    while read -r c s i h _; do
        if [ "$s" = "$system" ]; then cap=$c input="$g/$i" sha=$h; break; fi
    done < <({{facts}} "$g" captures || true)
    [ -n "$input" ] || { echo "play-web-arcade: $name's oracle/mame-captures.toml names no capture of $system" >&2; exit 1; }
    [ "$(shasum -a 256 < "$input" | cut -d' ' -f1)" = "$sha" ] || { echo "play-web-arcade: $input isn't the input $name's captures file names for $cap" >&2; exit 1; }
    zip=""
    for z in "${GP_ARCADE_ZIP:-}" {{quote(roms)}}"/arcade/$system.zip"; do
        if [ -n "$z" ] && [ -f "$z" ]; then zip="$z"; break; fi
    done
    [ -n "$zip" ] || { echo "play-web-arcade: skipped, no $system romset on this machine (GP_ARCADE_ZIP, or {{roms}}/arcade/$system.zip)"; exit 0; }
    target="${CARGO_TARGET_DIR:-$tool/target}"
    (cd "$tool" && just play-arcade-wasm "$target/play-web") > /dev/null
    examples="$target/debug/examples"
    [ -z "${GP_PROFILE:-}" ] || [ "$GP_PROFILE" = dev ] || examples="$target/$GP_PROFILE/examples"
    work={{quote(gp_home / "work" / "play-web")}}
    mkdir -p "$work"
    # The board knows a set only by its game's roms.toml chip rows: the
    # native examples read this checkout, the wasm module is handed them.
    export GP_GAMES_DIR={{quote(justfile_directory())}} ROMS="$g/roms.toml"
    "$examples/latches" "$zip" "$input" "$work/arcade-latches.bin"
    node "$tool/scripts/web/check-play-arcade.mjs" "$target/play-web/gp_arcade_play.wasm" "$zip" "$input" "$work/arcade-web.txt" "$work/arcade-web-sound.txt" "$work/arcade-latches.bin" > /dev/null
    "$examples/pictures" "$zip" "$input" > "$work/arcade-native.txt"
    "$examples/sounds" "$zip" "$input" > "$work/arcade-native-sound.txt"
    frames=$(wc -l < "$work/arcade-web.txt" | tr -d ' ')
    if ! cmp -s "$work/arcade-web.txt" "$work/arcade-native.txt"; then
        first=$(diff <(cat -n "$work/arcade-web.txt") <(cat -n "$work/arcade-native.txt") | sed -n 2p)
        echo "play-web-arcade: gp_arcade_play.wasm's picture hashes differ from the native replay's on $name, first at: $first" >&2
        exit 1
    fi
    if ! cmp -s "$work/arcade-web-sound.txt" "$work/arcade-native-sound.txt"; then
        first=$(diff <(cat -n "$work/arcade-web-sound.txt") <(cat -n "$work/arcade-native-sound.txt") | sed -n 2p)
        echo "play-web-arcade: gp_arcade_play.wasm's sound differs from the native replay's on $name, first at frame: $first" >&2
        exit 1
    fi
    samples=$(awk '{ n += $1 } END { print n / 2 }' "$work/arcade-web-sound.txt")
    mame="no MAME capture of it here"
    tsv={{quote(gp_home / "work" / "oracle")}}/"$cap/frames.tsv"
    if [ -f "$tsv" ]; then
        col=$(head -1 "$tsv" | tr '\t' '\n' | grep -nx picture | cut -d: -f1)
        if cmp -s "$work/arcade-web.txt" <(tail -n +2 "$tsv" | cut -f"$col"); then
            mame="and MAME's capture $cap agrees on every frame too"
        else
            echo "play-web-arcade: the picture hashes on $name differ from MAME's capture ($tsv)" >&2
            exit 1
        fi
    fi
    echo "play-web-arcade: $name's $(basename "$input") on $source ($frames frames) played in gp_arcade_play.wasm and replayed natively: every frame's picture hash agrees, $mame; so does every frame's sound ($samples samples a speaker), and the sound board alone's"

# This arcade game's model in the page's player against the native run
# (gameprinter's docs/web.md, "Parity"): from the original's state at each
# clip's start, its module under Node through the page's exports and gp
# interp run through the same module must give the same state hash on
# every frame. CLIPS are corpus clip IDs (default: checks.toml's
# [web_parity] clips). Needs model.toml, the romset its corpus is for and
# a gameprinter checkout (scripts/web/build-preview.sh and parity-arcade.sh).
web-parity-arcade *CLIPS: _gp _in-game
    #!/usr/bin/env bash
    set -euo pipefail
    g={{quote(game)}} name="$(basename {{quote(game)}})" tool={{quote(tool)}}
    if [ ! -f "$g/model.toml" ]; then echo "web-parity-arcade: skipped, $name has no model.toml"; exit 0; fi
    if [ "$({{facts}} "$g" platform || true)" != arcade ]; then echo "web-parity-arcade: skipped, $name isn't an arcade game"; exit 0; fi
    clips="{{CLIPS}}"
    [ -n "$clips" ] || clips=$({{facts}} "$g" check web_parity.clips | tr '\n' ' ' || true)
    [ -n "$clips" ] || { echo "web-parity-arcade: skipped, no clips given and none in $name's checks.toml [web_parity]"; exit 0; }
    zip={{quote(roms)}}"/arcade/$({{facts}} "$g" system || true).zip"
    [ -f "$zip" ] || { echo "web-parity-arcade: skipped, no romset at $zip, the one $name's corpus is for"; exit 0; }
    [ -f "$tool/scripts/web/parity-arcade.sh" ] || { echo "web-parity-arcade: skipped, no gameprinter checkout at $tool (set GAMEPRINTER)"; exit 0; }
    map=$({{facts}} "$g" asset-map) || { echo "web-parity-arcade: $name's package.toml names no asset_map for its scored set" >&2; exit 1; }
    work={{quote(gp_home / "work" / "web-parity-arcade")}}/"$name"
    # Lifted outside the work tree, as gp assets lift requires.
    mkdir -p "$work/assets"
    {{quote(gp)}} assets lift "$zip" --map "$g/$map" -o "$work/assets" --force > /dev/null
    {{quote(gp)}} game kit -o "$work/kit" > /dev/null
    {{quote(gp)}} game build "$g" --kit "$work/kit" -o "$work/$name.wasm" > /dev/null
    "$tool/scripts/web/build-preview.sh" "$work/web" "$g" "$work/assets" > /dev/null
    GP={{quote(gp)}} "$tool/scripts/web/parity-arcade.sh" "$work/web" "$g" "$work/assets" "$work/$name.wasm" "$zip" $clips | grep -v '^{'

# This game's benches (gp bench clips and gp bench scratch), when it has a
# bench.toml. Needs its ROM.
bench: _gp _in-game
    #!/usr/bin/env bash
    set -euo pipefail
    name="$(basename {{quote(game)}})"
    if [ ! -f {{quote(game / "bench.toml")}} ]; then echo "bench: skipped, $name has no bench.toml"; exit 0; fi
    GP_GAMES_DIR={{quote(justfile_directory())}} {{quote(gp)}} bench clips "$name"
    GP_GAMES_DIR={{quote(justfile_directory())}} {{quote(gp)}} bench scratch "$name"

# The public page's release check and deploy, moved here from gameprinter
# (gameprinter's docs/web.md, "Deploying the public page"). GP names a built
# or installed gp, default ../gameprinter/target/debug/gp; the page's files
# and checks are read from that gp's gameprinter checkout (GP_TOOLS, page/tools.sh).

# The release check: the page built from this checkout's page game with
# that gp's tools, the ROMs its routes play said (a missing one in a line),
# its game-data check, the headless check under its headers, the lift's
# tests, gp package scan on the game, and on a pass a record of the page's
# digest, which `just page-deploy` needs. `--full` reuses nothing;
# `--no-roms` runs with none.
page-release *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{args}}" in ""|--full|--no-roms|"--full --no-roms"|"--no-roms --full") ;; *) echo "page-release: give nothing, --full or --no-roms" >&2; exit 2 ;; esac
    export GP={{quote(gp)}}
    . page/tools.sh
    WEB_TOOLS=$("$tools/scripts/web/fetch-tools.sh" "${GP_HOME:-$HOME/.gameprinter}")
    export WEB_TOOLS
    page/release-check.sh {{args}}

# Runs the full release check, shows what would upload, uploads the page
# as a version that isn't live, checks it at the version's preview address,
# asks for the worker name, then makes it live. `list` stops before the
# upload with wrangler's dry run (no account needed); `first` puts a
# placeholder live once, for a Worker that doesn't exist yet. Needs
# `npx wrangler login` once.
page-deploy *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{args}}" in ""|list|first) ;; *) echo "page-deploy: give nothing, list or first" >&2; exit 2 ;; esac
    flag=""; [ "{{args}}" = list ] && flag="--list"; [ "{{args}}" = first ] && flag="--first"
    export GP={{quote(gp)}}
    if [ "$flag" != --first ]; then
        . page/tools.sh
        WEB_TOOLS=$("$tools/scripts/web/fetch-tools.sh" "${GP_HOME:-$HOME/.gameprinter}")
        export WEB_TOOLS
    fi
    GP={{quote(gp)}} page/deploy.sh $flag

# The old address's Worker: builds its redirect folder and runs wrangler's
# dry run; `upload` puts it up hidden, checks the preview's redirects, asks
# for the worker name and makes it live. Needs no gp.
page-deploy-old *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{args}}" in ""|upload) ;; *) echo "page-deploy-old: give nothing or upload" >&2; exit 2 ;; esac
    flag=""; [ "{{args}}" = upload ] && flag="--upload"
    page/deploy-old.sh $flag

# deploy.sh's flags and guards and deploy-old.sh's dry run against a
# wrangler stand-in: nothing uploaded, no account.
page-test-deploy:
    GP={{quote(gp)}} page/test-deploy.sh
