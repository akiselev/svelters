#!/usr/bin/env python3
"""Classifies Foundation/AppKit types from the objc2 ingest (N4-AppKit; plans/research/NON-GUI-TYPES.md section 3).

The Apple counterpart of tools/metadata/gtk/types_facts.py and tools/metadata/qt/types_qt_classify.py: one
taxonomy (record / value / object / statics / primitive / widget / excluded), the rules of section 3.1 for Apple,
a scope (roots + signature closure + ancestors), per-member binding with a reason for every blocked member, and
the VALUE mutator hide list (decision Q3). It reads only checked-in files, so it runs without a Mac and without the
cargo registry:

  metadata/appkit/types/ingested/{Foundation,AppKit}.json   raw facts (types_objc2_ingest.py)
  metadata/overrides/appkit/types/appkit-types.yaml           roots, curated set, class corrections, exclusions
  metadata/probe/appkit/types-*.json (optional)               runtime facts from the macOS CI probe: declared
                                                              properties (read-only ones become getters)

Output: metadata/appkit/types/{Foundation,AppKit}.json (schema svelters-types/v1) and report.txt.

  python3 tools/metadata/appkit/types_appkit_classify.py --report    print the classification report
  python3 tools/metadata/appkit/types_appkit_classify.py --write     write the facts and the report
  python3 tools/metadata/appkit/types_appkit_classify.py --check     fail if the checked-in output is stale
  python3 tools/metadata/appkit/types_appkit_classify.py --probe-list   the classes the macOS CI probe lists

Runtime contract with the core (crates/svelters-appkit/src/types.rs): kinds are ObjC class names; methods are
called by selector (the JS name is only the typing/docs name), props by KVC key; an `init…` selector through
`call_static` is alloc + init; async members carry the completion block's shape, which must be one the core
builds (`BLOCK_SHAPES`, mirrored by `svelters_apple::blocks::SHAPES`).
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

ROOT = Path(__file__).resolve().parents[3]
INGEST = ROOT / "metadata/appkit/types/ingested"
FACTS_DIR = ROOT / "metadata/appkit/types"
OVERRIDES = ROOT / "metadata/overrides/appkit/types/appkit-types.yaml"
PROBES = ROOT / "metadata/probe/appkit"
FRAMEWORKS = ["Foundation", "AppKit"]
JS_MODULE = {"Foundation": "foundation", "AppKit": "appkit"}

# Classes whose instances cross as plain JS values (the core's conversions), never as handles.
PRIMITIVE = {
    "NSString": "string", "NSMutableString": "string", "NSNumber": "number", "NSDecimalNumber": "number",
    "NSArray": "list", "NSMutableArray": "list", "NSSet": "list", "NSMutableSet": "list", "NSOrderedSet": "list",
    "NSMutableOrderedSet": "list", "NSDictionary": "map", "NSMutableDictionary": "map", "NSData": "bytes",
    "NSMutableData": "bytes", "NSDate": "date", "NSNull": "null", "NSError": "error", "NSObject": "any",
    "NSValue": "any",
}
WIDGET_ROOTS = {"NSResponder", "NSCell"}
# Structs the core converts (svelters_apple::enc::StructTy) and the shared JS shape they take.
RECORDS = {"NSRect": "Rect", "CGRect": "Rect", "NSSize": "Size", "CGSize": "Size", "NSPoint": "Point", "CGPoint": "Point",
           "NSEdgeInsets": "Insets", "NSRange": "Range"}
INTS = {"NSComparisonResult", "CGGlyph", "NSRectEdge", "CGWindowID", "NSInteger", "NSUInteger", "isize", "usize", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "c_int", "c_uint",
        "c_long", "c_ulong", "c_short", "c_ushort", "c_longlong", "c_ulonglong", "c_char", "c_uchar", "c_schar", "unichar",
        "UTF32Char", "NSStringEncoding", "OSType", "FourCharCode", "NSGlyph", "NSWindowNumber", "pid_t", "uid_t", "mode_t"}
FLOATS = {"CGFloat", "f32", "f64", "c_float", "c_double", "NSTimeInterval"}
# The completion-block shapes the core can build (one letter per block argument: o object, e NSError, b BOOL,
# i NSInteger/NSUInteger, d CGFloat/double). Keep in sync with svelters_apple::blocks::SHAPES.
BLOCK_SHAPES = {"", "e", "o", "oe", "b", "be", "i", "ie", "d"}
COMPLETION = re.compile(r"^(completion(_handler|_block)?|handler|reply|result_handler|completion_handler)$")
# NSObject protocol, KVC/KVO, coding and memory machinery: the core's job, never generated (section 2.4).
CORE_SELECTORS = {
    "alloc", "allocWithZone:", "dealloc", "retain", "release", "autorelease", "retainCount", "class", "superclass", "self",
    "zone", "isProxy", "hash", "description", "debugDescription", "isEqual:", "copy", "mutableCopy", "copyWithZone:",
    "mutableCopyWithZone:", "respondsToSelector:", "conformsToProtocol:", "isKindOfClass:", "isMemberOfClass:",
    "performSelector:", "performSelector:withObject:", "performSelector:withObject:withObject:", "initWithCoder:",
    "encodeWithCoder:", "awakeAfterUsingCoder:", "classForCoder", "replacementObjectForCoder:", "supportsSecureCoding",
    "classForKeyedArchiver", "replacementObjectForKeyedArchiver:", "classFallbacksForKeyedArchiver",
    "classForKeyedUnarchiver", "valueForKey:", "setValue:forKey:", "valueForKeyPath:", "setValue:forKeyPath:",
    "valueForUndefinedKey:", "setValue:forUndefinedKey:", "setNilValueForKey:", "dictionaryWithValuesForKeys:",
    "setValuesForKeysWithDictionary:", "addObserver:forKeyPath:options:context:", "removeObserver:forKeyPath:",
    "removeObserver:forKeyPath:context:", "observeValueForKeyPath:ofObject:change:context:", "willChangeValueForKey:",
    "didChangeValueForKey:", "automaticallyNotifiesObserversForKey:", "keyPathsForValuesAffectingValueForKey:",
    "observationInfo", "setObservationInfo:", "new", "init", "load", "initialize", "instancesRespondToSelector:",
    "instanceMethodForSelector:", "methodForSelector:", "doesNotRecognizeSelector:", "forwardingTargetForSelector:",
    "forwardInvocation:", "methodSignatureForSelector:", "instanceMethodSignatureForSelector:", "resolveClassMethod:",
    "resolveInstanceMethod:", "isSubclassOfClass:", "version", "setVersion:", "accessInstanceVariablesDirectly",
    "useStoredAccessor",
}
# Safety notes of objc2's `unsafe fn`s the core covers at run time: arguments are converted against the runtime
# method signature (and ObjC exceptions are caught), pointers and selectors are refused by the type mapping, and
# completion blocks only queue to the main thread. Any other note excludes the member ("unsafe").
SAFE_NOTES = [r"should be of the correct type", r"must be a valid pointer", r"might not allow", r"has further requirements",
              r"block must be sendable", r"must be a valid selector"]
MUTATING = re.compile(r"^(set|add|remove|insert|replace|append|delete|clear|reset|apply|fix|beginEditing|endEditing)[A-Z:]|^(set|add|remove|insert|replace|append)$")


def load_overrides() -> dict:
    ov = yaml.safe_load(OVERRIDES.read_text()) or {}
    for k in ("curated", "roots"):
        ov.setdefault(k, [])
    for k in ("class", "string_ctor", "subjects", "renames", "runtime_missing"):
        ov.setdefault(k, {})
    ov.setdefault("no_construct", [])
    return ov


def load_probe() -> dict:
    """Runtime facts from the macOS CI probe (newest SDK file wins), or {}."""
    files = sorted(glob.glob(str(PROBES / "types-*.json")))
    return json.loads(Path(files[-1]).read_text()) if files else {}


class Ingest:
    def __init__(self) -> None:
        self.fw: dict[str, dict] = {fw: json.loads((INGEST / f"{fw}.json").read_text()) for fw in FRAMEWORKS}
        self.classes: dict[str, dict] = {}
        self.enums: dict[str, dict] = {}
        self.typedefs: dict[str, str] = {}
        self.structs: dict[str, dict] = {}
        self.protocols: dict[str, dict] = {}
        for d in self.fw.values():
            self.classes.update(d["classes"])
            self.enums.update(d["enums"])
            self.typedefs.update(d["typedefs"])
            self.structs.update(d["structs"])
            self.protocols.update(d["protocols"])

    def chain(self, name: str) -> list[str]:
        return [name] + list(self.classes[name]["super"])

    def resolve_typedef(self, t: str, depth: int = 0) -> str:
        base = t
        while depth < 8 and base in self.typedefs and base not in self.enums and base not in INTS and base not in FLOATS:
            base = self.typedefs[base]
            depth += 1
        return base


def exclude_re(ov: dict) -> re.Pattern:
    return re.compile(ov.get("exclude_regex") or "^$")


def classify(ing: Ingest, name: str, ov: dict) -> str:
    c = ing.classes[name]
    if name in ov["class"]:
        return ov["class"][name]
    chain = ing.chain(name)
    if name in WIDGET_ROOTS or WIDGET_ROOTS & set(chain):
        return "widget"
    if name in PRIMITIVE:
        return "primitive"
    if name.startswith("_") or c.get("deprecated") or exclude_re(ov).match(name):
        return "excluded"
    if name.startswith("NSMutable"):
        return "object"
    if f"NSMutable{name[2:]}" in ing.classes:
        return "value"
    setters = any(m.get("setter_for") and not m.get("deprecated") for m in c["methods"] if not m["static"])
    if "NSCopying" in c["conforms"] and not setters and c.get("thread") != "main":
        return "value"
    inst = [m for m in c["methods"] if not m["static"] and not m.get("inherited") and m["sel"] not in CORE_SELECTORS]
    stat = [m for m in c["methods"] if m["static"] and m["sel"] not in CORE_SELECTORS]
    if not inst and stat and not any(m.get("init") for m in c["methods"]):
        return "statics"
    return "object"


# ---- type mapping ---------------------------------------------------------------------------------

def strip_ref(t: str) -> tuple[str, bool]:
    """(`T`, nullable) from `&T`, `Option<&T>`, `Retained<T>`, `Option<Retained<T>>`, `*mut T` (nullable)."""
    nullable = False
    t = t.strip()
    while True:
        m = re.fullmatch(r"Option<(.*)>", t)
        if m:
            t, nullable = m.group(1).strip(), True
            continue
        m = re.fullmatch(r"(?:Retained|Allocated)<(.*)>", t)
        if m:
            t = m.group(1).strip()
            continue
        if t.startswith("&'static "):
            t = t[len("&'static "):]
            continue
        if t.startswith("&") and not t.startswith("&mut"):
            t = t[1:].strip()
            continue
        break
    return t, nullable


def generic_args(t: str) -> tuple[str, list[str]]:
    m = re.fullmatch(r"([\w:]+)<(.*)>", t)
    if not m:
        return t, []
    depth, cur, out = 0, [], []
    for ch in m.group(2):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            out.append("".join(cur).strip())
            cur = []
        else:
            cur.append(ch)
    out.append("".join(cur).strip())
    return m.group(1), out


class Types:
    def __init__(self, ing: Ingest, cats: dict[str, str]) -> None:
        self.ing = ing
        self.cats = cats

    def slot(self, rust: str, ret: bool = False) -> dict:
        """The JS slot of an objc2 type: {k, t?, nullable?, of?} or {k: 'blocked', why}."""
        if rust is None or rust == "()":
            return {"k": "void"}
        t, nullable = strip_ref(rust)
        out = self._slot(t, ret)
        if nullable and out["k"] not in ("blocked", "void"):
            out["nullable"] = True
        return out

    def _slot(self, t: str, ret: bool) -> dict:
        if t in ("bool", "Bool"):
            return {"k": "bool"}
        base = self.ing.resolve_typedef(t)
        if base in INTS or t in INTS:
            return {"k": "int"}
        if base in FLOATS or t in FLOATS:
            return {"k": "float"}
        if t in self.ing.enums:
            e = self.ing.enums[t]
            return {"k": "flags" if e["kind"] == "options" else "enum", "t": t}
        if base in RECORDS or t in RECORDS:
            return {"k": "record", "t": RECORDS.get(base, RECORDS.get(t))}
        if base != t:
            return self._slot(base, ret)
        if t in ("Self",):
            return {"k": "self"}
        if t in ("AnyObject", "ObjectType", "KeyType", "ValueType", "UnitType", "ResultType", "ObjectType: ?Sized", "AnyObject: ?Sized"):
            return {"k": "any"}
        if t.startswith("ProtocolObject<"):
            return {"k": "any"}
        if t == "AnyClass":
            return {"k": "type"}
        if t == "Sel":
            return {"k": "blocked", "why": "selector"}
        if "DynBlock" in t:
            return {"k": "blocked", "why": "callback"}
        if t.startswith(("*", "NonNull<", "&mut")):
            return {"k": "blocked", "why": "pointer"}
        if t.startswith("Result<"):
            return {"k": "blocked", "why": "result"}
        name, args = generic_args(t)
        if name in PRIMITIVE:
            k = PRIMITIVE[name]
            if k == "error":
                return {"k": "blocked", "why": "error-value"}
            out = {"k": k}
            if k == "list" and args:
                out["of"] = self._slot(strip_ref(args[0])[0], ret)
                if out["of"]["k"] == "blocked":
                    return {"k": "blocked", "why": f"list-of:{out['of']['why']}"}
            if k == "map" and len(args) == 2:
                kk = self._slot(strip_ref(args[0])[0], ret)
                if kk["k"] not in ("string", "any"):
                    return {"k": "blocked", "why": "map-key"}
                out["of"] = self._slot(strip_ref(args[1])[0], ret)
                if out["of"]["k"] == "blocked":
                    return {"k": "blocked", "why": f"map-of:{out['of']['why']}"}
            if name.startswith("NSMutable") and not ret:
                return {"k": "blocked", "why": "mutable-in"}
            return out
        if name in self.ing.classes:
            cat = self.cats.get(name)
            if cat == "widget":
                return {"k": "blocked", "why": "widget"}
            if cat in ("excluded", None):
                return {"k": "blocked", "why": f"type:excluded:{name}"}
            if cat == "statics":
                return {"k": "blocked", "why": f"type:statics:{name}"}
            return {"k": "value" if cat == "value" else "object", "t": name}
        if name in self.ing.structs:
            return {"k": "blocked", "why": f"record-shape:{name}"}
        return {"k": "blocked", "why": f"type:unknown:{name}"}


def refs_of(s: dict) -> set[str]:
    out = set()
    if s.get("k") in ("object", "value"):
        out.add(s["t"])
    if "of" in s:
        out |= refs_of(s["of"])
    return out


def block_sig(rust: str) -> tuple[list[str], str] | None:
    """(argument types, return type) of `&DynBlock<dyn Fn(A, B) -> R + '_>` (or an Option of it)."""
    i = rust.find("dyn Fn(")
    if i < 0:
        return None
    j = i + len("dyn Fn(")
    depth, k = 1, j
    while k < len(rust) and depth:
        depth += {"(": 1, ")": -1}.get(rust[k], 0)
        k += 1
    args = [a.strip() for a in split_args(rust[j:k - 1]) if a.strip()]
    rest = rust[k:].strip()
    ret = ""
    if rest.startswith("->"):
        ret = re.split(r"\s*(?:\+|,|>$)", rest[2:].strip())[0].strip()
    return args, ret


def split_args(s: str) -> list[str]:
    out, depth, cur = [], 0, []
    i = 0
    while i < len(s):
        ch = s[i]
        if s[i:i + 2] == "->":
            cur.append("->")
            i += 2
            continue
        if ch in "<(":
            depth += 1
        elif ch in ">)":
            depth -= 1
        if ch == "," and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
        i += 1
    out.append("".join(cur))
    return out


def deref_arg(a: str) -> str:
    a = a.strip()
    for pre in ("*mut ", "*const "):
        if a.startswith(pre):
            return strip_ref(a[len(pre):])[0]
    m = re.fullmatch(r"NonNull<(.*)>", a)
    if m:
        return strip_ref(m.group(1))[0]
    return strip_ref(a)[0]


def block_shape(rust: str, ty: Types) -> tuple[str | None, dict | None, str | None]:
    """(shape, result slot, why-blocked) of a completion block `&DynBlock<dyn Fn(A, B)>`."""
    rust = ty.ing.resolve_typedef(strip_ref(rust)[0])
    sig = block_sig(rust)
    if sig is None:
        return None, None, "block-signature"
    args, ret = sig
    if ret not in ("", "()"):
        return None, None, "block-returns"
    shape, result = "", None
    for a in args:
        if a.startswith("NonNull<Bool>") or a.startswith("*mut Bool"):
            return None, None, "block-arg:stop-pointer"
        inner = deref_arg(a)
        if inner == "NSError":
            shape += "e"
            continue
        s = {"k": "bool"} if inner in ("Bool", "bool") else ty._slot(inner, True)
        if s["k"] == "bool":
            shape += "b"
        elif s["k"] in ("int", "enum", "flags"):
            shape += "i"
        elif s["k"] == "float":
            shape += "d"
        elif s["k"] == "blocked":
            return None, None, f"block-arg:{s['why']}"
        else:
            shape += "o"
        if result is not None:
            return None, None, "block-results"
        result = s
    if shape not in BLOCK_SHAPES:
        return None, None, f"block-shape:{shape}"
    return shape, result or {"k": "void"}, None


def camel(pieces: list[str]) -> str:
    return pieces[0] + "".join(p[:1].upper() + p[1:] for p in pieces[1:])


def js_name(sel: str, hidden_last: bool, init: bool) -> str:
    pieces = [p for p in sel.split(":") if p]
    if hidden_last and len(pieces) > 1:
        pieces = pieces[:-1]
    if init:
        first = pieces[0]
        pieces[0] = "new" + first[4:] if first != "init" else "new"
    return camel(pieces)


def key_of_setter(sel: str) -> str:
    k = sel[3:].rstrip(":")
    return k[:1].lower() + k[1:] if not (len(k) > 1 and k[1].isupper()) else k


def safety_ok(m: dict) -> bool:
    for s in m.get("safety", []):
        if not any(re.search(p, s) for p in SAFE_NOTES):
            return False
    return True


# ---- members --------------------------------------------------------------------------------------

def build_members(ing: Ingest, name: str, cat: str, ty: Types, ov: dict, probe: dict) -> dict:
    c = ing.classes[name]
    rec: dict = {"methods": [], "props": [], "blocked": {}, "hidden": [], "excluded": collections.Counter()}
    value = cat == "value"
    mutable_twin = f"NSMutable{name[2:]}" in ing.classes
    setters = {m["setter_for"]: m for m in c["methods"] if m.get("setter_for") and not m["static"]}
    static_setters = {m["setter_for"]: m for m in c["methods"] if m.get("setter_for") and m["static"]}
    getters_of_props = set(setters) | set(static_setters)
    probed = {p["name"]: p for p in probe.get("classes", {}).get(name, {}).get("props", [])}
    by_sel = {(m["sel"], m["static"]): m for m in c["methods"]}
    names_used: dict[tuple[str, bool], str] = {}

    def excl(why: str) -> None:
        rec["excluded"][why] += 1

    # properties: read-write pairs (objc2 "Setter for") and, from the probe, declared read-only properties
    for getter, sm in sorted(setters.items()):
        gm = by_sel.get((getter, False))
        if gm is None or gm.get("deprecated") or sm.get("deprecated"):
            excl("deprecated")
            continue
        key = key_of_setter(sm["sel"])
        s = ty.slot(gm["ret"], ret=True)
        sin = ty.slot(sm["params"][0]["rust"]) if sm["params"] else {"k": "blocked", "why": "setter"}
        if value:
            rec["hidden"].append(sm["sel"])
            sin = None
        if s["k"] == "blocked":
            rec["blocked"][getter] = s["why"]
            continue
        if not safety_ok(gm) or (sin is not None and not safety_ok(sm)):
            excl("unsafe")
            continue
        writable = sin is not None and sin["k"] != "blocked"
        if sin is not None and sin["k"] == "blocked":
            rec["blocked"][sm["sel"]] = sin["why"]
        rec["props"].append({"name": key, "key": key, "getter": getter, "setter": sm["sel"] if writable else None,
                             "type": s, "readonly": not writable, "origin": "objc2"})
    for pname, p in sorted(probed.items()):
        if pname in {x["key"] for x in rec["props"]} or p.get("class_prop"):
            continue
        getter = p.get("getter") or pname
        gm = by_sel.get((getter, False))
        if gm is None or gm["params"] or gm.get("deprecated") or gm["sel"] in CORE_SELECTORS or getter in setters:
            continue
        s = ty.slot(gm["ret"], ret=True)
        if s["k"] == "blocked" or not safety_ok(gm):
            continue
        rec["props"].append({"name": pname, "key": pname, "getter": getter, "setter": None, "type": s, "readonly": True,
                             "origin": "probe"})
        getters_of_props.add(getter)

    prop_sels = {p["getter"] for p in rec["props"]} | {p["setter"] for p in rec["props"] if p["setter"]}
    missing = set(ov["runtime_missing"].get(name, []))
    for m in c["methods"]:
        sel = m["sel"]
        if m.get("inherited") and sel in ("init", "new"):
            continue
        if sel in missing:
            rec["blocked"][sel] = "runtime:missing"
            continue
        if sel in prop_sels or (not m["static"] and m.get("setter_for") in getters_of_props):
            continue
        if sel in CORE_SELECTORS and not (m.get("init") and sel != "init"):
            excl("core")
            continue
        if m.get("deprecated"):
            excl("deprecated")
            continue
        if sel.startswith("_"):
            excl("private")
            continue
        if m.get("optional"):
            excl("optional")
            continue
        if not safety_ok(m):
            excl("unsafe")
            continue
        if value and not m["static"] and not m.get("init") and (m.get("setter_for") or (not mutable_twin and MUTATING.match(sel))):
            rec["hidden"].append(sel)
            continue
        params = list(m["params"])
        asy = None
        if params and ("DynBlock" in ing.resolve_typedef(strip_ref(params[-1]["rust"])[0])) and COMPLETION.match(params[-1]["name"]):
            shape, result, why = block_shape(params[-1]["rust"], ty)
            if why:
                rec["blocked"][sel] = why
                continue
            asy = {"shape": shape, "result": result}
            params = params[:-1]
        out_params, why = [], None
        params = [p for p in params if p["rust"] != "MainThreadMarker"]
        for p in params:
            s = ty.slot(p["rust"])
            if s["k"] == "blocked":
                why = s["why"]
                break
            out_params.append({"name": re.sub(r"_([a-z])", lambda q: q.group(1).upper(), p["name"].removeprefix("r#")), "type": s})
        if why:
            rec["blocked"][sel] = why
            continue
        ret_rust = m["ret"]
        throws = False
        if ret_rust and ret_rust.startswith("Result<"):
            inner = ret_rust[len("Result<"):-1]
            parts = [x.strip() for x in re.split(r",\s*(?=Retained<NSError>)", inner)]
            ret_rust, throws = parts[0], True
        r = ty.slot(ret_rust, ret=True) if ret_rust else {"k": "void"}
        if r["k"] == "blocked":
            rec["blocked"][sel] = r["why"]
            continue
        if asy is not None and throws:
            rec["blocked"][sel] = "async-throws"
            continue
        static = m["static"] or m.get("init", False)
        if m.get("init"):
            r = {"k": "value" if value else "object", "t": name, "nullable": True}
        elif r["k"] == "self":
            r = {"k": "value" if value else "object", "t": name, **({"nullable": True} if "Option<" in (ret_rust or "") else {})}
        jn = ov["renames"].get(f"{name}.{sel}") or js_name(sel, bool(m.get("error_out")) or asy is not None, bool(m.get("init")))
        slotkey = (jn, static)
        if slotkey in names_used and names_used[slotkey] != sel:
            rec["blocked"][sel] = f"ambiguous:{jn}"
            rec.setdefault("ambiguous", []).append(f"{sel} ~ {names_used[slotkey]}")
            continue
        names_used[slotkey] = sel
        out = {"js": jn, "sel": sel, "static": static, "params": out_params, "ret": r, "family": m["family"]}
        if m.get("init"):
            out["ctor"] = True
        if throws:
            out["throws"] = True
        if asy is not None:
            out["async"] = asy
        if m.get("category"):
            out["category"] = m["category"]
        if m.get("framework") and m["framework"] != c["framework"]:
            out["framework"] = m["framework"]
        rec["methods"].append(out)
    for g, sm in sorted(static_setters.items()):
        pass  # class properties stay static methods (getter + setter selectors), handled above
    rec["methods"].sort(key=lambda x: (x["static"], x["js"], x["sel"]))
    rec["props"].sort(key=lambda x: x["name"])
    rec["hidden"] = sorted(set(rec["hidden"]))
    rec["excluded"] = dict(sorted(rec["excluded"].items()))
    rec["blocked"] = dict(sorted(rec["blocked"].items()))
    return rec


def slot_types(rec: dict):
    for m in rec["methods"]:
        for p in m["params"]:
            yield p["type"]
        yield m["ret"]
        if m.get("async"):
            yield m["async"]["result"]
    for p in rec["props"]:
        yield p["type"]


def enums_of(s: dict) -> set[str]:
    out = {s["t"]} if s.get("k") in ("enum", "flags") else set()
    if "of" in s:
        out |= enums_of(s["of"])
    return out


def build() -> dict:
    ing = Ingest()
    ov = load_overrides()
    probe = load_probe()
    for k in ("curated", "roots"):
        for n in ov[k]:
            if n not in ing.classes:
                raise SystemExit(f"{OVERRIDES.relative_to(ROOT)}: {k}: unknown class {n}")
    for n in list(ov["class"]) + list(ov["string_ctor"]) + list(ov["subjects"]) + list(ov["runtime_missing"]) + list(ov["no_construct"]):
        if n not in ing.classes:
            raise SystemExit(f"{OVERRIDES.relative_to(ROOT)}: unknown class {n}")
    for n, sels in ov["runtime_missing"].items():
        for sel in sels:
            if not any(m["sel"] == sel for m in ing.classes[n]["methods"]):
                raise SystemExit(f"{OVERRIDES.relative_to(ROOT)}: runtime_missing: {n} has no {sel}")
    for n, sel in ov["string_ctor"].items():
        if not any(m["sel"] == sel for m in ing.classes[n]["methods"]):
            raise SystemExit(f"{OVERRIDES.relative_to(ROOT)}: string_ctor: {n} has no {sel}")
    cats = {n: classify(ing, n, ov) for n in ing.classes}
    ty = Types(ing, cats)
    bindable = {"value", "object", "statics"}
    members: dict[str, dict] = {}

    def mem(n: str) -> dict:
        if n not in members:
            members[n] = build_members(ing, n, cats[n], ty, ov, probe)
        return members[n]

    bound: set[str] = set()
    work = [n for n in ov["roots"] if cats[n] in bindable]
    while work:
        n = work.pop()
        if n in bound:
            continue
        bound.add(n)
        for a in ing.classes[n]["super"]:
            if a in ing.classes and cats.get(a) in bindable and a not in bound:
                work.append(a)
        for s in slot_types(mem(n)):
            for r in refs_of(s):
                if cats.get(r) in bindable and r not in bound:
                    work.append(r)
    types = {}
    for n in sorted(bound):
        c = ing.classes[n]
        rec = mem(n)
        parent = next((a for a in c["super"] if a in bound), None)
        fw = c["framework"]
        constructible = any(m["sel"] == "init" for m in c["methods"]) and cats[n] != "statics" and n not in ov["no_construct"]
        enums = sorted(set().union(*[enums_of(s) for s in slot_types(rec)]) if rec["methods"] or rec["props"] else set())
        types[n] = {
            "name": n, "framework": fw, "js_module": f"svelters:appkit/{JS_MODULE[fw]}", "category": cats[n],
            "lifetime": "shared", "parent": parent, "bases": [a for a in c["super"] if a in ing.classes],
            "thread": c.get("thread"), "send": c.get("send", False), "conforms": c["conforms"],
            "root": n in ov["roots"], "curated": n in ov["curated"], "constructible": constructible,
            "string_ctor": ov["string_ctor"].get(n), "subject": ov["subjects"].get(n),
            "enums": enums, **rec,
        }
    enums = {}
    for t in types.values():
        for e in t["enums"]:
            src = ing.enums[e]
            enums[e] = {"framework": src["framework"], "kind": src["kind"], "repr": src["repr"],
                        "members": {k: v for k, v in src["members"].items() if v is not None}}
    return {"types": types, "enums": dict(sorted(enums.items())), "cats": cats, "ing": ing, "probe": bool(probe)}


def split_by_framework(data: dict) -> dict[str, dict]:
    out = {}
    for fw in FRAMEWORKS:
        out[fw] = {
            "schema": "svelters-types/v1", "toolkit": "appkit", "module": fw, "js_module": f"svelters:appkit/{JS_MODULE[fw]}",
            "source": "objc2 0.3.2 (objc2-foundation, objc2-app-kit)" + (" + macOS CI probe" if data["probe"] else ""),
            "types": {n: t for n, t in data["types"].items() if t["framework"] == fw},
            "enums": {n: e for n, e in data["enums"].items() if e["framework"] == fw},
        }
    return out


def serialise(d: dict) -> str:
    return json.dumps(d, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def load_facts() -> dict:
    """The checked-in facts of both frameworks merged: {types, enums} (for the generators)."""
    types, enums = {}, {}
    for fw in FRAMEWORKS:
        d = json.loads((FACTS_DIR / f"{fw}.json").read_text())
        types.update(d["types"])
        enums.update(d["enums"])
    return {"types": types, "enums": enums}


def n_members(t: dict) -> int:
    return len(t["methods"]) + len(t["props"])


def report(data: dict) -> str:
    ing, cats, types = data["ing"], data["cats"], data["types"]
    L = []
    cols = ["record", "value", "object", "statics", "primitive", "widget", "excluded"]
    L.append("== classes by category (all classes; objc2-foundation 0.3.2 + objc2-app-kit 0.3.2)")
    L.append(f"{'framework':12}" + "".join(f"{c[:9]:>10}" for c in cols))
    tot = collections.Counter()
    for fw in FRAMEWORKS:
        cnt = collections.Counter(cats[n] for n, c in ing.classes.items() if c["framework"] == fw)
        cnt["record"] = sum(1 for s in ing.fw[fw]["structs"].values())
        tot.update(cnt)
        L.append(f"{fw:12}" + "".join(f"{cnt[c]:>10}" for c in cols))
    L.append(f"{'total':12}" + "".join(f"{tot[c]:>10}" for c in cols))
    nonw = [n for n, c in cats.items() if c not in ("widget",)]
    L.append(f"  non-widget classes {len(nonw)}: MainThreadOnly {sum(1 for n in nonw if ing.classes[n].get('thread') == 'main')}, "
             f"Send {sum(1 for n in nonw if ing.classes[n].get('send'))}; records the core converts: {', '.join(sorted(set(RECORDS.values())))}")
    L.append("")
    L.append("== bound scope (roots + signature closure + ancestors), by framework and category")
    for fw in FRAMEWORKS:
        ts = [t for t in types.values() if t["framework"] == fw]
        cc = collections.Counter(t["category"] for t in ts)
        L.append(f"  {fw:12} {len(ts):4}  " + ", ".join(f"{k} {v}" for k, v in sorted(cc.items())))
    L.append(f"  {'total':12} {len(types):4}  (roots {sum(1 for t in types.values() if t['root'])})")
    L.append("")
    L.append("== members of bound types: bound / blocked")
    reasons = collections.Counter()
    excluded = collections.Counter()
    hidden = 0
    asy = collections.Counter()
    for fw in FRAMEWORKS:
        ts = [t for t in types.values() if t["framework"] == fw]
        b = sum(n_members(t) for t in ts)
        k = sum(len(t["blocked"]) for t in ts)
        L.append(f"  {fw:12} members {b + k:5}  bound {b:5} ({100 * b // max(b + k, 1):3}%)  blocked {k:4}")
        for t in ts:
            for r in t["blocked"].values():
                reasons[re.sub(r":.*", "", r) if r.startswith(("ambiguous", "block-shape")) else r] += 1
            excluded.update(t["excluded"])
            hidden += len(t["hidden"])
            for m in t["methods"]:
                if m.get("async"):
                    asy[m["async"]["shape"] or "()"] += 1
    L.append("  excluded by rule: " + ", ".join(f"{k} {v}" for k, v in sorted(excluded.items())))
    L.append(f"  VALUE mutators hidden (decision Q3: setters and mutating selectors of value classes): {hidden}")
    L.append(f"  async members (completion handler -> Promise): {sum(asy.values())}; block shapes: "
             + ", ".join(f"({k}) {v}" for k, v in sorted(asy.items())))
    L.append("  blocking reasons: " + ", ".join(f"{k} {v}" for k, v in reasons.most_common(18)))
    L.append(f"  props from the macOS probe (read-only getters): {sum(1 for t in types.values() for p in t['props'] if p['origin'] == 'probe')}"
             + ("" if data["probe"] else " (no probe file yet: read-only properties are zero-argument methods)"))
    L.append("")
    amb = [a for t in types.values() for a in t.get("ambiguous", [])]
    L.append("== naming (selectors joined in camelCase; `error:` and completion pieces dropped; init… -> static new…)")
    L.append(f"  JS member names {sum(len(t['methods']) for t in types.values())}; ambiguous after dropping hidden pieces: {len(amb)}")
    for a in amb[:10]:
        L.append(f"    {a}")
    L.append("")
    L.append("== curated set")
    for n in sorted(t for t in types if types[t]["curated"]):
        t = types[n]
        L.append(f"  {n:22} {t['category']:7} bound {n_members(t):4} blocked {len(t['blocked']):3} hidden {len(t['hidden']):3}"
                 f"  async {sum(1 for m in t['methods'] if m.get('async'))}")
    return "\n".join(L) + "\n"


def outputs(data: dict) -> dict[Path, str]:
    files = {FACTS_DIR / f"{fw}.json": serialise(d) for fw, d in split_by_framework(data).items()}
    files[FACTS_DIR / "report.txt"] = report(data)
    return files


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--report", action="store_true")
    g.add_argument("--write", action="store_true")
    g.add_argument("--check", action="store_true")
    g.add_argument("--probe-list", action="store_true", help="print the classes the macOS types probe lists properties of")
    a = ap.parse_args()
    if a.probe_list:
        ing, ov = Ingest(), load_overrides()
        for n in sorted(ing.classes):
            if classify(ing, n, ov) in ("value", "object", "statics"):
                print(n)
        return 0
    data = build()
    if a.write:
        for p, text in outputs(data).items():
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        print(f"wrote {len(data['types'])} types to {FACTS_DIR.relative_to(ROOT)}")
        return 0
    if a.check:
        bad = [p for p, text in outputs(data).items() if not p.exists() or p.read_text() != text]
        for p in bad:
            print(f"stale: {p.relative_to(ROOT)} (run types_appkit_classify.py --write)", file=sys.stderr)
        return 1 if bad else 0
    print(report(data), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
