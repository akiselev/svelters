#!/usr/bin/env python3
"""Non-GUI types referenced by the Gtk-4.0 API, ranked, with today's svelters support status.

For every public, non-deprecated, introspectable property, method/constructor/function parameter,
return value and signal argument of the Gtk namespace (classes, interfaces, records), collect the
referenced type when it is not a primitive, string, enum/flags or GtkWidget subclass. Classify each
type by GIR kind and by the status the GTK backend gives it today (crates/svelters-gtk/src/convert.rs
and gi.rs, 2026-10-17). Also sizes the non-GUI namespaces themselves (what a full binding exposes).

Usage: python3 -I types_gir_inventory.py [--girdir /usr/share/gir-1.0] [--top 60] [--json out.json]
Notes: docs/notes/types-research.md section 1.
"""
import argparse
import collections
import json
import xml.etree.ElementTree as ET

C = "{http://www.gtk.org/introspection/core/1.0}"
GL = "{http://www.gtk.org/introspection/glib/1.0}"
FILES = ["Gtk-4.0", "Gdk-4.0", "Gsk-4.0", "Pango-1.0", "Gio-2.0", "GLib-2.0", "GObject-2.0",
         "Graphene-1.0", "cairo-1.0", "GdkPixbuf-2.0", "HarfBuzz-0.0", "PangoCairo-1.0"]
PRIMS = {"gboolean", "gint", "guint", "gint8", "guint8", "gint16", "guint16", "gint32", "guint32",
         "gint64", "guint64", "glong", "gulong", "gsize", "gssize", "gchar", "guchar", "gshort",
         "gushort", "goffset", "gintptr", "guintptr", "gdouble", "gfloat", "utf8", "filename", "none",
         "gpointer", "gconstpointer", "gunichar", "GType", "va_list"}
# Status rules, mirroring the GTK backend on 2026-10-17.
RECORDS = {"Graphene.Rect", "Gdk.Rectangle", "Graphene.Size", "Graphene.Point", "Gtk.Border", "Gdk.RGBA"}
STRING_FORM = {"Gtk.ShortcutTrigger", "Gio.File", "Pango.FontDescription", "Gio.Icon", "Gdk.Cursor",
               "Gdk.Paintable", "Gdk.Texture"}
SPECIAL = {"GLib.Variant": "variant", "GObject.Value": "variant", "GLib.DateTime": "datetime"}


def load(girdir):
    types = {}
    for f in FILES:
        try:
            root = ET.parse(f"{girdir}/{f}.gir").getroot()
        except FileNotFoundError:
            continue
        ns = root.find(C + "namespace")
        nsn = ns.get("name")
        for el in ns:
            tag = el.tag.split("}")[1]
            if el.get("name"):
                types[f"{nsn}.{el.get('name')}"] = (tag, el, nsn)
    return types


def qual(name, ns):
    if not name or name in PRIMS or "." in name:
        return name
    return f"{ns}.{name}"


def parent_chain(types, full):
    seen = []
    while full and full in types and full not in seen:
        seen.append(full)
        tag, el, ns = types[full]
        full = qual(el.get("parent"), ns) if tag == "class" else None
    return seen


def is_widget(types, full):
    return "Gtk.Widget" in parent_chain(types, full)


def slot_type(node, ns):
    """(type name, container) for a property/parameter/return node."""
    t = node.find(C + "type")
    if t is not None:
        name = t.get("name")
        if name in ("GLib.List", "GLib.SList", "GLib.HashTable", "GLib.PtrArray", "GLib.Array"):
            inner = t.find(C + "type")
            return (qual(inner.get("name"), ns) if inner is not None else None), name
        return qual(name, ns), None
    a = node.find(C + "array")
    if a is not None:
        inner = a.find(C + "type")
        return (qual(inner.get("name"), ns) if inner is not None else None), "array"
    return None, None


def status(types, full, container):
    tag = types.get(full, ("?",))[0]
    if container == "array" and full in ("utf8", "filename"):
        return "supported:strv"
    if full in ("utf8", "filename"):
        return None
    if container and container != "array":
        return "rejected:list"
    if container == "array":
        return "rejected:array"
    if full in RECORDS:
        return "supported:record"
    if full in STRING_FORM:
        return "supported:string-form+opaque"
    if full in SPECIAL:
        return "supported:" + SPECIAL[full]
    if tag in ("class", "interface"):
        if full.startswith("Gtk."):
            return "handle:gtk-kind"
        return "opaque:boxed-object"
    if tag in ("record", "boxed"):
        el = types[full][1]
        if el.get(GL + "get-type"):
            return "opaque:boxed"
        return "rejected:struct-no-gtype"
    if tag == "union":
        return "rejected:union"
    if tag == "callback":
        return "rejected:callback(gi path)"
    return "unknown:" + tag


