#!/usr/bin/env python3
"""Dump Objective-C class/protocol metadata from an Apple SDK on Linux (libclang).

Probe for docs/notes/apple-gui-backend.md §3. Parses the real SDK headers with the
system libclang (no Xcode needed) and prints, per selected class/protocol:
superclass, adopted protocols, @property attributes (readonly/copy/weak/...,
nullability, getter/setter), methods (selector, return type, @optional), and the
raw availability / Swift / main-actor macros as written in the header.

Usage (needs the `clang` PyPI bindings matching a system libclang >= 21):
  python -I apple_objc_dump.py --sdk MacOSX26.1.sdk --framework AppKit \
      --target arm64-apple-macos15 NSStackView NSWindowDelegate ... > out.json
  python -I apple_objc_dump.py --sdk MacOSX26.1.sdk --framework UIKit --catalyst \
      --target arm64-apple-ios17-macabi UIStackView ...
"""
import argparse
import ctypes
import json
import os
import subprocess
import sys

import clang.cindex as ci

PROP_ATTRS = [
    (0x01, "readonly"), (0x02, "getter"), (0x04, "assign"), (0x08, "readwrite"),
    (0x10, "retain"), (0x20, "copy"), (0x40, "nonatomic"), (0x80, "setter"),
    (0x100, "atomic"), (0x200, "weak"), (0x400, "strong"), (0x800, "unsafe_unretained"),
    (0x1000, "class"),
]
NULLABILITY = {0: "nonnull", 1: "nullable", 2: "unspecified", 3: "invalid", 4: "nullable_result"}
# macros worth keeping verbatim (they are erased by the preprocessor in the AST)
MACRO_PREFIXES = ("API_", "NS_SWIFT", "NS_DESIGNATED", "NS_REFINED", "NS_UNAVAILABLE",
                  "NS_AVAILABLE", "NS_DEPRECATED", "UIKIT_", "APPKIT_", "NS_REQUIRES_SUPER",
                  "NS_RETURNS", "IBAction", "IBOutlet", "IBInspectable", "NS_HEADER_AUDIT",
                  "MP_", "__attribute__")


def setup_lib():
    lib = ci.conf.lib
    lib.clang_Cursor_getObjCPropertyAttributes.argtypes = [ci.Cursor, ctypes.c_uint]
    lib.clang_Cursor_getObjCPropertyAttributes.restype = ctypes.c_uint
    lib.clang_Cursor_isObjCOptional.argtypes = [ci.Cursor]
    lib.clang_Cursor_isObjCOptional.restype = ctypes.c_uint
    lib.clang_Type_getNullability.argtypes = [ci.Type]
    lib.clang_Type_getNullability.restype = ctypes.c_int
    for fn in ("clang_Cursor_getObjCPropertyGetterName", "clang_Cursor_getObjCPropertySetterName"):
        getattr(lib, fn).argtypes = [ci.Cursor]
        getattr(lib, fn).restype = ci._CXString
    return lib


def cxs(s):
    return ci._CXString.from_result(s)


def macros(cur):
    """Tokens in the declaration's extent that look like annotation macros (+ their arg text)."""
    toks = [t.spelling for t in cur.get_tokens()]
    out, i = [], 0
    while i < len(toks):
        t = toks[i]
        if t.startswith(MACRO_PREFIXES):
            if i + 1 < len(toks) and toks[i + 1] == "(":
                depth, j = 0, i + 1
                while j < len(toks):
                    depth += toks[j] == "("
                    depth -= toks[j] == ")"
                    j += 1
                    if depth == 0:
                        break
                out.append(t + "".join(toks[i + 1:j]))
                i = j
                continue
            out.append(t)
        i += 1
    return out


def nullability(lib, ty):
    return NULLABILITY.get(lib.clang_Type_getNullability(ty), "?")


