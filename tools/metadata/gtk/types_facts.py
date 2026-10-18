#!/usr/bin/env python3
"""GTK non-GUI types (plans/research/NON-GUI-TYPES.md, N0): classification, binding scope and facts.

Promoted from docs/notes/scripts/types_gir_classify.py and types_value_mutators.py. Reads the GIR of
the GTK stack (GLib, GObject, Gio, Gdk, Gsk, Pango, Graphene, Gtk) and writes, per namespace,
`metadata/gtk/types/<Ns>.json`: one fact record per bound type (category, lifetime, hierarchy,
constructors, statics, methods, properties, signals, the signature references and the members the
rules hid). Deterministic: the same GIR always gives the same bytes.

Classification (section 3.1 of the plan), rules only; the lists live in
`metadata/overrides/gtk/types/gtk-types.yaml` and stay small (roots, exclusions, mutator exceptions):

  enumeration / bitfield                      -> enum / flags
  class / interface deriving from GObject     -> object (model when it is or implements Gio.ListModel)
  GObject.Object descendants of Gtk.Widget    -> widget (elements, not part of this taxonomy)
  fundamental class with a ref function       -> value-fundamental (not marshalled in N0)
  record with a GType and plain number fields,
    one of the six shared shapes              -> record (a plain JS object)
  record with a GType                         -> value (VALUE handle, mutators hidden)
  *Iter, TreePath, TreeRowReference           -> transient (not exposed)
  everything else                             -> needs-override

A member is bound ("auto") when every slot is one the backend's generic invoker marshals in N0
(crates/svelters-gtk/src/gi.rs): numbers, strings, enums, flags, objects, boxed structs by pointer,
GStrv, GType, unichar, GList/GSList and C arrays of those, out/inout parameters, caller-allocated
out structs, GError. Callbacks, async pairs, GVariant, hash tables, GArray/GPtrArray, fundamentals
and by-value structs are blocked with a reason (N1 lifts callbacks and async).

Usage:
  python3 tools/metadata/gtk/types_facts.py --report          # tables of the classification
  python3 tools/metadata/gtk/types_facts.py --write           # metadata/gtk/types/*.json
  python3 tools/metadata/gtk/types_facts.py --check           # the checked-in facts are current
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import naming  # noqa: E402
import yaml  # noqa: E402

C = "{http://www.gtk.org/introspection/core/1.0}"
GL = "{http://www.gtk.org/introspection/glib/1.0}"
CNS = "{http://www.gtk.org/introspection/c/1.0}"
GIR_FILES = {"GLib": "GLib-2.0", "GObject": "GObject-2.0", "Gio": "Gio-2.0", "Gdk": "Gdk-4.0", "Gsk": "Gsk-4.0",
             "Pango": "Pango-1.0", "Graphene": "Graphene-1.0", "Gtk": "Gtk-4.0", "cairo": "cairo-1.0",
             "GdkPixbuf": "GdkPixbuf-2.0"}
TARGET = ["GLib", "GObject", "Gio", "Gdk", "Gsk", "Pango", "Graphene", "Gtk"]
OVERRIDES = ROOT / "metadata/overrides/gtk/types/gtk-types.yaml"
FACTS_DIR = ROOT / "metadata/gtk/types"

INT = {"gint", "guint", "gint8", "guint8", "gint16", "guint16", "gint32", "guint32", "gint64", "guint64", "glong",
       "gulong", "gsize", "gssize", "gchar", "guchar", "gshort", "gushort", "goffset", "gintptr", "guintptr"}
FLOAT = {"gdouble", "gfloat"}
NUM = INT | FLOAT | {"gboolean", "gunichar"}
STR = {"utf8", "filename"}
# Types that are not part of the taxonomy: hidden behind Promise + AbortSignal (N1), or the core's job.
ASYNC_INFRA = {"Gio.AsyncResult", "Gio.Task"}
CORE_NAMES = {"ref", "unref", "copy", "free", "get_type", "ref_sink", "sink", "force_floating", "is_floating",
              "ref_count", "weak_ref", "weak_unref", "add_weak_pointer", "remove_weak_pointer",
              "add_toggle_ref", "remove_toggle_ref", "get_qdata", "set_qdata", "steal_qdata", "get_data",
              "set_data", "steal_data", "set_data_full", "set_qdata_full", "dup_data", "dup_qdata",
              "replace_data", "replace_qdata", "watch_closure", "run_dispose", "freeze_notify", "thaw_notify",
              "notify", "notify_by_pspec", "connect", "disconnect", "bind_property", "bind_property_full",
              "add_weak_ref_notify", "get_property", "set_property", "getv", "setv", "new_valist", "new_with_properties"}
# Record shapes that cross as a plain JS object (RECORD on the wire, crates/svelters-gtk/src/convert.rs `Rec`).
RECORD_SHAPES = {"Graphene.Rect", "Graphene.Size", "Graphene.Point", "Gdk.Rectangle", "Gdk.RGBA", "Gtk.Border"}

MUTATE = re.compile(r"^(set|append|prepend|insert|remove|add|clear|reset|change|merge|splice|take|steal|push|pop|"
                    r"update|init|unset|replace|filter|sort|scale|translate|rotate|transform|multiply|invert|"
                    r"normalize|negate|truncate|resize|fill|assign|swap|move|shrink|grow|union|intersect|subtract|"
                    r"offset|expand|inset|round|detach|squeeze|reserve|chop|erase|emplace|skew|overwrite|"
                    r"ascii_down|ascii_up|byteswap|invalidate|end|shift|adjust_time|destroy|next|cache|store|forward|backward|order)(_|$)")
READ = re.compile(r"^(get|is|has|to|equal|equals|hash|compare|contains|lookup|find|n|count|copy|dup|print|serialize|"
                  r"query|peek|iter|foreach|ref|match|parse|load|save|write|dump|as|can|bounds|area|distance|"
                  r"length|size|width|height|interpolate|intersects|near|dot|cross|format|matches|difference|"
                  r"equivalent|in|index|x|evaluate|includes|triggers|exists|generate|classify|check|range|"
                  r"enumerate|last|create|draw)(_|$)")


def skipped(el: ET.Element) -> bool:
    return el.get("introspectable") == "0" or el.get("deprecated") == "1" or el.get("moved-to") is not None


def camel(name: str) -> str:
    parts = re.split(r"[_\-]", name)
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def load_overrides() -> dict:
    if not OVERRIDES.exists():
        return {}
    return yaml.safe_load(OVERRIDES.read_text()) or {}


class Gir:
    def __init__(self, girdir: str = "/usr/share/gir-1.0") -> None:
        self.t: dict[str, tuple[str, ET.Element, str]] = {}
        self.versions: dict[str, str] = {}
        for ns, f in GIR_FILES.items():
            root = ET.parse(f"{girdir}/{f}.gir").getroot()
            n = root.find(C + "namespace")
            self.versions[ns] = n.get("version")
            for el in n:
                if el.get("name"):
                    self.t[f"{n.get('name')}.{el.get('name')}"] = (el.tag.split("}")[1], el, n.get("name"))

    def q(self, name: str | None, ns: str) -> str | None:
        if not name or name in NUM or name in STR or "." in name or name in ("none", "gpointer", "gconstpointer", "GType", "va_list"):
            return name
        return f"{ns}.{name}"

    def chain(self, full: str) -> list[str]:
        out: list[str] = []
        while full in self.t and full not in out:
            out.append(full)
            tag, el, ns = self.t[full]
            full = self.q(el.get("parent"), ns) if tag == "class" else None  # type: ignore[assignment]
        return out

    def interfaces(self, full: str) -> list[str]:
        """Interfaces a class implements (own and inherited), plus interface prerequisites."""
        out: list[str] = []
        for c in self.chain(full):
            tag, el, ns = self.t[c]
            for i in el.findall(C + "implements"):
                q = self.q(i.get("name"), ns)
                if q and q not in out:
                    out.append(q)
        return out

    def implements(self, full: str, iface: str) -> bool:
        for c in self.chain(full):
            tag, el, ns = self.t[c]
            if c == iface or any(self.q(i.get("name"), ns) == iface for i in el.findall(C + "implements")):
                return True
            if tag == "interface" and any(self.q(p.get("name"), ns) == iface for p in el.findall(C + "prerequisite")):
                return True
        return False

    def is_widget(self, full: str) -> bool:
        return "Gtk.Widget" in self.chain(full)


def plain_record(g: Gir, el: ET.Element, ns: str, seen: tuple = ()) -> bool:
    fields = [f for f in el.findall(C + "field") if f.get("private") != "1"]
    if not fields or el.get("disguised") == "1" or el.get("opaque") == "1":
        return False
    if any(f.get("private") == "1" for f in el.findall(C + "field")):
        return False
    for f in fields:
        t = f.find(C + "type")
        if t is None:
            return False
        n = g.q(t.get("name"), ns)
        if n in NUM:
            continue
        tag, sub, sns = g.t.get(n, ("?", None, None))
        if tag in ("enumeration", "bitfield"):
            continue
        if tag == "record" and n not in seen and plain_record(g, sub, sns, seen + (n,)) and "*" not in t.get(CNS + "type", ""):
            continue
        return False
    return True


def classify(g: Gir, full: str) -> str | None:
    tag, el, ns = g.t[full]
    name = full.split(".", 1)[1]
    if tag == "enumeration":
        return "enum"
    if tag == "bitfield":
        return "flags"
    if tag == "callback":
        return "callback"
    if tag == "alias":
        return "alias"
    if tag in ("constant", "function", "docsection", "boxed"):
        return None
    if tag in ("class", "interface"):
        if full in ASYNC_INFRA:
            return "async-infra"
        if tag == "class" and "GObject.Object" not in g.chain(full) and full != "GObject.Object":
            if any(g.t[c][1].get(GL + "ref-func") for c in g.chain(full)):
                return "value-fundamental"
            return "needs-override"
        if g.is_widget(full):
            return "widget"
        if g.implements(full, "Gio.ListModel"):
            return "model"
        return "object"
    if tag == "record":
        if el.get(GL + "is-gtype-struct-for"):
            return None
        if name.endswith("Private") or (name.endswith(("Iface", "Interface")) and not el.get(GL + "get-type")):
            return None
        # TextIter is a value here: its readers are bound and the buffer is unusable without it.
        if (name.endswith("Iter") and name != "TextIter") or name in ("TreePath", "TreeRowReference", "BitsetIter", "HashTableIter", "VariantIter"):
            return "transient"
        if full in RECORD_SHAPES:
            return "record"
        if el.get(GL + "get-type"):
            return "value"
        return "needs-override"
    if tag == "union":
        return "needs-override"
    return None


BINDABLE = {"object", "model", "value", "record"}


class Blocked(Exception):
    """A slot the N0 invoker cannot marshal."""


class Conv:
    """GIR slot -> the neutral type description facts carry, or Blocked(reason)."""

    def __init__(self, g: Gir, cats: dict[str, str]) -> None:
        self.g, self.cats = g, cats
        # Set once the scope is known: a class or struct outside it has no JS type to wrap it in.
        self.bound: set[str] | None = None

    def need(self, n: str) -> None:
        if self.bound is not None and n not in self.bound:
            raise Blocked(f"unbound:{n}")

    def named(self, n: str, ns: str, ctype: str, by_ref: bool) -> dict:
        g = self.g
        n = g.q(n, ns)  # type: ignore[assignment]
        if n in INT:
            return {"k": "int", "t": n}
        if n in FLOAT:
            return {"k": "float"}
        if n == "gboolean":
            return {"k": "bool"}
        if n in STR:
            return {"k": "str"}
        if n == "gunichar":
            return {"k": "unichar"}
        if n == "GType":
            return {"k": "gtype"}
        if n in ("none",):
            return {"k": "void"}
        if n in ("gpointer", "gconstpointer", "va_list"):
            raise Blocked("gpointer")
        if n in ("GLib.Variant", "GObject.Value", "GLib.VariantType"):
            raise Blocked("variant")
        if n in ("GLib.Error", "GLib.HashTable", "GLib.Array", "GLib.PtrArray", "GLib.ByteArray"):
            raise Blocked({"GLib.Error": "gerror", "GLib.HashTable": "hash table"}.get(n, "garray"))
        if n not in g.t:
            raise Blocked(f"unknown type {n}")
        tag, el, tns = g.t[n]
        c = self.cats.get(n)
        if tag == "alias":
            inner = el.find(C + "type")
            if inner is None:
                raise Blocked("alias")
            return self.named(inner.get("name"), tns, ctype, by_ref)
        if c == "enum":
            return {"k": "enum", "t": n}
        if c == "flags":
            return {"k": "flags", "t": n}
        if c == "callback":
            raise Blocked("callback")
        if c in ("object", "model", "widget", "async-infra"):
            if c == "async-infra":
                raise Blocked("async")
            self.need(n)
            return {"k": "object", "t": n}
        if c == "value-fundamental":
            raise Blocked("fundamental")
        if c == "record" and "*" in ctype:
            self.need(n)
            return {"k": "record", "t": n}
        if c in ("value", "transient") and tag == "record" and el.get(GL + "get-type"):
            if "*" not in ctype and not by_ref:
                raise Blocked("struct by value")
            self.need(n)
            return {"k": "boxed", "t": n}
        if c == "record":
            raise Blocked("struct by value")
        raise Blocked(f"type:{c or 'unknown'}:{n}")

    def slot(self, node: ET.Element, ns: str, by_ref: bool = False) -> dict:
        """The type of a parameter, return value, property, field or signal argument."""
        a = node.find(C + "array")
        t = node.find(C + "type")
        if a is not None:
            inner = a.find(C + "type")
            if inner is None:
                raise Blocked("nested array")
            if a.get("name") in ("GLib.ByteArray", "GLib.Array", "GLib.PtrArray"):
                raise Blocked("garray")
            et = self.elem(inner, ns)
            if a.get("length") is not None:
                return {"k": "array", "elem": et, "len": int(a.get("length"))}
            if a.get("zero-terminated") != "0":
                if et["k"] == "str":
                    return {"k": "strv"}
                return {"k": "array", "elem": et, "len": "zero"}
            if a.get("fixed-size"):
                return {"k": "array", "elem": et, "len": int(a.get("fixed-size")), "fixed": True}
            raise Blocked("array without length")
        if t is None:
            raise Blocked("varargs")
        n = self.g.q(t.get("name"), ns)
        if n in ("GLib.List", "GLib.SList"):
            inner = t.find(C + "type")
            if inner is None:
                raise Blocked("untyped list")
            return {"k": "list", "elem": self.elem(inner, ns), "slist": n == "GLib.SList"}
        return self.named(t.get("name"), ns, t.get(CNS + "type", ""), by_ref)

    def elem(self, inner: ET.Element, ns: str) -> dict:
        t = self.named(inner.get("name"), ns, inner.get(CNS + "type", "") or "x*", False)
        if t["k"] in ("str", "int", "float", "bool", "enum", "flags", "object", "boxed"):
            return t
        raise Blocked(f"list element {t['k']}")


def refs_of(t: dict | None) -> set[str]:
    if not t:
        return set()
    out = set()
    if t.get("k") in ("enum", "flags", "object", "boxed", "record") and t.get("t"):
        out.add(t["t"])
    if "elem" in t:
        out |= refs_of(t["elem"])
    return out


def sig_of(m: ET.Element, ns: str, cv: Conv, g: Gir, name_over: str | None = None) -> dict:
    """The bound signature of a callable, or {'blocked': reason}."""
    ps = m.find(C + "parameters")
    params = ps.findall(C + "parameter") if ps is not None else []
    hidden_idx: set[int] = set()
    for i, p in enumerate(params):
        a = p.find(C + "array")
        if a is not None and a.get("length") is not None:
            hidden_idx.add(int(a.get("length")))
    rv = m.find(C + "return-value")
    if rv is not None:
        a = rv.find(C + "array")
        if a is not None and a.get("length") is not None:
            hidden_idx.add(int(a.get("length")))
    out_params: list[dict] = []
    try:
        for i, p in enumerate(params):
            if p.find(C + "varargs") is not None or p.get("name") == "...":
                raise Blocked("varargs")
            direction = p.get("direction", "in")
            caller_alloc = p.get("caller-allocates") == "1"
            t = cv.slot(p, ns, by_ref=caller_alloc and direction == "out")
            if t["k"] == "void":
                raise Blocked("void parameter")
            entry = {"name": p.get("name"), "js": camel(p.get("name")), "type": t}
            if direction != "in":
                entry["dir"] = direction
            if p.get("nullable") == "1" or p.get("allow-none") == "1" or (p.get("optional") == "1" and direction == "out"):
                entry["nullable"] = True
            if i in hidden_idx:
                entry["hidden"] = True
            out_params.append(entry)
        ret = cv.slot(rv, ns) if rv is not None else {"k": "void"}
    except Blocked as b:
        return {"blocked": str(b)}
    r: dict = {"params": out_params, "ret": ret}
    if rv is not None and (rv.get("nullable") == "1" or rv.get("allow-none") == "1"):
        r["ret_nullable"] = True
    if m.get("throws") == "1":
        r["throws"] = True
    return r


def sig_refs(sig: dict) -> set[str]:
    out = refs_of(sig.get("ret"))
    for p in sig.get("params", []):
        out |= refs_of(p["type"])
    return out


def callables(g: Gir, full: str, cats: dict[str, str], cv: Conv, ov: dict) -> dict:
    tag, el, ns = g.t[full]
    cat = cats[full]
    hide_extra = set((ov.get("hide") or {}).get(full, []))
    keep_extra = set((ov.get("keep") or {}).get(full, []))
    builder = full.endswith("Builder")
    res: dict[str, list] = {"constructors": [], "statics": [], "methods": [], "properties": [], "signals": []}
    blocked: dict[str, str] = {}
    hidden: list[str] = []
    undecided: list[str] = []
    for kind in ("constructor", "function", "method"):
        for m in el.findall(C + kind):
            nm = m.get("name") or ""
            if skipped(m) or nm in CORE_NAMES or nm.endswith("_finish") or m.get("shadowed-by"):
                continue
            if m.get(GL + "finish-func") or m.get("async") == "1":
                blocked[nm] = "async"
                continue
            sig = sig_of(m, ns, cv, g)
            if "blocked" in sig:
                blocked[nm] = sig["blocked"]
                continue
            rec = {"name": nm, "js": camel(nm), **sig}
            if m.get("shadows"):
                rec["aliases"] = [camel(m.get("shadows"))]
            if m.get("version"):
                rec["since"] = m.get("version")
            if kind == "method":
                ip = m.find(C + "parameters/" + C + "instance-parameter")
                if cat == "value" and not builder:
                    ct = (ip.find(C + "type").get(CNS + "type") if ip is not None and ip.find(C + "type") is not None else "") or ""
                    consumes = ip is not None and ip.get("transfer-ownership") == "full"
                    const_self = "const" in ct
                    if nm in keep_extra:
                        pass
                    elif nm in hide_extra:
                        hidden.append(nm)
                        continue
                    elif consumes or const_self or READ.match(nm):
                        pass
                    elif MUTATE.match(nm):
                        hidden.append(nm)
                        continue
                    else:
                        undecided.append(nm)
                        hidden.append(nm)
                        continue
                res["methods"].append(rec)
            elif kind == "constructor":
                res["constructors"].append(rec)
            else:
                res["statics"].append(rec)
    if cat in ("object", "model"):
        for p in el.findall(C + "property"):
            if skipped(p):
                continue
            pn = p.get("name")
            try:
                t = cv.slot(p, ns)
            except Blocked as b:
                if str(b) != "variant":
                    blocked["prop:" + pn] = str(b)
                    continue
                t = {"k": "variant"}
            rec = {"name": pn, "js": camel(pn), "type": t,
                   "readable": p.get("readable", "1") != "0", "writable": p.get("writable") == "1",
                   "construct": p.get("construct") == "1", "construct_only": p.get("construct-only") == "1"}
            if p.get("version"):
                rec["since"] = p.get("version")
            res["properties"].append(rec)
        for s in el.findall(GL + "signal"):
            if skipped(s):
                continue
            ps = s.find(C + "parameters")
            args: list[dict] = []
            try:
                for p in (ps.findall(C + "parameter") if ps is not None else []):
                    args.append({"name": p.get("name"), "type": cv.slot(p, ns)})
            except Blocked as b:
                blocked["signal:" + s.get("name")] = str(b)
                continue
            rec = {"name": s.get("name"), "args": args}
            if s.get("version"):
                rec["since"] = s.get("version")
            res["signals"].append(rec)
    if cat == "record":
        res["fields"] = [{"name": f.get("name"), "type": f.find(C + "type").get("name")}
                         for f in el.findall(C + "field") if f.get("private") != "1" and f.find(C + "type") is not None]
    res["blocked"] = blocked  # type: ignore[assignment]
    res["hidden"] = sorted(hidden)  # type: ignore[assignment]
    res["undecided"] = sorted(undecided)  # type: ignore[assignment]
    return res


def lifetime_of(cat: str) -> str:
    return {"object": "shared", "model": "shared", "value": "value", "record": "record", "widget": "widget"}.get(cat, cat)


def scope(g: Gir, cats: dict[str, str], ov: dict) -> tuple[set[str], dict[str, str]]:
    """The bound types: Gtk's non-widget bindable types, the closure of non-Gtk types reachable from the
    Gtk API, and the explicit roots; minus the exclusions. Returns (types, why)."""
    exclude = set(ov.get("exclude") or [])
    why: dict[str, str] = {}

    def refs(full: str) -> set[str]:
        tag, el, ns = g.t[full]
        out = set()
        for node in el.iter(C + "type"):
            n = g.q(node.get("name"), ns)
            if n in cats and n.split(".")[0] in TARGET:
                out.add(n)
        parent = el.get("parent")
        if parent:
            out.add(g.q(parent, ns))  # type: ignore[arg-type]
        return {o for o in out if o}

    chosen: set[str] = set()
    for f, c in cats.items():
        if f.startswith("Gtk.") and c in BINDABLE:
            chosen.add(f)
            why[f] = "gtk"
    direct: set[str] = set()
    for f in [f for f in cats if f.startswith("Gtk.")]:
        direct |= {r for r in refs(f) if not r.startswith("Gtk.")}
    for r in ov.get("roots") or []:
        direct.add(r)
        why[r] = "root"
    todo = list(direct)
    seen: set[str] = set()
    while todo:
        f = todo.pop()
        if f in seen or f not in cats:
            continue
        seen.add(f)
        if cats[f] in BINDABLE and f not in exclude:
            chosen.add(f)
            why.setdefault(f, "closure")
            for r in refs(f):
                if not r.startswith("Gtk."):
                    todo.append(r)
        elif cats[f] in BINDABLE:
            pass
    # A bound class needs its ancestors and implemented interfaces bound too.
    changed = True
    while changed:
        changed = False
        for f in list(chosen):
            for a in g.chain(f)[1:] + g.interfaces(f):
                if cats.get(a) in BINDABLE and a not in chosen and a not in exclude:
                    chosen.add(a)
                    why.setdefault(a, "ancestor")
                    changed = True
    return {c for c in chosen if c not in exclude}, why


def build(girdir: str = "/usr/share/gir-1.0") -> dict:
    ov = load_overrides()
    g = Gir(girdir)
    cats: dict[str, str] = {}
    for full, (tag, el, ns) in g.t.items():
        if skipped(el):
            continue
        c = classify(g, full)
        if c:
            cats[full] = c
    cv = Conv(g, cats)
    bound, why = scope(g, cats, ov)
    cv.bound = bound | {f for f, c in cats.items() if c in ("enum", "flags")}
    curated = set(ov.get("curated") or [])
    types: dict[str, dict] = {}
    for full in sorted(bound):
        tag, el, ns = g.t[full]
        name = full.split(".", 1)[1]
        cat = cats[full]
        calls = callables(g, full, cats, cv, ov)
        chain = g.chain(full)
        rec: dict = {
            "name": name, "ns": ns, "qualified": full, "category": cat, "lifetime": lifetime_of(cat),
            "kind": "interface" if tag == "interface" else ("class" if tag == "class" else "boxed"),
            "gtype": el.get(GL + "type-name") or el.get(CNS + "type") or name,
            "scope": why.get(full, "closure"),
            "abstract": el.get("abstract") == "1",
            "parent": (g.q(el.get("parent"), ns) if tag == "class" and el.get("parent") else None),
            "interfaces": [i for i in (g.q(x.get("name"), ns) for x in el.findall(C + "implements")) if i in bound],
            "prerequisites": [p for p in (g.q(x.get("name"), ns) for x in el.findall(C + "prerequisite")) if p in bound],
            "builder": full.endswith("Builder"),
            "curated": full in curated,
            "since": el.get("version"),
        }
        if rec["parent"] and rec["parent"] not in bound:
            rec["parent"] = None
        rec.update(calls)
        # Signature references: every type reachable through a bound member's params, returns, props, signals.
        refs: set[str] = set()
        for m in calls["constructors"] + calls["statics"] + calls["methods"]:
            refs |= sig_refs(m)
        for p in calls["properties"]:
            refs |= refs_of(p["type"])
        for s in calls["signals"]:
            for a in s["args"]:
                refs |= refs_of(a["type"])
        rec["refs"] = sorted(r for r in refs if r != full and r in bound or (r != full and cats.get(r) in ("enum", "flags")))
        types[full] = rec
    return {"gir": g.versions, "types": types, "categories": cats, "bound": sorted(bound)}


def split_by_ns(data: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for full, rec in data["types"].items():
        ns = rec["ns"]
        out.setdefault(ns, {"schema": "svelters-types/v1", "toolkit": "gtk", "namespace": ns,
                            "version": data["gir"].get(ns), "types": {}})["types"][rec["name"]] = rec
    return out


def serialise(d: dict) -> str:
    return json.dumps(d, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def write(data: dict) -> int:
    FACTS_DIR.mkdir(parents=True, exist_ok=True)
    files = split_by_ns(data)
    for ns, d in files.items():
        (FACTS_DIR / f"{ns}.json").write_text(serialise(d))
    for old in FACTS_DIR.glob("*.json"):
        if old.stem not in files and old.stem not in ("report", "facts"):
            old.unlink()
    (FACTS_DIR / "report.txt").write_text(report(data))
    return len(files)


def check(data: dict) -> int:
    bad = []
    files = split_by_ns(data)
    for ns, d in files.items():
        p = FACTS_DIR / f"{ns}.json"
        if not p.exists() or p.read_text() != serialise(d):
            bad.append(f"{ns}.json")
    rp = FACTS_DIR / "report.txt"
    if not rp.exists() or rp.read_text() != report(data):
        bad.append("report.txt")
    if bad:
        print("types_facts: stale facts: " + ", ".join(bad) + " (run `python3 tools/metadata/gtk/types_facts.py --write`)", file=sys.stderr)
        return 1
    print(f"types_facts: {sum(len(d['types']) for d in files.values())} types in {len(files)} namespaces are current")
    return 0


def report(data: dict) -> str:
    """The classification report that is checked in, so drift shows up in review."""
    cats = data["categories"]
    lines: list[str] = []
    order = ["record", "value", "object", "model", "async-infra", "transient", "value-fundamental", "needs-override", "enum", "flags", "callback", "alias", "widget"]
    per: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for full, c in cats.items():
        ns = full.split(".")[0]
        if ns in TARGET:
            per[ns][c] += 1
    lines.append("== types by category (all types of each namespace, GIR " + ", ".join(f"{k} {v}" for k, v in sorted(data['gir'].items()) if k in TARGET) + ")")
    lines.append(f"{'ns':9}" + "".join(f"{o[:9]:>10}" for o in order))
    tot: collections.Counter = collections.Counter()
    for ns in TARGET:
        lines.append(f"{ns:9}" + "".join(f"{per[ns][o]:10}" for o in order))
        tot.update(per[ns])
    lines.append(f"{'total':9}" + "".join(f"{tot[o]:10}" for o in order))
    lines.append("")
    lines.append("== bound scope (types with a JS class or shape), by namespace and category")
    bound = data["types"]
    by_ns: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for full, rec in bound.items():
        by_ns[rec["ns"]][rec["category"]] += 1
    for ns in TARGET:
        if by_ns[ns]:
            lines.append(f"  {ns:9} {sum(by_ns[ns].values()):4}  " + ", ".join(f"{k} {v}" for k, v in sorted(by_ns[ns].items())))
    lines.append(f"  total     {len(bound):4}")
    lines.append("")
    lines.append("== members of bound types: bound / blocked")
    mem: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    reasons: collections.Counter = collections.Counter()
    hidden = 0
    undecided: dict[str, list[str]] = collections.defaultdict(list)
    for full, rec in bound.items():
        ns = rec["ns"]
        n_ok = sum(len(rec[k]) for k in ("constructors", "statics", "methods", "properties", "signals"))
        mem[ns]["bound"] += n_ok
        mem[ns]["blocked"] += len(rec["blocked"])
        hidden += len(rec["hidden"])
        for r in rec["blocked"].values():
            reasons[r.split(":")[0] if not r.startswith("type:") else r] += 1
        if rec["undecided"]:
            undecided[full] = rec["undecided"]
    for ns in TARGET:
        m = mem[ns]
        n = m["bound"] + m["blocked"]
        if n:
            lines.append(f"  {ns:9} members {n:5}  bound {m['bound']:5} ({100 * m['bound'] // n:3}%)  blocked {m['blocked']:4}")
    lines.append(f"  VALUE mutators hidden by the rules: {hidden}")
    lines.append(f"  residual hand list (undecided mutators, hidden until decided): {sum(len(v) for v in undecided.values())} methods in {len(undecided)} types")
    for t, ms in sorted(undecided.items()):
        lines.append(f"    {t}: {' '.join(ms)}")
    lines.append("  blocking reasons: " + ", ".join(f"{k} {v}" for k, v in reasons.most_common(14)))
    lines.append("")
    lines.append("== curated set")
    for full, rec in sorted(bound.items()):
        if rec["curated"]:
            n_ok = sum(len(rec[k]) for k in ("constructors", "statics", "methods", "properties", "signals"))
            lines.append(f"  {full:24} {rec['category']:7} bound {n_ok:4} blocked {len(rec['blocked']):3} hidden {len(rec['hidden']):3}")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    data = build(a.girdir)
    if a.report:
        sys.stdout.write(report(data))
    if a.write:
        print(f"types_facts: wrote {write(data)} namespace files to {FACTS_DIR.relative_to(ROOT)}")
    if a.check:
        return check(data)
    return 0


if __name__ == "__main__":
    sys.exit(main())
