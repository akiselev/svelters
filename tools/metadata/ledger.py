#!/usr/bin/env python3
"""Verified-facts ledger: metadata/verified/{gtk,qt}.json.

Every manual (hand/agent-annotated) fact has generated tests. The generators write a fact
manifest (fact id -> test names + a fingerprint over the fact record and its test source):

  gtk: crates/svelters-gtk/src/generated/facts.json   (written by tools/metadata/gtk/gen_backend.py)
  qt:  produced on the fly by tools/metadata/qt/gen_cpp.py --facts

`refresh` runs the generated tests and records, per test: the fact id, the test id, the result,
the fingerprint and the toolkit version. `check` fails when any manual fact lacks a passing
entry with the current fingerprint, so an annotation whose record, test or upstream source
changed must be re-verified before it can merge (a toolkit upgrade changes the ingested record
and therefore the fingerprint of every fact built on it).

  python3 tools/metadata/ledger.py refresh [--toolkit gtk|qt|all]   # needs a display or xvfb-run
  python3 tools/metadata/ledger.py check   [--toolkit gtk|qt|all]

Build limits come from the environment (CARGO_BUILD_JOBS, CARGO_BUILD_BUILD_DIR); `refresh`
defaults them to 4 jobs and target/build when unset.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEDGER_DIR = ROOT / "metadata/verified"
GTK_FACTS = ROOT / "crates/svelters-gtk/src/generated/facts.json"


def qt_facts() -> dict:
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "facts.json"
        subprocess.run([sys.executable, str(ROOT / "tools/metadata/qt/gen_cpp.py"), "--config", str(ROOT / "tools/metadata/qt/gen.dev.toml"),
                        "--out", d, "--facts", str(out)], check=True)
        return json.loads(out.read_text())


def manifest(tk: str) -> dict:
    return json.loads(GTK_FACTS.read_text()) if tk == "gtk" else qt_facts()


def load(tk: str) -> dict:
    p = LEDGER_DIR / f"{tk}.json"
    if not p.exists():
        return {"toolkit": tk, "entries": []}
    return json.loads(p.read_text())


def save(tk: str, version: str, entries: list[dict]) -> None:
    """One entry per line, sorted, so parallel branches merge without conflicts."""
    entries = sorted(entries, key=lambda e: (e["fact"], e["test"]))
    lines = [json.dumps(e, sort_keys=True, ensure_ascii=False) for e in entries]
    text = ('{"schema": "svelters-verified/v1", "toolkit": %s, "version": %s, "entries": [\n' % (json.dumps(tk), json.dumps(version))
            + ",\n".join(lines) + "\n]}\n")
    LEDGER_DIR.mkdir(parents=True, exist_ok=True)
    (LEDGER_DIR / f"{tk}.json").write_text(text)


def env() -> dict:
    e = dict(os.environ)
    e.setdefault("CARGO_BUILD_JOBS", "4")
    e.setdefault("CARGO_BUILD_BUILD_DIR", "target/build")
    e.setdefault("GSK_RENDERER", "cairo")
    return e


def display_prefix() -> list[str]:
    if os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        return []
    if shutil.which("xvfb-run"):
        return ["xvfb-run", "-a"]
    return []


def run_gtk() -> dict[str, str]:
    threads = os.environ.get("SVELTERS_TEST_THREADS", "4")
    cmd = display_prefix() + ["cargo", "test", "-p", "svelters-gtk", "--lib", "generated::verify", "--", "--test-threads", threads]
    p = subprocess.run(cmd, cwd=ROOT, env=env(), capture_output=True, text=True)
    res = {}
    for m in re.finditer(r"^test (generated::verify::\S+) \.\.\. (ok|FAILED|ignored)", p.stdout, re.M):
        res[m.group(1)] = {"ok": "pass", "FAILED": "fail", "ignored": "skip"}[m.group(2)]
    if not res:
        sys.stderr.write(p.stdout[-4000:] + p.stderr[-4000:])
        raise SystemExit("ledger: no GTK test results (build failure?)")
    return res


def run_qt() -> dict[str, str]:
    cmd = display_prefix() + ["cargo", "test", "-p", "counter-qt", "--test", "generated"]
    p = subprocess.run(cmd, cwd=ROOT, env=env(), capture_output=True, text=True)
    res = {}
    for m in re.finditer(r"^(ok  |FAIL) (\S+?)(?::|$)", p.stdout, re.M):
        res[m.group(2)] = "pass" if m.group(1).startswith("ok") else "fail"
    if not res:
        sys.stderr.write(p.stdout[-4000:] + p.stderr[-4000:])
        raise SystemExit("ledger: no Qt test results (build failure?)")
    for line in p.stdout.splitlines():
        if line.startswith("FAIL "):
            print("  " + line, file=sys.stderr)
    return res


def refresh(tk: str) -> int:
    man = manifest(tk)
    results = run_gtk() if tk == "gtk" else run_qt()
    entries, bad = [], 0
    for f in man["facts"]:
        for t in f["tests"]:
            r = results.get(t, "missing")
            bad += r != "pass"
            entries.append({"fact": f["fact"], "test": t, "result": r, "fingerprint": f["fingerprint"], "version": man["version"]})
    save(tk, man["version"], entries)
    n = sum(len(f["tests"]) for f in man["facts"])
    print(f"ledger {tk}: {len(man['facts'])} facts, {n} tests, {bad} not passing")
    return 1 if bad else 0


def check(tk: str) -> int:
    man = manifest(tk)
    led = {(e["fact"], e["test"]): e for e in load(tk)["entries"]}
    problems = []
    for f in man["facts"]:
        for t in f["tests"]:
            e = led.get((f["fact"], t))
            if e is None:
                problems.append(f"{f['fact']}: {t}: not in the ledger (run `ledger.py refresh --toolkit {tk}`)")
            elif e["fingerprint"] != f["fingerprint"]:
                problems.append(f"{f['fact']}: {t}: fact or test changed since it was verified (refresh)")
            elif e["result"] != "pass":
                problems.append(f"{f['fact']}: {t}: {e['result']}")
    if problems:
        print(f"ledger {tk}: {len(problems)} unverified manual fact test(s):", file=sys.stderr)
        for p in problems:
            print("  " + p, file=sys.stderr)
        return 1
    print(f"ledger {tk}: {len(man['facts'])} manual facts verified")
    return 0


def verified_facts(tk: str) -> set[str]:
    """Fact ids whose every test passed with the current fingerprint (for docs and coverage).
    Reads the ledger only; fingerprints are checked by `check`."""
    by: dict[str, bool] = {}
    for e in load(tk)["entries"]:
        by[e["fact"]] = by.get(e["fact"], True) and e["result"] == "pass"
    return {f for f, ok in by.items() if ok}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["refresh", "check"])
    ap.add_argument("--toolkit", default="all", choices=["gtk", "qt", "all"])
    a = ap.parse_args()
    rc = 0
    for tk in (["gtk", "qt"] if a.toolkit == "all" else [a.toolkit]):
        rc |= (refresh if a.cmd == "refresh" else check)(tk)
    return rc


if __name__ == "__main__":
    sys.exit(main())
