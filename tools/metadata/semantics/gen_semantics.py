#!/usr/bin/env python3
"""Generate the per-backend native semantic specifications (schema `svelters-semantics/v0`).

Inputs (nothing else; output is byte-identical on rerun):
  GTK: metadata/gtk/*.json through tools/metadata/gtk/gen_backend.py's Model (overrides already
       merged), cross-checked against crates/svelters-gtk/src/generated/tables.rs;
  Qt:  metadata/overrides/qt/*.yaml resolved against metadata/qt/*.json by
       tools/metadata/qt/gen_cpp.py's Model (only annotated elements exist at runtime);
  both: metadata/semantics/rules/{gtk,qt}.yaml (hand-written facts, each with a provenance).
Outputs: metadata/semantics/{gtk,qt}.json.

  python3 tools/metadata/semantics/gen_semantics.py           # write
  python3 tools/metadata/semantics/gen_semantics.py --check   # fail if the outputs are stale

Format and rules: docs/notes/native-compiler/semantic-spec.md.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools/metadata"
RULES = ROOT / "metadata/semantics/rules"
OUT = ROOT / "metadata/semantics"
GTK_TABLES = ROOT / "crates/svelters-gtk/src/generated/tables.rs"
SCHEMA = "svelters-semantics/v0"

sys.path.insert(0, str(TOOLS / "common"))
import svmeta  # noqa: E402


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gen_backend = _load("svelters_gen_backend", TOOLS / "gtk/gen_backend.py")
gen_cpp = _load("svelters_gen_cpp", TOOLS / "qt/gen_cpp.py")


class SpecError(Exception):
    pass


def fail(msg: str):
    raise SpecError(msg)


# ---- shared ----------------------------------------------------------------------------------

PROP_TYPE = {"bool": "bool", "i32": "i32", "u32": "u32", "i64": "i64", "u64": "i64", "f32": "f64", "f64": "f64",
             "string": "string", "enum": "enum", "flags": "flags", "object": "object", "list": "list"}
LIFETIMES = {"owned", "toplevel"}
ECHO = {"backend_blocks", "runtime_compare", "none"}
VALUE_KINDS = {"enum", "length", "bool"}
CONTRACTS = {"common", "native_intent"}


def ptype(ref: str) -> str:
    return PROP_TYPE.get(ref.split("<")[0], "other")


def inner(ref: str) -> str:
    return ref[ref.index("<") + 1:-1]


def need_prov(where: str, rec: dict) -> str:
    p = rec.get("provenance")
    if not p:
        fail(f"{where}: missing provenance")
    return p


def check_keys(where: str, rec: dict, allowed: set[str]) -> None:
    extra = sorted(set(rec) - allowed)
    if extra:
        fail(f"{where}: unknown field(s) {extra}")


def compact(v) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def render(doc: dict) -> str:
    """Top-level keys one per line; `kinds` and `tags` one entry per line (diffable, compact)."""
    lines = ["{"]
    keys = sorted(doc)
    for i, k in enumerate(keys):
        comma = "," if i < len(keys) - 1 else ""
        v = doc[k]
        if k in ("kinds", "tags") and isinstance(v, dict) and v:
            lines.append(f"{compact(k)}:{{")
            sub = sorted(v)
            for j, sk in enumerate(sub):
                lines.append(f"  {compact(sk)}:{compact(v[sk])}{',' if j < len(sub) - 1 else ''}")
            lines.append("}" + comma)
        else:
            lines.append(f"{compact(k)}:{compact(v)}{comma}")
    lines.append("}")
    return "\n".join(lines) + "\n"


def style_section(rules: dict, where: str, kind_exists, prop_on, coerce_ok) -> dict:
    """Validate and normalise `style` rules. `prop_on(applies_to, prop)` returns the property record
    (as emitted) if some kind matching `applies_to` has it, else None."""
    st = rules.get("style") or {}
    check_keys(f"{where}.style", st, {"units", "declarations"})
    units = {}
    for u, rec in sorted((st.get("units") or {}).items()):
        check_keys(f"{where}.style.units.{u}", rec, {"scale", "provenance"})
        if not isinstance(rec.get("scale"), (int, float)) or rec["scale"] <= 0:
            fail(f"{where}.style.units.{u}: scale must be a positive number")
        units[u] = {"scale": rec["scale"], "provenance": need_prov(f"{where}.style.units.{u}", rec)}
    decls = {}
    for name, rec in sorted((st.get("declarations") or {}).items()):
        w = f"{where}.style.declarations.{name}"
        check_keys(w, rec, {"prop", "value", "applies_to", "contract", "intents", "unsupported", "provenance"})
        prov = need_prov(w, rec)
        if rec.get("contract") not in CONTRACTS:
            fail(f"{w}: contract must be one of {sorted(CONTRACTS)}")
        if not kind_exists(rec.get("applies_to", "")):
            fail(f"{w}: applies_to {rec.get('applies_to')!r} is not a kind or class of this backend")
        out = {"applies_to": rec["applies_to"], "contract": rec["contract"], "provenance": prov}
        if rec["contract"] == "common":
            if rec.get("value") not in VALUE_KINDS:
                fail(f"{w}: value must be one of {sorted(VALUE_KINDS)}")
            p = prop_on(rec["applies_to"], rec.get("prop", ""))
            if p is None:
                fail(f"{w}: no kind matching {rec['applies_to']} has property {rec.get('prop')!r}")
            want = {"enum": {"enum"}, "length": {"i32", "u32", "i64", "f64"}, "bool": {"bool"}}[rec["value"]]
            if p["type"] not in want:
                fail(f"{w}: value {rec['value']} needs a {sorted(want)} property, {rec['prop']} is {p['type']}")
            if not p["writable"]:
                fail(f"{w}: property {rec['prop']} is read-only")
            out.update(prop=p["js"], value=rec["value"])
        else:
            intents = {}
            for iname, irec in sorted((rec.get("intents") or {}).items()):
                iw = f"{w}.intents.{iname}"
                check_keys(iw, irec, {"set", "add_token"})
                actions = []
                for op in sorted(irec):
                    a = irec[op]
                    p = prop_on(rec["applies_to"], a.get("prop", ""))
                    if p is None:
                        fail(f"{iw}: no kind matching {rec['applies_to']} has property {a.get('prop')!r}")
                    if not p["writable"]:
                        fail(f"{iw}: property {a['prop']} is read-only")
                    if op == "set":
                        check_keys(iw + ".set", a, {"prop", "value"})
                        if not coerce_ok(p, a.get("value")):
                            fail(f"{iw}: value {a.get('value')!r} does not fit {a['prop']} ({p['type']})")
                        actions.append({"op": "set", "prop": p["js"], "value": a["value"]})
                    else:
                        check_keys(iw + ".add_token", a, {"prop", "token"})
                        if p["type"] != "list" or p.get("of") != "string":
                            fail(f"{iw}: add_token needs a list<string> property, {a['prop']} is {p['type']}")
                        if not re.fullmatch(r"[A-Za-z0-9_-]+", a.get("token") or ""):
                            fail(f"{iw}: bad token {a.get('token')!r}")
                        actions.append({"op": "add_token", "prop": p["js"], "token": a["token"]})
                intents[iname] = actions
            if not intents:
                fail(f"{w}: native_intent needs intents")
            uns = rec.get("unsupported") or {}
            for k in uns:
                if k in intents:
                    fail(f"{w}: {k} is both an intent and unsupported")
            out.update(intents=intents, unsupported=dict(sorted(uns.items())))
        decls[name] = out
    return {"units": units, "declarations": decls}


def aliases(rules: dict, where: str, signal_exists) -> dict:
    al = rules.get("event_aliases") or {}
    if al:
        need_prov(where + ".event_aliases", {"provenance": rules.get("event_aliases_provenance")})
    for k, v in al.items():
        if not signal_exists(v):
            fail(f"{where}.event_aliases.{k}: no kind has signal {v!r}")
    return dict(sorted(al.items()))


def binding_rule(where: str, b: dict, prop: dict | None, sig: dict | None, pname: str) -> dict:
    check_keys(where, b, {"event", "value", "echo", "provenance"})
    prov = need_prov(where, b)
    if prop is None:
        fail(f"{where}: no property {pname!r} in the kind chain")
    if not (prop["readable"] and prop["writable"]):
        fail(f"{where}: {pname} must be readable and writable")
    if sig is None:
        fail(f"{where}: no signal {b.get('event')!r} in the kind chain")
    v = b.get("value")
    if v == "readback":
        pass
    elif isinstance(v, dict) and set(v) == {"arg"} and isinstance(v["arg"], int):
        if not 0 <= v["arg"] < len(sig["args"]):
            fail(f"{where}: arg {v['arg']} out of range for {sig['name']} args {sig['args']}")
        if sig["args"][v["arg"]]["type"] != prop["type"]:
            fail(f"{where}: arg type {sig['args'][v['arg']]['type']} differs from {pname} ({prop['type']})")
    else:
        fail(f"{where}: value must be {{arg: n}} or readback")
    if b.get("echo") not in ECHO:
        fail(f"{where}: echo must be one of {sorted(ECHO)}")
    return {"event": sig["name"], "value": v, "echo": b["echo"], "provenance": prov}


def coerce_ok(p: dict, v) -> bool:
    t = p["type"]
    if t == "bool":
        return isinstance(v, bool)
    if t in ("i32", "u32", "i64"):
        return isinstance(v, int) and not isinstance(v, bool)
    if t == "f64":
        return isinstance(v, (int, float)) and not isinstance(v, bool)
    if t == "string":
        return isinstance(v, str) or (v is None and p["nullable"])
    if t == "enum":
        return v in p.get("values", [])
    return False


# ---- GTK -------------------------------------------------------------------------------------

def gir_id(doc_id: str) -> str:
    """svelters-api:v1:type:gtk:Gtk:Button -> Gtk.Button"""
    parts = doc_id.split(":")
    return f"{parts[-2]}.{parts[-1]}"


def parse_gtk_tables() -> dict:
    src = GTK_TABLES.read_text()
    kinds = re.findall(r'KindInfo \{ name: "([^"]*)", tag: "([^"]*)", gtype: "([^"]*)"', src)
    props = re.findall(r'PropInfo \{ kind: (\d+), name: "([^"]*)", js_name: "([^"]*)", ty: ValueTy::(\w+)', src)
    sigs = re.findall(r'SignalInfo \{ kind: (\d+), name: "([^"]*)", js_event: "([^"]*)"', src)
    meths = re.findall(r'MethodInfo \{ kind: (\d+), name: "([^"]*)"', src)
    slots = re.findall(r'SlotInfo \{ kind: (\d+), name: "([^"]*)"', src)
    gt = [g for _, _, g in kinds]
    return {
        "tags": {t: g for _, t, g in kinds if t},
        "props": {(gt[int(k) - 1], n, j) for k, n, j, _ in props},
        "signals": {(gt[int(k) - 1], n, j) for k, n, j in sigs},
        "methods": {(gt[int(k) - 1], n) for k, n in meths},
        "slots": {(gt[int(k) - 1], n) for k, n in slots},
    }


def gtk_spec() -> dict:
    m = gen_backend.Model()
    rules = svmeta.load_yaml(RULES / "gtk.yaml")
    check_keys("gtk rules", rules, {"framework", "event_aliases", "event_aliases_provenance", "kinds", "defaults", "style"})
    tables = parse_gtk_tables()
    by_id = {gir_id(d["id"]): d for d in m.kinds}
    gid = {d["id"]: gir_id(d["id"]) for d in m.kinds}

    def chain(d: dict) -> list[str]:
        """Same order as svelters-gtk lookup::chain: kind, its interfaces, then each ancestor."""
        out, cur = [], d
        while cur is not None:
            out.append(gid[cur["id"]])
            ifaces = sorted(x for x in (m.kind_ref(i) for i in cur["interfaces"]) if x)
            out += [gir_id(m.kinds[i - 1]["id"]) for i in ifaces]
            parent = m.kind_ref(cur["bases"][0]) if cur["bases"] else 0
            cur = m.kinds[parent - 1] if parent else None
        seen, uniq = set(), []
        for c in out:
            if c not in seen:
                seen.add(c)
                uniq.append(c)
        return uniq

    chains = {gid[d["id"]]: chain(d) for d in m.kinds}

    def enum_members(ref: str) -> list[dict] | None:
        """Members of a Gtk enum/flags type; None for types outside the ingested namespace (Gdk, Pango)."""
        doc = m.by_id.get(inner(ref))
        return None if doc is None else doc["members"]

    def prop_rec(owner: str, p: dict) -> dict:
        ref = p["type"]["ref"]
        t = ptype(ref)
        a = p["access"]
        r = {"js": p["js_name"], "native": p["name"], "type": t, "nullable": bool(p["type"].get("nullable")),
             "readable": bool(a["read"]), "writable": bool(a["write"]), "construct_only": bool(a["construct_only"]),
             "default": p.get("default")}
        if t in ("enum", "flags"):
            mem = enum_members(ref)
            r["enum"] = gir_id(inner(ref))
            if mem is not None:
                r["values"] = [x["nick"] for x in mem]
            if mem is not None and t == "flags":
                r["bits"] = {x["nick"]: x["value"] for x in mem}
                if isinstance(r["default"], int):
                    r["default"] = [x["nick"] for x in mem if x["value"] and r["default"] & x["value"] == x["value"]]
            elif mem is not None and r["default"] is not None and r["default"] not in r["values"]:
                fail(f"{owner}.{p['name']}: default {r['default']!r} not among {r['values']}")
        elif t == "object":
            r["of"] = gir_id(inner(ref).replace(".", ":")) if ":" in inner(ref) else inner(ref)
        elif t == "list":
            r["of"] = ptype(inner(ref))
            if r["default"] == "[]":
                r["default"] = []
        if p.get("range"):
            r["range"] = p["range"]
        if p.get("virtual"):
            r["virtual"] = True
        if p.get("notify", {}).get("signal"):
            r["notify"] = p["notify"]["signal"]
        return r

    def sig_rec(s: dict) -> dict:
        return {"name": s["name"], "js_event": s["js_event"],
                "args": [{"name": q["name"], "type": ptype(q["type"])} for q in s["params"]],
                "handler_result": s["handler_result"], "sync_required": bool(s["sync_required"]),
                "disconnect_on_drop": "handle_drop" in s.get("connection", {}).get("disconnect_on", [])}

    kinds: dict[str, dict] = {}
    for d in m.kinds:
        k = gid[d["id"]]
        props = {}
        for p in sorted(d["properties"], key=lambda p: p["name"]):
            if p["js_name"] in props:
                fail(f"{k}: duplicate js name {p['js_name']}")
            props[p["js_name"]] = prop_rec(k, p)
        sigs = {s["name"]: sig_rec(s) for s in sorted(d["signals"], key=lambda s: s["name"])}
        ch = d.get("children") or {}
        children = None
        if ch.get("slots"):
            children = {"policy": ch["policy"], "default_slot": ch.get("default_slot"), "slots": {
                s["name"]: {"many": s["cardinality"] == "many", "accepts": list(s["accepts"]),
                            "route_by_kind": bool(s.get("route_by_kind")), "order": s.get("order") or "none"}
                for s in sorted(ch["slots"], key=lambda s: s["name"])}}
            for s in children["slots"].values():
                for a in s["accepts"]:
                    if a not in by_id:
                        fail(f"{k}: slot accepts unknown kind {a}")
        kinds[k] = {"native": d["native"]["gtype"], "abstract": bool(d["abstract"]),
                    "interface": d["kind"] == "interface", "chain": chains[k], "lifetime": "owned", "present": None,
                    "text_content": None, "props": props, "signals": sigs, "bindings": {}, "children": children,
                    "methods": sorted(me.get("js_name", me["name"]) for me in d["methods"] if "annotated" in me),
                    "provenance": {}}

    def find_prop(k: str, name: str) -> dict | None:
        for c in chains[k]:
            for p in kinds[c]["props"].values():
                if name in (p["js"], p["native"]):
                    return p
        return None

    def find_sig(k: str, name: str) -> dict | None:
        for c in chains[k]:
            for s in kinds[c]["signals"].values():
                if name in (s["name"], s["js_event"]):
                    return s
        return None

    # rules: per-kind facts (inherited along the chain for lifetime/present/text_content)
    krules = rules.get("kinds") or {}
    for k, r in sorted(krules.items()):
        w = f"gtk rules kinds.{k}"
        if k not in kinds:
            fail(f"{w}: no such kind")
        check_keys(w, r, {"lifetime", "present", "text_content", "signals", "bindings", "provenance"})
        prov = need_prov(w, r) if {"lifetime", "present", "text_content"} & set(r) else None
        if "lifetime" in r and r["lifetime"] not in LIFETIMES:
            fail(f"{w}: lifetime must be one of {sorted(LIFETIMES)}")
        if "present" in r and not any(k2 in chains[k] and r["present"] == n for k2, n in
                                      ((gir_id(m.kinds[i - 1]["id"]), me.get("js_name", me["name"])) for i, me in m.methods)):
            fail(f"{w}: present method {r['present']!r} is not an annotated method in the chain")
        if "text_content" in r:
            p = find_prop(k, r["text_content"])
            if p is None or not p["writable"] or p["type"] != "string":
                fail(f"{w}: text_content {r['text_content']!r} must be a writable string property")
        for sname, sr in sorted((r.get("signals") or {}).items()):
            sw = f"{w}.signals.{sname}"
            check_keys(sw, sr, {"args", "provenance"})
            sp = need_prov(sw, sr)
            if sname not in kinds[k]["signals"]:
                fail(f"{sw}: {k} declares no signal {sname}")
            for a in sr.get("args", []):
                check_keys(sw + ".args", a, {"name", "type"})
                if a.get("type") not in set(PROP_TYPE.values()) | {"other"}:
                    fail(f"{sw}: bad arg type {a.get('type')!r}")
            kinds[k]["signals"][sname]["args"] = [dict(a) for a in sr.get("args", [])]
            kinds[k]["provenance"][f"signals.{sname}"] = sp
        for pname, br in sorted((r.get("bindings") or {}).items()):
            bw = f"{w}.bindings.{pname}"
            p = find_prop(k, pname)
            kinds[k]["bindings"][p["js"] if p else pname] = binding_rule(bw, br, p, find_sig(k, br.get("event", "")), pname)
        for f in ("lifetime", "present", "text_content"):
            if f in r:
                kinds[k]["provenance"][f] = prov
    for k in kinds:
        for f, default in (("lifetime", "owned"), ("present", None), ("text_content", None)):
            src = next((c for c in chains[k] if c in krules and f in krules[c]), None)
            if src is not None:
                kinds[k][f] = krules[src][f]
                if src != k:
                    kinds[k]["provenance"][f] = f"inherited from {src}"
        if kinds[k]["lifetime"] == "toplevel" and kinds[k]["present"] is None:
            fail(f"{k}: toplevel kind without a present method")

    for key, rec in sorted((rules.get("defaults") or {}).items()):
        kname, _, pname = key.rpartition(".")
        if kname not in kinds or pname not in {p["native"] for p in kinds[kname]["props"].values()} | set(kinds[kname]["props"]):
            fail(f"gtk rules defaults.{key}: no such own property")
        p = find_prop(kname, pname)
        need_prov(f"gtk rules defaults.{key}", rec)
        if not coerce_ok(p, rec.get("value")):
            fail(f"gtk rules defaults.{key}: {rec.get('value')!r} does not fit {p['type']}")
        p["default"] = rec["value"]
        kinds[kname]["provenance"][f"defaults.{p['js']}"] = rec["provenance"]

    for k in kinds:
        if not kinds[k]["provenance"]:
            del kinds[k]["provenance"]

    tags = {t: gid[d["id"]] for t, d in sorted(m.tags.items())}

    # cross-check against the backend's generated tables
    if tags != {t: gir_id(by_gtype["id"]) for t, by_gtype in
                ((t, next(d for d in m.kinds if d["native"]["gtype"] == g)) for t, g in tables["tags"].items())}:
        fail("tags differ from crates/svelters-gtk/src/generated/tables.rs (rerun tools/metadata/gtk/gen_backend.py)")
    emitted_props = {(kinds[k]["native"], p["native"], p["js"]) for k in kinds for p in kinds[k]["props"].values()}
    if emitted_props != tables["props"]:
        diff = sorted(emitted_props ^ tables["props"])[:5]
        fail(f"properties differ from svelters-gtk tables (stale tables?): {diff}")
    emitted_sigs = {(kinds[k]["native"], s["name"], s["js_event"]) for k in kinds for s in kinds[k]["signals"].values()}
    if emitted_sigs != tables["signals"]:
        fail(f"signals differ from svelters-gtk tables: {sorted(emitted_sigs ^ tables['signals'])[:5]}")
    emitted_slots = {(kinds[k]["native"], s) for k in kinds for s in (kinds[k]["children"] or {}).get("slots", {})}
    if emitted_slots != tables["slots"]:
        fail(f"slots differ from svelters-gtk tables: {sorted(emitted_slots ^ tables['slots'])}")
    emitted_methods = {(kinds[k]["native"], n) for k in kinds for n in kinds[k]["methods"]}
    if emitted_methods != tables["methods"]:
        fail(f"methods differ from svelters-gtk tables: {sorted(emitted_methods ^ tables['methods'])}")

    def kind_exists(name: str) -> bool:
        return name in kinds

    def prop_on(applies: str, prop: str):
        return find_prop(applies, prop) if applies in kinds else None

    style = style_section(rules, "gtk rules", kind_exists, prop_on, coerce_ok)
    al = aliases(rules, "gtk rules", lambda s: any(s in kinds[k]["signals"] for k in kinds))
    return {"schema": SCHEMA, "backend": "gtk", "framework": rules["framework"], "event_aliases": al,
            "tags": tags, "kinds": kinds, "style": style,
            "source": {"metadata": "metadata/gtk/*.json", "rules": "metadata/semantics/rules/gtk.yaml",
                       "gir": m.docs["Widget"]["source"]["gir"]["version"]}}


# ---- Qt --------------------------------------------------------------------------------------

def qt_spec() -> dict:
    m = gen_cpp.Model(ROOT / "metadata/qt", ROOT / "metadata/overrides/qt", [])
    rules = svmeta.load_yaml(RULES / "qt.yaml")
    check_keys("qt rules", rules, {"framework", "event_aliases", "event_aliases_provenance", "signal_defaults", "kinds",
                                   "defaults", "element_hints", "style"})
    sd = rules.get("signal_defaults") or {}
    check_keys("qt rules signal_defaults", sd, {"disconnect_on_drop", "provenance"})
    need_prov("qt rules signal_defaults", sd)
    # Toplevel classification is the generated backend's (gen_cpp emit_toplevel), read off its output.
    tops = set(re.findall(r"case K_(\w+): return true;", gen_cpp.emit_toplevel(m)))

    kinds: dict[str, dict] = {}
    for el in m.elements:
        tag, cls = el["tag"], el["cls"]
        props = {}
        for p in el.get("props", []):
            if p.get("decl_ref") == "enum":
                r = {"js": p["js"], "native": p["setter"], "type": "enum", "values": sorted(p["values"]),
                     "nullable": False, "readable": True, "writable": True, "construct_only": False, "default": None}
            else:
                rec = p["prop"]
                r = {"js": p["js"], "native": p["native"], "type": ptype(p["decl_ref"]),
                     "nullable": bool(rec["type"].get("nullable")), "readable": bool(rec["access"]["read"]),
                     "writable": bool(rec["access"]["write"]), "construct_only": False, "default": None}
                if rec.get("notify"):
                    r["notify"] = rec["notify"]["signal"]
            if r["type"] in ("object", "list", "flags", "other"):
                fail(f"{tag}.{p['js']}: unsupported Qt property type {r['type']}")
            props[p["js"]] = r
        sigs = {}
        for s in el.get("signals", []):
            rec = s.get("rec") or {}
            sigs[s["js"]] = {"name": s["js"], "js_event": s["js"],
                             "args": [{"name": a["name"], "type": a["type"]} for a in s.get("args", [])],
                             "handler_result": rec.get("handler_result", "none"),
                             "sync_required": bool(rec.get("sync_required", False)),
                             "disconnect_on_drop": bool(sd["disconnect_on_drop"])}
            if s.get("synthetic"):
                sigs[s["js"]]["synthetic"] = s["synthetic"]
        ch = el.get("children")
        children = None
        if ch:
            children = {"policy": ch["policy"], "default_slot": ch["default_slot"], "slots": {
                s["name"]: {"many": s["cardinality"] == "many", "accepts": list(s["accepts"]),
                            "route_by_kind": bool(s.get("route_by_kind")),
                            "order": s.get("order") or "none"} for s in sorted(ch["slots"], key=lambda s: s["name"])}}
        kinds[tag] = {"native": cls, "abstract": False, "interface": False, "chain": m.chain(cls) and [tag] + m.chain(cls),
                      "lifetime": "toplevel" if gen_cpp.cident(tag) in tops else "owned", "present": None,
                      "text_content": None, "props": props, "signals": sigs, "bindings": {}, "children": children,
                      "methods": sorted(me["js"] for me in el.get("methods", [])),
                      "provenance": {"lifetime": "gen_cpp emit_toplevel (QMainWindow/QDialog kinds)"}}
        if sigs:
            kinds[tag]["provenance"]["signals.disconnect_on_drop"] = sd["provenance"]

    for k, r in sorted((rules.get("kinds") or {}).items()):
        w = f"qt rules kinds.{k}"
        if k not in kinds:
            fail(f"{w}: no such element (only annotated elements exist on Qt)")
        check_keys(w, r, {"present", "text_content", "bindings", "provenance"})
        prov = need_prov(w, r) if {"present", "text_content"} & set(r) else None
        kd = kinds[k]
        if "present" in r:
            if r["present"] not in kd["methods"]:
                fail(f"{w}: present method {r['present']!r} not annotated on {k}")
            kd["present"] = r["present"]
            kd["provenance"]["present"] = prov
        if "text_content" in r:
            p = kd["props"].get(r["text_content"])
            if p is None or not p["writable"] or p["type"] != "string":
                fail(f"{w}: text_content {r['text_content']!r} must be an annotated writable string property")
            kd["text_content"] = r["text_content"]
            kd["provenance"]["text_content"] = prov
        for pname, br in sorted((r.get("bindings") or {}).items()):
            kd["bindings"][pname] = binding_rule(f"{w}.bindings.{pname}", br, kd["props"].get(pname),
                                                 kd["signals"].get(br.get("event", "")), pname)
    for k, kd in kinds.items():
        if kd["lifetime"] == "toplevel" and kd["present"] is None:
            fail(f"{k}: toplevel kind without a present rule")

    for key, rec in sorted((rules.get("defaults") or {}).items()):
        tag, _, pname = key.partition(".")
        w = f"qt rules defaults.{key}"
        p = kinds.get(tag, {}).get("props", {}).get(pname)
        if p is None:
            fail(f"{w}: {pname} is not an annotated property of {tag}")
        need_prov(w, rec)
        if not coerce_ok(p, rec.get("value")):
            fail(f"{w}: {rec.get('value')!r} does not fit {p['type']} {p.get('values', '')}")
        p["default"] = rec["value"]
        kinds[tag]["provenance"][f"defaults.{pname}"] = rec["provenance"]

    hints = {}
    for tag, rec in sorted((rules.get("element_hints") or {}).items()):
        check_keys(f"qt rules element_hints.{tag}", rec, {"hint", "provenance"})
        need_prov(f"qt rules element_hints.{tag}", rec)
        if tag in kinds:
            fail(f"qt rules element_hints.{tag}: {tag} is an element; hints are for missing ones")
        hints[tag] = rec["hint"]

    for kd in kinds.values():
        for s in (kd["children"] or {}).get("slots", {}).values():
            for a in s["accepts"]:
                if not any(a in o["chain"] for o in kinds.values()):
                    print(f"note: no Qt element is a {a}", file=sys.stderr)

    def kind_exists(name: str) -> bool:
        return any(name in kd["chain"] for kd in kinds.values())

    def prop_on(applies: str, prop: str):
        for kd in kinds.values():
            if applies in kd["chain"]:
                for p in kd["props"].values():
                    if prop in (p["js"], p["native"]):
                        return p
        return None

    style = style_section(rules, "qt rules", kind_exists, prop_on, coerce_ok)
    al = aliases(rules, "qt rules", lambda s: any(s in kd["signals"] for kd in kinds.values()))
    return {"schema": SCHEMA, "backend": "qt", "framework": rules["framework"], "event_aliases": al,
            "tags": {t: t for t in sorted(kinds)}, "kinds": kinds, "style": style, "element_hints": hints,
            "source": {"metadata": "metadata/qt/*.json + metadata/overrides/qt/*.yaml",
                       "rules": "metadata/semantics/rules/qt.yaml"}}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="exit 1 if a checked-in output is stale")
    a = ap.parse_args()
    try:
        outs = {"gtk.json": render(gtk_spec()), "qt.json": render(qt_spec())}
    except (SpecError, gen_cpp.GenError) as ex:
        print(f"gen_semantics: {ex}", file=sys.stderr)
        return 1
    stale = []
    for name, text in outs.items():
        p = OUT / name
        if p.exists() and p.read_text() == text:
            continue
        stale.append(name)
        if not a.check:
            p.write_text(text)
    if a.check and stale:
        print(f"gen_semantics: stale {stale}; run python3 tools/metadata/semantics/gen_semantics.py", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
