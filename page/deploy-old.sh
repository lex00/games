#!/bin/bash
# The old address, nes-donkey-kong.intentius.workers.dev, kept working for a
# while after the page's Worker was renamed gameprinter (docs/web.md, "The
# old address"):
#   page/deploy-old.sh [--upload]
#
# Builds a folder ($GP_HOME/work/page-release/old-worker) holding a
# _redirects file that sends every path, query included, to the same path on
# the new address with a 301, and an index.html with a link to it, for the
# Worker named in page/old-worker/wrangler.toml. With no flag it shows
# the folder and runs `versions upload --dry-run`, which uploads nothing.
#
# --upload puts it up hidden first, as deploy.sh does the page:
#   1. refuses unless the new address serves the page (its page.json
#      answers 200), so nobody is sent to an address with nothing on it;
#   2. npx wrangler@WRANGLER versions upload --assets DIR --message ...
#      (not live), reading the version id and preview address;
#   3. asks the preview address for / and for a share link with a query,
#      without following redirects, and refuses unless each answers 301 to
#      the same path and query on the new address;
#   4. asks for the worker's name typed, then
#      npx wrangler@WRANGLER versions deploy VERSION_ID@100% --message ... --yes
#
# After a few weeks the Worker is deleted, from the Cloudflare dashboard or
# with `npx wrangler@4.157.0 delete` run from page/old-worker. It needs no
# gp and no gameprinter checkout.
# GP_WRANGLER replaces the wrangler command and GP_PAGE_ADDRESS the new
# address, for trying this script against a stand-in.
# Exit codes: 0 made live (or the dry run shown), 1 a check failed or you
# declined, 2 couldn't run.

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
wrangler_version="4.157.0"
read -r -a wrangler <<< "${GP_WRANGLER:-npx --yes wrangler@$wrangler_version}"
config="$here/old-worker"
project="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$config/wrangler.toml")"
[ -n "$project" ] || { echo "deploy-old: page/old-worker/wrangler.toml names no worker" >&2; exit 2; }
new_project="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$here/worker/wrangler.toml")"
[ -n "$new_project" ] || { echo "deploy-old: page/worker/wrangler.toml names no worker" >&2; exit 2; }
# The account's workers.dev subdomain is intentius.
target="${GP_PAGE_ADDRESS:-https://$new_project.intentius.workers.dev}"
target="${target%/}"
home="${GP_HOME:-$HOME/.gameprinter}"
out="$home/work/page-release/old-worker"

upload=0
case "${1:-}" in
  "") ;;
  --upload) upload=1 ;;
  *) echo "usage: page/deploy-old.sh [--upload]" >&2; exit 2 ;;
esac
[ $# -le 1 ] || { echo "usage: page/deploy-old.sh [--upload]" >&2; exit 2; }

mkdir -p "$out" || exit 2
# One splat rule: Cloudflare's _redirects keeps the request's query on the
# way, and step 3 of --upload checks that it does before going live.
printf '/* %s/:splat 301\n' "$target" > "$out/_redirects"
cat > "$out/index.html" <<EOF
<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Game Printer has moved</title>
<p>The page has moved to <a href="$target/">$target</a>.</p>
EOF

message="redirect to $target"
echo "The old worker $project, sending every path to $target:"
echo "  folder    $out"
echo "  _redirects $(cat "$out/_redirects")"
echo "  upload    ${wrangler[*]} versions upload --assets $out --message \"$message\""
echo "  go live   ${wrangler[*]} versions deploy VERSION_ID@100% --message \"$message\" --yes, after the preview redirects and the typed name"
echo "  later     ${wrangler[*]} delete, from page/old-worker, after a few weeks"
if [ $upload = 0 ]; then
  (cd "$config" && "${wrangler[@]}" versions upload --assets "$out" --dry-run) || exit 2
  echo "(no --upload: nothing uploaded)"
  exit 0
fi

code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "$target/page.json")"
if [ "$code" != 200 ]; then
  echo "deploy-old: $target/page.json answered $code, not 200, so the new address doesn't serve the page yet; nothing uploaded. Deploy the page first (page/deploy.sh)." >&2
  exit 1
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/gp-deploy-old.XXXXXX")" || exit 2
(cd "$config" && WRANGLER_OUTPUT_FILE_PATH="$work/out.ndjson" "${wrangler[@]}" versions upload --assets "$out" --message "$message") 2>&1 | tee "$work/upload.log"
[ "${PIPESTATUS[0]}" -eq 0 ] || { echo "deploy-old: wrangler versions upload failed; nothing changed at the old address." >&2; exit 2; }
read -r version preview < <(python3 -I "$here/wrangler-upload.py" "$work/out.ndjson" "$work/upload.log")
if [ -z "${version:-}" ] || [ "$version" = - ]; then
  echo "deploy-old: wrangler printed no version id, so there is nothing to check or make live." >&2
  exit 2
fi
if [ "$preview" = - ]; then
  echo "deploy-old: version $version is uploaded and not live, but has no preview address, so its redirect can't be checked. Run \`${wrangler[*]} triggers deploy\` once from page/old-worker (it turns on preview addresses and changes nothing served), then run this again." >&2
  exit 1
fi
echo "  version   $version"
echo "  preview   $preview"

bad=0
for path in "/" "/p?g=test/made-up&c=test/made-up-look" "/game/index.html?files=here"; do
  got="$(curl -s -o /dev/null -w '%{http_code} %{redirect_url}' --max-time 20 "$preview$path")"
  want="301 $target$path"
  if [ "$got" = "$want" ]; then echo "ok    $path answers $got"
  else echo "FAIL  $path answers \"$got\", not \"$want\""; bad=1; fi
done
if [ $bad = 1 ]; then
  echo "deploy-old: the preview doesn't redirect as it should; version $version stays uploaded and is not live." >&2
  exit 1
fi

printf 'Type the worker name (%s) to make the redirect live, anything else to stop: ' "$project"
read -r answer
[ "$answer" = "$project" ] || { echo "Stopped; version $version is uploaded and not live."; exit 1; }
cd "$config" || exit 2
"${wrangler[@]}" versions deploy "$version@100%" --message "$message" --yes || exit 2
echo "Live: the old address redirects to $target. Delete the worker in a few weeks (\`${wrangler[*]} delete\` from page/old-worker, or the dashboard)."
