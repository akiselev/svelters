#!/usr/bin/env python3
"""Static inference rules over the ingested metadata, scored against the hand-written overrides.

Rules (see plans/research/ANNOTATION-AUTOMATION.md for the catalogue):
  M*  method classes: prop accessor, pure getter, zero-arg action, scalar-arg call, child op,
      unsupported, dangerous (denylist), protected/virtual (Qt), deprecated
  B1  two-way binding candidate: writable prop + change signal (notify / NOTIFY / paired *-changed)
  B2  primary binding pick: Qt USER prop; GTK value/active/text/selected stem
  S1  one-slot candidate: writable widget-typed prop with get/set accessors
  S2  many-slot candidate: add/remove|insert/remove|append/prepend/pack families taking a widget
  T1  text prop: first writable string prop named label/text/title/content
  R1  signal result: bool return -> bool_stop + sync; other non-void -> value + sync
Prints per-toolkit counts and recall/precision against the overrides.

Usage: python3 docs/notes/scripts/annot/infer_static.py [--emit DIR]  (DIR gets <tk>/<Kind>.yaml drafts)
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
SCALAR = re.compile(r"^(bool|i32|u32|i64|u64|f32|f64|string|enum<|flags<)")
DANGEROUS_GTK = re.compile(r"^(run|.*_run|main|block.*|grab_add|dispose|destroy|realize|unrealize|map|unmap|"
                           r"measure|size_allocate|allocate|snapshot.*|queue_\w+|init_template|dispose_template|"
                           r"set_parent|unparent|insert_after|insert_before|.*_with_time|present_with_time|"
                           r"compute_\w+|translate_coordinates|pick|contains|get_allocat\w+|get_baseline|"
                           r"get_height|get_width|get_size)$")
DANGEROUS_QT = re.compile(r"^(exec|open|.*Event|event|eventFilter|paint\w*|metaObject|qt_\w+|setParent|"
                          r"create|destroy|winId|effectiveWinId|internalWinId|nativeEvent|initPainter|"
                          r"redirected|sharedPainter|grab\w*|releaseKeyboard|releaseMouse|releaseShortcut|"
                          r"setVisible|getOpen\w+|getSave\w+|getExisting\w+|getInt|getDouble|getItem|getText|"
                          r"getMultiLineText|getColor|getFont|information|warning|critical|question|about\w*)$")
TEXT_NAMES = ("label", "text", "title", "content")
VALUE_STEMS = {"active": ["toggled", "notify::active"], "value": ["value-changed"], "text": ["changed", "notify::text"],
               "selected": ["notify::selected"], "page": ["switch-page", "notify::page"]}


def load(tk: str) -> dict[str, dict]:
    return {Path(p).stem: json.loads(Path(p).read_text()) for p in sorted(glob.glob(str(ROOT / f"metadata/{tk}/*.json")))
            if not Path(p).stem.startswith("_")}


# ------------------------------------------------------------------------------------------- GTK

def gtk_widgetish(name: str, d: dict) -> bool:
    if d.get("kind") not in ("object", "interface"):
        return False
    chain = ["Gtk." + name] + d.get("bases", [])
    return "Gtk.Widget" in chain or d["kind"] == "interface" and name in (
        "Editable", "Orientable", "Actionable", "Scrollable", "CellEditable", "AppChooser", "ColorChooser",
        "FileChooser", "FontChooser")


def gtk_chain_props(docs: dict, name: str) -> dict[str, dict]:
    out: dict[str, dict] = {}
    d = docs.get(name)
    for k in [name] + [b.split(".", 1)[1] for b in (d or {}).get("bases", []) if b.startswith("Gtk.")] + \
             [i.split(".", 1)[1] for i in (d or {}).get("interfaces", []) if i.startswith("Gtk.")]:
        for p in docs.get(k, {}).get("properties", []):
            out.setdefault(p["name"], p)
    return out


def classify_gtk_method(m: dict, props: dict[str, dict]) -> str:
    n = m["name"]
    if m.get("deprecated"):
        return "deprecated"
    if m.get("unsupported_reason"):
        return "unsupported"
    if DANGEROUS_GTK.match(n):
        return "dangerous"
    pt = [p["type"] for p in m["params"]]
    rt = m["returns"].get("type", "void")
    acc = {p["accessors"].get("get") for p in props.values()} | {p["accessors"].get("set") for p in props.values()}
    if m["native"] in acc:
        return "prop_accessor"
    mm = re.match(r"(get|set|is|has)_(\w+)$", n)
    if mm and mm.group(2).replace("_", "-") in props:
        return "prop_accessor"
    if any("Gtk:Widget" in t or t.startswith("object<") and "Gtk:" in t and m["name"].split("_")[0] in
           ("add", "remove", "insert", "append", "prepend", "pack", "attach", "put", "move", "set", "reorder") for t in pt):
        return "child_op"
    if not pt and rt != "void" and re.match(r"(get|is|has|n|count)_?|.*_count$", n):
        return "pure_getter"
    if not pt and rt == "void":
        return "action"
    if pt and all(SCALAR.match(t) for t in pt):
        return "scalar_call"
    if any(t.startswith("list<") or t.startswith("callback") for t in pt):
        return "unsupported"
    return "object_call"


def gtk_infer(docs: dict, overrides: dict) -> dict:
    res = collections.Counter()
    per_kind: dict[str, dict] = {}
    for name, d in docs.items():
        if not gtk_widgetish(name, d):
            continue
        props = gtk_chain_props(docs, name)
        own = {p["name"]: p for p in d.get("properties", [])}
        kind: dict = {"methods": collections.Counter(), "bindings": [], "slots": [], "text_prop": None, "signals": []}
        for m in d.get("methods", []):
            c = classify_gtk_method(m, props)
            kind["methods"][c] += 1
            res["method:" + c] += 1
        # B1 bindings: writable, readable, scalar, notify, own prop
        sigs = {s["name"]: s for s in d.get("signals", [])}
        for p in own.values():
            a = p["access"]
            if a["read"] and a["write"] and not a["construct_only"] and SCALAR.match(p["type"]["ref"]) and p.get("notify"):
                change = next((s for s in VALUE_STEMS.get(p["name"], []) if s in sigs), f"notify::{p['name']}")
                kind["bindings"].append({"property": p["name"], "change": change,
                                         "primary": p["name"] in VALUE_STEMS})
        # S1 one-slots: writable widget-typed prop with accessors; reference-like names excluded
        for p in own.values():
            t = p["type"]["ref"]
            if t.startswith("object<") and ("Gtk:Widget" in t or "Gtk:Popover" in t) and p["access"]["write"] and \
                    p["accessors"].get("set") and not re.search(r"default|focus|mnemonic|transient|relative|attach|"
                                                                r"buddy|key-capture|target|entry", p["name"]):
                kind["slots"].append({"name": p["name"], "cardinality": "one", "set": p["accessors"]["set"],
                                      "get": p["accessors"].get("get")})
        # S2 many-slots
        ms = {m["name"] for m in d.get("methods", [])}
        for ins in ("append", "add", "add_child", "insert", "pack_start", "pack_end", "add_overlay", "put", "attach",
                    "append_page", "insert_child_after", "add_named", "add_titled", "prepend"):
            if ins in ms:
                rem = next((r for r in ("remove", "remove_child", "remove_overlay", "remove_page") if r in ms), None)
                if rem:
                    kind["slots"].append({"name": ins, "cardinality": "many", "insert": ins, "remove": rem})
        # T1
        for p in own.values():
            if p["type"]["ref"] == "string" and p["access"]["write"] and p["name"] in TEXT_NAMES:
                kind["text_prop"] = p["name"]
                break
        # R1
        for s in d.get("signals", []):
            rt = s.get("returns", {}).get("type", "void")
            kind["signals"].append((s["name"], "none" if rt == "void" else "bool_stop" if rt == "bool" else "value",
                                    rt != "void"))
        per_kind[name] = kind
    # score against overrides
    score = collections.Counter()
    for name, ov in overrides.items():
        k = per_kind.get(name)
        if not k:
            continue
        cand_b = {b["property"]: b for b in k["bindings"]}
        for b in ov.get("bindings", []):
            score["binding:annotated"] += 1
            if b["property"] in cand_b:
                score["binding:recalled"] += 1
                if cand_b[b["property"]]["change"] == b["change"]["signal"]:
                    score["binding:same_change_signal"] += 1
        slot_names = {s["name"] for s in k["slots"]}
        slot_set = {s.get("set", "") for s in k["slots"]} | {s.get("insert", "") for s in k["slots"]}
        for s in (ov.get("children") or {}).get("slots", []):
            score["slot:annotated"] += 1
            nat = json.dumps(s.get("ops", {}))
            hit = s["name"].replace("_", "-") in slot_names or any(
                x and re.search(r"::" + re.escape(re.sub(r"^gtk_\w+?_(?=(set|add|append|insert|pack|put|attach))", "", x)) + r"\(", nat)
                for x in slot_set)
            score["slot:recalled"] += hit
            if not hit:
                score["slot:missed:" + name + "." + s["name"]] += 1
        if ov.get("text_prop"):
            score["text_prop:annotated"] += 1
            score["text_prop:recalled"] += k["text_prop"] == ov["text_prop"]["name"]
    res.update(score)
    return {"totals": res, "kinds": per_kind}


# -------------------------------------------------------------------------------------------- Qt

def qt_chain(docs: dict, name: str) -> list[dict]:
    out, d, seen = [], docs.get(name), set()
    while d and d["native"]["name"] not in seen:
        seen.add(d["native"]["name"])
        out.append(d)
        b = d.get("bases") or []
        d = docs.get(b[0]) if b else None
    return out


def classify_qt_method(m: dict) -> str:
    n = m["name"]
    if m.get("deprecated"):
        return "deprecated"
    if m.get("access") != "public":
        return "protected"
    if m.get("static"):
        return "static"
    if DANGEROUS_QT.match(n) or m.get("returns", {}).get("native", "").endswith("*") and "Q" not in m["returns"]["native"]:
        return "dangerous"
    if m.get("associated_property"):
        return "prop_accessor"
    pt = [p["type"]["ref"] for p in m["params"]]
    rt = m["returns"].get("ref", m["returns"].get("type", "void"))
    if any("QWidget" in t or "QLayout" in t or "QAction" in t for t in pt) and re.match(
            r"(add|insert|remove|set|take|replace)", n):
        return "child_op"
    if not pt and m.get("const") and rt not in ("void", None):
        return "pure_getter"
    if not pt and rt in ("void", None):
        return "action"
    if pt and all(SCALAR.match(t or "") for t in pt):
        return "scalar_call"
    if any(p.get("default") is None and not SCALAR.match(p["type"]["ref"] or "") for p in m["params"]):
        return "object_call"
    return "scalar_call"


def qt_infer(docs: dict, overrides: dict) -> dict:
    res = collections.Counter()
    per = {}
    for name, d in docs.items():
        chain = [name] + d.get("bases", [])
        if not d.get("qobject") or not ("QWidget" in chain or "QLayout" in chain):
            continue
        k = {"methods": collections.Counter(), "bindings": [], "text_prop": None, "props": 0, "signals": 0}
        seen = set()
        for m in d["methods"]:
            if m["name"] in seen:      # count method names, like coverage.py
                continue
            seen.add(m["name"])
            c = classify_qt_method(m)
            k["methods"][c] += 1
            res["method:" + c] += 1
        allp = {}
        for x in qt_chain(docs, name):
            for p in x["properties"]:
                allp.setdefault(p["name"], p)
        for p in d["properties"]:
            mo = p.get("metaobject", {})
            res["prop:total"] += 1
            if mo.get("writable") and SCALAR.match(p["type"]["ref"] or ""):
                res["prop:scalar_writable"] += 1
            if mo.get("notify") and mo.get("writable"):
                k["bindings"].append({"property": p["name"], "change": mo["notify"].split("(")[0],
                                      "primary": bool(mo.get("user"))})
        for p in allp.values():
            if (p["type"]["ref"] == "string") and p.get("metaobject", {}).get("writable") and p["name"] in TEXT_NAMES:
                k["text_prop"] = p["name"]
                break
        per[name] = k
    score = collections.Counter()
    for name, ov in overrides.items():
        for e in ov.get("elements", []):
            cls = e.get("construct", {}).get("class", name)
            cands = {}
            for x in qt_chain(docs, cls):
                for b in per.get(x["native"]["name"], {}).get("bindings", []):
                    cands.setdefault(b["property"], b)
            pmap = {p["js"]: p.get("native") for p in e.get("props", [])}
            for b in e.get("bindings", []):
                score["binding:annotated"] += 1
                nat = pmap.get(b["property"], b["property"])
                c = cands.get(nat)
                score["binding:recalled"] += bool(c)
                score["binding:primary(USER)"] += bool(c and c["primary"])
    res.update(score)
    return {"totals": res, "kinds": per}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    a = ap.parse_args()
    out = {}
    gdocs = load("gtk")
    gov = {Path(p).stem: yaml.safe_load(open(p)) or {} for p in glob.glob(str(ROOT / "metadata/overrides/gtk/*.yaml"))}
    g = gtk_infer(gdocs, gov)
    qdocs = load("qt")
    qov = {Path(p).stem: yaml.safe_load(open(p)) or {} for p in glob.glob(str(ROOT / "metadata/overrides/qt/*.yaml"))}
    q = qt_infer(qdocs, qov)
    for tk, r in (("gtk", g), ("qt", q)):
        print(f"== {tk}: {len(r['kinds'])} widget kinds")
        tot = r["totals"]
        mt = sum(v for k, v in tot.items() if k.startswith("method:"))
        for k, v in sorted(tot.items()):
            if k.startswith("method:"):
                print(f"  {k:28} {v:5d}  {100*v/mt:5.1f}%")
        for k, v in sorted(tot.items()):
            if not k.startswith("method:"):
                print(f"  {k:40} {v}")
        nb = sum(len(k["bindings"]) for k in r["kinds"].values())
        print(f"  binding candidates (B1): {nb}; primary (B2): {sum(1 for k in r['kinds'].values() for b in k['bindings'] if b['primary'])}")
        if tk == "gtk":
            print(f"  slot candidates: one={sum(1 for k in r['kinds'].values() for s in k['slots'] if s['cardinality']=='one')} "
                  f"many={sum(1 for k in r['kinds'].values() for s in k['slots'] if s['cardinality']=='many')}")
        print(f"  text_prop candidates: {sum(1 for k in r['kinds'].values() if k['text_prop'])}")
        out[tk] = {"totals": dict(tot)}
    if a.json:
        Path(a.json).write_text(json.dumps(out, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
