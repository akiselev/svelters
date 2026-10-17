#!/usr/bin/env python3
"""Proof-of-concept: dump one GObject class from GIR XML into a neutral JSON shape.

Usage:
    python3 gir_dump.py Gtk-4.0 Button [--girdir /usr/share/gir-1.0] [--methods]

Stdlib only (xml.etree). Resolves <include> recursively so parent classes and
interfaces from other namespaces (Gtk.Widget -> GObject.InitiallyUnowned) work.
"""
import argparse
import json
import os
import sys
import xml.etree.ElementTree as ET

NS = {
    "core": "http://www.gtk.org/introspection/core/1.0",
    "c": "http://www.gtk.org/introspection/c/1.0",
    "glib": "http://www.gtk.org/introspection/glib/1.0",
}
C = "{%s}" % NS["core"]
CN = "{%s}" % NS["c"]
GL = "{%s}" % NS["glib"]


class Repo:
    def __init__(self, girdir):
        self.girdir = girdir
        self.namespaces = {}  # ns name -> <namespace> element

    def load(self, ns_ver):
        name = ns_ver.split("-")[0]
        if name in self.namespaces:
            return
        root = ET.parse(os.path.join(self.girdir, ns_ver + ".gir")).getroot()
        ns = root.find("core:namespace", NS)
        self.namespaces[ns.get("name")] = ns
        for inc in root.findall("core:include", NS):
            self.load(f"{inc.get('name')}-{inc.get('version')}")

    def lookup(self, qname, cur_ns):
        ns, _, name = qname.rpartition(".")
        ns = ns or cur_ns
        el = self.namespaces[ns]
        for tag in ("class", "interface", "record", "enumeration", "bitfield", "callback", "union", "alias"):
            hit = el.find(f"core:{tag}[@name='{name}']", NS)
            if hit is not None:
                return ns, hit
        return ns, None


def type_of(el, cur_ns):
    """Return a compact type descriptor for a property/param/return element."""
    t = el.find("core:type", NS)
    if t is not None:
        name = t.get("name")
        d = {"gir": name if ("." in name or name in PRIMS) else f"{cur_ns}.{name}", "c": t.get(CN + "type")}
        inner = t.findall("core:type", NS)
        if inner:  # GList/GSList/GHashTable element types
            d["elements"] = [type_of_el(i, cur_ns) for i in inner]
        return d
    a = el.find("core:array", NS)
    if a is not None:
        d = {"array": type_of(a, cur_ns), "c": a.get(CN + "type")}
        for k in ("length", "zero-terminated", "fixed-size", "name"):
            if a.get(k) is not None:
                d[k] = a.get(k)
        return d
    if el.find("core:varargs", NS) is not None:
        return {"varargs": True}
    return None


def type_of_el(t, cur_ns):
    name = t.get("name")
    return {"gir": name if ("." in name or name in PRIMS) else f"{cur_ns}.{name}"}


PRIMS = {
    "none", "gboolean", "gchar", "guchar", "gint", "guint", "gint8", "guint8", "gint16", "guint16",
    "gint32", "guint32", "gint64", "guint64", "gshort", "gushort", "glong", "gulong", "gsize", "gssize",
    "gfloat", "gdouble", "utf8", "filename", "gpointer", "gunichar", "GType", "gintptr", "guintptr",
}


def flag(el, k, default="0"):
    return el.get(k, default) == "1"


def param(p, cur_ns):
    d = {"name": p.get("name"), "type": type_of(p, cur_ns)}
    for k in ("direction", "transfer-ownership", "scope", "closure", "destroy"):
        if p.get(k) is not None:
            d[k] = p.get(k)
    for k in ("nullable", "allow-none", "optional", "caller-allocates"):
        if flag(p, k):
            d[k] = True
    return d


def callable_(m, cur_ns):
    params = m.find("core:parameters", NS)
    ret = m.find("core:return-value", NS)
    d = {
        "name": m.get("name"),
        "c_symbol": m.get(CN + "identifier"),
        "params": [param(p, cur_ns) for p in (params.findall("core:parameter", NS) if params is not None else [])],
        "return": param(ret, cur_ns) if ret is not None else None,
    }
    for k in ("throws", "introspectable", "deprecated", "version", "shadows", "shadowed-by",
              "glib:set-property", "glib:get-property", "glib:finish-func", "glib:async-func"):
        key = k if not k.startswith("glib:") else GL + k[5:]
        if m.get(key) is not None:
            d[k] = m.get(key)
    if m.find("core:parameters/core:instance-parameter", NS) is not None:
        d["instance"] = True
    return d


def dump_class(repo, ns, name, with_methods):
    ns, el = repo.lookup(name, ns)
    if el is None:
        sys.exit(f"not found: {ns}.{name}")
    out = {
        "id": f"gtk:{ns}.{name}",
        "c_type": el.get(CN + "type"),
        "gtype": el.get(GL + "type-name"),
        "kind": el.tag.replace(C, ""),
        "abstract": flag(el, "abstract"),
        "final": flag(el, "final"),
        "parent": None,
        "implements": [i.get("name") if "." in i.get("name") else f"{ns}.{i.get('name')}"
                       for i in el.findall("core:implements", NS)],
        "constructors": [callable_(c, ns) for c in el.findall("core:constructor", NS)],
        "properties": [],
        "signals": [],
        "vfuncs": [v.get("name") for v in el.findall("core:virtual-method", NS)],
        "method_count": len(el.findall("core:method", NS)),
    }
    if el.get("parent"):
        p = el.get("parent")
        out["parent"] = p if "." in p else f"{ns}.{p}"
    for p in el.findall("core:property", NS):
        out["properties"].append({
            "name": p.get("name"),
            "type": type_of(p, ns),
            "readable": p.get("readable", "1") == "1",
            "writable": flag(p, "writable"),
            "construct": flag(p, "construct"),
            "construct_only": flag(p, "construct-only"),
            "default_value": p.get("default-value"),  # textual C-ish literal; may be absent
            "getter": p.get("getter"),
            "setter": p.get("setter"),
            "transfer": p.get("transfer-ownership"),
            "deprecated": flag(p, "deprecated"),
            "since": p.get("version"),
        })
    for s in el.findall("glib:signal", NS):
        d = callable_(s, ns)
        d.update({"when": s.get("when"), "action": flag(s, "action"), "detailed": flag(s, "detailed"),
                  "no_hooks": flag(s, "no-hooks"), "no_recurse": flag(s, "no-recurse")})
        out["signals"].append(d)
    if with_methods:
        out["methods"] = [callable_(m, ns) for m in el.findall("core:method", NS)]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("namespace", help="e.g. Gtk-4.0")
    ap.add_argument("klass", help="e.g. Button")
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--methods", action="store_true")
    ap.add_argument("--chain", action="store_true", help="also dump every ancestor")
    a = ap.parse_args()
    repo = Repo(a.girdir)
    repo.load(a.namespace)
    ns = a.namespace.split("-")[0]
    result = []
    cur = f"{ns}.{a.klass}"
    while cur:
        cns, cname = cur.split(".")
        d = dump_class(repo, cns, cname, a.methods)
        result.append(d)
        cur = d["parent"] if a.chain else None
    chain = [d["id"] for d in result]
    print(json.dumps({"inheritance": chain, "classes": result}, indent=1))


if __name__ == "__main__":
    main()
