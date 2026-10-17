"""GIR parsing helpers for the GTK ingester (stdlib only; deterministic).

Derived from docs/notes/scripts/gir_dump.py. Loads Gtk-4.0.gir and its transitive includes,
indexes every named type, and maps GIR type elements to the neutral type vocabulary of
svelters-element-kind/v0 (bool, i32, ..., string, enum<Id>, object<Id>, list<T>, ...).
"""
from __future__ import annotations

import hashlib
import xml.etree.ElementTree as ET
from pathlib import Path

NS = {
    "core": "http://www.gtk.org/introspection/core/1.0",
    "c": "http://www.gtk.org/introspection/c/1.0",
    "glib": "http://www.gtk.org/introspection/glib/1.0",
}
C = "{%s}" % NS["core"]
CN = "{%s}" % NS["c"]
GL = "{%s}" % NS["glib"]

TYPE_TAGS = ("class", "interface", "record", "enumeration", "bitfield", "callback", "union", "alias")

PRIM = {
    "gboolean": "bool", "gint": "i32", "gint8": "i32", "gint16": "i32", "gint32": "i32", "gshort": "i32",
    "gchar": "i32", "guint": "u32", "guint8": "u32", "guint16": "u32", "guint32": "u32", "gushort": "u32",
    "guchar": "u32", "gunichar": "u32", "gint64": "i64", "glong": "i64", "gssize": "i64", "gintptr": "i64",
    "guint64": "u64", "gulong": "u64", "gsize": "u64", "guintptr": "u64", "GType": "u64",
    "gfloat": "f32", "gdouble": "f64", "utf8": "string", "filename": "string", "none": "void",
    "gpointer": "pointer", "gconstpointer": "pointer", "va_list": "pointer",
}
LIST_TYPES = {"GLib.List", "GLib.SList", "GLib.PtrArray", "GLib.Array"}


def type_id(ns: str, name: str) -> str:
    return f"svelters-api:v1:type:gtk:{ns}:{name}"


def sha256(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Repo:
    def __init__(self, girdir: str):
        self.girdir = Path(girdir)
        self.ns: dict[str, ET.Element] = {}
        self.files: dict[str, Path] = {}
        self.by_gtype: dict[str, tuple[str, str, str]] = {}  # gtype name -> (ns, name, tag)

    def load(self, ns_ver: str) -> None:
        name = ns_ver.rsplit("-", 1)[0]
        if name in self.ns:
            return
        path = self.girdir / f"{ns_ver}.gir"
        root = ET.parse(path).getroot()
        ns = root.find("core:namespace", NS)
        self.ns[ns.get("name")] = ns
        self.files[ns.get("name")] = path
        for tag in TYPE_TAGS:
            for el in ns.findall(f"core:{tag}", NS):
                gt = el.get(GL + "type-name")
                if gt:
                    self.by_gtype[gt] = (ns.get("name"), el.get("name"), tag)
        for inc in sorted(root.findall("core:include", NS), key=lambda i: i.get("name")):
            self.load(f"{inc.get('name')}-{inc.get('version')}")

    def find(self, ns: str, name: str):
        el_ns = self.ns.get(ns)
        if el_ns is None:
            return None, None
        for tag in TYPE_TAGS:
            hit = el_ns.find(f"core:{tag}[@name='{name}']", NS)
            if hit is not None:
                return tag, hit
        return None, None

    @staticmethod
    def qualify(name: str, cur_ns: str) -> tuple[str, str]:
        if "." in name:
            ns, _, n = name.rpartition(".")
            return ns, n
        return cur_ns, name

    def ref(self, name: str, cur_ns: str) -> str:
        """Map a GIR type name to the neutral vocabulary."""
        if name in PRIM:
            return PRIM[name]
        ns, n = self.qualify(name, cur_ns)
        if (ns, n) == ("GLib", "Variant"):
            return "variant"
        tag, el = self.find(ns, n)
        if tag in ("class", "interface"):
            return f"object<{type_id(ns, n)}>"
        if tag == "enumeration":
            return f"enum<{type_id(ns, n)}>"
        if tag == "bitfield":
            return f"flags<{type_id(ns, n)}>"
        if tag == "record":
            return f"boxed<{type_id(ns, n)}>" if el.get(GL + "get-type") else f"value<{type_id(ns, n)}>"
        if tag == "union":
            return f"value<{type_id(ns, n)}>"
        if tag == "callback":
            return f"callback<{type_id(ns, n)}>"
        if tag == "alias":
            t = el.find("core:type", NS)
            return self.ref(t.get("name"), ns) if t is not None else f"native:{name}"
        return f"native:{name}"

    def type_ref(self, el: ET.Element, cur_ns: str) -> tuple[str, str | None]:
        """Return (neutral ref, native C spelling) for an element holding a <type>/<array>."""
        t = el.find("core:type", NS)
        if t is not None:
            name = t.get("name")
            if name is None:
                return "unknown", t.get(CN + "type")
            inner = t.findall("core:type", NS)
            if name in LIST_TYPES and inner:
                return f"list<{self.ref(inner[0].get('name') or 'gpointer', cur_ns)}>", t.get(CN + "type")
            return self.ref(name, cur_ns), t.get(CN + "type")
        a = el.find("core:array", NS)
        if a is not None:
            inner, _ = self.type_ref(a, cur_ns)
            return f"list<{inner}>", a.get(CN + "type")
        if el.find("core:varargs", NS) is not None:
            return "varargs", None
        return "unknown", None
