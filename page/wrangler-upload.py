"""The version id and preview address of a `wrangler versions upload`.

    python3 -I page/wrangler-upload.py OUTPUT_FILE LOG

OUTPUT_FILE is what wrangler 4.157.0 wrote to WRANGLER_OUTPUT_FILE_PATH (one
JSON object a line; the upload's has type "version-upload", version_id and
preview_url). LOG is what it printed, read when the file lacks either one:
its "Worker Version ID:" and "Version Preview URL:" lines. Prints the two on
one line, with "-" for one not found. Used by deploy.sh and deploy-old.sh.
"""
import json
import re
import sys

version = preview = ""
try:
    with open(sys.argv[1]) as f:
        for line in f:
            try:
                o = json.loads(line)
            except ValueError:
                continue
            if isinstance(o, dict) and o.get("type") == "version-upload":
                version = o.get("version_id") or version
                preview = o.get("preview_url") or preview
except OSError:
    pass
try:
    with open(sys.argv[2], errors="replace") as f:
        log = f.read()
except OSError:
    log = ""
if not version:
    m = re.search(r"Worker Version ID:\s*([0-9a-f-]{8,})", log)
    version = m.group(1) if m else ""
if not preview:
    m = re.search(r"Version Preview URL:\s*(https?://\S+)", log)
    preview = m.group(1) if m else ""
print(version or "-", (preview or "-").rstrip("/"))
