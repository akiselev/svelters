#!/usr/bin/env python3
"""Quantify how much of a Qt module's public C++ API the QMetaObject system covers,
using the qdoc .index XML shipped with qt6-doc (stdlib only, no Qt needed).

qdoc index records, per function: meta=constructor|plain|signal|slot|destructor|...,
access, virtual, override, const, associated-property, parameter types + defaults,
signature, since, status. Q_INVOKABLE is NOT marked (it appears as meta="plain"), so
the "plain" bucket is an upper bound for "needs a generated thunk".

Usage: python3 -I qdoc_index_stats.py [qtwidgets] [--docdir /usr/share/doc/qt6] [--class QPushButton]
"""
import argparse
import collections
import xml.etree.ElementTree as ET

ap = argparse.ArgumentParser()
ap.add_argument("module", nargs="?", default="qtwidgets")
ap.add_argument("--docdir", default="/usr/share/doc/qt6")
ap.add_argument("--class", dest="klass")
a = ap.parse_args()

bases = {}
classes = {}
for mod in ("qtcore", "qtgui", a.module):
    root = ET.parse(f"{a.docdir}/{mod}/{mod}.index").getroot()
    for c in root.iter("class"):
        bases[c.get("name")] = [b.strip() for b in (c.get("bases") or "").split(",") if b.strip()]
        if mod == a.module:
            classes[c.get("name")] = c


def is_qobject(n, seen=()):
    if n == "QObject":
        return True
    return any(is_qobject(b.split("<")[0], seen + (n,)) for b in bases.get(n, []) if b not in seen)


c_tot = collections.Counter()
o_tot = collections.Counter()
nq = 0
for name, c in sorted(classes.items()):
    if c.get("access") != "public" or c.get("status") not in ("active", None):
        continue
    if not is_qobject(name):
        continue
    nq += 1
    by_name = collections.defaultdict(set)
    cnt = collections.Counter()
    for f in c.findall("function"):
        if f.get("access") != "public" or f.get("status") == "deprecated" or f.get("status") == "obsolete":
            continue
        meta = f.get("meta")
        if meta in ("destructor", "copy-assign", "copy-constructor"):
            continue
        if f.get("override") == "true":
            continue  # re-declaration of an inherited virtual; reachable through the base
        if meta in ("signal", "slot"):
            k = meta
        elif meta == "constructor":
            k = "constructor"
        elif f.get("associated-property"):
            k = "property accessor"
        else:
            k = "plain (ordinary or Q_INVOKABLE)"
        cnt[k] += 1
        by_name[k].add(f.get("name"))
    for k, v in cnt.items():
        o_tot[k] += v
    for k, v in by_name.items():
        c_tot[k] += len(v)
    nprops = len(c.findall("property"))
    o_tot["Q_PROPERTY (declared)"] += nprops
    if a.klass == name:
        for f in c.findall("function"):
            if f.get("access") == "public" and f.get("override") != "true" and not f.get("associated-property") \
                    and f.get("meta") == "plain":
                print("  plain:", f.get("signature"))

total = sum(v for k, v in o_tot.items() if k != "Q_PROPERTY (declared)")
print(f"{a.module}: {nq} public QObject-derived classes; {total} public non-override functions (overloads counted)")
for k, v in o_tot.most_common():
    pct = f"{v / total:5.1%}" if k != "Q_PROPERTY (declared)" else "     "
    print(f"  {v:5} {pct}  {k}   (distinct names: {c_tot.get(k, '-')})")
