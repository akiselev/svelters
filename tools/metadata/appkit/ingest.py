#!/usr/bin/env python3
"""Ingest AppKit classes from a macOS SDK into metadata/appkit/*.json (svelters-element-kind/v0).

Reads the SDK's Objective-C headers with libclang and writes, for each class named in
metadata/overrides/appkit/*.yaml and each of its superclasses up to NSResponder (exclusive):
properties (type, access, memory, accessors, availability), the instance methods the overrides
name, and the enums those use (metadata/appkit/_enums.json).

Licence: the SDK is Apple's (Xcode and Apple SDKs Agreement). Only derived facts are written:
names, types, numeric enum values, availability versions. No header text, no doc comments. Run it
on a Mac (`--sdk $(xcrun --show-sdk-path)`); the output is checked in, so Linux never needs the SDK.

Usage:
  python3 -I tools/metadata/appkit/ingest.py --sdk /path/to/MacOSX.sdk [--check]

Needs the libclang Python bindings (`pip install libclang` or the distro's python-clang) matching
a libclang that knows the SDK's language features (clang 17+ is fine).
"""
from __future__ import annotations

import argparse
import ctypes
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "common"))
import names  # noqa: E402
import svmeta  # noqa: E402

ROOT = HERE.parents[2]
OVERRIDES = ROOT / "metadata/overrides/appkit"
OUT = ROOT / "metadata/appkit"
STOP_AT = {"NSResponder", "NSObject"}

PROP_READONLY, PROP_ASSIGN, PROP_RETAIN, PROP_COPY, PROP_WEAK, PROP_STRONG, PROP_CLASS = 0x01, 0x04, 0x10, 0x20, 0x200, 0x400, 0x1000
NULLABILITY = {0: "nonnull", 1: "nullable", 2: "unspecified", 3: "invalid", 4: "nullable_result"}

# C spellings (canonical or typedef) -> neutral type refs. Anything else is classified below.
SCALARS = {
    "BOOL": "bool", "bool": "bool", "_Bool": "bool",
    "NSInteger": "i64", "long": "i64", "long long": "i64", "int": "i32", "short": "i32",
    "NSUInteger": "u64", "unsigned long": "u64", "unsigned long long": "u64", "unsigned int": "u32", "unsigned short": "u32",
    "CGFloat": "f64", "double": "f64", "float": "f32", "NSTimeInterval": "f64",
}
STRUCTS = {"NSRect": "rect", "CGRect": "rect", "NSSize": "size", "CGSize": "size", "NSPoint": "point", "CGPoint": "point",
           "NSEdgeInsets": "insets", "NSRange": "range"}


def setup_lib(ci):
    lib = ci.conf.lib
    lib.clang_Cursor_getObjCPropertyAttributes.argtypes = [ci.Cursor, ctypes.c_uint]
    lib.clang_Cursor_getObjCPropertyAttributes.restype = ctypes.c_uint
    lib.clang_Type_getNullability.argtypes = [ci.Type]
    lib.clang_Type_getNullability.restype = ctypes.c_int
    for fn in ("clang_Cursor_getObjCPropertyGetterName", "clang_Cursor_getObjCPropertySetterName"):
        getattr(lib, fn).argtypes = [ci.Cursor]
        getattr(lib, fn).restype = ci._CXString
    return lib


_FILES: dict[str, str] = {}


def decl_text(cur, stops: bytes = b";{") -> str:
    """The declaration's source text up to its terminating `;` (trailing attribute macros such as
    API_AVAILABLE are outside the cursor's extent for properties and methods)."""
    ext = cur.extent
    if ext.start.file is None:
        return ""
    name = str(ext.start.file)
    if name not in _FILES:
        _FILES[name] = Path(name).read_text(errors="replace")
    text = _FILES[name].encode()
    start, end = ext.start.offset, ext.end.offset
    stop = min([x for x in (text.find(bytes([c]), end) for c in stops) if x >= 0] + [len(text)])
    return text[start:stop].decode(errors="replace")


