#!/bin/bash
# Puts a built page on its address, a Cloudflare Worker that serves a folder
# (docs/web.md, "Deploying the public page"). A new version goes up hidden,
# is checked at its own preview address, and only then is made live:
#   page/deploy.sh [PAGE_DIR] [--list] [--no-roms] [WHERE]
#   page/deploy.sh --first [WHERE]
# GP and GP_TOOLS are as for page/release-check.sh (page/tools.sh); --first
# needs neither.
#
# WHERE is the worker and its address, for someone deploying their own copy
# of the page with no edit to the repo (docs/host-your-own.md):
#   --worker NAME      the worker's name (GP_PAGE_WORKER; default the name
#                      in page/worker/wrangler.toml, gameprinter)
#   --subdomain SUB    the account's workers.dev subdomain (GP_PAGE_SUBDOMAIN;
#                      default intentius, the owner's), so the address is
#                      https://NAME.SUB.workers.dev
#   --address URL      the page's whole address instead (GP_PAGE_ADDRESS),
#                      for a worker on a domain of its own
# Every wrangler command is given --name NAME, so wrangler.toml's other
# settings (compatibility_date, workers_dev, preview_urls) apply to any
# worker. A flag wins over its variable. --no-roms runs the release check
# as release-check.sh --no-roms (GP_PAGE_NO_ROMS=1 does the same): for
# someone with no ROMs, each check that needs one is skipped and says so,
# the pass is recorded as made without ROMs, and the upload is held to the
# page's digest as always.
#
# 1. With no PAGE_DIR, runs the full release check (release-check.sh
#    --full: every product made and every check run again, nothing
#    reused) and takes the page it built. Anything but PASS stops here.
# 2. Works out the page folder's digest (the tools' scripts/web/page-digest.py) and
#    refuses unless the release check recorded a pass for that digest
#    ($GP_HOME/work/page-release/passed/DIGEST): only a folder whose every
#    file is as it was when the check passed goes up.
# 3. Shows what would upload: the folder, the file count and size, the
#    worker, the commit, the pass it goes up under and the commands.
# 4. Takes the digest again and uploads the folder as a new version that
#    is not live:
#      npx wrangler@WRANGLER versions upload --assets PAGE_DIR --message ...
#    and reads the version id and its preview address from what wrangler
#    writes (WRANGLER_OUTPUT_FILE_PATH, else its "Worker Version ID:" and
#    "Version Preview URL:" lines). Visitors still get the live version.
# 5. Runs the page check (check-page.mjs --live PREVIEW_URL) on that
#    address, every section, with the release check's registry snapshot, the
#    games checkout it read (its pass record's `games`) and the Moss Hop and
#    arcade theme folders beside the page. A failure stops here; the
#    version stays uploaded and is never made live.
# 6. Asks you to type the worker's name, then makes the version live:
#      npx wrangler@WRANGLER versions deploy VERSION_ID@100% --message ... --yes
# --list stops after step 3 with `versions upload --dry-run`, which uploads
# nothing.
#
# --first is for a worker that doesn't exist yet: `versions upload` refuses
# one ("You cannot upload a new version of a Worker that does not yet
# exist"). It puts a one-file placeholder page live with `wrangler deploy`,
# after the typed name, which makes the worker and turns on its workers.dev
# address and preview addresses from wrangler.toml. It refuses when the
# address already serves a page. Run it once, then deploy as above.
#
# All of it needs `npx wrangler login` done once (it opens a browser).
# Cloudflare keeps earlier versions: `npx wrangler rollback` from
# page/worker makes the one before live again. GP_WRANGLER replaces
# the wrangler command (default `npx --yes wrangler@WRANGLER`), for trying
# this script against a stand-in; GP_PAGE_ADDRESS replaces the live address
# --first looks at; GP_PREVIEW_ONLY passes --only REGEX to the preview's page
# check. GP and WEB_TOOLS are as for release-check.sh.
# Exit codes: 0 made live (or --list shown), 1 the release check or the
# preview's check failed, no pass is recorded for this folder, or you
# declined, 2 couldn't run.

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
# The worker's own wrangler config, in this repo.
config="$here/worker"
wrangler_version="4.157.0"
read -r -a wrangler <<< "${GP_WRANGLER:-npx --yes wrangler@$wrangler_version}"
toml_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$config/wrangler.toml")"
[ -n "$toml_name" ] || { echo "deploy: page/worker/wrangler.toml names no worker" >&2; exit 2; }
home="${GP_HOME:-$HOME/.gameprinter}"
passed="$home/work/page-release/passed"
usage="usage: page/deploy.sh [PAGE_DIR] [--list] [--no-roms] [--worker NAME] [--subdomain SUB | --address URL] | --first [--worker NAME] [--subdomain SUB | --address URL]"

