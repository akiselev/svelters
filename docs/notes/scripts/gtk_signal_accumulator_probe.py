#!/usr/bin/env python3
"""Runtime probe: which non-void GTK signals stop emission when a handler returns TRUE.

GIR does not record the accumulator. This probe instantiates the owner class, connects
two handlers (h1 returns TRUE / a non-default value, h2 records that it ran), emits the
signal with synthesized arguments, and classifies:

  stop_on_true   h2 skipped after h1 returned TRUE  (g_signal_accumulator_true_handled or
                 _gtk_boolean_handled_accumulator)
  stop_after_first  non-bool return and h2 skipped (g_signal_accumulator_first_wins or a
                 stop-on-non-zero accumulator; the probe cannot tell them apart)
  last_wins      both ran (no accumulator; the last handler's - or class closure's - value wins)
  skipped        abstract / argument type not synthesizable / crashed

Each signal runs in a child process so a class handler that dislikes fake arguments only
loses that row. Needs a display: run under `xvfb-run -a`.

Usage: xvfb-run -a python3 -I gtk_signal_accumulator_probe.py [--json]
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

CHILD = r'''
import sys, gi
gi.require_version("Gtk", "4.0"); gi.require_version("Adw", "1")
from gi.repository import Gtk, Gdk, GObject, Adw
Adw.init()
ns, owner, sig = sys.argv[1:4]
cls = getattr({"Gtk": Gtk, "Adw": Adw, "Gdk": Gdk}[ns], owner)
gt = cls.__gtype__
CONCRETE = {"Widget": Gtk.Label, "IMContext": Gtk.IMMulticontext}
if gt.is_abstract() or gt.is_interface():
    if owner not in CONCRETE:
        print("skipped:abstract"); sys.exit()
    cls = CONCRETE[owner]; gt = cls.__gtype__
obj = GObject.new(cls) if gt != Gtk.Window.__gtype__ else Gtk.Window()
sid = GObject.signal_lookup(sig, gt)
q = GObject.signal_query(sid)
args = []
for pt in q.param_types:
    f = pt.fundamental
    if f in (GObject.TYPE_INT, GObject.TYPE_UINT, GObject.TYPE_LONG, GObject.TYPE_ULONG,
             GObject.TYPE_INT64, GObject.TYPE_UINT64):
        args.append(0)
    elif f in (GObject.TYPE_DOUBLE, GObject.TYPE_FLOAT):
        args.append(0.0)
    elif f == GObject.TYPE_BOOLEAN:
        args.append(False)
    elif f == GObject.TYPE_STRING:
        args.append("x")
    elif f in (GObject.TYPE_ENUM, GObject.TYPE_FLAGS):
        args.append(0)
    elif pt == GObject.Value.__gtype__:
        v = GObject.Value(GObject.TYPE_INT, 0); args.append(v)
    elif f in (GObject.TYPE_OBJECT, GObject.TYPE_BOXED, GObject.TYPE_POINTER):
        args.append(None)   # NULL object/boxed: class handlers may bail; isolated per process
    else:
        print("skipped:arg:" + pt.name); sys.exit()
rt = q.return_type
ran = []
def h1(*a):
    ran.append(1)
    if rt == GObject.TYPE_BOOLEAN: return True
    if rt.fundamental in (GObject.TYPE_INT, GObject.TYPE_ENUM, GObject.TYPE_FLAGS, GObject.TYPE_UINT): return 1
    if rt.fundamental == GObject.TYPE_STRING: return "h1"
    return None
def h2(*a):
    ran.append(2)
    if rt == GObject.TYPE_BOOLEAN: return False
    if rt.fundamental == GObject.TYPE_STRING: return "h2"
    return 0 if rt.fundamental != GObject.TYPE_OBJECT else None
obj.connect(sig, h1); obj.connect(sig, h2)
try:
    res = obj.emit(sig, *args)
except Exception as e:
    print("skipped:emit:" + type(e).__name__); sys.exit()
print(("stop_on_true" if rt == GObject.TYPE_BOOLEAN else "stop_after_first") if ran == [1]
      else "last_wins" if ran == [1, 2] else "other:" + repr(ran), "result=" + repr(res))
'''


def main():
    import gir_signal_returns as g
    rows = []
    for ns_ver in ("Gtk-4.0", "Adw-1", "Gdk-4.0"):
        sigs, _ = g.scan(ns_ver)
        for s in sigs:
            if s["returns"] == "none":
                continue
            ns = ns_ver.split("-")[0]
            try:
                p = subprocess.run([sys.executable, "-I", "-c", CHILD, ns, s["owner"], s["signal"]],
                                   capture_output=True, text=True, timeout=20)
                last = p.stdout.strip().splitlines()[-1] if p.stdout.strip() else ""
                out = last if last.split(":")[0].split()[0] in ("stop_on_true", "stop_after_first", "last_wins", "skipped", "other") \
                    else f"skipped:crash rc={p.returncode}"
            except subprocess.TimeoutExpired:
                out = "skipped:timeout"
            rows.append({**s, "probe": out})
            print(f"{ns:4} {s['owner']}::{s['signal']:28} -> {s['returns']:20} {out}", flush=True)
    import collections
    c = collections.Counter(r["probe"].split()[0].split(":")[0] for r in rows)
    print("summary:", dict(c))
    if "--json" in sys.argv:
        json.dump(rows, open(os.path.join(HERE, "gtk_signal_accumulator_probe.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
