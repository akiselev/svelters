#!/usr/bin/env python3
"""Type-category inventory of GTK4 API surface for the JS<->native serdes note.

Parses Gtk-4.0.gir plus the GIRs it references (stdlib only) and classifies every
property, method/constructor/function parameter, return value and signal argument into
marshalling categories (docs/notes/js-native-serdes.md section 1).

Usage: python3 -I serdes_gir_inventory.py [--girdir /usr/share/gir-1.0] [--json out.json]
"""
import argparse
import collections
import json
import xml.etree.ElementTree as ET

NS = {"c": "http://www.gtk.org/introspection/core/1.0",
      "glib": "http://www.gtk.org/introspection/glib/1.0",
      "cc": "http://www.gtk.org/introspection/c/1.0"}
CORE = "{%s}" % NS["c"]
GLIBNS = "{%s}" % NS["glib"]
CNS = "{%s}" % NS["cc"]
FILES = ["Gtk-4.0", "Gdk-4.0", "Gsk-4.0", "Pango-1.0", "Gio-2.0", "GLib-2.0", "GObject-2.0",
         "Graphene-1.0", "cairo-1.0", "GdkPixbuf-2.0", "HarfBuzz-0.0", "PangoCairo-1.0"]

# Small fixed-layout value structs that should cross as plain JS objects.
VALUE_STRUCTS = {"Gdk.RGBA", "Gdk.Rectangle", "Gtk.Border", "Gtk.Requisition", "Graphene.Rect",
                 "Graphene.Point", "Graphene.Size", "Graphene.Vec2", "Graphene.Vec3", "Graphene.Vec4",
                 "Graphene.Point3D", "Pango.Rectangle", "Pango.Color", "Graphene.Matrix",
                 "Graphene.Quaternion", "Gsk.RoundedRect", "Graphene.Box", "Pango.Matrix",
                 "Gdk.KeymapKey", "Gtk.PageRange", "Gdk.TimeCoord"}
VARIANTS = {"GLib.Variant", "GObject.Value", "GLib.VariantType"}
BYTES = {"GLib.Bytes", "GLib.ByteArray"}
DATES = {"GLib.DateTime", "GLib.Date", "GLib.TimeZone", "GLib.TimeVal"}
ITERS = {"Gtk.TextIter", "Gtk.TreeIter", "Gtk.TreePath", "Gtk.BitsetIter", "GLib.HashTableIter",
         "Gtk.CssLocation", "Gtk.TreeRowReference"}
STRINGS = {"utf8", "filename"}
INTS = {"gint", "guint", "gint8", "guint8", "gint16", "guint16", "gint32", "guint32", "glong",
        "gulong", "gsize", "gssize", "gchar", "guchar", "gunichar", "gshort", "gushort", "goffset",
        "gintptr", "guintptr", "int", "unsigned int", "char"}
INT64 = {"gint64", "guint64"}
FLOATS = {"gdouble", "gfloat", "double", "float"}


def load(girdir):
    kinds = {}  # "Ns.Name" -> (kind, element)
    for f in FILES:
        root = ET.parse(f"{girdir}/{f}.gir").getroot()
        ns = root.find(CORE + "namespace")
        nsname = ns.get("name")
        for el in ns:
            tag = el.tag.split("}")[1]
            if tag in ("class", "interface", "record", "enumeration", "bitfield", "callback",
                       "union", "alias", "boxed"):
                kinds[f"{nsname}.{el.get('name')}"] = (tag, el, nsname)
    return kinds


def qual(name, ns):
    if name is None:
        return None
    return name if "." in name or name in STRINGS or name.startswith("g") and name in (
        INTS | INT64 | FLOATS | {"gboolean", "gpointer", "gconstpointer", "none", "GType"}) else f"{ns}.{name}"


def resolve_alias(q, kinds, depth=0):
    k = kinds.get(q)
    if k and k[0] == "alias" and depth < 5:
        t = k[1].find(CORE + "type")
        if t is not None:
            return resolve_alias(qual(t.get("name"), k[2]), kinds, depth + 1)
    return q


