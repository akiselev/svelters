#!/usr/bin/env python3
"""Non-GUI types referenced by the ingested Qt classes (metadata/qt/*.json), ranked, with status.

Counts property, method parameter/return and signal argument slots whose type is not a primitive,
string, enum/flags or QWidget-derived pointer. The status comes from the neutral `type.ref` the
ingest assigns (tools/metadata/qt/ingest.py) and what tools/metadata/qt/templates/serdes.hpp.in
converts on 2026-10-17: record<..>, date/time, bytes, list<string>, variant and boxed<..> are
converted; object<T> of a non-widget QObject is a handle; `opaque` and bare `list` are rejected.

Usage: python3 -I types_qt_inventory.py [--meta metadata/qt] [--top 60]
Notes: docs/notes/types-research.md section 1.
"""
import argparse
import collections
import glob
import json
import os
import re

PRIM = {"bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "string", "void", "cstring"}


def core_of(native):
    n = re.sub(r"\bconst\b", "", native or "").replace("&", "").strip()
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--meta", default="metadata/qt")
    ap.add_argument("--top", type=int, default=60)
    a = ap.parse_args()
    docs = {}
    for f in glob.glob(os.path.join(a.meta, "*.json")):
        if os.path.basename(f).startswith("_"):
            continue
        d = json.load(open(f))
        if isinstance(d, dict) and "id" in d:
            docs[d.get("native", {}).get("name") or os.path.basename(f)[:-5]] = d

    def bases(name, seen=()):
        d = docs.get(name)
        if not d:
            return []
        out = []
        for b in d.get("bases", []):
            b = b if isinstance(b, str) else b.get("name", "")
            out += [b] + bases(b, seen + (name,))
        return out

    widgets = {n for n, d in docs.items() if n == "QWidget" or "QWidget" in [b if isinstance(b, str) else b.get("name") for b in d.get("bases", [])]}
    count = collections.Counter()
    status = {}
    where = collections.defaultdict(collections.Counter)

    def visit(t, role, owner_is_widget):
        if not t or not isinstance(t, dict):
            return
        ref = t.get("ref", "opaque")
        native = core_of(t.get("native", ""))
        if ref in PRIM or ref.startswith("enum<") or ref.startswith("flags<"):
            return
        if ref.startswith("object<"):
            cls = ref[7:-1]
            if cls in widgets:
                return
            key, st = cls + "*", "handle:qobject" + ("-kind" if cls in docs else "-no-kind")
        elif ref.startswith("record<"):
            key, st = native, "supported:record"
        elif ref in ("date", "time", "datetime"):
            key, st = native, "supported:date"
        elif ref == "bytes":
            key, st = native, "supported:bytes"
        elif ref == "variant":
            key, st = native, "supported:variant"
        elif ref.startswith("boxed<"):
            key, st = native, "supported:boxed"
        elif ref == "list<string>":
            key, st = native, "supported:list<string>"
        elif ref.startswith("list"):
            key, st = native, "partial:list" if ref.startswith("list<") else "rejected:list"
        else:
            key, st = native, "rejected:opaque"
        count[key] += 1
        status[key] = st
        where[key]["widget" if owner_is_widget else "non-widget"] += 1

    for name, d in docs.items():
        w = name in widgets
        for p in d.get("properties", []):
            visit(p.get("type"), "prop", w)
        for m in d.get("methods", []) + d.get("constructors", []):
            if m.get("deprecated") or m.get("access") not in (None, "public"):
                continue
            for p in m.get("params", []):
                visit(p.get("type"), "param", w)
            r = m.get("returns")
            if isinstance(r, dict) and "ref" in r:
                visit(r, "ret", w)
        for s in d.get("signals", []):
            for p in s.get("params", []):
                visit(p.get("type"), "signal", w)

    by = collections.Counter()
    ty = collections.Counter()
    for k, n in count.items():
        by[status[k]] += n
        ty[status[k]] += 1
    print(f"classes {len(docs)} (widgets {len(widgets)}); distinct non-GUI types {len(count)}, slots {sum(count.values())}")
    for s, n in by.most_common():
        print(f"  {s:28} {n:5} {ty[s]:4}")
    print(f"\ntop {a.top}:")
    for k, n in count.most_common(a.top):
        print(f"  {n:4}  {k:40} {status[k]}")


if __name__ == "__main__":
    main()
