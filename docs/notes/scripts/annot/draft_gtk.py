#!/usr/bin/env python3
"""Turn a GTK probe (gtk_prober.py) + the ingested metadata into draft overrides with confidence.

Writes <out>/<Kind>.yaml in the existing GTK override schema (so gen_backend.py could consume it as
an inferred layer) and <out>/<Kind>.evidence.json (confidence + evidence per fact). Each draft is
validated against tools/metadata/gtk/override.schema.json, then compared with the hand-written
override of the same kind: facts the human wrote that the draft reproduces, facts only the human
has, facts only the draft has.

Confidence (0..1): 0.95 probe and rule agree (exact round trip with notify; accessor equals the
property; slot parented + read back + cleared); 0.8 probe-only (needed a recipe; trigger found by a
side signal); 0.6 rule-only (synthetic emit for delivery; action with no visible effect);
anything with a crash or GLib critical is not drafted (it goes to the denylist/review list).

Usage: python3 docs/notes/scripts/annot/draft_gtk.py probe-gtk.json OUTDIR
"""
from __future__ import annotations

import collections
import json
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import svmeta  # noqa: E402

SCHEMA = ROOT / "tools/metadata/gtk/override.schema.json"
RECIPE_SETUP = {
    "range": 'h.call_args(o, "set_range", &[V::F64(0.0), V::F64(100.0)]).unwrap();',
    "text": 'h.set_str(o, "text", "hello world").unwrap();',
}
NOISE = {"state-flags-changed", "direction-changed"}
# slots are generated as typed gtk4-rs calls: members newer than the crate's feature baseline
# (crates/svelters-gtk/Cargo.toml: gtk features ["v4_6"]) need the runtime-dlsym path instead
BASELINE = (4, 6)


def newer_than_baseline(since: str | None) -> bool:
    return bool(since) and tuple(int(x) for x in since.split(".")[:2]) > BASELINE


def gir_method_versions(path: str = "/usr/share/gir-1.0/Gtk-4.0.gir") -> dict[str, str]:
    """c:identifier -> version. The ingested method records do not carry `since` yet."""
    import xml.etree.ElementTree as ET
    C = "{http://www.gtk.org/introspection/c/1.0}identifier"
    out = {}
    for _, el in ET.iterparse(path):
        if el.tag.endswith("}method") and el.get(C) and el.get("version"):
            out[el.get(C)] = el.get("version")
    return out


METHOD_SINCE = gir_method_versions()


def rust_v(v) -> str | None:
    if isinstance(v, bool):
        return f"V::Bool({str(v).lower()})"
    if isinstance(v, int):
        return f"V::I64({v})"
    if isinstance(v, float):
        return f"V::F64({v!r})"
    if isinstance(v, str):
        return f'V::Str({json.dumps(v)}.into())'
    return None


def slot_name(prop: str) -> str:
    return re.sub(r"-(child|widget)$", "", prop)


