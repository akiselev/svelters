#!/usr/bin/env python3
"""AppKit runtime prober (AI1 slice 4): metadata/probe/appkit/<SDK version>.json.

Runs on macOS (the macos-appkit CI job, or a maintainer's Mac). For each ingested class it builds a
plan from metadata/appkit/*.json, the merged annotations (layers.py) and metadata/rules/appkit.yaml,
then runs the prober binary (crates/svelters-appkit/examples/appkit_probe.rs, built with the
`exceptions` feature so Objective-C exceptions become errors) once per class:

- instantiate the class's subject element (the class's own tag, else the first element deriving
  from it: where the generated tests check its facts);
- per writable prop of a probe-able type: read the default, observe the KVC key with KVO, set the
  first candidate sample that differs from the default, read it back, and record the outcome
  (exact / normalized / ignored / error / unreadable), whether KVO fired, and which listened
  target-action or notification signals the programmatic set emitted (an echo);
- per class: whether its target-action and notification signals deliver (sendAction:to: and a
  posted notification), and whether the notification constants named by the rules exist in the
  loaded AppKit with the expected value.

Crash isolation: the binary streams one JSON line per result. If it dies (a native crash, an
uncaught exception, a hang past --timeout), the prop in flight is recorded as `crash` and the
class is rerun from the prop after it. Output is deterministic for a given machine (sorted keys,
no times or paths); the runtime macOS version is recorded next to the SDK version.

  python3 tools/metadata/appkit/probe.py [--binary PATH] [--out DIR] [--class NSButton ...]
  python3 tools/metadata/appkit/probe.py --plan-only plan.json      # any OS: just the plan

Needs pyyaml and jsonschema (like gen_backend.py).
"""
from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "common"))
import gen_backend  # noqa: E402
import svmeta  # noqa: E402

ROOT = HERE.parents[2]
RULES = ROOT / "metadata/rules/appkit.yaml"
OUT = ROOT / "metadata/probe/appkit"
SCHEMA = "svelters-probe-appkit/v1"
INTS = {"i32", "i64", "u32", "u64"}
FLOATS = {"f32", "f64"}
STRUCTS = {"rect", "size", "point", "insets"}
SIGNAL_VIAS = {"target_action": "send_action", "notification": "post"}


def load_rules(path: Path = RULES) -> dict:
    return svmeta.load_yaml(path)


def denied(rules: dict, m: gen_backend.Model, cls: str, js: str) -> str | None:
    for d in rules.get("deny_props", []):
        if d.get("class") and d["class"] not in m.chain(cls):
            continue
        if re.match(d["pattern"], js):
            return d["reason"]
    return None


def prop_enum(m: gen_backend.Model, cls: str, js: str) -> list[tuple[str, int]] | None:
    """Members of an override `enums` entry (an integer prop with names) along the chain."""
    for c in m.chain(cls):
        for e in m.ov.get(c, {}).get("enums", []):
            if e["prop"] == js:
                return sorted(e["members"].items(), key=lambda kv: kv[1])
    return None


def candidates(rules: dict, m: gen_backend.Model, cls: str, p: dict) -> list[dict] | None:
    """Typed candidate samples for a prop (the prober's plan format), or None if it is not probed."""
    ref = p["type"]["ref"]
    smp = rules["samples"]
    named = prop_enum(m, cls, p["js_name"])
    if named:
        return [{"t": "str", "v": n} for n, _ in named]
    if ref == "bool":
        return [{"t": "bool", "v": v} for v in smp["bool"]]
    if ref in INTS:
        return [{"t": "i64", "v": v} for v in smp["int"] if not (ref.startswith("u") and v < 0)]
    if ref in FLOATS:
        return [{"t": "f64", "v": float(v)} for v in smp["float"]]
    if ref == "string":
        return [{"t": "str", "v": v} for v in smp["string"]]
    if ref in STRUCTS:
        fields = gen_backend.STRUCT_FIELDS[ref][1]
        return [{"t": ref, "v": [float(s[f]) for f in fields]} for s in smp[ref]]
    if ref.startswith(("enum<", "flags<")):
        name = ref[ref.index("<") + 1:-1]
        e = m.enums.get(name)
        if not e or not e["members"]:
            return None
        if ref.startswith("enum<"):
            return [{"t": "str", "v": js} for js, _ in e["members"]]
        return [{"t": "names", "v": [js], "expect": {"t": "i64", "v": v}} for js, v in e["members"] if v != 0][:3]
    return None          # objects, lists, selectors, ranges: no round trip


