#!/usr/bin/env python3
"""Verified-facts ledger: metadata/verified/{gtk,qt,appkit,winui}.json.

Every annotated fact has generated tests: the manual facts (hand/agent annotations) and the inferred
ones (metadata/inferred/<tk>, written by infer.py). The generators write a fact manifest (fact id ->
origin, test names + a fingerprint over the fact record and its test source):

  gtk: crates/svelters-gtk/src/generated/facts.json   (written by tools/metadata/gtk/gen_backend.py)
  qt:  produced on the fly by tools/metadata/qt/gen_cpp.py --facts (inferred facts: table-driven rows)
  appkit: crates/svelters-appkit/src/generated/facts.json (tools/metadata/appkit/gen_backend.py)

`refresh` runs the generated tests and records, per test: the fact id, the origin, the test id, the
result, the fingerprint and the toolkit version. `check` fails when any manual fact lacks a passing
entry with the current fingerprint, so an annotation whose record, test or upstream source
changed must be re-verified before it can merge (a toolkit upgrade changes the ingested record
and therefore the fingerprint of every fact built on it).

Inferred facts are promoted by passing: a passing entry makes the fact verified. A failing inferred
fact is not a gate failure; `refresh` parks it in metadata/inferred/<tk>/_parked.json with the failure
message (fact id, record fingerprint, test, message), regenerates without it and reruns what changed.
A parked fact stays out until its record changes (a new probe or rule) or the toolkit version does.
`check` also fails when the checked-in probe (metadata/probe/<tk>/<version>.json) is not for the
toolkit version in the ledger: the probe is a maintainer step, CI only checks it is current.

  python3 tools/metadata/ledger.py refresh [--toolkit gtk|qt|appkit|all]   # needs a display or xvfb-run (appkit: a Mac)
  python3 tools/metadata/ledger.py check   [--toolkit gtk|qt|appkit|winui|all]
  python3 tools/metadata/ledger.py import  --toolkit appkit RESULTS [--dry-run]
  python3 tools/metadata/ledger.py import  --toolkit winui <results.jsonl or the unpacked CI artifact dir>

`import` is for toolkits whose tests only run elsewhere (AppKit on the macOS CI runner): it reads a
test-results file (JSON lines `{"test", "result": "pass"|"fail", "message"}` as written by
svelters_apple::testing with SVELTERS_TEST_RESULTS, a JSON `{"results": [...]}` of the same records,
or JUnit XML), matches the test names against the generator's facts manifest, parks failing inferred
facts and regenerates (like refresh), and writes the ledger. `all` covers gtk, qt and every other
toolkit that has a facts manifest and a ledger file.

Toolkits whose tests only run on a CI runner (winui: crates/svelters-winui/src/generated/facts.json, tests on the
windows-winui job) have no `refresh`: the job writes one JSON line per test (SVELTERS_TEST_RESULTS) next to a copy
of facts.json, and `import` records that artifact (`import_ci`). Only facts whose fingerprint in the artifact equals
the current one are recorded; failing inferred facts are parked as in `refresh` (tests that never ran stay pending).

Observe facts (milestone R0, tools/metadata/common/observe.py) are table rows: one GTK test per owner type
(`generated::verify::observe_<owner>`) reports each row as `<test>#<prop>` through SVELTERS_TEST_RESULTS, the Qt
verify binary prints `observe/<owner>.<prop>` lines. Parking one removes it from metadata/observe/<tk>.json; a row
its table never reached (an earlier row crashed the process) stays pending and runs again.

Cargo runs go through the machine-wide build queue `tools/dev/cargo-slot` (it sets
CARGO_BUILD_JOBS, NEXTEST_TEST_THREADS and CARGO_BUILD_BUILD_DIR); without it, the environment's
settings apply.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEDGER_DIR = ROOT / "metadata/verified"
GTK_FACTS = ROOT / "crates/svelters-gtk/src/generated/facts.json"
# Facts manifests written by a generator (gtk is extended with the hand-written kinds below; qt is built on the fly).
WINUI_FACTS = ROOT / "crates/svelters-winui/src/generated/facts.json"
APPKIT_FACTS = ROOT / "crates/svelters-appkit/src/generated/facts.json"
FACTS = {"gtk": GTK_FACTS, "appkit": APPKIT_FACTS, "winui": WINUI_FACTS}
# Toolkits verified on a CI runner only (no local `refresh`; `import` reads the CI artifact with import_ci).
CI_ONLY = {"winui": WINUI_FACTS}


def qt_facts() -> dict:
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "facts.json"
        subprocess.run([sys.executable, str(ROOT / "tools/metadata/qt/gen_cpp.py"), "--config", str(ROOT / "tools/metadata/qt/gen.dev.toml"),
                        "--out", d, "--facts", str(out)], check=True)
        return json.loads(out.read_text())


GTK_EXTRAS = ROOT / "metadata/extras/gtk-menus.yaml"
GTK_EXTRA_TESTS = ROOT / "crates/svelters-gtk/tests"


def gtk_extra_facts() -> list[dict]:
    """Facts of the hand-written GTK kinds (X4 menus): tests are `<binary>::<fn>` of crates/svelters-gtk/tests.
    The fingerprint covers the fact record and the source of the test files it names."""
    import hashlib
    sys.path.insert(0, str(ROOT / "tools/metadata/common"))
    import svmeta
    out = []
    for f in svmeta.load_yaml(GTK_EXTRAS).get("facts", []):
        h = hashlib.sha256(json.dumps(f, sort_keys=True).encode())
        for b in sorted({t.split("::")[0] for t in f["tests"]}):
            h.update((GTK_EXTRA_TESTS / f"{b}.rs").read_bytes())
        out.append({"fact": f"svelters-api:v1:extra:gtk:menus:{f['id']}", "fingerprint": h.hexdigest()[:16], "tests": list(f["tests"]),
                    "kind": "menus", "name": f["id"], "section": "extras"})
    return out + gtk_type_facts()


GTK_TYPE_FACTS = ROOT / "metadata/gtk/types/facts.json"


def gtk_type_facts() -> list[dict]:
    """Facts of the curated non-GUI types (NT0): generated by tools/metadata/gtk/gen_types.py from the type
    facts, tests are `types_verify::<fn>` of crates/svelters-gtk/tests/types_verify.rs. The fingerprint covers
    the fact record (with a digest of the type's facts) and the test source."""
    import hashlib
    if not GTK_TYPE_FACTS.exists():
        return []
    src = (GTK_EXTRA_TESTS / "types_verify.rs").read_bytes()
    out = []
    for f in json.loads(GTK_TYPE_FACTS.read_text())["facts"]:
        h = hashlib.sha256(json.dumps(f, sort_keys=True).encode())
        h.update(src)
        out.append({**f, "fingerprint": h.hexdigest()[:16]})
    return out


APPKIT_TYPE_FACTS = ROOT / "metadata/appkit/types/facts.json"
APPKIT_TYPE_TESTS = [ROOT / "examples/counter-mac/tests/types.rs", ROOT / "crates/svelters-appkit/src/testing_types.rs"]


def appkit_type_facts() -> list[dict]:
    """Facts of the non-GUI AppKit types (N4): generated by tools/metadata/appkit/gen_types.py, tests are the cases
    of examples/counter-mac/tests/types.rs (macOS CI; `import` reads them). The fingerprint covers the fact record
    (with a digest of the type's facts) and the test sources."""
    import hashlib
    if not APPKIT_TYPE_FACTS.exists():
        return []
    src = b"".join(p.read_bytes() for p in APPKIT_TYPE_TESTS)
    out = []
    for f in json.loads(APPKIT_TYPE_FACTS.read_text())["facts"]:
        h = hashlib.sha256(json.dumps(f, sort_keys=True).encode())
        h.update(src)
        out.append({**f, "fingerprint": h.hexdigest()[:16]})
    return out


def manifest(tk: str) -> dict:
    if tk == "appkit" and FACTS[tk] == APPKIT_FACTS:
        m = json.loads(FACTS[tk].read_text())
        m["facts"] = m["facts"] + appkit_type_facts()
        return m
    if tk == "gtk":
        m = json.loads(GTK_FACTS.read_text())
        m["facts"] = m["facts"] + gtk_extra_facts()
        return m
    if tk == "qt":
        return qt_facts()
    return json.loads(FACTS[tk].read_text())


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


SLOT = Path("/projects/svelters/tools/dev/cargo-slot")


def env() -> dict:
    e = dict(os.environ)
    e.setdefault("CARGO_BUILD_BUILD_DIR", "target/build")
    e.setdefault("GSK_RENDERER", "cairo")
    return e


def display_prefix() -> list[str]:
    """The build queue (when present) and a private virtual display: always, even when the session
    has one (WSLg, a desktop), so test windows never reach the user's screen (tools/dev/headless)."""
    out = [str(SLOT)] if SLOT.exists() else ([str(ROOT / "tools/dev/cargo-slot")] if (ROOT / "tools/dev/cargo-slot").exists() else [])
    if sys.platform.startswith("linux"):
        out += [str(ROOT / "tools/dev/headless")]
    return out


RESULT = re.compile(r"^\s*(?:TRY \d+ )?(PASS|FLAKY(?: \d+/\d+)?|FAIL|TIMEOUT|LEAK|SIG\w+)\b[^\n]*?\]\s+(?:\(\s*(?:\d+/\d+|\W+)\)\s+)?svelters-gtk\s+(\S+)\s*$", re.M)


def results_of(text: str) -> dict[str, str]:
    """test name -> pass|fail from a nextest report. The last line of a test is its final attempt (a retry prints
    `TRY 2 FAIL`), and the failure replay at the end repeats it."""
    res = {}
    for m in RESULT.finditer(text):
        res[m.group(2)] = "pass" if m.group(1).startswith(("PASS", "FLAKY")) else "fail"
    return res


def failure_messages(text: str) -> dict[str, str]:
    """test name -> the panic message of its last failed attempt, from the failure replay of a nextest report."""
    out: dict[str, str] = {}
    marks = list(RESULT.finditer(text))
    for i, m in enumerate(marks):
        if m.group(1).startswith(("PASS", "FLAKY")):
            continue
        body = text[m.end():marks[i + 1].start() if i + 1 < len(marks) else len(text)]
        lines = body.splitlines()
        for j, l in enumerate(lines):
            if "panicked at" in l:
                msg = " ".join(x.strip() for x in lines[j + 1:j + 3] if x.strip() and not x.strip().startswith("note:"))
                out[m.group(2)] = (msg or l.strip())[:240]
                break
        else:
            out.setdefault(m.group(2), "failed without a panic message")
    return out


def row_results(path: Path) -> tuple[dict[str, str], dict[str, str]]:
    """(row test -> pass|fail|crash, row test -> message) from the JSON lines a table test writes with
    SVELTERS_TEST_RESULTS (observe facts: `generated::verify::observe_<owner>#<prop>`). A row with a `start` line
    and no result crashed its process; a retried table's later lines win."""
    res: dict[str, str] = {}
    msgs: dict[str, str] = {}
    if not path.exists():
        return res, msgs
    for line in path.read_text().splitlines():
        try:
            r = json.loads(line)
        except ValueError:
            continue  # a torn line of a killed process: its row has no result and stays pending
        if r["result"] == "start":
            res[r["test"]] = "crash"
            msgs[r["test"]] = "the test process ended inside this row (crash)"
        else:
            res[r["test"]] = r["result"]
            if r["result"] != "pass" and r.get("message"):
                msgs[r["test"]] = r["message"][:240]
            else:
                msgs.pop(r["test"], None)
    return res, msgs


def run_gtk(only: set[str] | None = None) -> tuple[dict[str, str], dict[str, str]]:
    """(test -> pass|fail, test -> failure message). `only`: just these generated tests. Table rows (`<test>#<row>`)
    run with their table and report through SVELTERS_TEST_RESULTS."""
    tables = sorted({t.split("#", 1)[0] for t in only}) if only else []
    expr = "test(/^generated::verify::/)" if not only else " | ".join(f"test(={t})" for t in tables)
    cmd = display_prefix() + ["cargo", "nextest", "run", "-p", "svelters-gtk", "--lib", "--no-fail-fast",
                              "--failure-output", "final", "-E", expr]
    with tempfile.TemporaryDirectory() as d:
        rows = Path(d) / "rows.jsonl"
        e = env()
        e["SVELTERS_TEST_RESULTS"] = str(rows)
        p = subprocess.run(cmd, cwd=ROOT, env=e, capture_output=True, text=True)
        row_res, row_msgs = row_results(rows)
    text = p.stdout + p.stderr
    res = {t: r for t, r in results_of(text).items() if t.startswith("generated::verify::")}
    if not res:
        sys.stderr.write(text[-6000:])
        raise SystemExit("ledger: no GTK test results (build failure?)")
    msgs = failure_messages(text)
    res.update(row_res)
    msgs.update(row_msgs)
    if only:
        return res, msgs
    # Hand-written kinds: integration tests named `<binary>::<fn>`.
    bins = sorted({t.split("::")[0] for f in gtk_extra_facts() for t in f["tests"]})
    cmd = display_prefix() + ["cargo", "nextest", "run", "-p", "svelters-gtk", "--no-fail-fast"] + [a for b in bins for a in ("--test", b)]
    p = subprocess.run(cmd, cwd=ROOT, env=env(), capture_output=True, text=True)
    text = p.stdout + p.stderr
    for m in re.finditer(r"^\s*(?:TRY \d+ )?(PASS|FLAKY|FAIL|TIMEOUT|LEAK|SIG\w+)\b[^\n]*?\]\s+(?:\(\s*(?:\d+/\d+|\W+)\)\s+)?svelters-gtk::(\w+) (\w+)\s*$", text, re.M):
        res[f"{m.group(2)}::{m.group(3)}"] = "pass" if m.group(1) in ("PASS", "FLAKY") else "fail"
    return res, msgs


def run_qt(only: set[str] | None = None) -> tuple[dict[str, str], dict[str, str]]:
    """(test -> pass|fail, test -> failure message). One binary runs every generated Qt test; `only` is ignored."""
    cmd = display_prefix() + ["cargo", "test", "-p", "counter-qt", "--test", "generated"]
    p = subprocess.run(cmd, cwd=ROOT, env=env(), capture_output=True, text=True)
    res, msgs = {}, {}
    for m in re.finditer(r"^(ok  |FAIL) (\S+?)(?::(.*)|$)", p.stdout, re.M):
        res[m.group(2)] = "pass" if m.group(1).startswith("ok") else "fail"
        if m.group(3):
            msgs[m.group(2)] = m.group(3).strip()[:240]
    if not res:
        sys.stderr.write(p.stdout[-4000:] + p.stderr[-4000:])
        raise SystemExit("ledger: no Qt test results (build failure?)")
    for line in p.stdout.splitlines():
        if line.startswith("FAIL "):
            print("  " + line, file=sys.stderr)
    return res, msgs


PARKED = {"gtk": ROOT / "metadata/inferred/gtk/_parked.json", "qt": ROOT / "metadata/inferred/qt/_parked.json",
          "appkit": ROOT / "metadata/inferred/appkit/_parked.json", "winui": ROOT / "metadata/inferred/winui/_parked.json"}
# What turns the merged metadata into the generated backend, in order (a parked fact leaves all of them).
# Observe facts (milestone R0) are merged by tools/metadata/common/observe.py into metadata/observe/<tk>.json, which
# leaves parked ones out; GTK's ingest.py (PyGObject) only runs when other facts were parked.
OBSERVE_MERGE = "tools/metadata/common/observe.py"
REGEN = {"gtk": [["tools/metadata/gtk/ingest.py"], [OBSERVE_MERGE, "--toolkit", "gtk"], ["tools/metadata/gtk/gen_backend.py"],
                 ["tools/metadata/common/gen_bindings.py"], ["tools/metadata/semantics/gen_semantics.py"]],
         # Qt: the generator runs in the counter-qt build (it reads _parked.json); the rest consumes the merged layers.
         "qt": [[OBSERVE_MERGE, "--toolkit", "qt"], ["tools/metadata/common/gen_bindings.py"], ["tools/metadata/semantics/gen_semantics.py"]],
         "appkit": [[OBSERVE_MERGE, "--toolkit", "appkit"], ["tools/metadata/appkit/gen_backend.py"], ["tools/metadata/common/gen_bindings.py"],
                    ["tools/metadata/semantics/gen_semantics.py"]],
         # winui: windows-bindgen (tools/metadata/winui/regen.sh) is not rerun; parking only removes members from the filter.
         "winui": [[OBSERVE_MERGE, "--toolkit", "winui"], ["tools/metadata/winui/gen_backend.py"], ["tools/metadata/common/gen_bindings.py"],
                   ["tools/metadata/semantics/gen_semantics.py"]]}
_parked_sections: set[str] = set()   # the sections whose parked facts the last park() changed


def load_parked(tk: str) -> list[dict]:
    p = PARKED.get(tk)
    return json.loads(p.read_text())["parked"] if p and p.exists() else []


def save_parked(tk: str, version: str, parked: list[dict]) -> None:
    parked = sorted(parked, key=lambda r: (r["kind"], r["section"], r["name"], r.get("test", "")))
    PARKED[tk].parent.mkdir(parents=True, exist_ok=True)
    PARKED[tk].write_text(json.dumps({"schema": "svelters-parked/v1", "toolkit": tk, "version": version, "parked": parked},
                                     indent=1, sort_keys=True, ensure_ascii=False) + "\n")


def regenerate(tk: str) -> None:
    for step in REGEN[tk]:
        if step[0].endswith("gtk/ingest.py") and _parked_sections <= {"observe"}:
            continue  # only observe facts changed: the merged kinds do not include them
        subprocess.run([sys.executable, str(ROOT / step[0])] + step[1:], cwd=ROOT, check=True)


def park(tk: str, man: dict, results: dict[str, str], msgs: dict[str, str], park_missing: bool = True) -> bool:
    """Park the inferred facts whose test failed (or that the generator could not test). True if the list changed.
    `park_missing=False` (import): a test without a result was not run there, so its fact stays pending."""
    old = load_parked(tk)
    keep = {(r["kind"], r["section"], r["name"]): r for r in old if r.get("version", man["version"]) == man["version"]}
    new = dict(keep)
    for f in man["facts"]:
        if f.get("origin") != "inferred":
            continue
        # A table row without a result was not reached (an earlier row crashed the table's process): it stays
        # pending and runs again once the crashing row is parked.
        bad = [t for t in f["tests"] if results.get(t, "missing") != "pass" and (park_missing and "#" not in t or t in results)]
        if bad:
            new[(f["kind"], f["section"], f["name"])] = {
                "fact": f["fact"], "kind": f["kind"], "section": f["section"], "name": f["name"], "record_fp": f.get("record_fp"),
                "test": bad[0], "message": msgs.get(bad[0]) or results.get(bad[0], "missing"), "version": man["version"]}
    for u in man.get("untestable", []):
        new[(u["kind"], u["section"], u["name"])] = {**u, "test": "(not generated)", "version": man["version"]}
    if new == keep and len(old) == len(keep):
        return False
    olds = {(r["kind"], r["section"], r["name"]): r for r in old}
    _parked_sections.clear()
    _parked_sections.update(k[1] for k in set(new) ^ set(olds))
    _parked_sections.update(k[1] for k in set(new) & set(olds) if new[k] != olds[k])
    save_parked(tk, man["version"], list(new.values()))
    return True


def read_results(path: Path) -> tuple[dict[str, str], dict[str, str]]:
    """(test -> pass|fail|skip, test -> failure message) from JSON lines, a JSON document or JUnit XML. A test
    reported more than once keeps its last result (a rerun)."""
    text = path.read_text()
    res: dict[str, str] = {}
    msgs: dict[str, str] = {}
    if text.lstrip().startswith("<"):
        import xml.etree.ElementTree as ET
        for tc in ET.fromstring(text).iter("testcase"):
            name = tc.get("name") or ""
            bad = tc.find("failure") if tc.find("failure") is not None else tc.find("error")
            if bad is not None:
                res[name] = "fail"
                msgs[name] = (bad.get("message") or (bad.text or "").strip() or "failed")[:240]
            else:
                res[name] = "skip" if tc.find("skipped") is not None else "pass"
        return res, msgs
    try:
        doc = json.loads(text)
        recs = doc if isinstance(doc, list) else doc["results"] if "results" in doc else [doc]
    except ValueError:
        recs = [json.loads(line) for line in text.splitlines() if line.strip()]
    for r in recs:
        res[r["test"]] = r["result"]
        if r["result"] != "pass" and r.get("message"):
            msgs[r["test"]] = r["message"][:240]
        else:
            msgs.pop(r["test"], None)
    return res, msgs


def record(tk: str, man: dict, results: dict[str, str], write: bool = True, pending_ok: bool = False) -> tuple[list[dict], int, int]:
    """Ledger entries for every test of the manifest: (entries, manual not passing, inferred not passing).
    `pending_ok` (import): an inferred fact without a result is left out of the ledger (pending), not recorded missing."""
    entries, bad, soft = [], 0, 0
    for f in man["facts"]:
        for t in f["tests"]:
            r = results.get(t, "missing")
            if r == "missing" and pending_ok and f.get("origin") == "inferred":
                continue
            if r != "pass":
                if f.get("origin") == "inferred":
                    soft += 1
                else:
                    bad += 1
            entries.append({"fact": f["fact"], "origin": f.get("origin", "manual"), "test": t, "result": r,
                            "fingerprint": f["fingerprint"], "version": man["version"]})
    if write:
        save(tk, man["version"], entries)
    return entries, bad, soft


def import_results(tk: str, path: Path, dry_run: bool = False) -> int:
    """Record results produced elsewhere (CI artifact) against the current facts manifest; park failing inferred
    facts and regenerate (their tests leave the manifest; the remaining facts keep their fingerprints)."""
    results, msgs = read_results(path)
    man = manifest(tk)
    known = {t for f in man["facts"] for t in f["tests"]}
    stray = sorted(set(results) - known)
    if dry_run:
        entries, bad, soft = record(tk, man, results, write=False, pending_ok=True)
        old = {(e["fact"], e["test"]): (e["result"], e["fingerprint"]) for e in load(tk)["entries"]}
        changed = sum(1 for e in entries if old.get((e["fact"], e["test"])) != (e["result"], e["fingerprint"]))
        fails = sorted(t for f in man["facts"] for t in f["tests"] if results.get(t, "missing") != "pass")
        print(f"ledger {tk} (dry run): {len(results)} results, {len(man['facts'])} facts, {bad} manual and {soft} inferred not passing, "
              f"{changed} entries would change, {len(stray)} results without a fact")
        for t in fails[:60]:
            print(f"  {results.get(t, 'missing')}: {t}" + (f": {msgs[t]}" if t in msgs else ""))
        return 0
    if park(tk, man, results, msgs, park_missing=False):
        regenerate(tk)
        man = manifest(tk)
    _, bad, soft = record(tk, man, results, pending_ok=True)
    if tk in GEN_AFTER_LEDGER:
        subprocess.run([sys.executable, str(ROOT / GEN_AFTER_LEDGER[tk])], cwd=ROOT, check=True)
    n = sum(len(f["tests"]) for f in man["facts"])
    print(f"ledger {tk}: imported {path.name}: {len(man['facts'])} facts, {n} tests, {bad} manual not passing, {soft} inferred not passing, "
          f"{len(load_parked(tk))} parked, {len(stray)} results without a fact")
    return 1 if bad else 0


def read_ci_results(path: Path) -> tuple[dict[str, str], dict[str, str], dict | None]:
    """(test -> pass|fail|crash, test -> message, the facts.json the run was built from or None) from a results file
    (one JSON line per test start and result) or a directory holding one (the unpacked CI artifact)."""
    if path.is_dir():
        found = sorted(path.rglob("*results*.jsonl"))
        if len(found) != 1:
            raise SystemExit(f"ledger: expected one *results*.jsonl under {path}, found {[str(f) for f in found]}")
        jsonl = found[0]
    else:
        jsonl = path
    snaps = sorted(jsonl.parent.rglob("facts.json"))
    snap = json.loads(snaps[0].read_text()) if snaps else None
    results: dict[str, str] = {}
    msgs: dict[str, str] = {}
    for line in jsonl.read_text().splitlines():
        if not line.strip():
            continue
        r = json.loads(line)
        if r["result"] == "start":
            results[r["test"]] = "crash"          # stays so if the process died inside the test
            msgs[r["test"]] = "the test process ended inside this test (crash or fail-fast)"
        else:
            results[r["test"]] = r["result"]
            if r.get("message"):
                msgs[r["test"]] = r["message"][:240]
            else:
                msgs.pop(r["test"], None)
    return results, msgs, snap


def import_ci(tk: str, path: Path) -> int:
    """Record a CI run of a CI-only toolkit: entries for the facts the run tested with their current fingerprint, park
    the failing inferred ones (regenerating without them), keep earlier entries for facts the run did not test."""
    if tk not in CI_ONLY:
        raise SystemExit(f"ledger: import is for the CI-only toolkits {sorted(CI_ONLY)}; {tk} has `refresh`")
    results, msgs, snap = read_ci_results(path)
    if not results:
        raise SystemExit(f"ledger: no test results in {path}")
    man = manifest(tk)
    if snap is not None and snap.get("version") != man["version"]:
        raise SystemExit(f"ledger: the run tested {tk} {snap.get('version')}, the metadata is {man['version']}")
    ran_fp = {f["fact"]: f["fingerprint"] for f in (snap or man)["facts"]}

    def tested(m: dict) -> dict:
        """The manifest restricted to facts the run tested as they are now (same fingerprint, every test has a result)."""
        return {**m, "facts": [f for f in m["facts"] if ran_fp.get(f["fact"]) == f["fingerprint"] and all(t in results for t in f["tests"])],
                "untestable": m.get("untestable", [])}

    stale = [f["fact"] for f in man["facts"] if f["fact"] in ran_fp and ran_fp[f["fact"]] != f["fingerprint"]]
    for _ in range(4):
        if tk not in PARKED or not park(tk, tested(man), results, msgs):
            break
        regenerate(tk)
        man = manifest(tk)
    old = {(e["fact"], e["test"]): e for e in load(tk)["entries"]}
    entries, bad, soft, pending = [], 0, 0, 0
    for f in man["facts"]:
        ok = ran_fp.get(f["fact"]) == f["fingerprint"]
        for t in f["tests"]:
            if ok and t in results:
                r = results[t]
                entries.append({"fact": f["fact"], "origin": f.get("origin", "manual"), "test": t, "result": r,
                                "fingerprint": f["fingerprint"], "version": man["version"]})
            elif (f["fact"], t) in old and old[(f["fact"], t)]["fingerprint"] == f["fingerprint"]:
                r = old[(f["fact"], t)]["result"]
                entries.append(old[(f["fact"], t)])
            else:
                pending += 1
                continue
            if r != "pass":
                if f.get("origin") == "inferred":
                    soft += 1
                else:
                    bad += 1
                    print(f"  {f['fact']}: {t}: {r}: {msgs.get(t, '')}", file=sys.stderr)
    save(tk, man["version"], entries)
    print(f"ledger {tk}: imported {len(results)} test results; {len(man['facts'])} facts, {len(entries)} entries, "
          f"{bad} manual not passing, {soft} inferred not passing, {pending} pending, {len(stale)} stale in the run, "
          f"{len(load_parked(tk))} parked")
    return 1 if bad else 0


# Generators whose output reads the ledger (typings mark unverified facts); rerun after it changes.
GEN_AFTER_LEDGER = {"gtk": "tools/metadata/gtk/gen_backend.py"}


def run_appkit() -> tuple[dict[str, str], dict[str, str]]:
    """The generated AppKit tests on this Mac (svelters_apple::testing writes the results file)."""
    if sys.platform != "darwin":
        raise SystemExit("ledger: AppKit tests run on macOS; elsewhere import the CI results (ledger.py import --toolkit appkit FILE)")
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "results.jsonl"
        e = env()
        e["SVELTERS_TEST_RESULTS"] = str(out)
        subprocess.run(display_prefix() + ["cargo", "test", "-p", "counter-mac", "--test", "generated"], cwd=ROOT, env=e)
        if not out.exists():
            raise SystemExit("ledger: no AppKit test results (build failure?)")
        return read_results(out)


def refresh(tk: str) -> int:
    if tk in CI_ONLY:
        raise SystemExit(f"ledger: {tk} tests run on CI only: download the job's artifact and run "
                         f"`ledger.py import --toolkit {tk} <dir>`")
    man = manifest(tk)
    if tk == "appkit":
        results, msgs = run_appkit()
        if park(tk, man, results, msgs):
            regenerate(tk)
            man = manifest(tk)
            results, msgs = run_appkit()
    else:
        run = run_gtk if tk == "gtk" else run_qt
        results, msgs = run()
        for _ in range(4):
            if not park(tk, man, results, msgs):
                break
            regenerate(tk)
            new = manifest(tk)
            old_fp = {f["fact"]: f["fingerprint"] for f in man["facts"]}
            redo = {t for f in new["facts"] if old_fp.get(f["fact"]) != f["fingerprint"] or any(t not in results for t in f["tests"])
                    for t in f["tests"]}
            man = new
            if redo:
                r2, m2 = run(redo)
                results.update(r2)
                msgs.update(m2)
    _, bad, soft = record(tk, man, results)
    if tk in GEN_AFTER_LEDGER:
        subprocess.run([sys.executable, str(ROOT / GEN_AFTER_LEDGER[tk])], cwd=ROOT, check=True)   # typings read the ledger
    n = sum(len(f["tests"]) for f in man["facts"])
    parked = len(load_parked(tk))
    print(f"ledger {tk}: {len(man['facts'])} facts, {n} tests, {bad} manual not passing, {soft} inferred not passing, {parked} parked")
    return 1 if bad else 0


PROBED = {"gtk", "qt", "appkit"}


def probe_problems(tk: str, version: str) -> list[str]:
    """The checked-in probe must be for the toolkit version the ledger was refreshed on."""
    if tk not in PROBED:
        return []
    p = ROOT / f"metadata/probe/{tk}/{version}.json"
    if not p.exists():
        have = sorted(x.stem for x in p.parent.glob("*.json"))
        return [f"no probe for {tk} {version} (have {have or 'none'}): re-run tools/metadata/{tk}/probe.py on that toolkit, then infer.py"]
    doc = json.loads(p.read_text())
    if doc.get("version") != version:
        return [f"{p.relative_to(ROOT)} says it probed {tk} {doc.get('version')}, not {version}"]
    return []


def check(tk: str) -> int:
    man = manifest(tk)
    led = {(e["fact"], e["test"]): e for e in load(tk)["entries"]}
    problems = probe_problems(tk, load(tk).get("version") or man["version"])
    if load(tk).get("version") not in (None, man["version"]):
        problems.append(f"the ledger was refreshed on {tk} {load(tk)['version']}, the metadata is {man['version']} (refresh)")
    fix = (f"the windows-winui CI job, then `ledger.py import --toolkit {tk} <its artifact>`" if tk in CI_ONLY
           else f"`ledger.py refresh --toolkit {tk}`")
    inferred = {"pass": 0, "pending": 0, "failing": 0}
    for f in man["facts"]:
        for t in f["tests"]:
            e = led.get((f["fact"], t))
            if f.get("origin") == "inferred":
                # Strict for manual facts only. A failing inferred fact is parked by `refresh`, so one that is still
                # merged and fails with its current fingerprint means a refresh was skipped.
                if e is None or e["fingerprint"] != f["fingerprint"]:
                    inferred["pending"] += 1
                elif e["result"] != "pass":
                    inferred["failing"] += 1
                    problems.append(f"{f['fact']}: {t}: inferred fact failed and is not parked (run {fix})")
                else:
                    inferred["pass"] += 1
                continue
            if e is None:
                problems.append(f"{f['fact']}: {t}: not in the ledger (run {fix})")
            elif e["fingerprint"] != f["fingerprint"]:
                problems.append(f"{f['fact']}: {t}: fact or test changed since it was verified (run {fix})")
            elif e["result"] != "pass":
                problems.append(f"{f['fact']}: {t}: {e['result']}")
    if problems:
        print(f"ledger {tk}: {len(problems)} problem(s):", file=sys.stderr)
        for p in problems:
            print("  " + p, file=sys.stderr)
        return 1
    manual = sum(1 for f in man["facts"] if f.get("origin") != "inferred")
    extra = f"; inferred {inferred['pass']} verified, {inferred['pending']} pending, {len(load_parked(tk))} parked" if tk in PARKED else ""
    print(f"ledger {tk}: {manual} manual facts verified{extra}")
    return 0


def verified_facts(tk: str) -> set[str]:
    """Fact ids whose every test passed with the current fingerprint (for docs and coverage).
    Reads the ledger only; fingerprints are checked by `check`."""
    by: dict[str, bool] = {}
    for e in load(tk)["entries"]:
        by[e["fact"]] = by.get(e["fact"], True) and e["result"] == "pass"
    return {f for f, ok in by.items() if ok}


def toolkits_all() -> list[str]:
    """gtk and qt, plus each toolkit with a facts manifest whose results arrive by import once it has a ledger."""
    return ["gtk", "qt"] + [tk for tk, p in FACTS.items() if tk not in ("gtk", "qt") and p.exists() and (LEDGER_DIR / f"{tk}.json").exists()]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["refresh", "check", "import"])
    ap.add_argument("results", nargs="?", type=Path, help="import: the test-results file (JSON lines, JSON or JUnit XML), or for winui the unpacked CI artifact directory")
    ap.add_argument("--toolkit", default="all", choices=["gtk", "qt", "appkit", "winui", "all"])
    ap.add_argument("--dry-run", action="store_true", help="import: report what would change, write nothing")
    a = ap.parse_args()
    if a.cmd == "import":
        if a.toolkit in CI_ONLY and a.results is not None:
            return import_ci(a.toolkit, a.results)
        if a.toolkit not in FACTS or a.toolkit == "gtk" or a.results is None:
            ap.error("import needs --toolkit appkit and a results file")
        return import_results(a.toolkit, a.results, a.dry_run)
    rc = 0
    # `refresh --toolkit all` runs what runs here; CI-only toolkits (and AppKit off macOS) are imported instead.
    local = lambda t: t not in CI_ONLY and (t != "appkit" or sys.platform == "darwin")  # noqa: E731
    for tk in ([t for t in toolkits_all() if a.cmd != "refresh" or local(t)] if a.toolkit == "all" else [a.toolkit]):
        rc |= (refresh if a.cmd == "refresh" else check)(tk)
    return rc


if __name__ == "__main__":
    sys.exit(main())
