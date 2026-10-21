#!/usr/bin/env python3
"""Classifies the WinRT non-GUI types (N4, plans/research/NON-GUI-TYPES.md sections 3.1, 3.4, 4.1) into facts.

Pure: reads only metadata/winui/types/ingested/*.json (written by types_winui_ingest.py from winmd) and the overrides
metadata/overrides/winui/types/winui-types.yaml, so `--check` runs without the Windows App SDK.

Output (deterministic, checked in):
  metadata/winui/types/<Namespace>.json   per ingested namespace: every type's category, and for bound kinds their
                                          members at the ABI level (interface, vtable slot, parameter types)
  metadata/winui/types/report.txt         classification counts, bound scope, member coverage, overloads

Taxonomy (section 3.1, WinUI column): struct -> record (DateTime/TimeSpan -> JS Date / milliseconds); runtime class ->
object (SHARED, decision Q1/Q8), model when it implements IVector/IObservableVector/IMap/IObservableMap, statics when it
has only static members, widget when it derives from UIElement (elements, not bound here); `IAsyncOperation*` /
`IAsyncAction*` -> async (Promise); enum (`Flags` -> flags); delegate -> callback; Deprecated -> excluded.

Thread class (section 4.1): `dispatcher` when the class or a base exposes an instance `DispatcherQueue` property,
`ui` for the rest of Microsoft.UI.Xaml.*, else `any`.

Overloads: one JS name per native name; the backend picks the overload by argument count (WinRT forbids two overloads
of one arity on one interface except through `DefaultOverload`). When two overloads of a name in one kind share an
arity, the `DefaultOverload` keeps the name and the other is renamed to its `Overload` attribute name.

  python3 tools/metadata/winui/types_winui_classify.py --report
  python3 tools/metadata/winui/types_winui_classify.py --write
  python3 tools/metadata/winui/types_winui_classify.py --check
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import svmeta  # noqa: E402

ING = ROOT / "metadata/winui/types/ingested"
OUT = ROOT / "metadata/winui/types"
OVERRIDES = ROOT / "metadata/overrides/winui/types/winui-types.yaml"

PRIMS = {"bool", "char16", "u8", "i8", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64", "string", "object", "void"}
INTS = {"u8", "i8", "i16", "u16", "i32", "u32", "i64", "u64"}
UIELEMENT = "Microsoft.UI.Xaml.UIElement"
WFC = "Windows.Foundation.Collections."
COLLECTIONS = {"IVector", "IVectorView", "IIterable", "IIterator", "IMap", "IMapView", "IObservableVector", "IObservableMap",
               "IKeyValuePair", "IMapChangedEventArgs", "IVectorChangedEventArgs"}
ASYNC = {"Windows.Foundation.IAsyncAction", "Windows.Foundation.IAsyncOperation", "Windows.Foundation.IAsyncActionWithProgress",
         "Windows.Foundation.IAsyncOperationWithProgress", "Windows.Foundation.IAsyncInfo"}
# Structs with a JS form of their own (the JS projection's rule): DateTime is a Date, TimeSpan milliseconds.
SPECIAL_STRUCTS = {"Windows.Foundation.DateTime": "datetime", "Windows.Foundation.TimeSpan": "timespan"}
# Structs the core handles itself and JS never sees.
INTERNAL_STRUCTS = {"Windows.Foundation.EventRegistrationToken", "Windows.Foundation.HResult"}
MODEL_IFACES = {WFC + "IVector", WFC + "IObservableVector", WFC + "IMap", WFC + "IObservableMap"}


class Block(Exception):
    """A type that cannot be marshalled (the reason ends up in `blocked`)."""


# ---------------------------------------------------------------------------------------------- type spellings

def split_args(inner: str) -> list[str]:
    out, depth, cur = [], 0, ""
    for ch in inner:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def parse(t: str) -> tuple[str, list[str]]:
    """`A<B, C<D>>` -> ("A", ["B", "C<D>"])."""
    t = t.strip()
    if "<" not in t:
        return t, []
    head, rest = t.split("<", 1)
    return head, split_args(rest[:-1])


def spell(head: str, args: list[str]) -> str:
    return f"{head}<{', '.join(args)}>" if args else head


def subst(t: str, m: dict[str, str]) -> str:
    """Replaces generic parameter names (`T`, `TResult`, ...) in a type spelling."""
    if not m:
        return t
    if t.startswith("out "):
        return "out " + subst(t[4:], m)
    if t.endswith("[]"):
        return subst(t[:-2], m) + "[]"
    head, args = parse(t)
    if not args:
        return m.get(head, head)
    return spell(head, [subst(a, m) for a in args])


def camel(s: str) -> str:
    """JS projection casing: `AbsoluteUri` -> `absoluteUri`, `UIElement` -> `uiElement`, `ID` -> `id`."""
    n = len(s) - len(s.lstrip("ABCDEFGHIJKLMNOPQRSTUVWXYZ"))
    if n == 0:
        return s
    if n == len(s):
        return s.lower()
    if n == 1:
        return s[0].lower() + s[1:]
    return s[: n - 1].lower() + s[n - 1:]


def kebab(s: str) -> str:
    return re.sub(r"(?<=[a-z0-9])([A-Z])|(?<=[A-Z])([A-Z][a-z])", lambda m: "-" + (m.group(1) or m.group(2)), s).lower()


def js_module(ns: str) -> str:
    return "svelters:winui/" + ns.lower()


# ---------------------------------------------------------------------------------------------- IIDs

WINRT_NS = bytes.fromhex("11f47ad57b7342c0abae878b1e16adee")


def guid_from_signature(sig: str) -> str:
    """The IID of a parameterized type (the WinRT type system's UUIDv5 over the type signature)."""
    h = hashlib.sha1(WINRT_NS + sig.encode()).digest()
    d3 = (int.from_bytes(h[6:8], "big") & 0x0FFF) | (5 << 12)
    d4 = bytearray(h[8:16])
    d4[0] = (d4[0] & 0x3F) | 0x80
    return f"{h[0:4].hex()}-{h[4:6].hex()}-{d3:04x}-{d4[:2].hex()}-{d4[2:].hex()}"


SIG_PRIM = {"bool": "b1", "char16": "c2", "u8": "u1", "i8": "i1", "i16": "i2", "u16": "u2", "i32": "i4", "u32": "u4",
            "i64": "i8", "u64": "u8", "f32": "f4", "f64": "f8", "string": "string", "object": "cinterface(IInspectable)",
            "System.Guid": "g16"}


# ---------------------------------------------------------------------------------------------- ingest

class Ingest:
    def __init__(self) -> None:
        self.types: dict[str, dict] = {}
        self.inside: set[str] = set()
        self.namespaces: list[str] = []
        for p in sorted(ING.glob("*.json")):
            d = json.loads(p.read_text())
            for n, t in d["types"].items():
                self.types[n] = t
                if p.name != "_outside.json":
                    self.inside.add(n)
            if d.get("namespace"):
                self.namespaces.append(d["namespace"])

    def get(self, full: str) -> dict | None:
        return self.types.get(parse(full)[0])

    def cat(self, full: str) -> str | None:
        t = self.get(full)
        return t["category"] if t else None

    def attr(self, t: dict, name: str) -> list:
        return (t.get("attributes") or {}).get(name) or []

    # ---- signatures and IIDs
    def signature(self, full: str) -> str:
        if full in SIG_PRIM:
            return SIG_PRIM[full]
        head, args = parse(full)
        t = self.get(head)
        if t is None:
            raise Block(f"unknown:{head}")
        c = t["category"]
        if c == "enum":
            return f"enum({head};{'u4' if t.get('underlying') == 'u32' else 'i4'})"
        if c == "struct":
            return f"struct({head};{';'.join(self.signature(f['type']) for f in t['fields'])})"
        if c == "class":
            d = self.default_interface(head)
            if d is None:
                raise Block(f"no-default-interface:{head}")
            return f"rc({head};{self.signature(d)})"
        if c in ("interface", "delegate"):
            g = "{" + t["guid"] + "}"
            if args:
                return f"pinterface({g};{';'.join(self.signature(a) for a in args)})"
            return g if c == "interface" else f"delegate({g})"
        raise Block(f"unsignable:{head}")

    def iid(self, full: str) -> str:
        head, args = parse(full)
        t = self.get(head)
        if t is None:
            raise Block(f"unknown:{head}")
        if t["category"] == "class":
            d = self.default_interface(head)
            if d is None:
                raise Block(f"no-default-interface:{head}")
            return self.iid(d)
        if not args:
            return t["guid"]
        return guid_from_signature(self.signature(full))

    # ---- classes and interfaces
    def default_interface(self, cls: str) -> str | None:
        t = self.get(cls)
        for i in (t or {}).get("interfaces", []):
            if i["default"]:
                return i["name"]
        return None

    def bases(self, cls: str) -> list[str]:
        out, cur = [], self.get(cls)
        while cur and cur.get("category") == "class":
            ext = cur.get("extends")
            if not ext or ext.startswith("System."):
                break
            out.append(ext)
            cur = self.get(ext)
        return out

    def iface_methods(self, inst: str) -> list[dict]:
        """Methods of an (instantiated) interface, generic parameters substituted; `slot` is the vtable index."""
        head, args = parse(inst)
        t = self.get(head)
        if t is None or t["category"] != "interface":
            return []
        m = dict(zip(t.get("generics") or [], args))
        out = []
        for i, meth in enumerate(t.get("methods", [])):
            out.append({**meth, "slot": 6 + i, "iface": inst, "ret": subst(meth["ret"], m),
                        "params": [{**p, "type": subst(p["type"], m)} for p in meth["params"]]})
        return out

    def iface_closure(self, inst: str) -> list[str]:
        out: list[str] = []
        stack = [inst]
        while stack:
            i = stack.pop(0)
            if i in out:
                continue
            out.append(i)
            head, args = parse(i)
            t = self.get(head)
            if t and t["category"] == "interface":
                m = dict(zip(t.get("generics") or [], args))
                stack.extend(subst(r, m) for r in t.get("requires", []))
        return out

    def class_ifaces(self, cls: str, own_only: bool = True) -> list[str]:
        """Instance interfaces (with required ones) of a class; without those its bases already implement."""
        t = self.get(cls) or {}
        mine: list[str] = []
        for i in t.get("interfaces", []):
            if i["overridable"] or i["protected"]:
                continue
            for j in self.iface_closure(i["name"]):
                if j not in mine:
                    mine.append(j)
        if own_only:
            inherited = set()
            for b in self.bases(cls):
                inherited |= set(self.class_ifaces(b, own_only=False))
            mine = [i for i in mine if i not in inherited]
        return mine

    def implements(self, cls: str) -> set[str]:
        """Generic heads of every interface a class (or a base) implements."""
        out = set()
        for c in [cls] + self.bases(cls):
            out |= {parse(i)[0] for i in self.class_ifaces(c, own_only=False)}
        return out

    def factories(self, cls: str) -> tuple[bool, list[tuple[str, bool]]]:
        """(default-constructible, [(factory interface, composable)])."""
        t = self.get(cls) or {}
        default, out = False, []
        for vals in self.attr(t, "Activatable"):
            if vals and isinstance(vals[0], str) and "." in vals[0] and not vals[0].endswith("Contract"):
                out.append((vals[0], False))
            else:
                default = True
        for vals in self.attr(t, "Composable"):
            if len(vals) >= 2 and vals[1] == "CompositionType.2":
                out.append((vals[0], True))
        return default, out

    def statics_ifaces(self, cls: str) -> list[str]:
        t = self.get(cls) or {}
        return [v[0] for v in self.attr(t, "Static") if v and isinstance(v[0], str)]

    def deprecated(self, t: dict) -> bool:
        return bool(self.attr(t, "Deprecated"))


def load_overrides(ing: Ingest) -> dict:
    ov = svmeta.load_yaml(OVERRIDES)
    names = list(ov.get("curated", [])) + list(ov.get("roots", [])) + list(ov.get("exclude", [])) + list(ov.get("subjects", {}))
    names += [m.rsplit(".", 1)[0] for m in ov.get("exclude_members", [])]
    missing = sorted({n for n in names if n not in ing.types})
    if missing:
        raise SystemExit(f"types_winui_classify: overrides name unknown types: {missing}")
    for ns in ov.get("scope", []):
        if ns not in ing.namespaces:
            raise SystemExit(f"types_winui_classify: scope namespace {ns} is not ingested")
    return ov


# ---------------------------------------------------------------------------------------------- classification

def classify(ing: Ingest, full: str, ov: dict) -> str:
    t = ing.types[full]
    c = t["category"]
    if full in ov.get("exclude", []):
        return "excluded"
    if ing.deprecated(t):
        return "excluded"
    if c == "enum":
        return "flags" if t.get("flags") else "enum"
    if c == "struct":
        if full in INTERNAL_STRUCTS:
            return "internal"
        return "record"
    if c == "delegate":
        return "callback"
    if c == "interface":
        head = full
        if head in ASYNC:
            return "async"
        if t.get("generics"):
            if head.startswith(WFC) and head[len(WFC):] in COLLECTIONS:
                return "collection"
            if head == "Windows.Foundation.IReference" or head == "Windows.Foundation.IReferenceArray":
                return "reference"
            return "generic"
        if ing.attr(t, "ExclusiveTo"):
            return "class-interface"
        return "interface"
    if c == "class":
        chain = [full] + ing.bases(full)
        if UIELEMENT in chain:
            return "widget"
        default, facs = ing.factories(full)
        instance = [m for i in ing.class_ifaces(full, own_only=False) for m in ing.iface_methods(i)]
        if not default and not facs and not instance and not ing.bases(full):
            return "statics"
        if ing.implements(full) & MODEL_IFACES:
            return "model"
        if full.endswith("EventArgs"):
            return "args"
        return "object"
    return "unclassified"


def thread_class(ing: Ingest, full: str, dq: dict | None) -> str:
    if dq:
        return "dispatcher"
    if full.startswith("Microsoft.UI.Xaml"):
        return "ui"
    return "any"


# ---------------------------------------------------------------------------------------------- slots

class Types:
    """Type spelling -> JS slot dict (`k` plus details), or Block with the reason."""

    def __init__(self, ing: Ingest, cats: dict[str, str], ov: dict) -> None:
        self.ing, self.cats, self.ov = ing, cats, ov
        self.scope = set(ov.get("scope", []))
        self.roots = set(ov.get("roots", []))
        self.wanted: set[str] = set()   # kinds the closure asks for (classes, interfaces, generic instances)

    def bindable_kind(self, head: str) -> bool:
        ns = head.rsplit(".", 1)[0]
        return head in self.roots or ns in self.scope

    def record_ok(self, full: str, seen: tuple = ()) -> None:
        t = self.ing.get(full)
        for f in t["fields"]:
            ft = f["type"]
            if ft in INTS or ft in ("f32", "f64", "bool", "char16"):
                continue
            fc = self.ing.cat(ft)
            if fc == "enum":
                continue
            if fc == "struct" and ft not in seen:
                self.record_ok(ft, seen + (full,))
                continue
            raise Block(f"record-field:{full}.{f['name']}")

    def slot(self, t: str, ret: bool = False) -> dict:
        if t.endswith("[]"):
            raise Block("array")
        if t == "void":
            return {"k": "void"}
        if t == "bool":
            return {"k": "bool"}
        if t == "char16":
            return {"k": "char"}
        if t in INTS:
            return {"k": "int", "t": t}
        if t in ("f32", "f64"):
            return {"k": "float", "t": t}
        if t == "string":
            return {"k": "string"}
        if t == "object":
            return {"k": "variant"}
        if t == "System.Guid":
            return {"k": "guid"}
        head, args = parse(t)
        it = self.ing.get(head)
        if it is None:
            raise Block(f"unknown:{head}")
        cat = self.cats.get(head) or classify_outside(self.ing, head)
        c = it["category"]
        if c == "enum":
            return {"k": "flags" if it.get("flags") else "enum", "t": head}
        if c == "struct":
            if head in SPECIAL_STRUCTS:
                return {"k": SPECIAL_STRUCTS[head]}
            if head in INTERNAL_STRUCTS:
                raise Block(f"internal:{head}")
            self.record_ok(head)
            return {"k": "record", "t": head}
        if c == "delegate":
            raise Block("delegate-parameter")
        if c == "class":
            if cat == "widget":
                return {"k": "widget", "t": head}
            if cat == "excluded":
                raise Block(f"excluded:{head}")
            if not self.bindable_kind(head):
                raise Block(f"out-of-scope:{head}")
            self.wanted.add(head)
            return {"k": "object", "t": head}
        if c == "interface":
            if head in ASYNC:
                if not ret:
                    raise Block("async-argument")
                if head == "Windows.Foundation.IAsyncInfo":
                    raise Block("async-info")
                of = {"k": "void"} if "Action" in head else self.slot(args[0], ret=True)
                return {"k": "async", "of": of, "t": t}
            if head == "Windows.Foundation.IReference":
                inner = self.slot(args[0])
                if inner["k"] not in ("bool", "char", "int", "float", "enum", "flags", "datetime", "timespan", "guid", "string"):
                    raise Block(f"reference-of:{inner['k']}")
                return {"k": "nullable", "of": inner, "t": t}
            if head.startswith(WFC) and head[len(WFC):] in COLLECTIONS:
                name = head[len(WFC):]
                if name in ("IVectorView", "IIterable"):
                    if not ret:
                        raise Block("collection-argument")
                    return {"k": "list", "of": self.slot(args[0], ret=True), "t": t}
                if name == "IMapView":
                    if not ret:
                        raise Block("collection-argument")
                    key = self.slot(args[0])
                    if key["k"] != "string":
                        raise Block("map-key")
                    return {"k": "map", "of": self.slot(args[1], ret=True), "t": t}
                if name == "IKeyValuePair":
                    return {"k": "pair", "key": self.slot(args[0], ret=True), "of": self.slot(args[1], ret=True), "t": t}
                if name in ("IVector", "IObservableVector", "IMap", "IObservableMap"):
                    if not ret:
                        raise Block("collection-argument")
                    for a in args:
                        self.slot(a, ret=True)
                        self.slot(a)
                    self.wanted.add(t)
                    return {"k": "object", "t": t, "generic": True}
                if name in ("IMapChangedEventArgs", "IVectorChangedEventArgs"):
                    for a in args:
                        self.slot(a, ret=True)
                    self.wanted.add(t)
                    return {"k": "object", "t": t, "generic": True}
                raise Block(f"transient:{head}")
            if args:
                raise Block(f"generic:{head}")
            if cat == "class-interface":
                raise Block(f"class-interface:{head}")
            if cat == "excluded":
                raise Block(f"excluded:{head}")
            if not self.bindable_kind(head):
                raise Block(f"out-of-scope:{head}")
            self.wanted.add(head)
            return {"k": "object", "t": head, "interface": True}
        raise Block(f"unknown:{head}")


def classify_outside(ing: Ingest, full: str) -> str:
    t = ing.get(full)
    if t and t["category"] == "class" and UIELEMENT in [full] + ing.bases(full):
        return "widget"
    if t and t["category"] == "interface" and ing.attr(t, "ExclusiveTo"):
        return "class-interface"
    return "outside"


def refs_of(s: dict) -> set[str]:
    out = set()
    if s.get("k") in ("object", "record", "enum", "flags", "widget") and s.get("t"):
        out.add(s["t"])
    for key in ("of", "key"):
        if isinstance(s.get(key), dict):
            out |= refs_of(s[key])
    return out


# ---------------------------------------------------------------------------------------------- members

def method_params(ty: Types, m: dict, drop_tail: int = 0) -> tuple[list[dict], list[dict], dict]:
    """(in params, out params, return slot) of an interface method; raises Block."""
    params = m["params"][: len(m["params"]) - drop_tail] if drop_tail else m["params"]
    ins, outs = [], []
    for p in params:
        if p["type"].startswith("out "):
            outs.append({"name": p["name"], "type": p["type"][4:], "slot": ty.slot(p["type"][4:], ret=True)})
        elif p.get("out"):
            raise Block("array")   # FillArray: an `out` array the caller allocates
        else:
            ins.append({"name": p["name"], "type": p["type"], "slot": ty.slot(p["type"])})
    ret = ty.slot(m["ret"], ret=True)
    if ret["k"] == "async" and outs:
        raise Block("async-with-outs")
    return ins, outs, ret


def overload_names(ovs: list[dict]) -> None:
    """Same-arity overloads: the DefaultOverload keeps the name, the other takes its `Overload` attribute name."""
    by_arity: dict[int, list[dict]] = defaultdict(list)
    for o in ovs:
        by_arity[len(o["params"])].append(o)
    for group in by_arity.values():
        if len(group) < 2:
            continue
        keep = next((o for o in group if (o.get("attributes") or {}).get("DefaultOverload")), group[0])
        for o in group:
            if o is not keep:
                alias = ((o.get("attributes") or {}).get("Overload") or [[None]])[0][0]
                o["rename"] = camel(alias) if alias else None


def build_kind(ing: Ingest, full: str, cat: str, ty: Types, ov: dict) -> dict:
    """Members of a bound kind (class, interface, or generic interface instance) at the ABI level."""
    t = ing.get(full) or {}
    is_class = t.get("category") == "class"
    excluded_members = {m.rsplit(".", 1)[1] for m in ov.get("exclude_members", []) if m.rsplit(".", 1)[0] == full}
    rec: dict = {"category": cat, "props": [], "methods": {}, "statics": {}, "events": [], "constructors": [],
                 "blocked": {}, "hidden": [], "excluded": {}, "refs": set()}
    ifaces = ing.class_ifaces(full) if is_class else ing.iface_closure(full)
    if is_class:
        rec["default_interface"] = ing.default_interface(full)
    raw: list[dict] = []
    for i in ifaces:
        if parse(i)[0] in ASYNC:
            continue
        raw.extend(ing.iface_methods(i))
    static_raw: list[dict] = []
    for i in ing.statics_ifaces(full) if is_class else []:
        static_raw.extend(ing.iface_methods(i))

    def note_block(key: str, why: str) -> None:
        rec["blocked"][key] = why

    def excluded_member(m: dict) -> str | None:
        if (m.get("attributes") or {}).get("Deprecated"):
            return "deprecated"
        base = re.sub(r"^(get_|put_|add_|remove_)", "", m["name"])
        if base in excluded_members or m["name"] in excluded_members:
            return "override"
        return None

    # ---- props and events (SpecialName accessors), methods
    props: dict[str, dict] = {}
    events: dict[str, dict] = {}
    methods: dict[str, list[dict]] = defaultdict(list)
    for m in raw:
        why = excluded_member(m)
        if why:
            rec["excluded"][m["name"]] = why
            continue
        n = m["name"]
        if m.get("special") and n.startswith(("get_", "put_")):
            p = props.setdefault(n[4:], {"native": n[4:]})
            p["get" if n.startswith("get_") else "set"] = m
        elif m.get("special") and n.startswith("add_"):
            events.setdefault(n[4:], {})["add"] = m
        elif m.get("special") and n.startswith("remove_"):
            events.setdefault(n[7:], {})["remove"] = m
        else:
            methods[n].append(m)

    # DP statics (`FooProperty`): the instance prop `Foo` is observable through RegisterPropertyChangedCallback.
    dps: dict[str, dict] = {}
    statics_props: dict[str, dict] = {}
    static_methods: dict[str, list[dict]] = defaultdict(list)
    for m in static_raw:
        why = excluded_member(m)
        if why:
            rec["excluded"]["static:" + m["name"]] = why
            continue
        n = m["name"]
        if m.get("special") and n.startswith("get_") and m["ret"] == "Microsoft.UI.Xaml.DependencyProperty" and n.endswith("Property"):
            dps[n[4:-len("Property")]] = {"iface": m["iface"], "slot": m["slot"]}
            rec["hidden"].append(n[4:])
        elif m.get("special") and n.startswith(("get_", "put_")):
            statics_props.setdefault(n[4:], {})["get" if n.startswith("get_") else "set"] = m
        elif m.get("special") and n.startswith(("add_", "remove_")):
            if n.startswith("add_"):
                note_block("static-event:" + n[4:], "static-event")
        else:
            static_methods[n].append(m)

    for name in sorted(props):
        p = props[name]
        g, s = p.get("get"), p.get("set")
        try:
            slot = ty.slot(g["ret"], ret=True) if g else ty.slot(s["params"][0]["type"])
            if g and s:
                ty.slot(s["params"][0]["type"])
            if slot["k"] == "async":
                raise Block("async-property")
        except Block as e:
            note_block(name, str(e))
            continue
        ptype = g["ret"] if g else s["params"][0]["type"]
        rec["props"].append({
            "name": camel(name), "native": name, "type": ptype, "slot": slot,
            "get": {"iface": g["iface"], "slot": g["slot"]} if g else None,
            "set": {"iface": s["iface"], "slot": s["slot"]} if s else None,
            "dp": dps.get(name)})
        rec["refs"] |= refs_of(slot)

    for name in sorted(events):
        e = events[name]
        add = e.get("add")
        if not add or "remove" not in e:
            note_block("event:" + name, "event-without-remove")
            continue
        dname = add["params"][0]["type"]
        dhead, dargs = parse(dname)
        dt = ing.get(dhead)
        try:
            if not dt or dt["category"] != "delegate" or not dt.get("invoke"):
                raise Block(f"unknown-delegate:{dhead}")
            m = dict(zip(dt.get("generics") or [], dargs))
            args = []
            for p in dt["invoke"]["params"]:
                pt = subst(p["type"], m)
                if pt.startswith("out ") or p.get("out"):
                    raise Block("event-out-argument")
                s = ty.slot(pt, ret=True)
                if s["k"] == "float":
                    raise Block("event-float-argument")
                args.append({"name": p["name"], "type": pt, "slot": s})
            if dt["invoke"]["ret"] != "void":
                raise Block("event-with-result")
        except Block as ex:
            note_block("event:" + name, str(ex))
            continue
        rec["events"].append({"name": kebab(name), "native": name, "iface": add["iface"], "add": add["slot"],
                              "remove": e["remove"]["slot"], "delegate": dname, "args": args})
        for a in args:
            rec["refs"] |= refs_of(a["slot"])

    def build_group(native: str, ovs: list[dict], static: bool) -> list[tuple[str, dict]]:
        out = []
        for o in ovs:
            try:
                ins, outs, ret = method_params(ty, o)
            except Block as ex:
                note_block(("static:" if static else "") + native + f"/{len(o['params'])}", str(ex))
                continue
            entry = {"native": native, "iface": o["iface"], "slot": o["slot"], "params": ins, "outs": outs, "ret": ret,
                     "ret_native": o["ret"], "attributes": o.get("attributes") or {}}
            out.append(entry)
        overload_names(out)
        res = []
        taken: set[tuple[str, int]] = set()
        for e in out:
            js = e.pop("rename", None) or camel(native)
            e.pop("attributes")
            if (js, len(e["params"])) in taken:
                note_block(("static:" if static else "") + native + f"/{len(e['params'])}", "duplicate-arity")
                continue
            taken.add((js, len(e["params"])))
            res.append((js, e))
            for p in e["params"] + e["outs"]:
                rec["refs"] |= refs_of(p["slot"])
            rec["refs"] |= refs_of(e["ret"])
        return res

    for native in sorted(methods):
        for js, e in build_group(native, methods[native], False):
            rec["methods"].setdefault(js, []).append(e)
    for native in sorted(static_methods):
        for js, e in build_group(native, static_methods[native], True):
            rec["statics"].setdefault(js, []).append(e)
    for name in sorted(statics_props):
        sp = statics_props[name]
        for which, m in (("get", sp.get("get")), ("set", sp.get("set"))):
            if not m:
                continue
            try:
                ins, outs, ret = method_params(ty, m)
            except Block as ex:
                note_block(f"static:{which}_{name}", str(ex))
                continue
            js = camel(name) if which == "get" else "set" + name
            rec["statics"].setdefault(js, []).append({"native": m["name"], "iface": m["iface"], "slot": m["slot"],
                                                       "params": ins, "outs": outs, "ret": ret, "ret_native": m["ret"], "prop": which})
            rec["refs"] |= refs_of(ret)

    # ---- constructors
    if is_class and cat not in ("statics",):
        default, facs = ing.factories(full)
        if default:
            rec["constructors"].append({"iface": None, "slot": 6, "params": [], "composable": False})
        for fi, composable in facs:
            for m in ing.iface_methods(fi):
                try:
                    ins, outs, ret = method_params(ty, m, drop_tail=2 if composable else 0)
                except Block as ex:
                    note_block(f"new/{m['name']}", str(ex))
                    continue
                if outs:
                    note_block(f"new/{m['name']}", "constructor-with-outs")
                    continue
                rec["constructors"].append({"iface": fi, "slot": m["slot"], "params": ins, "composable": composable,
                                            "native": m["name"]})
                for p in ins:
                    rec["refs"] |= refs_of(p["slot"])
        # Two constructors of one arity: keep the first (WinRT factories do not overload by type in practice).
        seen: dict[int, str] = {}
        keep = []
        for c in rec["constructors"]:
            n = len(c["params"])
            if n in seen:
                note_block(f"new/{c.get('native', 'ActivateInstance')}", f"ambiguous-constructor-arity:{n}")
                continue
            seen[n] = c.get("native", "")
            keep.append(c)
        rec["constructors"] = keep

    # Props shadowing a same-named method group (rare): the method wins, the prop is blocked.
    for p in list(rec["props"]):
        if p["name"] in rec["methods"]:
            note_block(p["native"], "shadowed-by-method")
            rec["props"].remove(p)
    rec["refs"] = sorted(rec["refs"] - {full})
    return rec


def iid_or_none(ing: Ingest, n: str) -> str | None:
    try:
        return ing.iid(n)
    except Block:
        return None


def count_members(rec: dict) -> tuple[int, int]:
    bound = len(rec["props"]) + len(rec["events"]) + sum(len(v) for v in rec["methods"].values()) \
        + sum(len(v) for v in rec["statics"].values()) + len(rec["constructors"])
    return bound, len(rec["blocked"])


# ---------------------------------------------------------------------------------------------- build

def build() -> dict:
    ing = Ingest()
    ov = load_overrides(ing)
    cats = {n: classify(ing, n, ov) for n in sorted(ing.inside)}
    ty = Types(ing, cats, ov)
    bound: dict[str, dict] = {}
    queue = list(ov.get("roots", []))
    while queue:
        n = queue.pop(0)
        if n in bound:
            continue
        head = parse(n)[0]
        cat = cats.get(head) or classify(ing, head, ov) if head in ing.types else None
        if parse(n)[1]:
            cat = "collection" if head.startswith(WFC) else "generic"
        if cat in ("excluded", "widget", None):
            continue
        rec = build_kind(ing, n, cat, ty, ov)
        bound[n] = rec
        if ing.get(n) and ing.get(n)["category"] == "class":
            for b in ing.bases(n):
                bc = cats.get(b) or classify(ing, b, ov)
                if bc not in ("excluded", "widget") and b not in bound:
                    queue.append(b)
        for w in sorted(ty.wanted):
            if w not in bound and w not in queue:
                queue.append(w)
        ty.wanted.clear()

    # Parent: nearest bound base class.
    for n, rec in bound.items():
        rec["parent"] = next((b for b in ing.bases(n) if b in bound), None) if not parse(n)[1] else None
        rec["implements"] = sorted(i for i in (ing.class_ifaces(n, own_only=False) if ing.get(n) and ing.get(n)["category"] == "class" else [])
                                   if i in bound and i != n)

    # Thread class (section 4.1): an instance DispatcherQueue property on the class or a base.
    def dq_getter(n: str) -> dict | None:
        for c in [n] + ing.bases(n):
            for i in ing.class_ifaces(c, own_only=False) if ing.get(c) and ing.get(c)["category"] == "class" else []:
                for m in ing.iface_methods(i):
                    if m["name"] == "get_DispatcherQueue" and m["ret"] == "Microsoft.UI.Dispatching.DispatcherQueue":
                        return {"iface": m["iface"], "slot": m["slot"]}
        return None

    curated = set(ov.get("curated", []))
    roots = set(ov.get("roots", []))
    data: dict = {"namespaces": {}, "generic_kinds": {}, "enums": {}, "structs": {}, "categories": cats}
    for n, rec in bound.items():
        head, args = parse(n)
        t = ing.get(head)
        dq = dq_getter(n) if t and t["category"] == "class" else None
        rec.update({
            "full": n, "name": head.rsplit(".", 1)[1], "namespace": head.rsplit(".", 1)[0],
            "js_module": js_module(head.rsplit(".", 1)[0]), "thread": thread_class(ing, head, dq), "thread_getter": dq,
            "lifetime": "shared", "curated": n in curated, "root": n in roots, "kind": t["category"] if t else "interface",
            "abstract": not rec["constructors"], "iid": iid_or_none(ing, n),
        })
        if n in ov.get("subjects", {}):
            rec["subject"] = ov["subjects"][n]
        if n in ov.get("async_checks", {}):
            rec["async_checks"] = ov["async_checks"][n]
        if args:
            data["generic_kinds"][n] = rec
        else:
            data["namespaces"].setdefault(rec["namespace"], {})[n] = rec
    # Enums and records every bound member mentions (plus all of the ingested namespaces, for the report).
    used: set[str] = set()
    for rec in bound.values():
        used |= set(rec["refs"])
    for n in sorted(ing.types):
        t = ing.types[n]
        if t["category"] == "enum" and (n in used or n in ing.inside):
            data["enums"][n] = {"flags": bool(t.get("flags")), "unsigned": t.get("underlying") == "u32",
                                "members": [[camel(m[0]), m[1]] for m in t["members"]], "native": [m[0] for m in t["members"]],
                                "used": n in used}
        if t["category"] == "struct" and n in used and n not in SPECIAL_STRUCTS:
            data["structs"][n] = {"fields": [{"name": camel(f["name"]), "native": f["name"], "type": f["type"]} for f in t["fields"]]}
    # Structs nested in used structs.
    changed = True
    while changed:
        changed = False
        for n in list(data["structs"]):
            for f in data["structs"][n]["fields"]:
                ft = f["type"]
                if ing.cat(ft) == "struct" and ft not in data["structs"]:
                    t = ing.get(ft)
                    data["structs"][ft] = {"fields": [{"name": camel(x["name"]), "native": x["name"], "type": x["type"]} for x in t["fields"]]}
                    changed = True
                if ing.cat(ft) == "enum" and ft not in data["enums"]:
                    t = ing.get(ft)
                    data["enums"][ft] = {"flags": bool(t.get("flags")), "unsigned": t.get("underlying") == "u32",
                                         "members": [[camel(m[0]), m[1]] for m in t["members"]], "native": [m[0] for m in t["members"]], "used": True}
    data["ing"] = ing
    data["ov"] = ov
    return data


# ---------------------------------------------------------------------------------------------- facts files

def strip(rec: dict) -> dict:
    return {k: v for k, v in rec.items() if k not in ()}


def serialise(data: dict) -> dict[Path, str]:
    ing: Ingest = data["ing"]
    files: dict[Path, str] = {}
    for ns in sorted(ing.namespaces):
        types = {}
        for n in sorted(x for x in ing.inside if ing.types[x].get("namespace") == ns):
            cat = data["categories"][n]
            entry: dict = {"category": cat}
            if n in data["namespaces"].get(ns, {}):
                entry = {**data["namespaces"][ns][n], "bound": True}
            types[n] = entry
        generic = {k: v for k, v in sorted(data["generic_kinds"].items()) if v["namespace"] == ns}
        enums = {k: v for k, v in sorted(data["enums"].items()) if k.rsplit(".", 1)[0] == ns and v["used"]}
        structs = {k: v for k, v in sorted(data["structs"].items()) if k.rsplit(".", 1)[0] == ns}
        doc = {"schema": "svelters-winui-types/v1", "namespace": ns, "js_module": js_module(ns), "types": types,
               "generic_kinds": generic, "enums": enums, "structs": structs}
        files[OUT / f"{ns}.json"] = json.dumps(doc, indent=1, sort_keys=True) + "\n"
    # Enums and structs of namespaces that are not ingested (used by bound members).
    other_enums = {k: v for k, v in sorted(data["enums"].items()) if k.rsplit(".", 1)[0] not in ing.namespaces and v["used"]}
    other_structs = {k: v for k, v in sorted(data["structs"].items()) if k.rsplit(".", 1)[0] not in ing.namespaces}
    files[OUT / "_other.json"] = json.dumps({"schema": "svelters-winui-types/v1", "namespace": None, "types": {}, "generic_kinds": {},
                                             "enums": other_enums, "structs": other_structs}, indent=1, sort_keys=True) + "\n"
    files[OUT / "report.txt"] = report(data)
    return files


def load_facts() -> dict:
    """All facts files merged: {"kinds": {full: rec}, "enums", "structs"} (what the generators read)."""
    kinds: dict = {}
    enums: dict = {}
    structs: dict = {}
    namespaces: dict = {}
    for p in sorted(OUT.glob("*.json")):
        d = json.loads(p.read_text())
        if d.get("namespace"):
            namespaces[d["namespace"]] = {n: t["category"] for n, t in d["types"].items()}
        for n, t in d["types"].items():
            if t.get("bound"):
                kinds[n] = t
        kinds.update(d["generic_kinds"])
        enums.update(d["enums"])
        structs.update(d["structs"])
    return {"kinds": kinds, "enums": enums, "structs": structs, "namespaces": namespaces}


# ---------------------------------------------------------------------------------------------- report

def report(data: dict) -> str:
    ing: Ingest = data["ing"]
    cats = data["categories"]
    order = ["record", "enum", "flags", "callback", "object", "model", "statics", "args", "interface", "class-interface",
             "collection", "async", "reference", "generic", "widget", "internal", "excluded", "unclassified"]
    lines = ["== types by category (all types of the ingested namespaces; winmd: Windows App SDK 2.5.1 + Windows.winmd 0.100)"]
    short = {"class-interface": "cls-ifc", "collection": "collect", "reference": "ref", "unclassified": "unclass"}
    lines.append(f"{'namespace':40s}" + "".join(f"{short.get(c, c)[:8]:>8s}" for c in order) + f"{'total':>8s}")
    tot = Counter()
    for ns in sorted(ing.namespaces):
        c = Counter(cats[n] for n in ing.inside if ing.types[n].get("namespace") == ns)
        tot.update(c)
        lines.append(f"{ns:40s}" + "".join(f"{c.get(k, 0):8d}" for k in order) + f"{sum(c.values()):8d}")
    lines.append(f"{'total':40s}" + "".join(f"{tot.get(k, 0):8d}" for k in order) + f"{sum(tot.values()):8d}")
    lines.append("")
    kinds = {**{n: r for ns in data["namespaces"].values() for n, r in ns.items()}, **data["generic_kinds"]}
    lines.append(f"== bound scope (roots + signature closure within the scope namespaces + bases), by namespace")
    by_ns: dict[str, Counter] = defaultdict(Counter)
    for n, r in kinds.items():
        by_ns[r["namespace"]][r["category"]] += 1
    for ns in sorted(by_ns):
        c = by_ns[ns]
        lines.append(f"  {ns:40s} {sum(c.values()):4d}  " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    lines.append(f"  {'total':40s} {len(kinds):4d}  (roots {len(data['ov'].get('roots', []))}, generic instances {len(data['generic_kinds'])})")
    th = Counter(r["thread"] for r in kinds.values())
    lines.append("  thread classes (section 4.1): " + ", ".join(f"{k} {v}" for k, v in sorted(th.items())))
    lines.append("")
    lines.append("== members of bound kinds: bound / blocked (by namespace)")
    reasons: Counter = Counter()
    excl: Counter = Counter()
    allb = allt = 0
    for ns in sorted(by_ns):
        b = t = 0
        for n, r in kinds.items():
            if r["namespace"] != ns:
                continue
            nb, nk = count_members(r)
            b += nb
            t += nb + nk
            for why in r["blocked"].values():
                reasons[re.sub(r":.*", "", why) if why.startswith(("out-of-scope", "unknown")) else why] += 1
            for why in r["excluded"].values():
                excl[why] += 1
        allb += b
        allt += t
        lines.append(f"  {ns:40s} members {t:5d}  bound {b:5d} ({(100 * b // t) if t else 100:3d}%)  blocked {t - b:4d}")
    lines.append(f"  {'total':40s} members {allt:5d}  bound {allb:5d} ({(100 * allb // allt) if allt else 100:3d}%)  blocked {allt - allb:4d}")
    lines.append("  excluded by rule: " + ", ".join(f"{k} {v}" for k, v in sorted(excl.items())))
    hidden = sum(len(r["hidden"]) for r in kinds.values())
    lines.append(f"  hidden (DependencyProperty statics; the instance property carries `dp`): {hidden}")
    lines.append("  blocking reasons: " + ", ".join(f"{k} {v}" for k, v in reasons.most_common()))
    lines.append("")
    lines.append("== overloads (runtime dispatch by argument count; DefaultOverload keeps the name)")
    names = overloaded = renamed = 0
    for r in kinds.values():
        for sec in ("methods", "statics"):
            for js, ovs in r[sec].items():
                names += 1
                if len(ovs) > 1:
                    overloaded += 1
                renamed += sum(1 for o in ovs if camel(o["native"]) != js and not o.get("prop") and js != "set" + o["native"][4:])
    lines.append(f"  JS method names {names}, overloaded (by arity) {overloaded}, renamed from the Overload attribute {renamed}")
    lines.append("")
    lines.append("== curated set")
    for n in sorted(kinds):
        r = kinds[n]
        if r["curated"]:
            nb, nk = count_members(r)
            lines.append(f"  {n:52s} {r['category']:8s} {r['thread']:10s} bound {nb:4d} blocked {nk:3d}")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    data = build()
    files = serialise(data)
    if a.report:
        print(files[OUT / "report.txt"], end="")
    if a.write or a.check:
        bad = []
        for p, text in files.items():
            if a.check:
                if not p.exists() or p.read_text() != text:
                    bad.append(str(p.relative_to(ROOT)))
            elif not p.exists() or p.read_text() != text:
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
        for p in OUT.glob("*.json"):
            if p not in files:
                if a.check:
                    bad.append(f"{p.relative_to(ROOT)} (stale)")
                else:
                    p.unlink()
        if bad:
            print("types_winui_classify: stale facts: " + ", ".join(bad) + "\n  run `python3 tools/metadata/winui/types_winui_classify.py --write`",
                  file=sys.stderr)
            return 1
        if not a.report:
            print(f"types_winui_classify: {len(files)} files {'written' if a.write else 'are current'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