def draft_kind(kind: str, k: dict, meta: dict) -> tuple[dict, dict]:
    ov: dict = {}
    ev: dict = {}
    gname = meta["native"]["gtype"]
    props_meta = {p["name"]: p for p in meta.get("properties", [])}
    sig_meta = {s["name"]: s for s in meta.get("signals", [])}
    # properties
    props = []
    for n, p in sorted(k.get("props", {}).items()):
        if p.get("outcome") not in ("exact", "identity") or n not in props_meta:
            continue
        rec = {"name": n, "provenance": [{"origin": "probe", "ref": f"round trip {p.get('outcome')}"
                                          + (f" with recipe {p['recipe']}" if p.get("recipe") else "")
                                          + ("; notify fired" if p.get("notify") else "")}]}
        conf = 0.95 if p.get("outcome") == "exact" and p.get("notify") and not p.get("recipe") else 0.8
        if p.get("recipe") in RECIPE_SETUP:
            rec["setup"] = RECIPE_SETUP[p["recipe"]]
        elif p.get("recipe"):
            conf = 0.5            # recipe without a Rust equivalent yet: needs review
        side = sorted(s for s in p.get("signals") or [] if s not in NOISE and not s.startswith("notify::"))
        if side and p.get("notify") and p.get("outcome") == "exact":   # echo tests need a scalar sample
            rec["echo_suppress"] = sorted({f"notify::{n}", *side})
        props.append(rec)
        ev[f"prop:{n}"] = {"confidence": conf, "evidence": p}
    if props:
        ov["properties"] = props
    # signals
    fired_by: dict[str, tuple[str, dict]] = {}
    for n, p in k.get("props", {}).items():
        if p.get("outcome") == "exact":
            for s in p.get("signals") or []:
                fired_by.setdefault(s, (n, p))
    sigs = []
    for n, s in sorted(k.get("signals", {}).items()):
        if n not in sig_meta or s.get("emit") == "crash":
            continue
        rec = {"name": n, "handler_result": s["handler_result"], "sync_required": s["sync_required"],
               "provenance": [{"origin": "runtime", "ref": f"returns {s['returns']}, run-{s['run']}"}]}
        conf = 0.95
        if n in fired_by:
            prop, p = fired_by[n]
            rec["trigger"] = {"property": prop, "values": [p["sample"], p["default"], p["sample"]]}
            rec["provenance"].append({"origin": "probe", "ref": f"emitted when {prop} is set"})
        elif s.get("emit") == "delivered":
            rec["provenance"].append({"origin": "probe", "ref": "action signal, emitted and delivered"})
        else:
            conf = 0.6            # no native trigger found; delivery-only test by synthetic emission
        sigs.append(rec)
        ev[f"signal:{n}"] = {"confidence": conf, "evidence": s}
    if sigs:
        ov["signals"] = sigs
    # methods: accessors and pure getters with probe evidence
    meths = []
    for n, m in sorted(k.get("methods", {}).items()):
        if m.get("log"):
            continue
        if m["class"] == "accessor_set" and m.get("outcome") == "exact" and rust_v(m.get("sample")) is not None:
            rec = {"name": n, "dispatch": "dynamic", "args": [m["sample"]],
                   "expect": f'h.get(o, "{m["prop"]}") == {rust_v(m["sample"])}',
                   "provenance": [{"origin": "probe", "ref": f"set_ then read {m['prop']}: exact"}]}
            if m.get("recipe") in RECIPE_SETUP:
                rec["setup"] = RECIPE_SETUP[m["recipe"]]
            meths.append(rec)
            ev[f"method:{n}"] = {"confidence": 0.95, "evidence": m}
        elif m["class"] == "accessor_get" and m.get("outcome") == "exact":
            meths.append({"name": n, "dispatch": "dynamic", "args": [], "expect": f'r == h.get(o, "{m["prop"]}")',
                          "provenance": [{"origin": "probe", "ref": f"equals property {m['prop']}"}]})
            ev[f"method:{n}"] = {"confidence": 0.95, "evidence": m}
        elif m["class"] == "action" and m.get("outcome") == "ok" and m.get("changed"):
            (prop, (_, after)), = list(m["changed"].items())[:1]
            if rust_v(after) is not None:
                meths.append({"name": n, "dispatch": "dynamic", "args": [], "expect": f'h.get(o, "{prop}") == {rust_v(after)}',
                              "provenance": [{"origin": "probe", "ref": f"changes {prop} to {after!r}"}]})
                ev[f"method:{n}"] = {"confidence": 0.8, "evidence": m}
    if meths:
        ov["methods"] = meths
    # slots
    slots = []
    for n, s in sorted(k.get("slots", {}).items()):
        setter_c = (props_meta.get(n) or {}).get("accessors", {}).get("set") or ""
        if s.get("role") == "slot" and newer_than_baseline(METHOD_SINCE.get(setter_c) or
                                                           (meta.get("availability") or {}).get("since")):
            ev[f"slot:{slot_name(n)}"] = {"confidence": 0.5, "evidence": s, "note": "newer than the typed baseline"}
        elif s.get("role") == "slot" and s.get("readback") and s.get("clear_unparents") and not n.startswith("<"):
            setter = f"{gname}::set_{n.replace('-', '_')}(GtkWidget*)"
            getter = f"{gname}::get_{n.replace('-', '_')}() const"
            slots.append({
                "name": slot_name(n), "content": "widget", "accepts": ["Gtk.Widget"], "route_by_kind": False,
                "cardinality": "one", "order": "none", "default": {"kind": "none"}, "on_remove": "empty",
                "attrs": [], "requires": [],
                "ops": {"get": {"native": getter},
                        "set": {"native": setter, "args": ["child"], "effects": {"child": "parented", "old": "unparented"}},
                        "remove": {"native": setter, "args": [None], "effects": {"child": "unparented"}}},
                "verify": {},
                "provenance": [{"origin": "probe", "ref": f"set parents the child ({s['parent']}"
                                + (f", revealed by {s['revealed_by']}" if s.get("revealed_by") else "")
                                + f"); old child {s['old_on_replace']} on replace; set(None) unparents"}]})
            ev[f"slot:{slot_name(n)}"] = {"confidence": 0.95 if s["parent"] == "direct" else 0.8, "evidence": s}
        elif s.get("role") == "reference":
            ev[f"reference:{n}"] = {"confidence": 0.9, "evidence": s}
        elif s.get("role") == "many_slot" and s.get("remove_unparents") and all(x == "direct" for x in s["parent"]):
            ev[f"slot:<{s['insert']}>"] = {"confidence": 0.8, "evidence": s,
                                           "note": "many-slot candidate; op args (prev_sibling/index) need the family table"}
    if slots:
        ov["children"] = {"policy": "single_child" if len(slots) == 1 and slots[0]["name"] == "child" else "named_slots",
                          "default_slot": next((s["name"] for s in slots if s["name"] in ("child", "content")), slots[0]["name"]),
                          "slots": slots}
    return ov, ev


