#!/usr/bin/env python3
"""Probe: ingest a Qt library with no qdoc index (ADS) through `moc --output-json` + libclang.

Usage: python3 -I docs/notes/scripts/cw_moc_json.py <src-dir> [--moc /usr/lib/qt6/moc] [--json out.json]
Runs moc on every header with Q_OBJECT/Q_GADGET, then libclang on the same headers to list the
public methods moc cannot see (plain C++ members). Prints per-class counts and the facts a
svelters element kind needs. Compiles nothing.
"""
import argparse, glob, json, os, subprocess, sys, tempfile

ap = argparse.ArgumentParser()
ap.add_argument("src")
ap.add_argument("--moc", default="/usr/lib/qt6/moc")
ap.add_argument("--json")
a = ap.parse_args()

def qt_includes():
    out = subprocess.run(["pkg-config", "--cflags", "Qt6Widgets"], capture_output=True, text=True).stdout
    return [t for t in out.split() if t.startswith("-I")]

classes = {}
with tempfile.TemporaryDirectory() as td:
    for h in sorted(glob.glob(os.path.join(a.src, "*.h"))):
        text = open(h, encoding="utf-8", errors="replace").read()
        if "Q_OBJECT" not in text and "Q_GADGET" not in text:
            continue
        out = os.path.join(td, os.path.basename(h) + ".moc")
        r = subprocess.run([a.moc, "--output-json", "-o", out, *qt_includes(), "-I", a.src, h],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print("moc failed", h, r.stderr[:300], file=sys.stderr)
            continue
        meta = json.load(open(out + ".json"))
        for c in meta.get("classes", []):
            c["header"] = os.path.basename(h)
            classes[c["qualifiedClassName"]] = c

# libclang: public methods per class (what moc misses).
methods = {}
try:
    import clang.cindex as ci
    res = subprocess.run(["clang", "-print-resource-dir"], capture_output=True, text=True).stdout.strip()
    idx = ci.Index.create()
    for c in classes.values():
        h = os.path.join(a.src, c["header"])
        tu = idx.parse(h, args=["-x", "c++", "-std=c++17", "-fPIC", f"-resource-dir={res}", *qt_includes(), f"-I{a.src}"])
        def walk(n, ns=""):
            for k in n.get_children():
                if k.location.file and os.path.basename(str(k.location.file)) != c["header"]:
                    continue
                if k.kind in (ci.CursorKind.NAMESPACE,):
                    walk(k, ns + k.spelling + "::")
                elif k.kind == ci.CursorKind.CLASS_DECL and k.is_definition():
                    q = ns + k.spelling
                    ms = methods.setdefault(q, [])
                    for m in k.get_children():
                        if m.kind == ci.CursorKind.CXX_METHOD and m.access_specifier == ci.AccessSpecifier.PUBLIC:
                            ms.append(f"{'static ' if m.is_static_method() else ''}{m.result_type.spelling} {m.spelling}({', '.join(x.type.spelling for x in m.get_arguments())})")
                        elif m.kind == ci.CursorKind.CONSTRUCTOR and m.access_specifier == ci.AccessSpecifier.PUBLIC:
                            ms.append(f"ctor({', '.join(x.type.spelling for x in m.get_arguments())})")
        walk(tu.cursor)
except Exception as e:  # libclang optional
    print("libclang pass skipped:", e, file=sys.stderr)

tot = {"classes": 0, "props": 0, "signals": 0, "slots": 0, "invokables": 0, "enums": 0, "methods": 0}
for q, c in sorted(classes.items()):
    supers = ",".join(s.get("fullyQualifiedName", s.get("name")) for s in c.get("superClasses", []))
    n = {k: len(c.get(k, [])) for k in ("properties", "signals", "slots", "methods", "enums")}
    pub = methods.get(q, [])
    tot["classes"] += 1; tot["props"] += n["properties"]; tot["signals"] += n["signals"]
    tot["slots"] += n["slots"]; tot["invokables"] += n["methods"]; tot["enums"] += n["enums"]; tot["methods"] += len(pub)
    print(f"{q:40s} <- {supers:28s} Q_PROPERTY {n['properties']:2d} signals {n['signals']:2d} slots {n['slots']:2d} "
          f"invokable {n['methods']:2d} enums {n['enums']:2d} | public C++ methods (clang) {len(pub):3d}")
print("totals:", tot)
for q in ("ads::CDockManager", "ads::CDockWidget"):
    c = classes.get(q)
    if not c:
        continue
    print(f"\n== {q}")
    print("  Q_PROPERTY:", [p["name"] + ":" + p["type"] for p in c.get("properties", [])])
    print("  signals:", [s["name"] + "(" + ",".join(x["type"] for x in s.get("arguments", [])) + ")" for s in c.get("signals", [])])
    print("  slots:", [s["name"] for s in c.get("slots", [])][:30])
    print("  enums:", [e["name"] for e in c.get("enums", [])])
    print("  ctors (clang):", [m for m in methods.get(q, []) if m.startswith("ctor")])
    print("  sample methods (clang):", [m for m in methods.get(q, []) if "Dock" in m or "State" in m][:25])
if a.json:
    json.dump({"moc": classes, "clang_public_methods": methods}, open(a.json, "w"), indent=1)
