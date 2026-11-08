# The user's own games. `just check` is what every commit must pass.

target := env_var_or_default("CARGO_TARGET_DIR", justfile_directory() / ".." / "gameprinter" / "target")
gp := env_var_or_default("GP", target / "debug" / "gp")

setup:
    git config core.hooksPath .githooks

# gp, built from the tool repo beside this one, unless $GP names one.
_gp:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${GP:-}" ]; then
        [ -d ../gameprinter ] || { echo "no gp: set GP or keep the gameprinter repo beside this one" >&2; exit 1; }
        cargo build -q --manifest-path ../gameprinter/Cargo.toml -p gp-cli
    fi

# Every package folder through gp package scan. With no ROM here it says
# nothing was compared; with ROMs it compares them.
_scan-packages:
    #!/usr/bin/env bash
    set -euo pipefail
    git ls-files -- 'package.toml' '*/package.toml' | while IFS= read -r p; do
        {{quote(gp)}} package scan "$(dirname "$p")"
    done

check: _gp check-hosted
    just _scan-packages
    @echo "just check: passed"

# What needs no gp: the job on GitHub's machines, which can't build the private tool.
check-hosted:
    @[ "$(git config core.hooksPath)" = ".githooks" ] || { echo "The ROM-blocking hook isn't enabled. Run: just setup" >&2; exit 1; }
    python3 -I -B scripts/scan.py
    scripts/test-scan.sh
    scripts/test-hook.sh
    python3 -I -B -m unittest discover -s scripts/romguard -q
