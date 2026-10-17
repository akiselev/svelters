#!/usr/bin/env python3
"""Generate one PNG per element kind per toolkit, headless.

  python3 tools/docs/screenshots/run.py [--toolkit gtk|qt|all] [--only TAG[,TAG]]
                                        [--jobs N<=4] [--force] [--out DIR]

Re-execs itself under xvfb-run when DISPLAY is unset. Each kind is rendered
by a fresh subprocess of the toolkit module, so a crash or hang affects only
that kind.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SCRIPT_VERSION = "1"
SCHEMA = "svelters-docs-screenshots/v0"
KIND_TIMEOUT = 60
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DEFAULT_OUT = REPO / "docs" / "site" / ".generated" / "screenshots"
TOOLKITS = ("gtk", "qt")


def ensure_display():
    if os.environ.get("DISPLAY"):
        return
    xvfb = shutil.which("xvfb-run")
    if not xvfb:
        sys.exit("DISPLAY is unset and xvfb-run is not installed")
    os.execvp(xvfb, [xvfb, "-a", "-s", "-screen 0 1280x1024x24",
                     sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]])


def run_module(toolkit, *args, timeout=KIND_TIMEOUT):
    env = dict(os.environ, PYTHONHASHSEED="0")
    return subprocess.run([sys.executable, str(HERE / (toolkit + ".py")), *args],
                          capture_output=True, text=True, timeout=timeout, env=env)


def kind_hash(info, toolkit_version):
    h = hashlib.sha256()
    for part in ((REPO / info["meta"]).read_bytes(), info["recipe"].encode(),
                 toolkit_version.encode(), SCRIPT_VERSION.encode()):
        h.update(len(part).to_bytes(8, "big"))
        h.update(part)
    return h.hexdigest()


def render_one(toolkit, tag, png):
    png.unlink(missing_ok=True)
    try:
        r = run_module(toolkit, "render", tag, str(png))
    except subprocess.TimeoutExpired:
        return "timeout after %ds" % KIND_TIMEOUT
    if r.returncode != 0 or not png.exists():
        lines = [l for l in r.stderr.splitlines() if l.strip()]
        errors = [l for l in lines if "Error:" in l or "Exception:" in l]
        if r.returncode < 0:
            return "crashed with signal %d" % -r.returncode
        pick = errors or lines
        return pick[-1] if pick else "exit code %d" % r.returncode
    return None


def read_json(path, default):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return default


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--toolkit", choices=("gtk", "qt", "all"), default="all")
    ap.add_argument("--only", help="render only these tags (comma-separated)")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = ap.parse_args()
    ensure_display()

    jobs = max(1, min(args.jobs, 4))
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    cache_path = out / ".cache.json"
    manifest_path = out / "manifest.json"
    cache = read_json(cache_path, {})
    old = read_json(manifest_path, {})
    old_failed = old.get("failed", {})
    toolkits = TOOLKITS if args.toolkit == "all" else (args.toolkit,)

    keys, failed, todo = [], {}, []
    # Toolkits not selected this run keep their previous manifest entries.
    for k in old.get("keys", []):
        if k.split("/")[0] not in toolkits:
            keys.append(k)
    for k, v in old_failed.items():
        if k.split("/")[0] not in toolkits:
            failed[k] = v
    for tk in toolkits:
        try:
            listing = json.loads(run_module(tk, "list").stdout)
        except (ValueError, subprocess.TimeoutExpired) as e:
            failed[tk + "/*"] = "toolkit unavailable: %s" % e
            continue
        (out / tk).mkdir(exist_ok=True)
        for tag, info in sorted(listing["kinds"].items()):
            key = "%s/%s" % (tk, tag)
            keys.append(key)
            digest = kind_hash(info, listing["toolkit"])
            png = out / tk / (tag + ".png")
            if args.only and tag not in args.only.split(","):
                # untouched this run: keep prior outcome
                if key in old_failed and not png.exists():
                    failed[key] = old_failed[key]
                continue
            if (not args.force and cache.get(key) == digest
                    and (png.exists() or key in old_failed)):
                if key in old_failed and not png.exists():
                    failed[key] = old_failed[key]
                continue
            todo.append((tk, tag, key, png, digest))

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(pool.map(lambda t: render_one(t[0], t[1], t[3]), todo))
    for (tk, tag, key, png, digest), reason in zip(todo, results):
        cache[key] = digest
        if reason:
            failed[key] = reason

    present = sorted(k for k in keys if (out / (k + ".png")).exists())
    failed = {k: failed[k] for k in sorted(failed) if k not in present}
    cache = {k: cache[k] for k in sorted(cache)}
    cache_path.write_text(json.dumps(cache, indent=1) + "\n")
    manifest_path.write_text(json.dumps(
        {"schema": SCHEMA, "keys": present, "failed": failed}, indent=1) + "\n")
    print("rendered %d, ok %d, failed %d (out: %s)"
          % (len(todo), len(present), len(failed), out))
    return 0


if __name__ == "__main__":
    t0 = time.time()
    rc = main()
    print("%.1fs" % (time.time() - t0), file=sys.stderr)
    sys.exit(rc)
