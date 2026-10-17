#!/usr/bin/env python3
"""Probe: what libadwaita's GIR gives a svelters pack (custom-widgets research).

Usage: python3 -I docs/notes/scripts/cw_adw_gir.py [/usr/share/gir-1.0/Adw-1.gir]
Prints class counts, widget subclasses, `since` versions, non-widget GObjects that
would be element kinds (Breakpoint, Toast, NavigationPage...), and the tag each gets
under the `ad-` rule.
"""
import re, sys, collections
import xml.etree.ElementTree as ET

NS = {"c": "http://www.gtk.org/introspection/core/1.0",
      "glib": "http://www.gtk.org/introspection/glib/1.0"}
path = sys.argv[1] if len(sys.argv) > 1 else "/usr/share/gir-1.0/Adw-1.gir"
ns = ET.parse(path).getroot().find("c:namespace", NS)
classes = {c.get("name"): c for c in ns.findall("c:class", NS)}
n_adw = len(classes)
# Parents outside Adw (Gtk.ListBoxRow, Gtk.Window...) are followed through Gtk-4.0.gir.
gtk = ET.parse(path.replace("Adw-1.gir", "Gtk-4.0.gir")).getroot().find("c:namespace", NS)
for c in gtk.findall("c:class", NS):
    classes.setdefault("Gtk." + c.get("name"), c)

def chain(name):
    out = []
    while name:
        out.append(name)
        c = classes.get(name)
        if c is None:
            break
        p = c.get("parent") or ""
        if name.startswith("Gtk.") and p and "." not in p:
            p = "Gtk." + p
        name = p or None
    return out

def is_widget(name):
    return "Gtk.Widget" in chain(name)

def kebab(n):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "-", n).lower()

widgets, other = [], []
for n, c in sorted(classes.items()):
    if "." in n:
        continue
    rec = (n, c.get("version") or "1.0", c.get("abstract") == "1", c.get("deprecated") == "1",
           len(c.findall("c:property", NS)), len(c.findall("glib:signal", NS)), c.get("parent"))
    (widgets if is_widget(n) else other).append(rec)

def show(title, rows):
    print(f"\n## {title} ({len(rows)})")
    for n, v, a, d, np, ns_, parent in rows:
        flags = ("abstract " if a else "") + ("DEPRECATED " if d else "")
        print(f"  ad-{kebab(n):32s} Adw{n:28s} since {v:5s} props {np:2d} sig {ns_:2d} {flags}<- {parent}")

print(f"GIR {path}: namespace {ns.get('name')} {ns.get('version')}, {n_adw} classes, "
      f"{len(ns.findall('c:interface', NS))} interfaces, {len(ns.findall('c:enumeration', NS))} enums, "
      f"{len(ns.findall('c:bitfield', NS))} flags")
show("Gtk.Widget subclasses", widgets)
show("other GObject classes", other)
by_ver = collections.Counter(v for _, v, *_ in widgets + other)
print("\nclasses by `since`:", dict(sorted(by_ver.items(), key=lambda kv: [int(x) for x in kv[0].split('.')])))
fns = [f.get("name") for f in ns.findall("c:function", NS)]
print("namespace functions:", ", ".join(fns))
