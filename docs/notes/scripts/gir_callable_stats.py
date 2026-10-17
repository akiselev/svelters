#!/usr/bin/env python3
"""Classify GIR callables by how hard they are for a generic GI/libffi invoker.

Counts constructors/methods/functions of Gtk.Widget subclasses (default) or all classes
(--all) by feature: plain, out/inout params, caller-allocates, arrays (+length param),
callbacks (scope call/async/notified/forever), GError throws, varargs, non-introspectable,
boxed/struct by-value, shadows, async/finish pairs.

Usage: python3 -I gir_callable_stats.py [Gtk-4.0] [--all]
"""
import collections
import sys
import xml.etree.ElementTree as ET

NS = {"core": "http://www.gtk.org/introspection/core/1.0",
      "glib": "http://www.gtk.org/introspection/glib/1.0"}


def main():
    ns_ver = next((a for a in sys.argv[1:] if not a.startswith("--")), "Gtk-4.0")
    root = ET.parse(f"/usr/share/gir-1.0/{ns_ver}.gir").getroot()
    ns = root.find("core:namespace", NS)
    classes = {c.get("name"): c for c in ns.findall("core:class", NS)}
    callbacks = {c.get("name") for c in ns.findall("core:callback", NS)}

    def is_widget(n):
        while n:
            if n == "Widget":
                return True
            c = classes.get(n)
            n = c.get("parent") if c is not None else None
        return False

    sel = [c for n, c in classes.items() if "--all" in sys.argv or is_widget(n)]
    feat = collections.Counter()
    total = 0
    plain = 0
    for c in sel:
        for tag in ("constructor", "method", "function"):
            for m in c.findall(f"core:{tag}", NS):
                total += 1
                fs = set()
                if m.get("introspectable") == "0":
                    fs.add("non-introspectable")
                if m.get("throws") == "1":
                    fs.add("throws GError")
                if m.get("shadowed-by"):
                    fs.add("shadowed (alt binding name)")
                if m.get("{%s}async-func" % NS["glib"]) or m.get("{%s}finish-func" % NS["glib"]):
                    fs.add("async/finish pair")
                params = m.findall("core:parameters/core:parameter", NS)
                rv = m.find("core:return-value", NS)
                for p in params + ([rv] if rv is not None else []):
                    d = p.get("direction")
                    if d in ("out", "inout"):
                        fs.add(f"{d} param")
                    if p.get("caller-allocates") == "1":
                        fs.add("caller-allocates out")
                    if p.find("core:array", NS) is not None:
                        a = p.find("core:array", NS)
                        fs.add("array w/ length param" if a.get("length") else "array (zero-term/fixed)")
                    if p.find("core:varargs", NS) is not None:
                        fs.add("varargs")
                    if p.get("scope"):
                        fs.add(f"callback scope={p.get('scope')}")
                    t = p.find("core:type", NS)
                    if t is not None and t.get("name") in callbacks and not p.get("scope"):
                        fs.add("callback (no scope)")
                    if p.get("transfer-ownership") in ("full", "container") and p is not rv:
                        fs.add("transfer-full in-arg")
                if not fs:
                    plain += 1
                feat.update(fs)
    print(f"{ns_ver}: {len(sel)} classes, {total} callables, {plain} with no special feature")
    for k, v in feat.most_common():
        print(f"  {v:5}  {k}")


if __name__ == "__main__":
    main()