def main() -> int:
    probe = json.loads(Path(sys.argv[1]).read_text())
    out = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    c = collections.Counter()
    for kind, k in sorted(probe["kinds"].items()):
        mp = ROOT / f"metadata/gtk/{kind}.json"
        if not mp.exists() or k.get("crash") or k.get("error"):
            continue
        ov, ev = draft_kind(kind, k, json.loads(mp.read_text()))
        if not ov:
            continue
        f = out / f"{kind}.yaml"
        f.write_text(yaml.safe_dump(ov, sort_keys=False, width=140))
        (out / f"{kind}.evidence.json").write_text(json.dumps(ev, indent=1, sort_keys=True))
        errs = svmeta.check_override(f, SCHEMA)
        c["drafts"] += 1
        c["schema_errors"] += len(errs)
        for e in errs[:2]:
            print("  schema:", e)
        for key, e in ev.items():
            t = key.split(":")[0]
            c[f"{t}:drafted"] += 1
            c[f"{t}:conf>=0.9" if e["confidence"] >= 0.9 else f"{t}:conf<0.9"] += 1
        # compare with the hand-written override
        hp = ROOT / f"metadata/overrides/gtk/{kind}.yaml"
        if hp.exists():
            h = yaml.safe_load(hp.read_text()) or {}
            for sec in ("properties", "signals", "methods"):
                hn = {r["name"] for r in h.get(sec, []) if not r.get("virtual") and not r.get("derived_from")}
                dn = {r["name"] for r in ov.get(sec, [])}
                c[f"{sec}:manual"] += len(hn)
                c[f"{sec}:manual_reproduced"] += len(hn & dn)
            hs = {s["name"] for s in (h.get("children") or {}).get("slots", []) if s.get("cardinality") == "one"}
            ds = {s["name"] for s in (ov.get("children") or {}).get("slots", [])}
            c["one_slots:manual"] += len(hs)
            c["one_slots:manual_reproduced(same name)"] += len(hs & ds)
            for s in sorted(hs - ds):
                c[f"one_slots:manual_only:{kind}.{s}"] += 1
    for key, v in sorted(c.items()):
        print(f"  {key:52} {v}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
