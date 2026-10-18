#!/usr/bin/env python3
"""Summarize a prober output (gtk_prober.py / qt_prober.py) and score it against the overrides.

  python3 docs/notes/scripts/annot/summarize_probe.py probe-gtk.json [--verbose]
"""
from __future__ import annotations

import argparse
import collections
import glob
import json
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[4]
NOISE = {"state-flags-changed", "notify::css-classes", "direction-changed", "notify::has-focus", "notify::visible"}


def gtk_overrides() -> dict:
    return {Path(p).stem: yaml.safe_load(open(p)) or {} for p in glob.glob(str(ROOT / "metadata/overrides/gtk/*.yaml"))}


def summarize_gtk(doc: dict, verbose: bool) -> None:
    ov = gtk_overrides()
    c = collections.Counter()
    trig = collections.Counter()
    disagreements = []
    for kind, k in sorted(doc["kinds"].items()):
        for cr in k.get("crashes", []):
            c["op:crash"] += 1
            if verbose:
                print("  crash op:", cr["op"], cr["why"])
        if k.get("crash") or k.get("error"):
            c["kind:" + ("crash" if k.get("crash") else "error")] += 1
            if verbose:
                print("  !!", kind, k.get("crash") or k.get("error"))
            continue
        c["kind:ok"] += 1
        o = ov.get(kind, {})
        annotated = {p["name"] for p in o.get("properties", []) if not p.get("virtual")}
        for n, p in k["props"].items():
            out = p.get("outcome")
            c[f"prop:{out}"] += 1
            if out == "exact":
                c["prop:exact_notify" if p.get("notify") else "prop:exact_no_notify"] += 1
                if p.get("recipe"):
                    c[f"prop:recipe:{p['recipe']}"] += 1
            sigs = [s for s in p.get("signals") or [] if s not in NOISE and not s.startswith("notify::")]
            if sigs:
                trig["props_with_side_signals"] += 1
                for s in sigs:
                    trig[f"{kind}.{s}<-{n}"] += 0  # keep key space small
                trig["side_signal_edges"] += len(sigs)
            if n in annotated:
                c["prop:annotated"] += 1
                c[f"prop:annotated:{out}"] += 1
                if out not in ("exact", "identity"):
                    disagreements.append(f"{kind}.{n}: annotated, probe {out} {p.get('log') or p.get('read')}")
        for n, m in k["methods"].items():
            c[f"method:{m['class']}:{m.get('outcome', '-')}"] += 1
            if m["class"] == "action" and m.get("outcome") == "ok":
                c["method:action:observable" if (m.get("changed") or [s for s in m.get("signals", []) if s not in NOISE]) else "method:action:no_effect"] += 1
        for n, s in k["slots"].items():
            c[f"slot:{s.get('role')}"] += 1
            if s.get("role") == "slot":
                c[f"slot:old_on_replace:{s.get('old_on_replace')}"] += 1
        osl = {s["name"]: json.dumps(s.get("ops", {})) for s in (o.get("children") or {}).get("slots", [])}
        probe_ops = set()
        for n, s in k["slots"].items():
            if s.get("role") == "slot":
                probe_ops.add("set_" + n.replace("-", "_"))
            elif s.get("role") == "many_slot":
                probe_ops.add(s["insert"])
        probe_slot_names = sorted(probe_ops)
        for s, ops in osl.items():
            c["slot:annotated"] += 1
            rem = {x["remove"] for x in k["slots"].values() if x.get("role") == "many_slot" and x.get("remove")}
            hit = any(re.search(r"::" + op + r"\(", ops) for op in probe_ops) or \
                any(re.search(r'"remove": \{"native": "\w+::' + r + r"\(", ops) for r in rem)
            c["slot:annotated:found" if hit else "slot:annotated:missed"] += 1
            if not hit and verbose:
                print("  slot missed:", kind, s, sorted(probe_slot_names))
        for n, s in k["signals"].items():
            c["signal:total"] += 1
            if s.get("emit"):
                c[f"signal:emit:{s['emit'][:13]}"] += 1
        # signals with a discovered property trigger
        fired = {s for p in k["props"].values() for s in (p.get("signals") or [])}
        for n in k["signals"]:
            if n in fired:
                c["signal:trigger_found(prop set)"] += 1
            elif k["signals"][n].get("emit") == "delivered":
                c["signal:trigger_found(emit)"] += 1
    for key, v in sorted(c.items()):
        print(f"  {key:44} {v}")
    print(f"  side-signal edges (prop set -> other signal): {trig['side_signal_edges']} on {trig['props_with_side_signals']} props")
    if disagreements:
        print("  annotated props the probe could not round-trip:")
        for d in disagreements:
            print("    ", d)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("probe")
    ap.add_argument("--verbose", action="store_true")
    a = ap.parse_args()
    doc = json.loads(Path(a.probe).read_text())
    print(f"== {doc['toolkit']} {doc.get('version')}: {len(doc['kinds'])} kinds")
    if doc["toolkit"] == "gtk":
        summarize_gtk(doc, a.verbose)
    else:
        summarize_qt(doc, a.verbose)
    return 0


def summarize_qt(doc: dict, verbose: bool) -> None:
    """Agreement of the Qt probe with the hand-written Qt overrides (props, slot effects)."""
    c = collections.Counter()
    for f in sorted(glob.glob(str(ROOT / "metadata/overrides/qt/*.yaml"))):
        ov = yaml.safe_load(open(f)) or {}
        groups = [(e.get("construct", {}).get("class", Path(f).stem), e) for e in ov.get("elements", [])]
        if ov.get("inherit"):
            groups.append((Path(f).stem, ov["inherit"]))
        for cls, e in groups:
            probes = [k for k in doc["kinds"].values()]
            for p in e.get("props", []):
                nat = p.get("native")
                hit = next((k["props"][nat] for k in probes if nat in k.get("props", {}) and
                            (k["kind"] == cls or k["kind"] == Path(f).stem)), None)
                if hit is None:
                    hit = next((k["props"][nat] for k in probes if nat in k.get("props", {})), None)
                if hit is None:
                    c["prop:annotated:not_probed"] += 1
                    continue
                c[f"prop:annotated:{hit.get('outcome')}"] += 1
                if hit.get("outcome") not in ("exact",) and verbose:
                    print(f"    {cls}.{nat}: annotated, probe {hit.get('outcome')} {hit.get('read')!r:.60}")
            for s_ in (e.get("children") or {}).get("slots", []):
                ops = s_.get("ops", {})
                st = ops.get("set") or ops.get("insert") or {}
                nat = re.sub(r".*::(\w+)\(.*", r"\1", st.get("native", ""))
                eff = (st.get("effects") or {}).get("old")
                k = doc["kinds"].get(cls, {})
                pr = k.get("slots", {}).get(nat) or k.get("slots", {}).get(f"<{nat}>")
                if not pr:
                    c["slot:annotated:not_probed"] += 1
                    continue
                c["slot:annotated:probed"] += 1
                if eff and pr.get("old_on_replace"):
                    same = {"deleted": "deleted", "unparented": "unparented"}.get(eff) == pr["old_on_replace"]
                    c["slot:effect_agrees" if same else "slot:effect_differs"] += 1
                    if not same and verbose:
                        print(f"    {cls}.{nat}: annotated old={eff}, probe {pr['old_on_replace']}")
    for key, v in sorted(c.items()):
        print(f"  {key:44} {v}")


if __name__ == "__main__":
    sys.exit(main())