def availability(cur, stops: bytes = b";{") -> dict:
    """macOS introduced / deprecated versions from the API_AVAILABLE / API_DEPRECATED macros in the decl."""
    toks = " ".join(re.findall(r"[A-Za-z_]\w*|\d+(?:\.\d+)*|\S", decl_text(cur, stops)))
    out = {}
    m = re.search(r"API_AVAILABLE \( (?:[a-z]+ \( [\d.]+ \) , )*macos \( ([\d.]+) \)", toks)
    if m:
        out["since"] = m.group(1).replace(" ", "")
    m = re.search(r"API_DEPRECATED(?:_WITH_REPLACEMENT)? \(.*?macos \( ([\d.]+) , ([\d.]+|API_TO_BE_DEPRECATED) \)", toks)
    if m:
        out["deprecated"] = m.group(2).replace(" ", "")
    if re.search(r"API_UNAVAILABLE \([^)]*macos", toks):
        out["unavailable"] = True
    return out


class Ingest:
    def __init__(self, ci, lib, tu):
        self.ci, self.lib, self.tu = ci, lib, tu
        self.classes: dict[str, list] = {}  # name -> [interface cursor, category cursors...]
        self.enums: dict[str, dict] = {}    # typedef name -> enum record (filled lazily)
        self.enum_cursors: dict[str, object] = {}
        for c in tu.cursor.get_children():
            k = c.kind
            if k == ci.CursorKind.OBJC_INTERFACE_DECL and any(True for _ in c.get_children()):
                self.classes.setdefault(c.spelling, []).insert(0, c)
            elif k == ci.CursorKind.OBJC_CATEGORY_DECL:
                cls = next((ch.spelling for ch in c.get_children() if ch.kind == ci.CursorKind.OBJC_CLASS_REF), None)
                if cls:
                    self.classes.setdefault(cls, []).append(c)
            elif k == ci.CursorKind.TYPEDEF_DECL:
                und = c.underlying_typedef_type
                decl = und.get_declaration()
                if decl.kind == ci.CursorKind.ENUM_DECL:
                    self.enum_cursors[c.spelling] = decl.get_definition() or decl

    def superclass(self, name: str) -> str | None:
        for cur in self.classes.get(name, [])[:1]:
            for ch in cur.get_children():
                if ch.kind == self.ci.CursorKind.OBJC_SUPER_CLASS_REF:
                    return ch.spelling
        return None

    def bases(self, name: str) -> list[str]:
        out, cur = [], self.superclass(name)
        while cur:
            out.append(cur)
            cur = self.superclass(cur)
        return out

    def enum_ref(self, spelling: str) -> str | None:
        """`enum<Name>` / `flags<Name>` for an NS_ENUM / NS_OPTIONS typedef, recording its members."""
        name = spelling.strip()
        decl = self.enum_cursors.get(name)
        if decl is None:
            return None
        if name not in self.enums:
            members = []
            for ch in decl.get_children():
                if ch.kind == self.ci.CursorKind.ENUM_CONSTANT_DECL:
                    av = availability(ch, b",}")
                    if av.get("unavailable") or "deprecated" in av:
                        continue
                    members.append({"native": ch.spelling, "value": ch.enum_value})
            toks = [t.spelling for t in decl.get_tokens()][:8]
            flags = "NS_OPTIONS" in toks or any(
                ch.kind.name in ("FLAG_ENUM", "FLAG_ENUM_ATTR") for ch in decl.get_children())
            prefix = common_prefix(name, [m["native"] for m in members])
            for m in members:
                m["js"] = names.kebab(m["native"][len(prefix):] or m["native"])
            self.enums[name] = {"name": name, "flags": flags, "members": members}
        return f"{'flags' if self.enums[name]['flags'] else 'enum'}<{name}>"

    def type_ref(self, ty) -> dict:
        sp = ty.spelling
        sp = re.sub(r"\b(?:API|NS|APPKIT)_[A-Z_]+\((?:[^()]|\([^()]*\))*\)", "", sp)
        for q in ("_Nullable", "_Nonnull", "_Null_unspecified", "__kindof", "const", "__strong", "__weak"):
            sp = sp.replace(q, "")
        sp = " ".join(sp.split())
        base = sp.rstrip(" *")
        nullable = NULLABILITY.get(self.lib.clang_Type_getNullability(ty), "?")
        out = {"native": sp}
        if nullable in ("nullable", "nullable_result"):
            out["nullable"] = True
        if base in SCALARS:
            out["ref"] = SCALARS[base]
        elif base in STRUCTS:
            out["ref"] = STRUCTS[base]
        elif (e := self.enum_ref(base)) is not None:
            out["ref"] = e
        elif base in ("NSString", "NSAttributedString") and sp.endswith("*"):
            out["ref"] = "string"
        elif base == "SEL":
            out["ref"] = "selector"
        elif base.startswith("id") or (base.startswith("NS") and sp.endswith("*")) or base.startswith("CA") and sp.endswith("*"):
            cls = re.match(r"(\w+)", base).group(1)
            out["ref"] = "object" if cls == "id" else f"object<{cls}>"
            if "<" in base and cls.startswith("NSArray"):
                out["ref"] = "list"
            if cls.startswith(("NSArray", "NSSet", "NSDictionary")):
                out["ref"] = "list" if cls.startswith(("NSArray", "NSSet")) else "map"
        else:
            canon = " ".join(ty.get_canonical().spelling.split())
            out["ref"] = SCALARS.get(canon, "unsupported")
        return out

    def members(self, name: str, want_methods: set[str]) -> tuple[list, list]:
        props, methods, seen = [], [], set()
        lib = self.lib
        for cur in self.classes.get(name, []):
            for c in cur.get_children():
                if c.kind == self.ci.CursorKind.OBJC_PROPERTY_DECL:
                    bits = lib.clang_Cursor_getObjCPropertyAttributes(c, 0)
                    if bits & PROP_CLASS or c.spelling in seen:
                        continue
                    av = availability(c)
                    if av.get("unavailable") or "deprecated" in av:
                        continue
                    seen.add(c.spelling)
                    memory = "weak" if bits & PROP_WEAK else "copy" if bits & PROP_COPY else "strong" if bits & (PROP_STRONG | PROP_RETAIN) else "assign"
                    rec = {
                        "name": c.spelling,
                        "js_name": names.kebab(c.spelling),
                        "type": self.type_ref(c.type),
                        "access": {"read": True, "write": not bits & PROP_READONLY},
                        "memory": memory,
                        "accessors": {"get": ci_str(lib.clang_Cursor_getObjCPropertyGetterName(c), self.ci)},
                        "origin": "sdk",
                    }
                    if not bits & PROP_READONLY:
                        rec["accessors"]["set"] = ci_str(lib.clang_Cursor_getObjCPropertySetterName(c), self.ci)
                    if av:
                        rec["availability"] = av
                    props.append(rec)
                elif c.kind == self.ci.CursorKind.OBJC_INSTANCE_METHOD_DECL and c.spelling in want_methods:
                    if any(m["selector"] == c.spelling for m in methods):
                        continue
                    rec = {
                        "name": names.selector_name(c.spelling),
                        "selector": c.spelling,
                        "params": [{"name": a.spelling, "type": self.type_ref(a.type)} for a in c.get_arguments()],
                        "returns": {"type": {"ref": "void"} if c.result_type.spelling == "void" else self.type_ref(c.result_type)},
                        "origin": "sdk",
                    }
                    av = availability(c)
                    if av:
                        rec["availability"] = av
                    methods.append(rec)
        props.sort(key=lambda p: p["name"])
        methods.sort(key=lambda m: m["selector"])
        return props, methods