def classify_type(t, ns, kinds):
    """t is a <type> or <array> element. Returns (category, detail)."""
    if t is None:
        return ("varargs_or_unknown", None)
    tag = t.tag.split("}")[1]
    if tag == "varargs":
        return ("varargs_or_unknown", "varargs")
    if tag == "array":
        name = t.get("name")
        inner = t.find(CORE + "type")
        if inner is None:
            inner = t.find(CORE + "array")
        ic, idet = classify_type(inner, ns, kinds) if inner is not None else ("?", None)
        if name in ("GLib.PtrArray", "GLib.Array"):
            return ("list", f"{name}<{ic}>")
        if name == "GLib.ByteArray":
            return ("bytes", name)
        if ic in ("i32", "u8") and inner is not None and inner.get("name") in ("guint8", "gchar", "guchar"):
            return ("bytes", "uint8[]")
        if ic == "string":
            return ("strv", "string[]")
        how = "length" if t.get("length") else ("zt" if t.get("zero-terminated") == "1" else
                                                 ("fixed" if t.get("fixed-size") else "zt"))
        return ("list", f"c_array[{how}]<{ic}>")
    name = t.get("name")
    if name is None:
        return ("varargs_or_unknown", t.get(CNS + "type"))
    if name == "none":
        return ("void", None)
    if name == "gboolean":
        return ("bool", None)
    if name in INTS:
        return ("i32", name)
    if name in INT64:
        return ("i64", name)
    if name in FLOATS:
        return ("f64", name)
    if name in STRINGS:
        return ("string", name)
    if name in ("gpointer", "gconstpointer"):
        return ("raw_pointer", name)
    if name == "GType":
        return ("gtype", None)
    q = resolve_alias(qual(name, ns), kinds)
    if q in ("GLib.List", "GLib.SList"):
        inner = t.find(CORE + "type")
        ic = classify_type(inner, ns, kinds)[0] if inner is not None else "?"
        return ("list", f"{q}<{ic}>")
    if q == "GLib.HashTable":
        return ("map", q)
    if q == "GLib.Error":
        return ("error", q)
    if q in VARIANTS:
        return ("variant", q)
    if q in BYTES:
        return ("bytes", q)
    if q in DATES:
        return ("date", q)
    if q == "Gio.ListModel":
        return ("list_model", q)
    if q == "Gio.AsyncReadyCallback":
        return ("async_callback", q)
    if q == "Gio.Cancellable":
        return ("cancellable", q)
    if q in ("Gdk.Texture", "Gdk.Paintable", "GdkPixbuf.Pixbuf", "Gio.Icon", "Gdk.MemoryTexture"):
        return ("image_object", q)
    k = kinds.get(q)
    if k is None:
        if name in ("glong", "gulong"):
            return ("i32", name)
        return ("varargs_or_unknown", q)
    kind, el, _ = k
    if kind == "enumeration":
        return ("enum", q)
    if kind == "bitfield":
        return ("flags", q)
    if kind in ("class", "interface"):
        return ("object", q)
    if kind == "callback":
        return ("callback", q)
    if kind in ("record", "union", "boxed"):
        if q in VALUE_STRUCTS:
            return ("value_struct", q)
        if q in ITERS:
            return ("iter_struct", q)
        if el.get(GLIBNS + "is-gtype-struct-for"):
            return ("class_struct", q)
        if el.get(GLIBNS + "get-type"):
            return ("boxed_ref", q)
        return ("plain_record", q)
    return ("varargs_or_unknown", q)


def param_mods(p):
    mods = []
    d = p.get("direction", "in")
    if d != "in":
        mods.append(d)
    if p.get("caller-allocates") == "1":
        mods.append("caller_allocates")
    if p.get("nullable") == "1" or p.get("allow-none") == "1":
        mods.append("nullable")
    if p.get("transfer-ownership") in ("full", "container"):
        mods.append("transfer_" + p.get("transfer-ownership"))
    if p.get("scope"):
        mods.append("scope_" + p.get("scope"))
    return mods


def ptype(p):
    t = p.find(CORE + "type")
    if t is None:
        t = p.find(CORE + "array")
    if t is None:
        t = p.find(CORE + "varargs")
    return t


def walk(kinds, ns_filter="Gtk", classes=None):
    """Yield (site, owner, member, category, detail, mods) for every typed slot."""
    for q, (kind, el, ns) in sorted(kinds.items()):
        if ns != ns_filter or kind not in ("class", "interface"):
            continue
        owner = q.split(".", 1)[1]
        if classes is not None and owner not in classes:
            continue
        for pr in el.findall(CORE + "property"):
            c, det = classify_type(ptype(pr), ns, kinds)
            yield ("property", owner, pr.get("name"), c, det, [])
        for sig in el.findall(GLIBNS + "signal"):
            ps = sig.find(CORE + "parameters")
            for p in (ps.findall(CORE + "parameter") if ps is not None else []):
                c, det = classify_type(ptype(p), ns, kinds)
                yield ("signal_arg", owner, sig.get("name"), c, det, param_mods(p))
            rv = sig.find(CORE + "return-value")
            c, det = classify_type(ptype(rv), ns, kinds) if rv is not None else ("void", None)
            yield ("signal_return", owner, sig.get("name"), c, det, [])
        for tag in ("constructor", "method", "function"):
            for m in el.findall(CORE + tag):
                if m.get("introspectable") == "0" or m.get("deprecated") == "1":
                    continue
                ps = m.find(CORE + "parameters")
                plist = ps.findall(CORE + "parameter") if ps is not None else []
                # length/closure/destroy params are consumed by their owner param
                hidden = set()
                for p in plist:
                    for attr in ("closure", "destroy"):
                        if p.get(attr):
                            hidden.add(int(p.get(attr)))
                    a = p.find(CORE + "array")
                    if a is not None and a.get("length"):
                        hidden.add(int(a.get("length")))
                rv = m.find(CORE + "return-value")
                ra = rv.find(CORE + "array") if rv is not None else None
                if ra is not None and ra.get("length"):
                    hidden.add(int(ra.get("length")))
                for i, p in enumerate(plist):
                    if i in hidden:
                        continue
                    c, det = classify_type(ptype(p), ns, kinds)
                    yield ("param", owner, m.get("name"), c, det, param_mods(p))
                if m.get("throws") == "1":
                    yield ("param", owner, m.get("name"), "error", "throws", ["out"])
                if rv is not None:
                    c, det = classify_type(ptype(rv), ns, kinds)
                    if tag == "constructor":
                        c, det = "void", None
                    yield ("return", owner, m.get("name"), c, det,
                               ["nullable"] if rv.get("nullable") == "1" else [])


