"""The style property table of one toolkit: metadata/style/<toolkit>.yaml checked against the
backend's kinds and expanded to tags.

Used by tools/metadata/semantics/gen_semantics.py, which embeds the result as the `style` section
of metadata/semantics/<toolkit>.json (the compiler reads it there), and rendered by gen_props.py
into packages/style/src/props.generated.js and the docs reference.

Every setter target must be a writable property on every tag the entry expands to, and every
child attribute must exist on every parent it names: a mistake is an error, not a silent drop.
"""
from __future__ import annotations

import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[3]
STYLE = ROOT / "metadata/style"

VIAS = {"css", "setter", "child-attr"}
DIALECTS = {"css", "qss", "none"}
SELECTORS = {"type", "class", "id"}
# setter / child-attr value type -> native property types it can target
TYPES = {
    "length": {"i32", "u32", "i64", "f64"},
    "int": {"i32", "u32", "i64"},
    "number": {"f64", "i32", "u32", "i64"},
    "bool": {"bool"},
    "enum": {"enum"},
    "insets": {"record"},
    "string": {"string"},
}
NAME = re.compile(r"[a-z][a-z0-9]*(-[a-z0-9]+)*")


class StyleError(Exception):
    pass


def _fail(msg: str):
    raise StyleError(msg)


def _keys(where: str, rec: dict, allowed: set[str]) -> None:
    extra = sorted(set(rec) - allowed)
    if extra:
        _fail(f"{where}: unknown field(s) {extra}")


def load(toolkit: str) -> dict:
    return yaml.safe_load((STYLE / f"{toolkit}.yaml").read_text())


def _norm(t: str) -> str:
    """Spec property type of an attribute/property record (`u8` -> `u32`, ...)."""
    return {"i8": "i32", "i16": "i32", "u8": "u32", "u16": "u32", "u64": "i64", "f32": "f64"}.get(t, t)


def build(toolkit: str, doc: dict, tags: dict[str, str], kinds: dict[str, dict]) -> dict:
    """`tags`: tag -> kind id; `kinds`: kind id -> {chain, props{js: {js, native, type, writable,
    values?, of?}}, children{slots{name: {attrs{name: {type, values?}}}}}} as gen_semantics emits."""
    where = f"metadata/style/{toolkit}.yaml"
    _keys(where, doc, {"toolkit", "dialect", "setter_selectors", "units", "properties"})
    if doc.get("toolkit") != toolkit:
        _fail(f"{where}: toolkit must be {toolkit!r}")
    if doc.get("dialect") not in DIALECTS:
        _fail(f"{where}: dialect must be one of {sorted(DIALECTS)}")
    sels = doc.get("setter_selectors") or []
    if not sels or set(sels) - SELECTORS:
        _fail(f"{where}: setter_selectors must be a non-empty subset of {sorted(SELECTORS)}")
    units = {}
    for u, rec in sorted((doc.get("units") or {}).items()):
        _keys(f"{where} units.{u}", rec, {"scale", "provenance"})
        if not isinstance(rec.get("scale"), (int, float)) or rec["scale"] <= 0 or not rec.get("provenance"):
            _fail(f"{where} units.{u}: needs a positive scale and a provenance")
        units[u] = {"scale": rec["scale"], "provenance": rec["provenance"]}

    def chain(tag: str) -> list[str]:
        return kinds[tags[tag]]["chain"]

    def find_prop(tag: str, name: str):
        for c in chain(tag):
            for p in (kinds.get(c) or {}).get("props", {}).values():
                if name in (p["js"], p["native"]):
                    return p
        return None

    def expand(w: str, classes: list[str]) -> list[str]:
        if not classes or not all(isinstance(c, str) for c in classes):
            _fail(f"{w}: needs a list of classes")
        out = []
        for c in classes:
            hit = sorted(t for t in tags if c in chain(t))
            if not hit:
                _fail(f"{w}: no element of {toolkit} is a {c}")
            out += [t for t in hit if t not in out]
        return sorted(out)

    props = {}
    for name, rec in (doc.get("properties") or {}).items():
        w = f"{where} properties.{name}"
        if not NAME.fullmatch(name):
            _fail(f"{w}: property names are kebab-case")
        via = rec.get("via")
        if via not in VIAS:
            _fail(f"{w}: via must be one of {sorted(VIAS)}")
        if via == "css":
            _keys(w, rec, {"via", "note", "inline_only"})
            if doc["dialect"] == "none":
                _fail(f"{w}: {toolkit} has no native styling dialect; only setters and child attributes")
            out = {"via": "css"}
            if rec.get("inline_only"):
                out["inline_only"] = True
        elif via == "setter":
            _keys(w, rec, {"via", "prop", "type", "kinds", "note"})
            ty = rec.get("type")
            if ty not in TYPES:
                _fail(f"{w}: type must be one of {sorted(TYPES)}")
            tl = expand(w, rec.get("kinds"))
            first = None
            bad = []
            for t in tl:
                p = find_prop(t, rec.get("prop") or "")
                if p is None or not p.get("writable") or p.get("construct_only"):
                    bad.append(t)
                    continue
                if _norm(p["type"]) not in TYPES[ty] or (ty == "insets" and p.get("of") != "Insets"):
                    _fail(f"{w}: type {ty} cannot target {rec['prop']} on <{t}>, a {p['type']} property")
                if first is None:
                    first = p
                elif p["js"] != first["js"] or p.get("values") != first.get("values"):
                    _fail(f"{w}: {rec['prop']} differs between <{tl[0]}> and <{t}>")
            if bad:
                _fail(f"{w}: {rec.get('prop')!r} is not a writable property of <{'>, <'.join(bad)}>")
            out = {"via": "setter", "prop": first["js"], "native": first["native"], "type": ty, "kinds": rec["kinds"], "tags": tl}
            if ty == "enum":
                if not first.get("values"):
                    _fail(f"{w}: enum {first.get('enum')} of {rec['prop']} has no known values in the metadata")
                out["values"] = first["values"]
        else:
            _keys(w, rec, {"via", "attr", "type", "parents", "note"})
            ty = rec.get("type")
            if ty not in TYPES:
                _fail(f"{w}: type must be one of {sorted(TYPES)}")
            tl = expand(w, rec.get("parents"))
            first = None
            for t in tl:
                hits = [s["attrs"][rec.get("attr")] for c in chain(t) for s in (((kinds.get(c) or {}).get("children") or {}).get("slots") or {}).values()
                        if rec.get("attr") in (s.get("attrs") or {})]
                if not hits:
                    _fail(f"{w}: <{t}> has no child attribute {rec.get('attr')!r}")
                a = hits[0]
                if _norm(a["type"]) not in TYPES[ty]:
                    _fail(f"{w}: type {ty} cannot target child attribute {rec['attr']} ({a['type']}) of <{t}>")
                first = first or a
            out = {"via": "child-attr", "attr": rec["attr"], "type": ty, "parents": rec["parents"], "tags": tl}
            if ty == "enum":
                out["values"] = first["values"]
        if rec.get("note"):
            out["note"] = rec["note"]
        props[name] = out
    return {"dialect": doc["dialect"], "setter_selectors": sels, "units": units, "properties": dict(sorted(props.items()))}
