#!/usr/bin/env python3
"""Classify the virtual functions of QtWidgets classes (plus the model/validator bases they
use) from qdoc .index files: which are reachable via an event filter, which need a
generated C++ subclass, and which need a synchronous return value from JS.

Counts *distinct (introducing-class, name)* virtuals: an `override` is folded into the
class that first declares the name in its base chain, so QWidget::closeEvent is one
virtual no matter how many subclasses override it.

Usage: python3 -I qt_virtual_categories.py [--docdir /usr/share/doc/qt6] [--list]
"""
import argparse
import collections
import re
import xml.etree.ElementTree as ET

ap = argparse.ArgumentParser()
ap.add_argument("--docdir", default="/usr/share/doc/qt6")
ap.add_argument("--list", action="store_true")
a = ap.parse_args()

classes = {}
for mod in ("qtcore", "qtgui", "qtwidgets"):
    root = ET.parse(f"{a.docdir}/{mod}/{mod}.index").getroot()
    for c in root.iter("class"):
        bases = [b.strip().replace("public ", "") for b in (c.get("bases") or "").split(",") if b.strip()]
        fns = []
        for f in c.findall("function"):
            if f.get("virtual") in ("virtual", "pure") and f.get("access") in ("public", "protected") \
                    and f.get("meta") not in ("destructor", "signal"):
                fns.append(f)
        classes[c.get("name")] = {"mod": mod, "bases": bases, "fns": fns}


def chain(n, seen=None):
    seen = seen or set()
    out = []
    for b in classes.get(n, {}).get("bases", []):
        b = re.sub(r"<.*", "", b)
        if b in classes and b not in seen:
            seen.add(b)
            out += [b] + chain(b, seen)
    return out


def derives(n, root):
    return n == root or root in chain(n)


# scope: every QtWidgets QObject class + the abstract item models / validators / delegates
# that a QtWidgets app subclasses to feed views and line edits.
EXTRA = {"QAbstractItemModel", "QAbstractListModel", "QAbstractTableModel", "QValidator",
         "QSortFilterProxyModel", "QAbstractProxyModel", "QObject"}
scope = [n for n, c in classes.items()
         if (c["mod"] == "qtwidgets" and derives(n, "QObject")) or n in EXTRA]


def category(cls, f):
    name, sig, rt = f.get("name"), f.get("signature") or "", f.get("type") or ""
    if name in ("event", "eventFilter", "sceneEvent", "viewportEvent", "nativeEvent", "editorEvent"):
        return "event_dispatch_bool"
    if re.search(r"\b(Q\w*Event) \*", sig) and name.endswith("Event"):
        if name == "closeEvent":
            return "event_handler_veto"           # accept()/ignore() is the result
        return "event_handler"
    if name in ("paintEvent", "paint", "drawRow", "drawBranches", "drawItemText", "initStyleOption"):
        return "paint"
    if name in ("sizeHint", "minimumSizeHint", "heightForWidth", "hasHeightForWidth",
                "sizeHintForRow", "sizeHintForColumn", "sectionSizeFromContents"):
        return "geometry_query"
    if derives(cls, "QAbstractItemModel"):
        return "model"
    if derives(cls, "QValidator") or name in ("validate", "fixup", "textFromValue", "valueFromText", "stepEnabled"):
        return "validator_formatter"
    if derives(cls, "QAbstractItemDelegate"):
        return "delegate"
    if rt.startswith("void"):
        return "void_other"
    return "returns_other"


intro = {}
for n in scope:
    for f in classes[n]["fns"]:
        name = f.get("name")
        # fold overrides into the first base in the chain declaring the same name
        owner = n
        for b in chain(n):
            if any(g.get("name") == name for g in classes.get(b, {}).get("fns", [])):
                owner = b
        key = (owner, name)
        if key not in intro:
            intro[key] = category(owner, classes[owner]["fns"][[g.get("name") for g in classes[owner]["fns"]].index(name)]
                                  if any(g.get("name") == name for g in classes[owner]["fns"]) else f)

cnt = collections.Counter(intro.values())
print(f"classes in scope: {len(scope)}; distinct introducing virtuals: {len(intro)}")
for k, v in cnt.most_common():
    print(f"  {k:22} {v}")
by_owner = collections.Counter(o for o, _ in intro)
print("top introducing classes:", ", ".join(f"{o}={n}" for o, n in by_owner.most_common(12)))
if a.list:
    for (o, n), c in sorted(intro.items(), key=lambda x: (x[1], x[0])):
        print(f"{c:22} {o}::{n}")