SIMPLE = {"void", "bool", "i32", "i64", "f64", "string", "enum", "flags", "object"}


def summarize(rows, label):
    out = {"label": label}
    by_site = collections.defaultdict(collections.Counter)
    for site, _, _, c, _, mods in rows:
        site2 = "method" if site in ("param", "return") else ("signal" if site.startswith("signal") else site)
        cat = c
        if c == "void":
            continue
        if "out" in mods or "inout" in mods:
            cat = c + "(out)" if c in SIMPLE else c
        by_site[site2][cat] += 1
    out["by_site"] = {k: dict(v.most_common()) for k, v in by_site.items()}
    # member-level: does the member need only simple categories?
    members = collections.defaultdict(set)
    for site, owner, mem, c, _, mods in rows:
        site2 = "method" if site in ("param", "return") else ("signal" if site.startswith("signal") else site)
        tag = c + ("(out)" if ("out" in mods or "inout" in mods) else "")
        members[(site2, owner, mem)].add(tag)
    simple_m = collections.Counter()
    tot_m = collections.Counter()
    blockers = collections.Counter()
    for (site2, _, _), cats in members.items():
        tot_m[site2] += 1
        if cats <= SIMPLE:
            simple_m[site2] += 1
        else:
            for x in cats - SIMPLE:
                blockers[x] += 1
    out["members_total"] = dict(tot_m)
    out["members_simple"] = dict(simple_m)
    out["blockers"] = dict(blockers.most_common(25))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--json")
    a = ap.parse_args()
    kinds = load(a.girdir)
    p_set = {"Window", "ApplicationWindow", "Box", "Button", "Entry", "HeaderBar", "Label",
             "ScrolledWindow", "Widget", "Editable", "Orientable"}
    common = p_set | {"Grid", "ToggleButton", "CheckButton", "PasswordEntry", "SearchEntry",
                      "SpinButton", "Scale", "Range", "Switch", "ProgressBar", "LevelBar", "Image",
                      "Picture", "Stack", "StackSwitcher", "Notebook", "Paned", "MenuButton",
                      "DropDown", "ListView", "ListBox", "ListBoxRow", "TextView", "TextBuffer",
                      "Expander", "Frame", "Revealer", "Popover", "Separator", "Spinner",
                      "CenterBox", "Overlay", "Calendar", "Adjustment", "LinkButton",
                      "GestureClick", "EventControllerKey", "Dialog", "AlertDialog",
                      "FileDialog", "ColorDialogButton", "FontDialogButton", "StringList",
                      "SingleSelection", "SignalListItemFactory", "ListItem", "ColumnView",
                      "ColumnViewColumn", "Actionable", "Accessible"}
    res = [summarize(list(walk(kinds, classes=p_set)), "P kinds (+Widget)"),
           summarize(list(walk(kinds, classes=common)), "common (~60 kinds)"),
           summarize(list(walk(kinds)), "all Gtk classes+interfaces")]
    details = collections.Counter()
    for site, _, _, c, det, _ in walk(kinds):
        if c in ("value_struct", "boxed_ref", "variant", "bytes", "date", "callback", "list",
                 "image_object", "iter_struct", "plain_record", "strv", "map", "list_model"):
            details[f"{c}: {det}"] += 1
    res.append({"label": "detail (all Gtk)", "top": dict(details.most_common(60))})
    scopes = collections.Counter()
    asyncs = 0
    for site, _, mem, c, det, mods in walk(kinds):
        if c in ("callback", "async_callback"):
            scopes[next((m for m in mods if m.startswith("scope_")), "scope_none") + f" ({c})"] += 1
        if c == "async_callback":
            asyncs += 1
    res.append({"label": "callback scopes (all Gtk)", "scopes": dict(scopes)})
    print(json.dumps(res, indent=1))
    if a.json:
        json.dump(res, open(a.json, "w"), indent=1)


if __name__ == "__main__":
    main()
