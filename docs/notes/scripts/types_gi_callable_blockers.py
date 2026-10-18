#!/usr/bin/env python3
"""Which GI features block the callables of the non-GUI namespaces (GLib/Gio/Gdk/Gsk/Pango/Graphene).

Classifies every non-deprecated, introspectable method/constructor/function by the hardest feature
it needs, in the order a dynamic GI invoker would add them (crates/svelters-gtk/src/gi.rs today
supports tier "now"). Prints the cumulative coverage per tier and namespace.

Usage: python3 -I types_gi_callable_blockers.py [--girdir /usr/share/gir-1.0]
Notes: docs/notes/types-research.md section 3.
"""
import argparse
import collections
import xml.etree.ElementTree as ET

C = "{http://www.gtk.org/introspection/core/1.0}"
GL = "{http://www.gtk.org/introspection/glib/1.0}"
NSS = ["GLib-2.0", "GObject-2.0", "Gio-2.0", "Gdk-4.0", "Gsk-4.0", "Pango-1.0", "Graphene-1.0", "Gtk-4.0"]
PRIM = {"gboolean", "gint", "guint", "gint8", "guint8", "gint16", "guint16", "gint32", "guint32", "gint64",
        "guint64", "glong", "gulong", "gsize", "gssize", "gchar", "guchar", "gshort", "gushort", "goffset",
        "gintptr", "guintptr", "gdouble", "gfloat", "utf8", "filename", "none"}
# Tiers, in the order an invoker would grow. A callable lands in the first tier that covers all of it.
TIERS = ["now", "+out/inout", "+static/ctor", "+records-by-value", "+arrays/lists", "+callbacks/async", "never"]


def load(girdir):
    types = {}
    for f in NSS:
        root = ET.parse(f"{girdir}/{f}.gir").getroot()
        ns = root.find(C + "namespace")
        for el in ns:
            if el.get("name"):
                types[f"{ns.get('name')}.{el.get('name')}"] = (el.tag.split("}")[1], el)
    return types


def qual(n, ns):
    return n if not n or n in PRIM or "." in n else f"{ns}.{n}"


def slot_need(node, ns, types, direction):
    t = node.find(C + "type")
    a = node.find(C + "array")
    if a is not None:
        inner = a.find(C + "type")
        if inner is not None and inner.get("name") in ("utf8", "filename") and a.get("zero-terminated", "1") == "1" and a.get("length") is None:
            return "now"
        return "+arrays/lists"
    if t is None:
        return "never"  # varargs
    n = qual(t.get("name"), ns)
    if n in ("gpointer", "gconstpointer", "va_list"):
        return "never"
    if n in ("GType", "gunichar"):
        return "+records-by-value"
    if n in ("GLib.List", "GLib.SList", "GLib.HashTable", "GLib.PtrArray", "GLib.Array", "GLib.ByteArray"):
        return "+arrays/lists"
    if n in PRIM:
        return "now"
    tag, el = types.get(n, ("?", None))
    if tag in ("enumeration", "bitfield", "class", "interface"):
        return "now"
    if tag == "callback":
        return "+callbacks/async"
    if tag in ("record", "boxed"):
        if el is not None and el.get(GL + "get-type"):
            return "now"
        return "+records-by-value"
    if tag == "union":
        return "+records-by-value"
    if tag == "alias":
        return "now"
    return "never"


def classify(m, kind, ns, types):
    need = 0
    if kind in ("constructor", "function"):
        need = max(need, TIERS.index("+static/ctor"))
    rv = m.find(C + "return-value")
    if rv is not None:
        need = max(need, TIERS.index(slot_need(rv, ns, types, "out")))
    ps = m.find(C + "parameters")
    skip_idx = set()
    if ps is not None:
        params = ps.findall(C + "parameter")
        for p in params:
            for attr in ("closure", "destroy"):
                if p.get(attr) is not None:
                    skip_idx.add(int(p.get(attr)))
        for i, p in enumerate(params):
            if i in skip_idx:
                continue
            if p.get("direction") in ("out", "inout"):
                need = max(need, TIERS.index("+out/inout"))
            need = max(need, TIERS.index(slot_need(p, ns, types, p.get("direction", "in"))))
    if m.get("throws") == "1":
        pass  # GError is supported (`throws` -> NativeError)
    return TIERS[need]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    a = ap.parse_args()
    types = load(a.girdir)
    per = collections.defaultdict(collections.Counter)
    for f in NSS:
        root = ET.parse(f"{a.girdir}/{f}.gir").getroot()
        ns = root.find(C + "namespace")
        nsn = ns.get("name")
        for el in ns.iter():
            tag = el.tag.split("}")[-1]
            if tag not in ("method", "constructor", "function"):
                continue
            if el.get("introspectable") == "0" or el.get("deprecated") == "1" or el.get("moved-to"):
                continue
            per[nsn][classify(el, tag, nsn, types)] += 1
    print(f"{'namespace':10} {'total':>6}  " + "  ".join(f"{t:>17}" for t in TIERS))
    for nsn, c in per.items():
        tot = sum(c.values())
        cum, cells = 0, []
        for t in TIERS:
            cum += c[t]
            cells.append(f"{c[t]:5} ({100 * cum // tot:3}%)")
        print(f"{nsn:10} {tot:6}  " + "  ".join(f"{x:>17}" for x in cells))


if __name__ == "__main__":
    main()