def subject_class(m: gen_backend.Model, tag: str) -> str:
    return next(c for c in m.classes if (m.ov.get(c, {}).get("element") or {}).get("tag") == tag)


def binding_source(rules: dict, m: gen_backend.Model, cls: str) -> dict | None:
    """The nearest binding source (rules `binding_sources`) in the chain of `cls`."""
    for c in m.chain(cls):
        for s in rules.get("binding_sources", []):
            if s["class"] == c:
                return s
    return None


def build_plan(m: gen_backend.Model, rules: dict, only: set[str] | None = None) -> dict:
    classes = []
    for cls in m.classes:
        if only and cls not in only:
            continue
        tag = m.element_for(cls)
        if tag is None:
            continue
        own = {p["js_name"]: p for p in m.meta[cls]["properties"]}
        names = sorted(own)
        is_element = bool((m.ov.get(cls, {}).get("element") or {}).get("tag"))
        if is_element and binding_source(rules, m, cls):
            # value props declared on a base are probed on the element itself (binding evidence and samples)
            for js in rules.get("binding_props", []):
                if js not in own and m.find_prop(cls, js)[1] is not None:
                    names.append(js)
        props = []
        for js in names:
            _, p = m.find_prop(cls, js)
            if not p["access"]["write"] or denied(rules, m, cls, js):
                continue
            cands = candidates(rules, m, cls, p)
            if not cands:
                continue
            rec = {"name": js, "key": p["name"], "samples": cands}
            if js not in own:
                rec["inherited"] = True
            props.append(rec)
        subj = subject_class(m, tag)
        sigs = []
        for c in m.chain(subj):
            for s in m.ov.get(c, {}).get("signals", []):
                if s["via"] in SIGNAL_VIAS and s["name"] not in [x["signal"] for x in sigs]:
                    sigs.append({"signal": s["name"], "how": SIGNAL_VIAS[s["via"]], "notification": s.get("notification", "")})
        classes.append({"class": cls, "subject": tag, "subject_class": subj, "props": props,
                        "listen": [s["signal"] for s in sigs], "deliver": sigs,
                        "notifications": list(rules.get("notifications", {}).get(cls, []))})
    return {"schema": "svelters-probe-plan-appkit/v1", "version": m.version, "classes": classes}


# ---- running ---------------------------------------------------------------------------------------

def build_binary() -> Path:
    cmd = ["cargo", "build", "-p", "svelters-appkit", "--features", "exceptions", "--example", "appkit_probe",
           "--message-format=json-render-diagnostics"]
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    sys.stderr.write(p.stderr[-4000:])
    if p.returncode != 0:
        raise SystemExit("probe: building the prober failed")
    for line in p.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("target", {}).get("name") == "appkit_probe" and msg.get("executable"):
            return Path(msg["executable"])
    raise SystemExit("probe: cargo did not report the appkit_probe executable")


def run_once(binary: Path, plan_path: Path, cls: str, after: str | None, timeout: float, skip_class: bool = False) -> tuple[list[dict], str | None]:
    """The JSON lines of one prober run and, if it did not finish, why."""
    cmd = [str(binary), "--plan", str(plan_path), "--class", cls] + (["--after", after] if after else []) + (["--skip-class"] if skip_class else [])
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        out, err, rc = p.stdout, p.stderr, p.returncode
        why = None if rc == 0 else (f"signal {-rc}" if rc < 0 else f"exit {rc}")
    except subprocess.TimeoutExpired as e:
        out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        err = e.stderr.decode() if isinstance(e.stderr, bytes) else (e.stderr or "")
        why = f"timeout {timeout:.0f}s"
    lines = []
    for line in out.splitlines():
        try:
            lines.append(json.loads(line))
        except ValueError:
            continue
    if why is None and not any(x.get("done") for x in lines):
        why = "no completion line"
    if why:
        tail = [x for x in err.strip().splitlines() if x.strip()][-3:]
        why = why + (": " + " | ".join(tail)[:300] if tail else "")
    return lines, why


def scrub(text: str) -> str:
    """Drop machine paths and addresses from messages, so outputs compare across machines."""
    text = re.sub(r"0x[0-9a-fA-F]{6,}", "0x…", text)
    return re.sub(r"(/[\w.@+-]+){2,}", "<path>", text)


