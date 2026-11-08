# The user's own games. `just check` is what every commit must pass.

setup:
    git config core.hooksPath .githooks

check:
    @[ "$(git config core.hooksPath)" = ".githooks" ] || { echo "The ROM-blocking hook isn't enabled. Run: just setup" >&2; exit 1; }
    python3 -I -B scripts/scan.py
    scripts/test-scan.sh
    scripts/test-hook.sh
    python3 -I -B -m unittest discover -s scripts/romguard -q
    @echo "just check: passed"