def dump_container(lib, cur):
    d = {"name": cur.spelling, "kind": cur.kind.name, "file": os.path.basename(str(cur.location.file)),
         "super": None, "protocols": [], "properties": [], "methods": []}
    d["decl_macros"] = macros(cur)[:6]   # macros on/near the @interface line
    for c in cur.get_children():
        k = c.kind
        if k == ci.CursorKind.OBJC_SUPER_CLASS_REF:
            d["super"] = c.spelling
        elif k == ci.CursorKind.OBJC_PROTOCOL_REF:
            d["protocols"].append(c.spelling)
        elif k == ci.CursorKind.OBJC_PROPERTY_DECL:
            bits = lib.clang_Cursor_getObjCPropertyAttributes(c, 0)
            d["properties"].append({
                "name": c.spelling,
                "type": c.type.spelling,
                "nullability": nullability(lib, c.type),
                "attrs": [n for b, n in PROP_ATTRS if bits & b],
                "getter": cxs(lib.clang_Cursor_getObjCPropertyGetterName(c)),
                "setter": None if bits & 0x01 else cxs(lib.clang_Cursor_getObjCPropertySetterName(c)),
                "optional": bool(lib.clang_Cursor_isObjCOptional(c)),
                "macros": macros(c),
            })
        elif k in (ci.CursorKind.OBJC_INSTANCE_METHOD_DECL, ci.CursorKind.OBJC_CLASS_METHOD_DECL):
            d["methods"].append({
                "sel": ("+" if k == ci.CursorKind.OBJC_CLASS_METHOD_DECL else "-") + c.spelling,
                "ret": c.result_type.spelling,
                "ret_null": nullability(lib, c.result_type),
                "params": [(a.spelling, a.type.spelling, nullability(lib, a.type)) for a in c.get_arguments()],
                "optional": bool(lib.clang_Cursor_isObjCOptional(c)),
                "implicit_accessor": False,
                "macros": macros(c),
            })
    # property accessors also appear as method decls; mark them
    accessors = {p["getter"] for p in d["properties"]} | {p["setter"] for p in d["properties"] if p["setter"]}
    for m in d["methods"]:
        m["implicit_accessor"] = m["sel"][1:] in accessors
    return d


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sdk", required=True)
    ap.add_argument("--framework", required=True)
    ap.add_argument("--target", required=True)
    ap.add_argument("--catalyst", action="store_true", help="add System/iOSSupport search paths")
    ap.add_argument("--summary", action="store_true", help="print counts for the whole framework")
    ap.add_argument("names", nargs="*")
    a = ap.parse_args()
    lib = setup_lib()
    sdk = os.path.abspath(a.sdk)
    res = subprocess.run(["clang", "-print-resource-dir"], capture_output=True, text=True).stdout.strip()
    args = ["-x", "objective-c", "-target", a.target, "-isysroot", sdk, "-fobjc-arc", "-resource-dir", res]
    if a.catalyst:
        args += ["-iframework", f"{sdk}/System/iOSSupport/System/Library/Frameworks",
                 "-isystem", f"{sdk}/System/iOSSupport/usr/include"]
    idx = ci.Index.create()
    tu = idx.parse("probe.m", args=args, unsaved_files=[("probe.m", f"#import <{a.framework}/{a.framework}.h>\n")],
                   options=ci.TranslationUnit.PARSE_SKIP_FUNCTION_BODIES
                   | 0x1000    # CXTranslationUnit_IncludeAttributedTypes: keep _Nullable/_Nonnull sugar
                   | 0x2000)   # CXTranslationUnit_VisitImplicitAttributes
    errs = [str(d) for d in tu.diagnostics if d.severity >= ci.Diagnostic.Error]
    want = set(a.names)
    out, stats = {}, {"classes": 0, "protocols": 0, "properties": 0, "methods": 0, "errors": errs[:5]}
    fwdir = f"/{a.framework}.framework/"
    for c in tu.cursor.get_children():
        if c.kind not in (ci.CursorKind.OBJC_INTERFACE_DECL, ci.CursorKind.OBJC_PROTOCOL_DECL,
                          ci.CursorKind.OBJC_CATEGORY_DECL):
            continue
        if fwdir not in str(c.location.file):
            continue
        if not any(True for _ in c.get_children()):
            continue  # forward declaration
        if a.summary:
            stats["classes" if c.kind == ci.CursorKind.OBJC_INTERFACE_DECL else "protocols"] += (
                c.kind != ci.CursorKind.OBJC_CATEGORY_DECL)
            for ch in c.get_children():
                stats["properties"] += ch.kind == ci.CursorKind.OBJC_PROPERTY_DECL
                stats["methods"] += ch.kind in (ci.CursorKind.OBJC_INSTANCE_METHOD_DECL,
                                                ci.CursorKind.OBJC_CLASS_METHOD_DECL)
        name = c.spelling
        if c.kind == ci.CursorKind.OBJC_CATEGORY_DECL:
            cls = next((ch.spelling for ch in c.get_children() if ch.kind == ci.CursorKind.OBJC_CLASS_REF), "?")
            if cls in want:
                d = dump_container(lib, c)
                out.setdefault(cls, {"categories": []}).setdefault("categories", []).append(d)
            continue
        if name in want:
            out.setdefault(name, {}).update(dump_container(lib, c))
    json.dump(stats if a.summary else out, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