def probe_class(binary: Path, plan_path: Path, c: dict, timeout: float, runner=run_once) -> dict:
    """Runs the prober for one class with crash isolation; returns the class record."""
    rec: dict = {"subject": c["subject"], "props": {}}
    order = [p["name"] for p in c["props"]]
    after = None
    skip_class = False
    for _ in range(len(order) + 3):
        lines, why = runner(binary, plan_path, c["class"], after, timeout, skip_class)
        in_flight = None
        for x in lines:
            if "created" in x:
                rec.update({k: x[k] for k in ("native_class", "signals", "notifications") if k in x})
                rec["created"] = True
            elif x.get("start"):
                in_flight = x["prop"]
            elif "result" in x:
                r = x["result"]
                if isinstance(r.get("error"), str):
                    r["error"] = scrub(r["error"])      # exception messages carry object addresses
                rec["props"][x["prop"]] = r
                in_flight = None
        if why is None:
            break
        if after is None and not skip_class and "created" not in rec and in_flight is None:
            # the class-level part (instantiation, signal delivery) died: record it, still probe the props
            rec["crash"] = scrub(why)
            skip_class = True
            continue
        if in_flight is None:
            # died between props: the next one not reported
            pending = [n for n in order if n not in rec["props"]]
            if not pending:
                break
            in_flight = pending[0]
        rec["props"][in_flight] = {"outcome": "crash", "error": scrub(why)}
        after = in_flight
    for p in c["props"]:
        if p.get("inherited") and p["name"] in rec["props"]:
            rec["props"][p["name"]]["inherited"] = True
    return rec


def sw_vers() -> dict:
    out = {"arch": platform.machine()}
    try:
        out["macos"] = subprocess.run(["sw_vers", "-productVersion"], capture_output=True, text=True).stdout.strip()
    except FileNotFoundError:
        pass
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", help="a built appkit_probe (default: cargo build it with the exceptions feature)")
    ap.add_argument("--out", default=str(OUT), help="directory for <version>.json (default metadata/probe/appkit)")
    ap.add_argument("--class", dest="classes", action="append", help="only these classes (repeatable); merged into an existing file")
    ap.add_argument("--timeout", type=float, default=120.0, help="seconds per prober run")
    ap.add_argument("--plan-only", metavar="FILE", help="write the plan and stop (works on any OS)")
    a = ap.parse_args()
    m = gen_backend.Model()
    rules = load_rules()
    plan = build_plan(m, rules, set(a.classes) if a.classes else None)
    if a.plan_only:
        Path(a.plan_only).write_text(json.dumps(plan, indent=1, sort_keys=True) + "\n")
        print(f"probe: plan for {len(plan['classes'])} classes, {sum(len(c['props']) for c in plan['classes'])} props")
        return 0
    if sys.platform != "darwin":
        print("probe: AppKit probing runs on macOS (use --plan-only elsewhere)", file=sys.stderr)
        return 1
    binary = Path(a.binary) if a.binary else build_binary()
    out_path = Path(a.out) / f"{m.version}.json"
    doc = {"schema": SCHEMA, "toolkit": "appkit", "version": m.version, "sdk": f"MacOSX{m.version}", "runtime": sw_vers(),
           "classes": {}}
    if a.classes and out_path.exists():
        doc["classes"] = json.loads(out_path.read_text()).get("classes", {})
    with tempfile.TemporaryDirectory() as d:
        plan_path = Path(d) / "plan.json"
        plan_path.write_text(json.dumps(plan))
        for c in plan["classes"]:
            rec = probe_class(binary, plan_path, c, a.timeout)
            doc["classes"][c["class"]] = rec
            outs = {}
            for r in rec["props"].values():
                outs[r.get("outcome")] = outs.get(r.get("outcome"), 0) + 1
            print(f"probe: {c['class']} on <{c['subject']}>: " + ", ".join(f"{k} {v}" for k, v in sorted(outs.items()))
                  + (f"; CRASH {rec['crash']}" if rec.get("crash") else ""), file=sys.stderr)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=1, sort_keys=True, ensure_ascii=False) + "\n")
    print(f"probe: wrote {out_path.relative_to(ROOT) if out_path.is_relative_to(ROOT) else out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
