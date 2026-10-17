#!/usr/bin/env python3
"""Heuristic child-slot candidates from GIR (input for hand-annotated child policy).

For each Gtk.Widget subclass, lists
  * writable properties whose type is Gtk.Widget (single named slots: child, start-child,
    titlebar, title-widget, ...), and
  * methods whose name looks like a child operation (append/prepend/insert*/remove/
    pack_*/add_*/set_*_widget/reorder*) and take a Gtk.Widget parameter
  * the "GtkFoo as GtkBuildable" doc paragraph (prose; the only upstream source for
    <child type="..."> names).

Usage: python3 -I gir_child_slots.py [Gtk-4.0] [--json]
"""
import json
import re
import sys
import xml.etree.ElementTree as ET

NS = {"core": "http://www.gtk.org/introspection/core/1.0"}
CHILD_RE = re.compile(r"^(append|prepend|insert.*|remove|pack_.*|add_.*|set_.*_widget|set_child|reorder.*|put|attach.*|move)$")
WIDGET = {"Widget", "Gtk.Widget"}


def main():
    ns_ver = next((a for a in sys.argv[1:] if not a.startswith("--")), "Gtk-4.0")
    root = ET.parse(f"/usr/share/gir-1.0/{ns_ver}.gir").getroot()
    ns = root.find("core:namespace", NS)
    classes = {c.get("name"): c for c in ns.findall("core:class", NS)}

    def is_widget(name):
        while name:
            if name in WIDGET:
                return True
            c = classes.get(name)
            name = c.get("parent") if c is not None else None
            if name in WIDGET:
                return True
        return False

    out = {}
    for name, c in sorted(classes.items()):
        if not is_widget(name):
            continue
        slots = []
        for p in c.findall("core:property", NS):
            t = p.find("core:type", NS)
            if t is not None and t.get("name") in WIDGET and p.get("writable") == "1":
                slots.append(p.get("name"))
        ops = []
        for m in c.findall("core:method", NS):
            if not CHILD_RE.match(m.get("name")):
                continue
            ps = m.findall("core:parameters/core:parameter/core:type", NS)
            if any(t.get("name") in WIDGET for t in ps):
                ops.append(m.get("name"))
        doc = c.find("core:doc", NS)
        buildable = None
        if doc is not None and doc.text:
            m = re.search(r"#+ \S+ as (?:a )?(?:\[iface@Gtk\.Buildable\]|GtkBuildable)\n+(.*?)(?:\n#|\Z)", doc.text, re.S)
            if m:
                buildable = " ".join(m.group(1).split())[:400]
        if slots or ops or buildable:
            out[name] = {"widget_props": slots, "child_ops": ops, "buildable_doc": buildable}
    if "--json" in sys.argv:
        print(json.dumps(out, indent=1))
    else:
        for k, v in out.items():
            print(f"{k:28} props={v['widget_props']} ops={v['child_ops']}" + ("  [buildable doc]" if v["buildable_doc"] else ""))
        print(f"\n{len(out)} widget classes with child-related surface", file=sys.stderr)


if __name__ == "__main__":
    main()
