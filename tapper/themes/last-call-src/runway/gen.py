#!/usr/bin/env python3
"""Runway calls for the Last Call theme. Key comes from RUNWAY_API_KEY and is never printed.
  gen.py NAME MODEL RATIO CREDITS PROMPT_FILE TAG=PNG [TAG=PNG ...]
Writes samples/NAME.png and adds a line to spend.jsonl. Makes one call, no retries."""
import subprocess, base64, json, os, sys, time, urllib.error, urllib.request

HERE = os.path.expanduser("~/.gameprinter/work/themes/tapper-pixel/runway")
API = "https://api.dev.runwayml.com/v1"
KEY = (os.environ.get("RUNWAY_API_KEY") or subprocess.run(["security", "find-generic-password", "-s", "game-printer-runway", "-w"], capture_output=True, text=True).stdout or open(os.path.expanduser("~/.runway")).read()).strip()
CAP = 480  # 2026-11-24 ('we can keep going with budget', 'use ... RUNWAY'): the space bar's pieces, 50 more


def spent():
    p = f"{HERE}/spend.jsonl"
    return sum(json.loads(l)["credits"] for l in open(p)) if os.path.exists(p) else 0


def call(method, path, body=None):
    req = urllib.request.Request(f"{API}{path}", method=method, data=None if body is None else json.dumps(body).encode(), headers={
        "Authorization": f"Bearer {KEY}", "X-Runway-Version": "2024-11-06", "Content-Type": "application/json", "User-Agent": "theme-gen/1",
    })
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        sys.exit(f"HTTP {e.code}: {e.read().decode()[:400]}")


name, model, ratio, credits, prompt_file, *refs = sys.argv[1:]
credits = int(credits)
if spent() + credits > CAP:
    sys.exit("over the approved budget, no call made")
body = {"model": model, "promptText": open(prompt_file).read().strip(), "ratio": ratio, "referenceImages": []}
for r in refs:
    tag, path = r.split("=", 1)
    body["referenceImages"].append({"tag": tag, "uri": "data:image/png;base64," + base64.b64encode(open(path, "rb").read()).decode()})
if model == "gpt_image_2":
    body["quality"], body["background"] = os.environ.get("GEN_QUALITY", "high"), "opaque"
task = call("POST", "/text_to_image", body)["id"]
print("task", task, flush=True)
for _ in range(180):
    time.sleep(5)
    t = call("GET", f"/tasks/{task}")
    if t["status"] == "SUCCEEDED":
        os.makedirs(f"{HERE}/samples", exist_ok=True)
        req = urllib.request.Request(t["output"][0], headers={"User-Agent": "theme-gen/1"})
        with urllib.request.urlopen(req, timeout=120) as r:
            open(f"{HERE}/samples/{name}.png", "wb").write(r.read())
        with open(f"{HERE}/spend.jsonl", "a") as f:
            f.write(json.dumps({"name": name, "model": model, "credits": credits, "task": task}) + "\n")
        print("saved", name, "credits", credits, "session total", spent())
        sys.exit(0)
    if t["status"] in ("FAILED", "CANCELLED"):
        sys.exit(f"task {t['status']}: {str(t.get('failure'))[:300]} (failed calls are not charged)")
sys.exit("timed out")
