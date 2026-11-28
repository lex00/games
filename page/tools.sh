# Sourced by page/release-check.sh and page/deploy.sh: sets gp (the gp to
# run, absolute) and tools (the gameprinter checkout whose page files and
# checks the scripts read).
#
# GP names a built or installed gp; default ../gameprinter/target/debug/gp
# beside this checkout. tools is GP_TOOLS when set, else the checkout a
# built gp sits in (TOOLS/target/PROFILE/gp, or TOOLS/target/TRIPLE/PROFILE/gp),
# else ../gameprinter beside this checkout. It must hold
# scripts/web/build-page.sh.
_page_checkout="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
gp="${GP:-$_page_checkout/../gameprinter/target/debug/gp}"
if [ "${gp#*/}" = "$gp" ]; then
  # A bare name: the gp on PATH.
  gp="$(command -v "$gp")" || { echo "page: GP=$GP isn't on PATH" >&2; return 2; }
fi
[ -x "$gp" ] || { echo "page: no gp at $gp; build one (cargo build -p gp-cli in gameprinter) or give GP=path" >&2; return 2; }
gp="$(cd "$(dirname "$gp")" && pwd)/$(basename "$gp")"
tools="${GP_TOOLS:-}"
if [ -z "$tools" ]; then
  for t in "$(dirname "$gp")/../.." "$(dirname "$gp")/../../.." "$_page_checkout/../gameprinter"; do
    if [ -f "$t/scripts/web/build-page.sh" ]; then tools="$t"; break; fi
  done
fi
if [ -z "$tools" ] || [ ! -f "$tools/scripts/web/build-page.sh" ]; then
  echo "page: $gp has no gameprinter checkout beside it (an installed gp carries a built page, not the scripts that build and check one); give GP_TOOLS, a gameprinter checkout at the commit that gp was built from" >&2
  return 2
fi
tools="$(cd "$tools" && pwd)"
export GP="$gp"
