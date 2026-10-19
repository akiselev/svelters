#!/usr/bin/env python3
"""API-surface coverage report: metadata/coverage/{gtk,qt}.md (and a machine-readable .json).

Per kind: exposed as an element (or why not), props/signals/methods exposed out of the kind's own
total, slots annotated, ownership annotated, signal results annotated, bindings annotated,
verified facts out of the facts (manual and inferred), and whether a gallery page shows it.

Every fact is in one state: verified (manual), verified (inferred), parked (an inferred fact whose
generated test failed), pending (inferred, no passing test yet) or not covered. A signal whose test
emits it synthetically is `verified: delivery`: delivery, listen-off and destroy are checked, not
native causation. The inferred candidates below the promotion threshold are counted from
metadata/inferred/<tk>/_review.json.

What "exposed" means differs per toolkit, so each toolkit has a small adapter:
- GTK: properties and signals of every element are reachable by reflection (dynamic GObject
  access). A property counts as exposed when the backend can convert its type; a signal counts
  as exposed when it is reachable. Methods count only when annotated (no GI invocation yet).
- Qt: only annotated facts exist in the generated backend, so exposed = annotated on an element
  (directly or through an `inherit:` block declared on this class).

Deterministic: reads metadata/, the ledger and the gallery pages; writes sorted output.

  python3 tools/metadata/coverage.py [--check]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE / "common"))
sys.path.insert(0, str(HERE))
import svmeta  # noqa: E402
import ledger  # noqa: E402

OUT = ROOT / "metadata/coverage"
GTK_CONVERTIBLE = ("bool", "i32", "u32", "i64", "u64", "f32", "f64", "string", "enum", "flags", "object")
PAGE_KINDS = re.compile(r"<!--\s*kinds:\s*([^>]*?)\s*-->")


def gallery_kinds(tk: str) -> dict[str, str]:
    """Kind name -> gallery page, from the `<!-- kinds: A B -->` header of each page."""
    out = {}
    for p in sorted((ROOT / f"fixtures/gallery-{tk}/src/pages").glob("*.svelte")):
        m = PAGE_KINDS.search(p.read_text())
        for k in (m.group(1).split() if m else []):
            out.setdefault(k, p.stem)
    return out


def ratio(a: int, b: int) -> str:
    return f"{a}/{b}" if b else "-"


# ---- GTK ---------------------------------------------------------------------------------------

def gtk_rows() -> list[dict]:
    meta = ROOT / "metadata/gtk"
    docs = {p.stem: svmeta.load_json(p) for p in sorted(meta.glob("*.json"))}
    man = json.loads(ledger.GTK_FACTS.read_text())
    verified = ledger.verified_facts("gtk")
    facts_by_kind: dict[str, list[str]] = {}
    for f in man["facts"]:
        facts_by_kind.setdefault(f["kind"], []).append(f["fact"])
    info = {f["fact"]: f for f in man["facts"]}
    pages = gallery_kinds("gtk")
    rows = []
    for name, d in docs.items():
        if d["kind"] not in ("object", "interface"):
            continue
        chain = ["Gtk." + name] + d.get("bases", [])
        widgetish = "Gtk.Widget" in chain or d["kind"] == "interface" and any(
            name == x for x in ("Editable", "Orientable", "Actionable", "Scrollable", "CellEditable", "AppChooser", "ColorChooser", "FileChooser", "FontChooser"))
        if not widgetish:
            continue
        props = d.get("properties", [])
        conv = [p for p in props if p["type"]["ref"].split("<")[0] in GTK_CONVERTIBLE and not p.get("deprecated")]
        sigs = d.get("signals", [])
        meths = d.get("methods", [])
        facts = facts_by_kind.get(name, [])
        inh = d.get("inherited", {})
        rows.append({
            "kind": name, "native": d["native"]["gtype"], "abstract": d["abstract"] or d["kind"] == "interface",
            "element": (d.get("element") or {}).get("tag"), "unexposed": (d.get("unexposed") or {}).get("reason"),
            "props": [len(conv), len(props)], "props_annotated": sum(1 for p in props if "annotated" in p),
            "signals": [len(sigs), len(sigs)], "methods": [sum(1 for m in meths if "annotated" in m), len(meths)],
            "slots": len(d.get("children", {}).get("slots", [])) + len(inh.get("slots", [])),
            "slots_own": len(d.get("children", {}).get("slots", [])),
            "ownership": bool(d.get("children", {}).get("slots")) or (d.get("lifetime") or {}).get("origin") == "manual",
            "signal_results": sum(1 for s in sigs if "handler_result" in (s.get("annotated") or {}).get("fields", [])),
            "bindings": len(d.get("bindings", [])) + len(inh.get("bindings", [])),
            "verified": [sum(1 for f in facts if f in verified), len(facts)],
            "verified_inferred": [sum(1 for f in facts if f in verified and info[f].get("origin") == "inferred"),
                                  sum(1 for f in facts if info[f].get("origin") == "inferred")],
            "delivery": sum(1 for f in facts if f in verified and info[f].get("strength") == "delivery"),
            "inherited": sum(len(v) for v in inh.values()),
            "gallery": pages.get(name),
            "lifetime": (d.get("lifetime") or {}).get("class"),
        })
    return rows


def gtk_fact_table() -> dict:
    """Facts by section and state: manual / inferred verified, delivery-only, pending; parked and unreviewed counts."""
    man = json.loads(ledger.GTK_FACTS.read_text())
    verified = ledger.verified_facts("gtk")
    sections: dict[str, dict[str, int]] = {}
    for f in man["facts"]:
        row = sections.setdefault(f["section"], {"manual": 0, "inferred": 0, "delivery": 0, "pending": 0})
        origin = f.get("origin", "manual")
        if f["fact"] in verified:
            row[origin] += 1
            row["delivery"] += f.get("strength") == "delivery"
        else:
            row["pending"] += 1
    parked: dict[str, int] = {}
    for r in ledger.load_parked("gtk"):
        parked[r["section"]] = parked.get(r["section"], 0) + 1
    review = {}
    rp = ROOT / "metadata/inferred/gtk/_review.json"
    if rp.exists():
        review = json.loads(rp.read_text()).get("unreviewed", {})
    return {"sections": dict(sorted(sections.items())), "parked": dict(sorted(parked.items())), "unreviewed": review}


# ---- Qt ----------------------------------------------------------------------------------------

def qt_rows() -> list[dict]:
    meta = ROOT / "metadata/qt"
    ovdir = ROOT / "metadata/overrides/qt"
    docs = {p.stem: svmeta.load_json(p) for p in sorted(meta.glob("*.json"))}
    ovs = {p.stem: svmeta.load_yaml(p) for p in sorted(ovdir.glob("*.yaml"))}
    man = ledger.qt_facts()
    verified = ledger.verified_facts("qt")
    pages = gallery_kinds("qt")
    rows = []
    for name, d in docs.items():
        chain = [name] + d.get("bases", [])
        if not d.get("qobject") or not ("QWidget" in chain or "QLayout" in chain):
            continue
        ov = ovs.get(name, {})
        els = ov.get("elements", [])
        inh = ov.get("inherit", {})
        own_props = {p["name"] for p in d["properties"]}
        own_sigs = {s["name"] for s in d["signals"]}
        own_meths = {m["name"] for m in d["methods"]}
        ap = {p.get("native") for e in els for p in e.get("props", []) if p.get("native") in own_props} | \
             {p.get("native") for p in inh.get("props", []) if p.get("native") in own_props}
        asig = {s.get("native") for e in els for s in e.get("signals", []) if s.get("native") in own_sigs} | \
               {s.get("native") for s in inh.get("signals", []) if s.get("native") in own_sigs}
        am = {m.get("native") for e in els for m in e.get("methods", []) if m.get("native") in own_meths} | \
             {m.get("native") for m in inh.get("methods", []) if m.get("native") in own_meths}
        prefix = f"svelters-api:v1:type:qt:QtWidgets:{name}:"
        facts = [f["fact"] for f in man["facts"] if f["fact"].startswith(prefix)]
        slots = [s for e in els for s in (e.get("children") or {}).get("slots", [])]
        rows.append({
            "kind": name, "native": name, "abstract": bool(d.get("abstract")),
            "element": ", ".join(e["tag"] for e in els) or None, "unexposed": (ov.get("unexposed") or {}).get("reason"),
            "props": [len(ap), len(own_props)], "props_annotated": len(ap),
            "signals": [len(asig), len(own_sigs)], "methods": [len(am), len(own_meths)],
            "slots": len(slots), "slots_own": len(slots), "ownership": any("effects" in str(s.get("ops")) for s in slots),
            "signal_results": sum(1 for e in els for s in e.get("signals", []) if s.get("synthetic")),
            "bindings": sum(len(e.get("bindings", [])) for e in els),
            "verified": [sum(1 for f in facts if f in verified), len(facts)],
            "inherited": sum(len(v) for v in inh.values()),
            "gallery": pages.get(name),
            "lifetime": (d.get("lifetime") or {}).get("class"),
            "defaults": sum(1 for p in d["properties"] if p.get("default") is not None),
        })
    return rows


# ---- output ------------------------------------------------------------------------------------

def render(tk: str, rows: list[dict]) -> str:
    rows = sorted(rows, key=lambda r: r["kind"])
    concrete = [r for r in rows if not r["abstract"]]
    exposed = [r for r in concrete if r["element"]]
    tot = lambda key, i: sum(r[key][i] for r in rows)
    vf, vt = tot("verified", 0), tot("verified", 1)
    o = [f"# {tk.upper()} API coverage\n",
         "Generated by `tools/metadata/coverage.py` from the metadata, the overrides, the verified ledger "
         f"(`metadata/verified/{tk}.json`) and the gallery pages. Do not edit.\n",
         "## Summary\n",
         "| Measure | Value |", "| --- | --- |",
         f"| Concrete widget kinds exposed as elements | {len(exposed)}/{len(concrete)} |",
         f"| Kinds with annotations (slots, props, signals, methods or bindings) | {sum(1 for r in rows if r['props_annotated'] or r['slots'] or r['bindings'] or r['methods'][0] or r['verified'][1])} |",
         f"| Properties exposed | {tot('props', 0)}/{tot('props', 1)} |",
         f"| Signals exposed | {tot('signals', 0)}/{tot('signals', 1)} |",
         f"| Methods exposed | {tot('methods', 0)}/{tot('methods', 1)} |",
         f"| Child slots annotated (own, not inherited) | {sum(r['slots_own'] for r in rows)} |",
         f"| Two-way bindings | {sum(r['bindings'] for r in rows)} |",
         f"| Facts verified (manual and inferred) | {vf}/{vt} |",
         f"| Kinds on a gallery page | {sum(1 for r in rows if r['gallery'])} |",
         ""]
    if tk == "gtk":
        ft = gtk_fact_table()
        o += ["## Facts by state\n",
              "| Section | Verified (manual) | Verified (inferred) | of which `verified: delivery` | Pending | Parked |",
              "| --- | --- | --- | --- | --- | --- |"]
        for sec, r in ft["sections"].items():
            o.append(f"| {sec} | {r['manual']} | {r['inferred']} | {r['delivery'] or '-'} | {r['pending'] or '-'} | {ft['parked'].get(sec, '-')} |")
        tot = lambda k: sum(r[k] for r in ft["sections"].values())  # noqa: E731
        o.append(f"| **all** | {tot('manual')} | {tot('inferred')} | {tot('delivery') or '-'} | {tot('pending') or '-'} | {sum(ft['parked'].values()) or '-'} |\n")
        un = ft["unreviewed"]
        if un:
            o += ["Inferred candidates below the promotion threshold stay unreviewed "
                  f"({un.get('middle', 0)} middle band, {un.get('low', 0)} low band); by pattern:\n",
                  "| Band and pattern | Candidates |", "| --- | --- |"]
            o += [f"| `{k}` | {v} |" for k, v in un.get("by_pattern", {}).items()]
            o.append("")
        o.append("GTK exposes every property whose type the backend converts and every signal by reflection, and "
                 "every introspectable method through GIRepository (X1; no out params, arrays or callbacks). The "
                 "methods column counts annotated methods, which have generated tests; the annotated and verified "
                 "columns say which facts are tested. Properties without a passing test are marked `@unverified` in the typings.\n")
    else:
        o.append("Qt exposes only annotated facts (the generated C++ backend has no reflection fallback); "
                 f"{sum(r.get('defaults', 0) for r in rows)} properties carry a typed default read from a default-constructed instance.\n")
    o += ["## Kinds\n",
          "| Kind | Element | Props | Signals | Methods | Slots | Ownership | Signal results | Bindings | Verified | Gallery | Lifetime |",
          "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for r in rows:
        el = f"`<{r['element']}>`" if r["element"] else ("abstract" if r["abstract"] else (f"no: {r['unexposed']}" if r["unexposed"] else "no"))
        if r["element"] and "," in r["element"]:
            el = ", ".join(f"`<{t.strip()}>`" for t in r["element"].split(","))
        props = ratio(*r["props"]) + (f" ({r['props_annotated']} annotated)" if tk == "gtk" and r["props_annotated"] else "")
        o.append(f"| {r['kind']} | {el} | {props} | {ratio(*r['signals'])} | {ratio(*r['methods'])} | {r['slots'] or '-'} | "
                 f"{'yes' if r['ownership'] else '-'} | {r['signal_results'] or '-'} | {r['bindings'] or '-'} | {ratio(*r['verified'])} | "
                 f"{r['gallery'] or '-'} | {r['lifetime'] or '-'} |")
    return "\n".join(o) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if the checked-in reports are stale")
    a = ap.parse_args()
    stale = []
    for tk, rows in (("gtk", gtk_rows()), ("qt", qt_rows())):
        for path, text in ((OUT / f"{tk}.md", render(tk, rows)),
                           (OUT / f"{tk}.json", json.dumps(sorted(rows, key=lambda r: r["kind"]), indent=1, sort_keys=True) + "\n")):
            if not path.exists() or path.read_text() != text:
                stale.append(path)
                if not a.check:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(text)
    if a.check and stale:
        for p in stale:
            print(f"coverage: {p.relative_to(ROOT)} is stale (run tools/metadata/coverage.py)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