def ci_str(s, ci) -> str:
    return ci._CXString.from_result(s)


def common_prefix(type_name: str, members: list[str]) -> str:
    """The prefix to strip from enum member names: the type name if all members start with it,
    else the longest camel-case-word prefix the members share (with each other and, for a single
    member, with the type name)."""
    if members and all(m.startswith(type_name) and len(m) > len(type_name) for m in members):
        return type_name
    if not members:
        return ""
    p = members[0]
    for m in members[1:] if len(members) > 1 else [type_name]:
        while not m.startswith(p):
            p = p[:-1]
    # back off to a word boundary (an uppercase letter starts the next word)
    while p and not (len(p) < len(members[0]) and members[0][len(p)].isupper()):
        p = p[:-1]
    return p


def load_overrides() -> dict[str, dict]:
    return {path.stem: svmeta.load_yaml(path) for path in sorted(OVERRIDES.glob("*.yaml"))}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sdk", required=True, help="path to MacOSX*.sdk (on a Mac: $(xcrun --show-sdk-path))")
    ap.add_argument("--target", default="arm64-apple-macos14")
    ap.add_argument("--check", action="store_true", help="fail if the checked-in metadata differs")
    a = ap.parse_args()

    import clang.cindex as ci
    lib = setup_lib(ci)
    sdk = Path(a.sdk).resolve()
    res = subprocess.run(["clang", "-print-resource-dir"], capture_output=True, text=True).stdout.strip()
    args = ["-x", "objective-c", "-target", a.target, "-isysroot", str(sdk), "-fobjc-arc"]
    if res:
        args += ["-resource-dir", res]
    tu = ci.Index.create().parse("ingest.m", args=args, unsaved_files=[("ingest.m", "#import <AppKit/AppKit.h>\n")],
                                 options=ci.TranslationUnit.PARSE_SKIP_FUNCTION_BODIES | 0x1000)
    errs = [str(d) for d in tu.diagnostics if d.severity >= ci.Diagnostic.Error]
    if errs:
        print("clang errors:\n  " + "\n  ".join(errs[:10]), file=sys.stderr)
        return 1
    ing = Ingest(ci, lib, tu)

    overrides = load_overrides()
    wanted: dict[str, set[str]] = {}
    for cls, ov in overrides.items():
        if cls not in ing.classes:
            print(f"ingest: {cls} (from overrides) is not in the SDK", file=sys.stderr)
            return 1
        sels = {m["selector"] for m in ov.get("methods", [])} | {s["selector"] for s in ov.get("setters", [])}
        for slot in ov.get("slots", []):
            ops = slot.get("ops", {})
            sels |= {v for k, v in ops.items() if k in ("set", "append", "insert_at", "insert_relative")}
            sels |= set(ops.get("remove", [])) | set(ops.get("detach", []))
        wanted.setdefault(cls, set()).update(sels)
        for b in ing.bases(cls):
            if b in STOP_AT:
                break
            wanted.setdefault(b, set())
    # methods named on a class may be declared on a base: look them up along the chain
    all_sels = set().union(*wanted.values())

    sdk_name = sdk.name.removesuffix(".sdk")
    outs: dict[Path, object] = {}
    for cls in sorted(wanted):
        props, methods = ing.members(cls, all_sels)
        chain = ing.bases(cls)
        decl = ing.classes[cls][0]
        rec = {
            "schema": svmeta.SCHEMA,
            "toolkit": "appkit",
            "id": f"svelters-api:v1:type:appkit:AppKit:{cls}",
            "native": {"name": cls, "framework": "AppKit", "header": Path(str(decl.location.file)).name},
            "kind": "object",
            "abstract": False,
            "bases": chain,
            "properties": props,
            "methods": methods,
            "source": {"sdk": sdk_name, "target": a.target, "docs": f"https://developer.apple.com/documentation/appkit/{cls.lower()}?language=objc"},
        }
        outs[OUT / f"{cls}.json"] = rec
    outs[OUT / "_enums.json"] = {"schema": "svelters-appkit-enums/v0", "sdk": sdk_name,
                                 "enums": [ing.enums[k] for k in sorted(ing.enums)]}

    stale = []
    for path, data in outs.items():
        text = json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
        if not path.exists() or path.read_text() != text:
            stale.append(path)
            if not a.check:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
    extra = sorted(set(OUT.glob("*.json")) - set(outs))
    if a.check:
        for p in stale + extra:
            print(f"ingest: {p.relative_to(ROOT)} is stale", file=sys.stderr)
        return 1 if stale or extra else 0
    for p in extra:
        p.unlink()
    total = sum(p.stat().st_size for p in outs)
    print(f"{len(outs) - 1} classes, {len(ing.enums)} enums, {len(stale)} files written, {total // 1024} KiB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