list=0
first=0
no_roms="${GP_PAGE_NO_ROMS:-0}"
page=""
project="${GP_PAGE_WORKER:-$toml_name}"
# The owner's account's workers.dev subdomain is intentius.
subdomain="${GP_PAGE_SUBDOMAIN:-intentius}"
address="${GP_PAGE_ADDRESS:-}"
while [ $# -gt 0 ]; do
  case "$1" in
    --list) list=1; shift ;;
    --first) first=1; shift ;;
    --no-roms) no_roms=1; shift ;;
    --worker|--subdomain|--address)
      [ -n "${2:-}" ] || { echo "deploy: $1 needs a value" >&2; echo "$usage" >&2; exit 2; }
      case "$1" in
        --worker) project="$2" ;;
        --subdomain) subdomain="$2" ;;
        --address) address="$2" ;;
      esac
      shift 2 ;;
    -*) echo "$usage" >&2; exit 2 ;;
    *) [ -z "$page" ] || { echo "$usage" >&2; exit 2; }; page="$1"; shift ;;
  esac
done
[ $first = 0 ] || { [ -z "$page" ] && [ $list = 0 ]; } || { echo "$usage" >&2; exit 2; }
[ "$no_roms" = 0 ] || [ "$no_roms" = 1 ] || { echo "deploy: GP_PAGE_NO_ROMS is 1 or unset, not $no_roms" >&2; exit 2; }
# Cloudflare's rule for a worker's name and a subdomain: lower-case letters,
# digits and dashes, at most 63, not starting or ending with a dash.
label='^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'
[[ "$project" =~ $label ]] || { echo "deploy: $project isn't a worker name (lower-case letters, digits and dashes)" >&2; exit 2; }
[[ "$subdomain" =~ $label ]] || { echo "deploy: $subdomain isn't a workers.dev subdomain (lower-case letters, digits and dashes)" >&2; exit 2; }
[ -n "$address" ] || address="https://$project.$subdomain.workers.dev"
address="${address%/}"
[[ "$address" =~ ^https://[^/?#[:space:]]+(/[^?#[:space:]]*)?$ ]] || { echo "deploy: the address must be https://HOST, not $address" >&2; exit 2; }
named=(--name "$project")

confirm() {
  printf 'Type the worker name (%s) to %s, anything else to stop: ' "$project" "$1"
  read -r answer
  [ "$answer" = "$project" ]
}

if [ $first = 1 ]; then
  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "$address/page.json")"
  if [ "$code" = 200 ]; then
    echo "deploy: $address already serves a page (page.json answered 200), so the worker exists; deploy without --first. Nothing uploaded." >&2
    exit 1
  fi
  holder="$home/work/page-release/first-placeholder"
  mkdir -p "$holder" || exit 2
  cat > "$holder/index.html" <<'EOF'
<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Game Printer</title>
<p>This page is being set up. Come back soon.</p>
EOF
  echo "The worker $project gets a placeholder page, live at $address:"
  echo "  folder    $holder (index.html alone)"
  echo "  command   ${wrangler[*]} deploy ${named[*]} --assets $holder --message \"placeholder before the first page\""
  echo "($address/page.json answered $code.)"
  confirm "put the placeholder live" || { echo "Stopped; nothing uploaded."; exit 1; }
  cd "$config" || exit 2
  "${wrangler[@]}" deploy "${named[@]}" --assets "$holder" --message "placeholder before the first page" || exit 2
  echo "The worker exists now; deploy the page with page/deploy.sh (just page-deploy)."
  exit 0
fi

# Past --first every step reads the tools' page files: the digest, the page
# check and the headless browser (page/tools.sh).
. "$here/tools.sh" || exit 2
repo="$tools"

if [ -z "$page" ]; then
  log="$home/work/page-release/last-release-check.log"
  mkdir -p "$home/work/page-release" || exit 2
  check_flags=(--full)
  [ "$no_roms" = 1 ] && check_flags+=(--no-roms)
  "$here/release-check.sh" "${check_flags[@]}" | tee "$log"
  status=${PIPESTATUS[0]}
  page="$(sed -n 's/^bundle: //p' "$log" | tail -n 1)"
  if [ "$status" -ne 0 ] || [ -z "$page" ]; then
    echo "deploy: the release check did not pass; nothing uploaded." >&2
    exit 1
  fi
fi

[ -d "$page" ] || { echo "deploy: $page isn't a folder" >&2; exit 2; }
page="$(cd "$page" && pwd)"
for f in index.html page.json _headers _redirects game/module-host.js; do
  [ -e "$page/$f" ] || { echo "deploy: $page has no $f, so it isn't a built page" >&2; exit 2; }
done
digest="$(python3 -I "$repo/scripts/web/page-digest.py" "$page")" || exit 2
if [ ! -f "$passed/$digest" ]; then
  echo "deploy: no release check passed on this page as it is now (digest $digest; passes are recorded in $passed). Run page/release-check.sh, or deploy with no PAGE_DIR to run it; nothing uploaded." >&2
  exit 1
fi
record_id="$(sed -n 's/^id //p' "$passed/$digest")"
record_when="$(sed -n 's/^when //p' "$passed/$digest")"
record_roms="$(sed -n 's/^roms //p' "$passed/$digest")"
# The games checkout the release check read, for the preview's page check
# (a record from before it was kept: GP_GAMES_DIR, else check-page.mjs's own
# default).
record_games="$(sed -n 's/^games //p' "$passed/$digest")"
games=()
if [ -n "${GP_GAMES_DIR:-}" ]; then games=(--games "$GP_GAMES_DIR")
elif [ -n "$record_games" ]; then games=(--games "$record_games"); fi
roms_note=""
[ "$record_roms" = none ] && roms_note=", made without ROMs (--no-roms): the checks that need a ROM were skipped"

files="$(find "$page" -type f | wc -l | tr -d ' ')"
size="$(du -sk "$page" | cut -f1)"
commit="$(git -C "$repo" rev-parse --short=12 HEAD)"
message="gameprinter $commit, page $record_id"
echo
echo "Would upload to Cloudflare Workers (static assets), hidden until checked:"
echo "  worker    $project ($address)"
echo "  folder    $page"
echo "  files     $files, about $((size / 1024)) MiB (the list and hashes: $page.manifest.txt)"
echo "  passed    the release check on $record_id at $record_when (digest $digest)$roms_note"
echo "  commit    $commit"
echo "  upload    ${wrangler[*]} versions upload ${named[*]} --assets $page --message \"$message\""
echo "  check     check-page.mjs --live PREVIEW_URL, on the version's own address"
echo "  go live   ${wrangler[*]} versions deploy ${named[*]} VERSION_ID@100% --message \"$message\" --yes, after the typed name"
if [ $list = 1 ]; then
  # wrangler's own dry run: reads the folder and runs its checks, uploads nothing.
  (cd "$config" && "${wrangler[@]}" versions upload "${named[@]}" --assets "$page" --dry-run) || exit 2
  echo "(--list: nothing uploaded)"
  exit 0
fi

# The folder could have changed since its digest was taken (a release check
# run first takes minutes).
now="$(python3 -I "$repo/scripts/web/page-digest.py" "$page")" || exit 2
if [ "$now" != "$digest" ]; then
  echo "deploy: the page changed since its digest was taken ($digest, now $now); nothing uploaded." >&2
  exit 1
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/gp-deploy.XXXXXX")" || exit 2
echo
echo "Uploading a version that is not live:"
(cd "$config" && WRANGLER_OUTPUT_FILE_PATH="$work/out.ndjson" "${wrangler[@]}" versions upload "${named[@]}" --assets "$page" --message "$message") 2>&1 | tee "$work/upload.log"
[ "${PIPESTATUS[0]}" -eq 0 ] || {
  echo "deploy: wrangler versions upload failed; nothing is live that wasn't before. A worker that doesn't exist yet needs page/deploy.sh --first once." >&2
  exit 2
}
read -r version preview < <(python3 -I "$here/wrangler-upload.py" "$work/out.ndjson" "$work/upload.log")
if [ -z "${version:-}" ] || [ "$version" = - ]; then
  echo "deploy: wrangler printed no version id, so there is nothing to check or make live." >&2
  exit 2
fi
if [ "$preview" = - ]; then
  echo "deploy: version $version is uploaded and not live, but wrangler gave it no preview address, so it can't be checked; nothing made live. Preview addresses are off for $project: run \`${wrangler[*]} triggers deploy ${named[*]}\` once from page/worker (it applies workers_dev and preview_urls from wrangler.toml and changes nothing the address serves), then deploy again." >&2
  exit 1
fi
echo "  version   $version"
echo "  preview   $preview"

echo
echo "The page check on the preview address:"
browser="$("$repo/scripts/inspector/fetch-browser.sh" "$home")" || { echo "deploy: no headless browser; nothing made live" >&2; exit 2; }
if [ -z "${PLAYWRIGHT_FROM:-}" ] && [ ! -d "$repo/scripts/web/node_modules/playwright-core" ]; then
  npm ci --prefix "$repo/scripts/web" --silent --no-audit --no-fund || exit 2
fi
# The release check's snapshot of the page's source (with the made-up second
# game), so the check reads what it read then.
registry_folder=()
if [ -d "$page.registry.any" ]; then registry_folder=(--registry-folder "$page.registry.any")
elif [ -d "$page.registry" ]; then registry_folder=(--registry-folder "$page.registry"); fi
only=()
[ -n "${GP_PREVIEW_ONLY:-}" ] && only=(--only "$GP_PREVIEW_ONLY")
node "$repo/scripts/web/public/check-page.mjs" --live "$preview" --dist "$page" --browser "$browser" --shots "$page.preview-shots" ${games[@]+"${games[@]}"} ${registry_folder[@]+"${registry_folder[@]}"} ${only[@]+"${only[@]}"}
s=$?
if [ $s -ne 0 ]; then
  echo "deploy: the page check failed on the preview (exit $s; shots in $page.preview-shots). Version $version stays uploaded and is not live; nothing changed at $address." >&2
  exit 1
fi

echo
echo "Version $version passed the page check at $preview."
confirm "make it live at $address" || { echo "Stopped; version $version is uploaded and not live."; exit 1; }
cd "$config" || exit 2
"${wrangler[@]}" versions deploy "${named[@]}" "$version@100%" --message "$message" --yes || {
  echo "deploy: wrangler versions deploy failed; check what is live with \`${wrangler[*]} deployments list ${named[*]}\` from page/worker." >&2
  exit 2
}
echo "Live at $address: version $version. To go back: \`${wrangler[*]} rollback ${named[*]}\` from page/worker."
