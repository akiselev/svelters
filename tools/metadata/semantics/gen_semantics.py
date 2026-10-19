#!/usr/bin/env python3
"""Generate the per-backend native semantic specifications (schema `svelters-semantics/v0`).

Inputs (nothing else; output is byte-identical on rerun):
  GTK: metadata/gtk/*.json through tools/metadata/gtk/gen_backend.py's Model (overrides already
       merged), cross-checked against crates/svelters-gtk/src/generated/tables.rs;
  Qt:  metadata/overrides/qt/*.yaml resolved against metadata/qt/*.json by
       tools/metadata/qt/gen_cpp.py's Model (only annotated elements exist at runtime);
  both: metadata/semantics/rules/{gtk,qt}.yaml (hand-written facts, each with a provenance) and
        metadata/style/{gtk,qt}.yaml (the style property table, tools/metadata/style/style_table.py).
  AppKit: metadata/appkit/*.json + metadata/overrides/appkit/*.yaml through tools/metadata/appkit/gen_backend.py's Model;
  WinUI: metadata/winui/*.json + metadata/overrides/winui/*.yaml + metadata/inferred/winui/*.yaml through tools/metadata/winui/gen_backend.py's Model;
  all: `text_prop` and `bindings` from the overrides; metadata/style/<toolkit>.yaml.
Outputs: metadata/semantics/{gtk,qt,appkit,winui}.json.

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
OVERRIDES = ROOT / "metadata/overrides"
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
style_table = _load("svelters_style_table", TOOLS / "style/style_table.py")
appkit_gen = _load("svelters_appkit_gen", TOOLS / "appkit/gen_backend.py")
winui_gen = _load("svelters_winui_gen", TOOLS / "winui/gen_backend.py")


class SpecError(Exception):
    pass


def fail(msg: str):
    raise SpecError(msg)


# ---- shared ----------------------------------------------------------------------------------

PROP_TYPE = {"bool": "bool", "i32": "i32", "u32": "u32", "i64": "i64", "u64": "i64", "i8": "i32", "u8": "u32",
             "i16": "i32", "u16": "u32", "f32": "f64", "f64": "f64", "string": "string", "cstring": "string",
             "enum": "enum", "flags": "flags", "object": "object", "list": "list", "record": "record", "boxed": "boxed"}
# Types the compiler only passes through as dynamic values (no static coercion): the raw `ref` is kept.
SCALARS = {"bool", "i32", "u32", "i64", "f64", "string"}
LIFETIMES = {"owned", "toplevel"}


def ptype(ref: str) -> str:
    """Spec type for a neutral type ref (ABI v3 / S1 vocabulary). Anything without a static form
    (date, time, datetime, bytes, variant, pointer, unknown, ...) is `other`."""
    return PROP_TYPE.get(ref.split("<")[0], "other")


def inner(ref: str) -> str:
    return ref[ref.index("<") + 1:-1]


def need_prov(where: str, rec: dict) -> str:
    p = rec.get("provenance")
    if not p:
        fail(f"{where}: missing provenance")
    if isinstance(p, list):
        # Inferred facts (AI1) carry [{origin, ref}, ...]; the spec keeps one string.
        p = "; ".join(f"{e.get('origin')}: {e.get('ref')}" if isinstance(e, dict) else str(e) for e in p)
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


def aliases(rules: dict, where: str, signal_exists) -> dict:
    al = rules.get("event_aliases") or {}
    if al:
        need_prov(where + ".event_aliases", {"provenance": rules.get("event_aliases_provenance")})
    for k, v in al.items():
        if not signal_exists(v):
            fail(f"{where}.event_aliases.{k}: no kind has signal {v!r}")
    return dict(sorted(al.items()))


def row_outlet(rules: dict, where: str, tags) -> str | None:
    """The tag of the container a virtualized list renders each row into (lists.rs, @svelters/lists)."""
    t = rules.get("row_outlet")
    if t is None:
        return None
    need_prov(where + ".row_outlet", {"provenance": rules.get("row_outlet_provenance")})
    if t not in tags:
        fail(f"{where}.row_outlet: no element {t!r}")
    return t


ECHO_STRATEGY = {"host_suppressed": "backend_blocks", "compare": "runtime_compare", "none": "none"}
DIRECTIONS = {"two_way", "read_only"}


def binding_from_metadata(where: str, b: dict, find_prop, find_sig) -> dict:
    """A `bindings[]` annotation (schema extension, docs/notes/fork-bindings.md section 6) as a spec
    binding. `find_prop`/`find_sig` look a name up along the kind chain."""
    name = b.get("name") or fail(f"{where}: binding without a name")
    w = f"{where} bind:{name}"
    prov = need_prov(w, b)
    if b.get("direction") not in DIRECTIONS:
        fail(f"{w}: direction must be one of {sorted(DIRECTIONS)}")
    prop = find_prop(b.get("property") or "")
    if prop is None:
        fail(f"{w}: no property {b.get('property')!r} in the kind chain")
    if not prop["readable"] or (b["direction"] == "two_way" and not prop["writable"]):
        fail(f"{w}: {prop['js']} is not readable and writable")
    ref = (b.get("type") or {}).get("ref")
    if ref is not None and ptype(ref) != prop["type"]:
        fail(f"{w}: binding type {ref} differs from {prop['js']} ({prop['type']})")
    change = b.get("change") or {}
    sig = find_sig(change.get("signal") or "")
    if sig is None:
        fail(f"{w}: no signal {change.get('signal')!r} in the kind chain")
    vf = change.get("value", "arg0")
    if vf == "prop":
        value = "readback"
    elif isinstance(vf, str) and re.fullmatch(r"arg\d+", vf):
        n = int(vf[3:])
        if n >= len(sig["args"]):
            fail(f"{w}: {vf} out of range for {sig['name']} args {sig['args']}")
        if sig["args"][n]["type"] != prop["type"]:
            fail(f"{w}: {sig['name']} arg {n} is {sig['args'][n]['type']}, {prop['js']} is {prop['type']}")
        value = {"arg": n}
    else:
        fail(f"{w}: change.value must be argN or prop")
    echo = b.get("echo") or {}
    if echo.get("strategy") not in ECHO_STRATEGY:
        fail(f"{w}: echo.strategy must be one of {sorted(ECHO_STRATEGY)}")
    life = {k: v for k, v in (b.get("lifetime") or {}).items() if k != "origin"}
    if not life.get("on_native_death") or not life.get("on_destroy"):
        fail(f"{w}: lifetime.on_native_death and lifetime.on_destroy are required")
    return {"property": prop["js"], "direction": b["direction"], "event": sig["name"], "value": value,
            "echo": ECHO_STRATEGY[echo["strategy"]], "compare_before_set": bool(echo.get("compare_before_set")),
            "read_back": bool(echo.get("read_back")),
            "natively_normalizes": bool((b.get("write") or {}).get("natively_normalizes")),
            "sync_signals_on_set": list(echo.get("sync_signals_on_set") or []), "lifetime": dict(sorted(life.items())),
            "provenance": prov}


# Non-scalar values the backends also take as a string (ABI v3 `SVL_D_STRING_FORM`): GTK
# crates/svelters-gtk/src/types.rs `has_string_form`; Qt the `| string` cases of gen_cpp ts_leaf.
GTK_STRING_FORMS = {"Pango:FontDescription", "Gio:Icon", "Gdk:RGBA", "Gtk:ShortcutTrigger", "Gio:File"}
QT_STRING_FORMS = {"QIcon", "QKeySequence", "QUrl", "QFont", "Color"}


def type_rec(ref: str | None, nullable=False) -> dict:
    """A method parameter / return type: `{type, nullable}` (+ `ref` for non-scalars); `void`."""
    ref = ref or "void"
    if ref == "void":
        return {"type": "void", "nullable": False}
    t = ptype(ref)
    r = {"type": t, "nullable": bool(nullable)}
    if t not in SCALARS:
        r["ref"] = ref
    return r


ATTR_TYPES = {"bool", "i32", "u32", "i64", "f64", "string", "enum"}


def attr_rec(where: str, a: dict) -> dict:
    """An X3 child attribute of a slot (`parent:<name>`): type, reset default, enum names."""
    t = a.get("type")
    if t not in ATTR_TYPES:
        fail(f"{where}: attr {a.get('name')!r} has type {t!r} (expected one of {sorted(ATTR_TYPES)})")
    r = {"type": t, "default": a.get("default")}
    if t == "enum":
        r["values"] = sorted(a.get("values") or {})
        if r["default"] not in r["values"]:
            fail(f"{where}: attr {a['name']} default {r['default']!r} not among {r['values']}")
    return r


def phases_rec(sig: dict, phases: list[str]) -> dict:
    """Adds the X2/X3 dispatch facts to a signal record: `sync` (the backend calls JS through
    `call_sync` and uses the result) and `phases` (listen flags the backend accepts besides 0)."""
    sig["sync"] = sig["handler_result"] != "none" or sig["sync_required"]
    sig["phases"] = phases
    return sig


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
    attrs = re.findall(r'AttrInfo \{ kind: (\d+), slot: (\d+), name: "([^"]*)"', src)
    gt = [g for _, _, g in kinds]
    return {
        "tags": {t: g for _, t, g in kinds if t},
        "props": {(gt[int(k) - 1], n, j) for k, n, j, _ in props},
        "signals": {(gt[int(k) - 1], n, j) for k, n, j in sigs},
        "methods": {(gt[int(k) - 1], n) for k, n in meths},
        "slots": {(gt[int(k) - 1], n) for k, n in slots},
        "attrs": {(gt[int(k) - 1], slots[int(si) - 1][1], n) for k, si, n in attrs},
    }


def gtk_spec() -> dict:
    m = gen_backend.Model()
    rules = svmeta.load_yaml(RULES / "gtk.yaml")
    check_keys("gtk rules", rules, {"framework", "event_aliases", "event_aliases_provenance", "kinds", "defaults",
                                    "row_outlet", "row_outlet_provenance"})
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
        if t not in SCALARS:
            r["ref"] = ref
        if t in ("record", "boxed"):
            r["of"] = inner(ref).rsplit(":", 1)[-1]
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
            r["of"] = ptype(inner(ref)) if "<" in ref else "other"
            if r["default"] == "[]":
                r["default"] = []
        if t in ("record", "boxed", "object") and any(inner(ref).endswith(":" + f) for f in GTK_STRING_FORMS):
            r["string_form"] = True
        if p.get("range"):
            r["range"] = p["range"]
        if p.get("virtual"):
            r["virtual"] = True
        if p.get("notify", {}).get("signal"):
            r["notify"] = p["notify"]["signal"]
        return r

    def sig_rec(k: str, s: dict) -> dict:
        # X3 (docs/notes/x3-childattr-listen.md): `after` on any signal (connect_after); `capture`
        # only on event controllers (their propagation phase).
        phases = ["after", "capture"] if "Gtk.EventController" in chains[k] else ["after"]
        return phases_rec({"name": s["name"], "js_event": s["js_event"],
                           "args": [{"name": q["name"], "type": ptype(q["type"])} for q in s["params"]],
                           "handler_result": s["handler_result"], "sync_required": bool(s["sync_required"]),
                           "disconnect_on_drop": "handle_drop" in s.get("connection", {}).get("disconnect_on", [])}, phases)

    def meth_rec(me: dict) -> dict:
        ret = me.get("returns") or {}
        return {"dispatch": me.get("dispatch", "zero_arg"), "native": me.get("native") or me["name"],
                "params": [{"name": q["name"], **type_rec(q.get("type"), q.get("nullable"))} for q in me.get("params") or []],
                "returns": type_rec(ret.get("type"), ret.get("nullable"))}

    kinds: dict[str, dict] = {}
    for d in m.kinds:
        k = gid[d["id"]]
        props = {}
        for p in sorted(d["properties"], key=lambda p: p["name"]):
            if p["js_name"] in props:
                fail(f"{k}: duplicate js name {p['js_name']}")
            props[p["js_name"]] = prop_rec(k, p)
        sigs = {s["name"]: sig_rec(k, s) for s in sorted(d["signals"], key=lambda s: s["name"])}
        ch = d.get("children") or {}
        children = None
        if ch.get("slots"):
            children = {"policy": ch["policy"], "default_slot": ch.get("default_slot"), "slots": {
                s["name"]: {"many": s["cardinality"] == "many", "accepts": list(s["accepts"]),
                            "route_by_kind": bool(s.get("route_by_kind")), "order": s.get("order") or "none",
                            "attrs": {a["name"]: attr_rec(f"{k} slot {s['name']}", a) for a in s.get("attrs") or []}}
                for s in sorted(ch["slots"], key=lambda s: s["name"])}}
            for s in children["slots"].values():
                for a in s["accepts"]:
                    if a not in by_id:
                        fail(f"{k}: slot accepts unknown kind {a}")
        kinds[k] = {"native": d["native"]["gtype"], "abstract": bool(d["abstract"]),
                    "interface": d["kind"] == "interface", "chain": chains[k], "lifetime": "owned", "present": None,
                    "text_content": None, "props": props, "signals": sigs, "bindings": {}, "children": children,
                    "methods": {me.get("js_name", me["name"]): meth_rec(me)
                                for me in sorted(d["methods"], key=lambda me: me.get("js_name", me["name"])) if "annotated" in me},
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
        check_keys(w, r, {"lifetime", "present", "text_content", "attaches_to", "signals", "provenance"})
        prov = need_prov(w, r) if {"lifetime", "present", "text_content", "attaches_to"} & set(r) else None
        if "attaches_to" in r and r["attaches_to"] not in kinds:
            fail(f"{w}: attaches_to {r['attaches_to']!r} is not a kind")
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
        for f in ("lifetime", "present", "text_content", "attaches_to"):
            if f in r:
                kinds[k]["provenance"][f] = prov
    # text_prop: metadata/overrides/gtk/<Kind>.yaml (not merged into metadata/gtk); inherited like the rules
    for path in sorted(OVERRIDES.joinpath("gtk").glob("*.yaml")):
        tp = svmeta.load_yaml(path).get("text_prop")
        if not tp:
            continue
        k = f"Gtk.{path.stem}"
        w = f"metadata/overrides/gtk/{path.name} text_prop"
        if k not in kinds:
            fail(f"{w}: no such kind")
        if "text_content" in krules.get(k, {}):
            fail(f"{w}: also set in the rules")
        p = find_prop(k, tp["name"])
        if p is None or not p["writable"] or p["type"] != "string":
            fail(f"{w}: {tp['name']!r} must be a writable string property")
        krules.setdefault(k, {})["text_content"] = p["js"]
        kinds[k]["provenance"]["text_content"] = need_prov(w, tp)
    for k in kinds:
        for f, default in (("lifetime", "owned"), ("present", None), ("text_content", None)):
            src = next((c for c in chains[k] if c in krules and f in krules[c]), None)
            if src is not None:
                kinds[k][f] = krules[src][f]
                if src != k:
                    kinds[k]["provenance"][f] = f"inherited from {src}"
        if kinds[k]["lifetime"] == "toplevel" and kinds[k]["present"] is None:
            fail(f"{k}: toplevel kind without a present method")
        # attaches_to (RT1): only on the kinds that have it, inherited like the facts above
        src = next((c for c in chains[k] if c in krules and "attaches_to" in krules[c]), None)
        if src is not None:
            kinds[k]["attaches_to"] = krules[src]["attaches_to"]
            if src != k:
                kinds[k]["provenance"]["attaches_to"] = f"inherited from {src}"

    # bindings: the `bindings` annotations (metadata/overrides/gtk, merged into metadata/gtk)
    for d in m.kinds:
        k = gid[d["id"]]
        for b in sorted(d.get("bindings", []), key=lambda b: b["name"]):
            kinds[k]["bindings"][b["name"]] = binding_from_metadata(
                k, b, lambda n, k=k: find_prop(k, n), lambda n, k=k: find_sig(k, n))

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
    # METHODS holds the zero-arg generated calls; `dynamic` ones go through GIRepository (X1)
    emitted_methods = {(kinds[k]["native"], n) for k in kinds for n, me in kinds[k]["methods"].items()
                       if me["dispatch"] == "zero_arg"}
    if emitted_methods != tables["methods"]:
        fail(f"methods differ from svelters-gtk tables: {sorted(emitted_methods ^ tables['methods'])}")
    emitted_attrs = {(kinds[k]["native"], sn, an) for k in kinds
                     for sn, sl in ((kinds[k]["children"] or {}).get("slots") or {}).items() for an in sl["attrs"]}
    if emitted_attrs != tables["attrs"]:
        fail(f"child attributes differ from svelters-gtk tables: {sorted(emitted_attrs ^ tables['attrs'])}")

    # the style table covers the generated kinds only (setters are resolved against their props)
    style = style_table.build("gtk", style_table.load("gtk"), tags, kinds)
    extra_kinds("gtk", kinds, tags, lambda b: kinds[b]["chain"])
    al = aliases(rules, "gtk rules", lambda s: any(s in kinds[k]["signals"] for k in kinds))
    return {"schema": SCHEMA, "backend": "gtk", "framework": rules["framework"], "event_aliases": al,
            "row_outlet": row_outlet(rules, "gtk rules", tags),
            "tags": tags, "kinds": kinds, "style": style,
            # `methods` lists the annotated (verified) calls; any other GIR method of the kind is
            # still callable through GIRepository at runtime (X1), unchecked.
            "method_lookup": "typelib",
            "source": {"metadata": "metadata/gtk/*.json", "rules": "metadata/semantics/rules/gtk.yaml", "style": "metadata/style/gtk.yaml",
                       "gir": m.docs["Widget"]["source"]["gir"]["version"]}}


# ---- Qt --------------------------------------------------------------------------------------

def qt_meth(me: dict) -> dict:
    rec = me.get("rec") or {}
    ret = rec.get("returns") or {}
    params = []
    for q in rec.get("params") or []:
        t = q.get("type") or {}
        r = {"name": q["name"], **type_rec(t.get("ref"), t.get("nullable"))}
        if q.get("default") is not None:
            r["optional"] = True
        params.append(r)
    return {"dispatch": "generated", "native": me.get("native") or me["js"], "params": params,
            "returns": type_rec(ret.get("ref"), ret.get("nullable"))}


def qt_spec() -> dict:
    m = gen_cpp.Model(ROOT / "metadata/qt", ROOT / "metadata/overrides/qt", [])
    rules = svmeta.load_yaml(RULES / "qt.yaml")
    check_keys("qt rules", rules, {"framework", "event_aliases", "event_aliases_provenance", "signal_defaults", "kinds",
                                   "defaults", "element_hints", "row_outlet", "row_outlet_provenance"})
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
            cat, pay = gen_cpp.classify(p["ref"])
            t = ptype(p["ref"])
            rec = p.get("prop") or {}
            access = rec.get("access") or {"read": True, "write": True}
            r = {"js": p["js"], "native": p.get("native") or p.get("setter"), "type": t,
                 "nullable": bool((rec.get("type") or {}).get("nullable")), "readable": bool(access.get("read", True)),
                 "writable": bool(access.get("write", True)) and not p["readonly"], "construct_only": False,
                 "default": None}
            if t not in SCALARS:
                r["ref"] = p["ref"]
            if cat in ("enum", "flags"):
                r["enum"] = pay
                r["values"] = sorted(m.enums_used[pay])
            elif cat in ("record", "boxed", "object"):
                r["of"] = pay
                if pay in QT_STRING_FORMS:
                    r["string_form"] = True
            elif cat == "list":
                r["of"] = ptype(pay)
            if rec.get("notify"):
                r["notify"] = rec["notify"]["signal"]
            if p["js"] in props:
                fail(f"{tag}: duplicate property {p['js']}")
            props[p["js"]] = r
        sigs = {}
        for s in el.get("signals", []):
            rec = s.get("rec") or {}
            # X3 Qt phase mapping (docs/notes/x3-childattr-listen.md): `after` on QObject signals,
            # neither on close-request; `capture` on the synthetic event-filter signals (X3 key-press
            # and the X8 event families, docs/notes/x8-events-draw.md: `after` is refused there).
            X8_FAMILIES = {"key_release", "mouse_press", "mouse_release", "mouse_move", "mouse_double_click", "wheel",
                           "focus_in", "focus_out", "enter", "leave", "resize", "context_menu"}
            syn = s.get("synthetic")
            phases = ["capture"] if syn in X8_FAMILIES else {None: ["after"], "key_press": ["capture"], "close_request": []}.get(syn)
            if phases is None:
                fail(f"{tag}: synthetic signal {s['synthetic']!r} has no phase mapping")
            sigs[s["js"]] = phases_rec({
                "name": s["js"], "js_event": s["js"],
                "args": [{"name": a["name"], "type": ptype(a["type"])} for a in s.get("args", [])],
                # Synthetic signals are event filters whose JS result decides (close veto,
                # key consumed): gen_cpp _resolve_signal, docs/notes/x2-sync-frame.md.
                "handler_result": "bool_stop" if s.get("synthetic") else rec.get("handler_result", "none"),
                "sync_required": bool(s.get("synthetic")) or bool(rec.get("sync_required", False)),
                "disconnect_on_drop": bool(sd["disconnect_on_drop"])}, phases)
            if s.get("synthetic"):
                sigs[s["js"]]["synthetic"] = s["synthetic"]
        ch = el.get("children")
        children = None
        if ch:
            children = {"policy": ch["policy"], "default_slot": ch["default_slot"], "slots": {
                s["name"]: {"many": s["cardinality"] == "many", "accepts": list(s["accepts"]),
                            "route_by_kind": bool(s.get("route_by_kind")),
                            "order": s.get("order") or "none",
                            "attrs": {a["name"]: attr_rec(f"{tag} slot {s['name']}", a) for a in s.get("attrs") or []}}
                for s in sorted(ch["slots"], key=lambda s: s["name"])}}
        kinds[tag] = {"native": cls, "abstract": False, "interface": False, "chain": m.chain(cls) and [tag] + m.chain(cls),
                      "lifetime": "toplevel" if gen_cpp.cident(tag) in tops else "owned", "present": None,
                      "text_content": None, "props": props, "signals": sigs, "bindings": {}, "children": children,
                      "methods": {me["js"]: qt_meth(me) for me in sorted(el.get("methods", []), key=lambda me: me["js"])},
                      "provenance": {"lifetime": "gen_cpp emit_toplevel (QMainWindow/QDialog kinds)"}}
        if sigs:
            kinds[tag]["provenance"]["signals.disconnect_on_drop"] = sd["provenance"]

    # text_prop: `elements[].text_prop` in metadata/overrides/qt (native or JS name of an annotated prop)
    for el in m.elements:
        tp = el.get("text_prop")
        if not tp:
            continue
        kd = kinds[el["tag"]]
        p = next((p for p in kd["props"].values() if tp in (p["js"], p["native"])), None)
        if p is None or not p["writable"] or p["type"] != "string":
            fail(f"qt <{el['tag']}> text_prop {tp!r} must be an annotated writable string property")
        kd["text_content"] = p["js"]
        kd["provenance"]["text_content"] = f"metadata/overrides/qt text_prop ({p['native']})"
    for k, r in sorted((rules.get("kinds") or {}).items()):
        w = f"qt rules kinds.{k}"
        if k not in kinds:
            fail(f"{w}: no such element (only annotated elements exist on Qt)")
        check_keys(w, r, {"present", "text_content", "provenance"})
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
    # bindings: the `bindings` annotations on the elements (metadata/overrides/qt)
    for el in m.elements:
        kd = kinds[el["tag"]]
        def fp(n, kd=kd):
            return next((p for p in kd["props"].values() if n in (p["js"], p["native"])), None)
        for b in sorted(el.get("bindings", []), key=lambda b: b["name"]):
            kd["bindings"][b["name"]] = binding_from_metadata(el["tag"], b, fp, lambda n, kd=kd: kd["signals"].get(n))
    for k, kd in kinds.items():
        if kd["lifetime"] == "toplevel" and kd["present"] is None:
            # Not a broken reference: the element just has no annotated method to show it yet.
            print(f"note: qt {k} is a toplevel without a present method (annotate one in metadata/overrides/qt)",
                  file=sys.stderr)

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

    # `construct.context_menu` elements (QMenu) inserted into any other widget are its context menu (X4)
    for el in m.elements:
        if el["construct"].get("context_menu"):
            kinds[el["tag"]]["attaches_to"] = "QWidget"
            kinds[el["tag"]]["provenance"]["attaches_to"] = "construct.context_menu (menus.hpp.in ctx_attach)"
    qtags = {t: t for t in kinds}
    style = style_table.build("qt", style_table.load("qt"), dict(qtags), kinds)
    extra_kinds("qt", kinds, qtags, lambda b: kinds[b]["chain"], m.chain)
    al = aliases(rules, "qt rules", lambda s: any(s in kd["signals"] for kd in kinds.values()))
    return {"schema": SCHEMA, "backend": "qt", "framework": rules["framework"], "event_aliases": al,
            "row_outlet": row_outlet(rules, "qt rules", kinds),
            "tags": dict(sorted(qtags.items())), "kinds": kinds, "style": style, "element_hints": hints,
            # only annotated methods exist at runtime
            "method_lookup": "annotated",
            "source": {"metadata": "metadata/qt/*.json + metadata/overrides/qt/*.yaml",
                       "rules": "metadata/semantics/rules/qt.yaml", "style": "metadata/style/qt.yaml"}}


EXTRAS = ROOT / "metadata/extras"
EXTRA_SPEC_KEYS = {"kind", "native", "base", "props", "signals", "signals_from", "methods", "children", "attaches_to",
                   "provenance"}


def extra_kinds(tk: str, kinds: dict, tags: dict, base_chain, native_chain=lambda n: []) -> None:
    """Hand-written kinds with no ingested class (metadata/extras/<tk>-*.yaml `elements[].spec`, RT1): GTK
    menus and the recording canvas, the Qt canvas. Added after the generated kinds (and after the GTK table
    cross-check: the backend serves them from hand-written code, not the generated tables).
    `base_chain(base)` is the chain a kind with `base` continues with."""
    added = []
    for path in sorted(EXTRAS.glob(f"{tk}-*.yaml")):
        for el in svmeta.load_yaml(path).get("elements", []):
            sp = el.get("spec")
            if not sp:
                continue
            tag = el["tag"]
            w = f"metadata/extras/{path.name} <{tag}> spec"
            check_keys(w, sp, EXTRA_SPEC_KEYS)
            prov = need_prov(w, sp)
            k = sp["kind"]
            if k in kinds or tag in tags:
                fail(f"{w}: kind {k} or tag {tag} already exists")
            base = sp.get("base")
            if base is not None and base not in kinds:
                fail(f"{w}: base {base} is not a kind")
            props = {}
            for name, t in sorted((sp.get("props") or {}).items()):
                t = t if isinstance(t, dict) else {"type": t}
                if t["type"] not in SCALARS:
                    fail(f"{w}: prop {name}: only scalar types are supported, not {t['type']!r}")
                props[name] = {"js": name, "native": name, "type": t["type"], "nullable": bool(t.get("nullable")),
                               "readable": True, "writable": True, "construct_only": False, "default": t.get("default")}
            sigs = {}
            for name, args in sorted((sp.get("signals") or {}).items()):
                sigs[name] = phases_rec({"name": name, "js_event": name, "args": [dict(a) for a in args],
                                         "handler_result": "none", "sync_required": False,
                                         "disconnect_on_drop": True}, [])
            sf = sp.get("signals_from")
            if sf:
                src = kinds.get(tags.get(sf["element"], ""))
                if src is None:
                    fail(f"{w}: signals_from: no element {sf['element']}")
                for name in sf["names"]:
                    if name not in src["signals"]:
                        fail(f"{w}: signals_from: <{sf['element']}> has no signal {name}")
                    sigs[name] = json.loads(json.dumps(src["signals"][name]))
            children = None
            if sp.get("children"):
                children = {"policy": "ordered", "default_slot": "items", "slots": {"items": {
                    "many": True, "accepts": list(sp["children"]["accepts"]), "route_by_kind": False,
                    "order": "before_sibling", "attrs": {}}}}
            rec = {"native": sp["native"], "abstract": False, "interface": False,
                   "chain": [k] + (base_chain(base) if base else native_chain(sp["native"])), "lifetime": "owned", "present": None,
                   "text_content": None, "props": props, "signals": sigs, "bindings": {}, "children": children,
                   "methods": {n: {"dispatch": "extra", "native": n, "params": [], "returns": type_rec(None)}
                               for n in sp.get("methods") or []},
                   "provenance": {"kind": prov}}
            if sp.get("attaches_to"):
                if sp["attaches_to"] not in kinds and tk != "qt":
                    fail(f"{w}: attaches_to {sp['attaches_to']} is not a kind")
                rec["attaches_to"] = sp["attaches_to"]

            def fp(n, rec=rec):
                return next((p for p in rec["props"].values() if n in (p["js"], p["native"])), None)
            for b in sorted(el.get("bindings", []), key=lambda b: b["name"]):
                rec["bindings"][b["name"]] = binding_from_metadata(k, b, fp, lambda n, rec=rec: rec["signals"].get(n))
            kinds[k] = rec
            tags[tag] = k
            added.append(k)
    for k in added:
        for a in ((kinds[k]["children"] or {}).get("slots") or {}).get("items", {}).get("accepts", []):
            if a not in kinds:
                fail(f"extra kind {k}: slot accepts unknown kind {a}")


# ---- AppKit ----------------------------------------------------------------------------------

APPKIT_RECORDS = {"rect": "Rect", "size": "Size", "point": "Point", "insets": "Insets", "range": "Range"}


def appkit_spec() -> dict:
    """Kinds are the ingested AppKit classes (NSView, NSControl, NSButton, ...): an element's chain
    is its class and its ingested superclasses, so inherited props/signals/slots resolve as in
    crates/svelters-appkit (lookup.rs). Props are every ingested property the backend's tables
    carry (gen_backend.build's type filter) plus the override `setters`; signals, slots and methods
    are the override annotations (the backend's dynamic dev mode is not a compile-time fact)."""
    m = appkit_gen.Model()
    enums = {n: [js for js, _ in e["members"]] for n, e in m.enums.items()}
    flags = {n for n, e in m.enums.items() if e["flags"]}
    prop_enums = {}
    for cls in m.classes:
        for e in m.ov.get(cls, {}).get("enums", []):
            prop_enums[(cls, e["prop"])] = sorted(e["members"], key=lambda k: e["members"][k])

    def prop_rec(cls: str, p: dict, setter: bool = False) -> dict | None:
        ref = p["type"]["ref"]
        r = {"js": p["js_name"], "native": p.get("name") or p["js_name"], "type": None, "nullable": False,
             "readable": not setter, "writable": True if setter else bool(p["access"]["write"]),
             "construct_only": False, "default": None}
        if (cls, p["js_name"]) in prop_enums:
            r.update(type="enum", enum=f"{cls}.{p['js_name']}", values=prop_enums[(cls, p["js_name"])], ref=f"enum<{cls}.{p['js_name']}>")
        elif ref in ("bool", "string") or ref in appkit_gen.INTEGER or ref in appkit_gen.FLOAT:
            r["type"] = ptype(ref)
            r["nullable"] = ref == "string"  # null is "" (backend)
        elif ref in APPKIT_RECORDS:
            r.update(type="record", of=APPKIT_RECORDS[ref], ref=ref)
        elif ref.startswith(("enum<", "flags<")):
            n = inner(ref)
            if n not in enums:
                return None
            t = "flags" if n in flags else "enum"
            r.update(type=t, enum=n, values=enums[n], ref=ref)
        elif ref == "object" or ref.startswith("object<"):
            r.update(type="object", nullable=True, ref=ref, of=inner(ref) if "<" in ref else "NSObject")
        elif ref == "list":
            r.update(type="list", of="other", ref=ref)
        else:
            return None
        return r

    kinds: dict[str, dict] = {}
    tags: dict[str, str] = {}
    for cls in m.classes:
        ov = m.ov.get(cls, {})
        el = ov.get("element") or {}
        props = {}
        for p in m.meta[cls]["properties"]:
            r = prop_rec(cls, p)
            if r is not None:
                props[r["js"]] = r
        for s in ov.get("setters", []):
            r = prop_rec(cls, {"js_name": s["name"], "name": s["selector"], "type": {"ref": s["type"]}}, setter=True)
            if r is None:
                fail(f"appkit {cls}: setter {s['name']} has no spec type ({s['type']})")
            props[r["js"]] = r
        kinds[cls] = {"native": cls, "abstract": not el, "interface": False, "chain": m.chain(cls),
                      "lifetime": el.get("lifetime", "owned"), "present": None, "text_content": None,
                      "props": dict(sorted(props.items())), "signals": {}, "bindings": {}, "children": None,
                      "methods": {}, "provenance": {}}
        if el:
            tags[el["tag"]] = cls

    def find_prop(cls: str, name: str) -> dict | None:
        for c in kinds[cls]["chain"]:
            for p in kinds[c]["props"].values():
                if name in (p["js"], p["native"]):
                    return p
        return None

    def find_sig(cls: str, name: str) -> dict | None:
        for c in kinds[cls]["chain"]:
            if name in kinds[c]["signals"]:
                return kinds[c]["signals"][name]
        return None

    for cls in m.classes:
        ov, kd = m.ov.get(cls, {}), kinds[cls]
        for s in sorted(ov.get("signals", []), key=lambda s: s["name"]):
            args = []
            for a in s.get("args", []):
                p = find_prop(cls, a)
                if p is None:
                    fail(f"appkit {cls}: signal {s['name']}: unknown arg prop {a}")
                args.append({"name": a, "type": p["type"]})
            sync = s.get("result", "none") != "none"
            kd["signals"][s["name"]] = phases_rec({
                "name": s["name"], "js_event": s["name"], "args": args,
                "handler_result": "bool_stop" if s.get("result") == "veto_bool" else "none",
                "sync_required": sync, "disconnect_on_drop": True}, [])
        for me in sorted(ov.get("methods", []), key=lambda me: me["name"]):
            kd["methods"][me["name"]] = {"dispatch": "generated", "native": me["selector"]}
        slots = ov.get("slots", [])
        if slots:
            kd["children"] = {"policy": "slots", "default_slot": next((s["name"] for s in slots if s.get("default")), None), "slots": {
                s["name"]: {"many": s["cardinality"] == "many", "accepts": ["NSView"], "route_by_kind": False,
                            "order": ("native_index" if s["ops"].get("insert_at") else "before_sibling" if s["ops"].get("insert_relative")
                                      else "append_only") if s["cardinality"] == "many" else "none", "attrs": {}}
                for s in sorted(slots, key=lambda s: s["name"])}}
    for cls in m.classes:
        el = (m.ov.get(cls, {}).get("element") or {})
        kd = kinds[cls]
        if el.get("lifetime") == "toplevel":
            # The renderer's TOPLEVELS show method (gen_bindings.py appkit_toplevels).
            if not any("make-key-and-order-front" in kinds[c]["methods"] for c in kd["chain"]):
                fail(f"appkit {cls}: toplevel without an annotated make-key-and-order-front")
            kd["present"] = "make-key-and-order-front"
            kd["provenance"]["present"] = "NSWindow makeKeyAndOrderFront: (MAC1: the renderer's TOPLEVELS open method)"
        if el.get("text_prop"):
            p = find_prop(cls, el["text_prop"])
            if p is None or not p["writable"] or p["type"] != "string":
                fail(f"appkit {cls}: text_prop {el['text_prop']!r} must be a writable string property")
            kd["text_content"] = p["js"]
            kd["provenance"]["text_content"] = "metadata/overrides/appkit text_prop"
        for b in sorted(m.ov.get(cls, {}).get("bindings", []), key=lambda b: b["name"]):
            if b["name"] != b["property"]:
                fail(f"appkit {cls} bind:{b['name']}: binding names are the native prop name ({b['property']})")
            kd["bindings"][b["name"]] = binding_from_metadata(
                f"appkit {cls}", b, lambda n, c=cls: find_prop(c, n), lambda n, c=cls: find_sig(c, n))
    for kd in kinds.values():
        if not kd["provenance"]:
            del kd["provenance"]
    style = style_table.build("appkit", style_table.load("appkit"), tags, kinds)
    return {"schema": SCHEMA, "backend": "appkit", "framework": "AppKit (svelters-appkit, objc2)", "event_aliases": {},
            "tags": dict(sorted(tags.items())), "kinds": kinds, "style": style,
            # dev mode resolves unannotated props/methods at run time; the spec holds the tables' facts
            "method_lookup": "annotated",
            "source": {"metadata": "metadata/appkit/*.json + metadata/overrides/appkit/*.yaml", "style": "metadata/style/appkit.yaml"}}


# ---- WinUI -----------------------------------------------------------------------------------

def winui_spec() -> dict:
    """Kinds are the short class names of every class in an element's chain (Button, ButtonBase,
    ContentControl, Control, FrameworkElement, UIElement, ...); members are the override
    annotations, which are all the backend exposes (crates/svelters-winui/src/generated/names.rs)."""
    m = winui_gen.Model()
    short = winui_gen.short
    kebab = winui_gen.kebab

    def wtype(where: str, ty: dict) -> dict:
        t = ty["t"]
        if t in ("Bool", "I32", "F64", "Str"):
            return {"type": {"Bool": "bool", "I32": "i32", "F64": "f64", "Str": "string"}[t], "nullable": t == "Str"}
        if t == "NullableBool":
            return {"type": "bool", "nullable": True}
        if t in ("ObjectString", "Custom"):
            return {"type": "string", "nullable": True}
        if t == "Thickness":
            return {"type": "record", "nullable": False, "of": "Insets", "ref": "value<Microsoft.UI.Xaml.Thickness>", "string_form": True}
        if t == "Enum":
            tt = m.types.get(ty["enum"])
            if not tt or tt["category"] != "enum":
                fail(f"{where}: enum {ty['enum']} missing from metadata/winui/_types.json")
            return {"type": "enum", "nullable": False, "enum": ty["enum"], "values": [kebab(n) for n, _ in tt["members"]],
                    "ref": f"enum<{ty['enum']}>"}
        fail(f"{where}: wire type {t} has no spec type")

    kinds: dict[str, dict] = {}
    for c in m.class_names:
        kinds[short(c)] = {"native": c, "abstract": True, "interface": False,
                           "chain": [short(x) for x in [c] + m.classes[c]["bases"] if x in m.class_index],
                           "lifetime": "owned", "present": None, "text_content": None, "props": {}, "signals": {},
                           "bindings": {}, "children": None, "methods": {}, "provenance": {}}
    tags = {}
    for k in m.kinds:
        kd = kinds[short(k["class"])]
        kd["abstract"] = False
        tags[k["tag"]] = short(k["class"])
        if k["el"].get("lifetime") == "toplevel":
            kd["lifetime"] = "toplevel"
        tc = m.kind_text_content(k)
        if tc:
            kd["text_content"] = tc
            kd["provenance"]["text_content"] = "metadata/overrides/winui text_prop"
    # `notify::<prop>` events (DependencyProperty callbacks; not ActualWidth/ActualHeight, which never notify).
    notify = {(e["class"], e["native"]): e["name"] for e in m.events if e["notify"]}
    for p in m.props:
        r = {"js": p["name"], "native": p["native"], **wtype(f"{p['file']} prop {p['native']}", p["ty"]),
             "readable": bool(p["read"]), "writable": bool(p["write"]), "construct_only": False, "default": None}
        if (p["class"], p["native"]) in notify:
            r["notify"] = notify[(p["class"], p["native"])]
        kinds[short(p["class"])]["props"][p["name"]] = {k: r[k] for k in sorted(r)}
    for e in m.events:
        o = e["o"]
        args = [{"name": x["name"], "type": ptype(x["ref"].replace("nullable<bool>", "bool"))} for x in e["payload"]]
        kinds[short(e["class"])]["signals"][e["name"]] = phases_rec({
            "name": e["name"], "js_event": e["name"], "args": args,
            # sync events hand `true` (preventDefault/result) to Cancel/Handled (WIN1 notes)
            "handler_result": "bool_stop" if o.get("result") else "none", "sync_required": bool(o.get("sync")),
            "disconnect_on_drop": True}, [])
    for i, s in enumerate(m.slots):
        kd = kinds[short(s["class"])]
        ch = kd["children"] or {"policy": "slots", "default_slot": None, "slots": {}}
        kd["children"] = ch
        ch["slots"][s["name"]] = {"many": s["many"], "accepts": ["UIElement"], "route_by_kind": False,
                                  "order": "native_index" if s["many"] else "none", "attrs": {}}
        if s["default"]:
            ch["default_slot"] = s["name"]
    for a in m.attrs:
        # an attached property of `class` (Grid.Row) is an attribute of the slot it names, on that
        # class's kind: Grid gets its own copy of the inherited Panel `children` slot with the attrs
        slot = m.slots[a["slot"]]
        kd = kinds[short(a["class"])]
        ch = kd["children"] or {"policy": "slots", "default_slot": None, "slots": {}}
        kd["children"] = ch
        sl = ch["slots"].setdefault(slot["name"], {"many": slot["many"], "accepts": ["UIElement"], "route_by_kind": False,
                                                   "order": "native_index" if slot["many"] else "none", "attrs": {}})
        if slot["default"] and ch["default_slot"] is None:
            ch["default_slot"] = slot["name"]
        sl["attrs"][a["name"]] = {"type": "i32", "default": a["default"]}
    for mt in m.methods:
        kinds[short(mt["class"])]["methods"][mt["name"]] = {
            "dispatch": "generated", "native": mt["native"],
            "params": [{"name": kebab(p["name"]), **{k: v for k, v in wtype(f"method {mt['native']}", p["ty"]).items() if k in ("type", "nullable")}}
                       for p in mt["params"]],
            "returns": {k: v for k, v in wtype(f"method {mt['native']}", mt["ret"]).items() if k in ("type", "nullable")}
            if mt["ret"] else {"type": "void", "nullable": False}}

    def find_prop(kid: str, name: str) -> dict | None:
        for c in kinds[kid]["chain"]:
            for p in kinds[c]["props"].values():
                if name in (p["js"], p["native"]):
                    return p
        return None

    def find_sig(kid: str, name: str) -> dict | None:
        for c in kinds[kid]["chain"]:
            if name in kinds[c]["signals"]:
                return kinds[c]["signals"][name]
        return None

    for k in m.kinds:
        kid = short(k["class"])
        kd = kinds[kid]
        if kd["lifetime"] == "toplevel":
            if not any("activate" in kinds[c]["methods"] for c in kd["chain"]):
                fail(f"winui {kid}: toplevel without an annotated Activate")
            kd["present"] = "activate"
            kd["provenance"]["present"] = "Window.Activate (WIN1: the renderer's TOPLEVELS open method)"
    for cls, o in sorted(m.ovr.items()):
        for b in sorted(o.get("bindings", []), key=lambda b: b["name"]):
            kid = short(cls)
            if b["name"] != b["property"]:
                fail(f"winui {kid} bind:{b['name']}: binding names are the native prop name ({b['property']})")
            kinds[kid]["bindings"][b["name"]] = binding_from_metadata(
                f"winui {kid}", b, lambda n, c=kid: find_prop(c, n), lambda n, c=kid: find_sig(c, n))
    for kd in kinds.values():
        kd["props"] = dict(sorted(kd["props"].items()))
        kd["signals"] = dict(sorted(kd["signals"].items()))
        kd["methods"] = dict(sorted(kd["methods"].items()))
        if not kd["provenance"]:
            del kd["provenance"]
    style = style_table.build("winui", style_table.load("winui"), tags, kinds)
    return {"schema": SCHEMA, "backend": "winui", "framework": "WinUI 3 (svelters-winui, windows-rs)", "event_aliases": {},
            "tags": dict(sorted(tags.items())), "kinds": kinds, "style": style, "method_lookup": "annotated",
            "source": {"metadata": "metadata/winui/*.json + metadata/overrides/winui/*.yaml + metadata/inferred/winui/*.yaml", "style": "metadata/style/winui.yaml"}}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="exit 1 if a checked-in output is stale")
    a = ap.parse_args()
    try:
        outs = {"gtk.json": render(gtk_spec()), "qt.json": render(qt_spec()), "appkit.json": render(appkit_spec()),
                "winui.json": render(winui_spec())}
    except (SpecError, gen_cpp.GenError, style_table.StyleError, appkit_gen.GenError, winui_gen.GenError) as ex:
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
