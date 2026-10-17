#!/usr/bin/env python3
"""Non-void signals and virtual methods in GIR (input for `handler_result` / `sync_required`).

For each namespace, lists every <glib:signal> whose <return-value> is not none, grouped by
return type and emission phase (`when`), and counts class vfuncs (the GTK analogue of Qt
virtual overrides; these are only reachable by subclassing, never by connect()).

GIR does not record the accumulator (g_signal_accumulator_true_handled & co.); use
gtk_signal_accumulator_probe.py for that.

Usage: python3 -I gir_signal_returns.py [Gtk-4.0 Adw-1 Gdk-4.0 ...] [--json] [--list]
"""
import collections
import json
import sys
import xml.etree.ElementTree as ET

NS = {"core": "http://www.gtk.org/introspection/core/1.0",
      "glib": "http://www.gtk.org/introspection/glib/1.0"}


def rtype(rv):
    if rv is None:
        return "none"
    t = rv.find("core:type", NS)
    if t is None:
        return "array" if rv.find("core:array", NS) is not None else "?"
    return t.get("name")


def scan(ns_ver):
    root = ET.parse(f"/usr/share/gir-1.0/{ns_ver}.gir").getroot()
    ns = root.find("core:namespace", NS)
    out = []
    vfuncs = 0
    for kind in ("class", "interface"):
        for c in ns.findall(f"core:{kind}", NS):
            vfuncs += len(c.findall("core:virtual-method", NS))
            for s in c.findall("glib:signal", NS):
                rt = rtype(s.find("core:return-value", NS))
                params = [p.find("core:type", NS).get("name") if p.find("core:type", NS) is not None else "array"
                          for p in s.findall("core:parameters/core:parameter", NS)]
                outs = [p.get("name") for p in s.findall("core:parameters/core:parameter", NS)
                        if p.get("direction") in ("out", "inout")]
                out.append({
                    "deprecated": c.get("deprecated") == "1" or s.get("deprecated") == "1",
                    "out_params": outs,
                    "ns": ns_ver, "owner": c.get("name"), "signal": s.get("name"),
                    "returns": rt, "when": s.get("when", "?"),
                    "action": s.get("action") == "1", "detailed": s.get("detailed") == "1",
                    "params": params,
                })
    return out, vfuncs


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")] or ["Gtk-4.0", "Adw-1", "Gdk-4.0"]
    allsig = []
    for a in args:
        sigs, vf = scan(a)
        allsig += sigs
        nonvoid = [s for s in sigs if s["returns"] != "none"]
        print(f"{a}: signals={len(sigs)} non-void={len(nonvoid)} "
              f"(deprecated owner/signal={sum(s['deprecated'] for s in nonvoid)}, "
              f"action/keybinding={sum(s['action'] for s in nonvoid)}) "
              f"signals-with-out-params={sum(bool(s['out_params']) for s in sigs)} "
              f"vfuncs(class+iface)={vf}")
        by = collections.Counter((s["returns"], s["when"]) for s in nonvoid)
        for (rt, when), n in sorted(by.items(), key=lambda x: -x[1]):
            print(f"    {rt:28} when={when:8} {n}")
    if "--list" in sys.argv:
        for s in allsig:
            if s["returns"] != "none":
                flags = ",".join(f for f in ("action", "detailed", "deprecated") if s[f])
                if s["out_params"]:
                    flags += " out=" + "/".join(s["out_params"])
                print(f"{s['ns']:8} {s['owner']}::{s['signal']} -> {s['returns']} "
                      f"when={s['when']} {flags} ({', '.join(s['params'])})")
    if "--json" in sys.argv:
        json.dump([s for s in allsig if s["returns"] != "none"], sys.stdout, indent=1)


if __name__ == "__main__":
    main()