def skip(el):
    return el.get("introspectable") == "0" or el.get("deprecated") == "1"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--top", type=int, default=60)
    ap.add_argument("--json")
    a = ap.parse_args()
    types = load(a.girdir)

    count = collections.Counter()
    where = collections.defaultdict(collections.Counter)
    st_of = {}
    for full, (tag, el, ns) in types.items():
        if ns != "Gtk" or tag not in ("class", "interface", "record"):
            continue
        if skip(el):
            continue
        owner_widget = tag == "class" and is_widget(types, full)
        scope = "widget" if owner_widget else "non-widget"

        def visit(node, role):
            t, cont = slot_type(node, ns)
            if t is None or (t in PRIMS and cont is None):
                return
            if t in PRIMS and not (cont == "array" and t in ("utf8", "filename")):
                if cont:
                    t = f"{cont}<{t}>"
                    count[t] += 1
                    where[t][f"{scope}:{role}"] += 1
                    st_of[t] = "rejected:" + ("array" if cont == "array" else "list")
                return
            ttag = types.get(t, ("?",))[0]
            if ttag in ("enumeration", "bitfield") and cont is None:
                return
            if ttag == "class" and is_widget(types, t) and cont is None:
                return
            key = t if cont is None else f"{cont}<{t}>"
            count[key] += 1
            where[key][f"{scope}:{role}"] += 1
            st_of[key] = status(types, t, cont) or "supported:strv"

        for p in el.findall(C + "property"):
            if not skip(p):
                visit(p, "prop")
        for m in list(el.findall(C + "method")) + list(el.findall(C + "constructor")) + list(el.findall(C + "function")):
            if skip(m):
                continue
            rv = m.find(C + "return-value")
            if rv is not None:
                visit(rv, "ret")
            ps = m.find(C + "parameters")
            if ps is not None:
                for p in ps.findall(C + "parameter"):
                    if p.get("closure") is not None and p.find(C + "type") is not None and p.find(C + "type").get("name") == "gpointer":
                        continue
                    visit(p, "param")
        for s in el.findall(GL + "signal"):
            if skip(s):
                continue
            ps = s.find(C + "parameters")
            if ps is not None:
                for p in ps.findall(C + "parameter"):
                    visit(p, "signal")

    by_status = collections.Counter()
    types_by_status = collections.Counter()
    by_ns = collections.Counter()
    for k, n in count.items():
        by_status[st_of[k]] += n
        types_by_status[st_of[k]] += 1
        base = k.split("<")[-1].rstrip(">")
        by_ns[base.split(".")[0] if "." in base else base] += n
    total = sum(count.values())
    print(f"distinct non-GUI types: {len(count)}, slot references: {total}")
    print("\nby status (slots / distinct types):")
    for s, n in by_status.most_common():
        print(f"  {s:34} {n:5}  {types_by_status[s]:4}")
    print("\nby namespace (slots):")
    for s, n in by_ns.most_common():
        print(f"  {s:12} {n:5}")
    print(f"\ntop {a.top}:")
    for k, n in count.most_common(a.top):
        w = where[k]
        wid = sum(v for kk, v in w.items() if kk.startswith("widget"))
        print(f"  {n:4} (widget {wid:3})  {k:34} {st_of[k]}")

    # Size of the non-GUI namespaces themselves.
    print("\nnamespace sizes (non-deprecated, introspectable):")
    for nsn in ("GLib", "GObject", "Gio", "Gdk", "Gsk", "Pango", "Graphene"):
        c = collections.Counter()
        for full, (tag, el, ns) in types.items():
            if ns != nsn or skip(el):
                continue
            c[tag] += 1
            for m in el.findall(C + "method") + el.findall(C + "constructor") + el.findall(C + "function"):
                if not skip(m):
                    c["callables"] += 1
            c["properties"] += len(el.findall(C + "property"))
            c["signals"] += len(el.findall(GL + "signal"))
        print(f"  {nsn:9} " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    if a.json:
        json.dump({"count": count, "status": st_of, "where": where}, open(a.json, "w"), indent=1, default=dict)


if __name__ == "__main__":
    main()
