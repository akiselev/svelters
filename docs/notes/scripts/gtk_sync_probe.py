#!/usr/bin/env python3
"""Probe GTK sync-result and echo semantics that the runtime design depends on.

1. close-request: returning TRUE keeps the window; FALSE lets it close (destroy unless
   hide-on-close).
2. notify on programmatic set: emitted on change; for EXPLICIT_NOTIFY props not emitted
   when the value is unchanged; g_signal_handler_block suppresses; clamped values
   (SpinButton/Adjustment) emit the *clamped* value.
3. Which widget props are EXPLICIT_NOTIFY (count over Gtk widget classes).
4. Re-entrancy: removing a widget from its parent inside its own signal handler.

Usage: xvfb-run -a python3 -I gtk_sync_probe.py
"""
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GObject, GLib

Gtk.init()

# 1. close-request
for ret in (True, False):
    w = Gtk.Window()
    destroyed = []
    w.connect("destroy", lambda *_: destroyed.append(1))
    w.connect("close-request", lambda *_: ret)
    w.present()
    w.close()
    ctx = GLib.MainContext.default()
    while ctx.pending():
        ctx.iteration(False)
    print(f"close-request -> {ret}: visible={w.get_visible()} destroyed={bool(destroyed)}")

# 2. notify / blocking / clamping
e = Gtk.Entry()
log = []
hid = e.connect("notify::text", lambda o, p: log.append(o.get_text()))
e.set_text("x"); e.set_text("x")
e.handler_block(hid); e.set_text("y"); e.handler_unblock(hid)
e.set_text("y")
print("Entry set_text x,x,(blocked y),y -> notify::text log", log)

c = []
ch = Gtk.Entry().get_delegate() if False else None
sb = Gtk.SpinButton.new_with_range(0, 99, 1)
slog = []
sb.connect("notify::value", lambda o, p: slog.append(o.get_value()))
sb.connect("value-changed", lambda o: slog.append(("value-changed", o.get_value())))
sb.set_value(5); sb.set_value(5); sb.set_value(500)
print("SpinButton set_value 5,5,500 (max 99) ->", slog)

sw = Gtk.Switch()
swlog = []
sw.connect("notify::active", lambda o, p: swlog.append(("active", o.get_active())))
sw.connect("state-set", lambda o, s: swlog.append(("state-set", s)) or False)
sw.set_active(True)
print("Switch set_active(True) ->", swlog)

# 3. EXPLICIT_NOTIFY census
tot = expl = 0
for name in dir(Gtk):
    t = getattr(Gtk, name)
    if isinstance(t, type) and issubclass(t, Gtk.Widget):
        for ps in GObject.list_properties(t):
            if ps.owner_type != t.__gtype__ or not (ps.flags & GObject.ParamFlags.WRITABLE):
                continue
            tot += 1
            expl += bool(ps.flags & GObject.ParamFlags.EXPLICIT_NOTIFY)
print(f"own writable props on Gtk widget classes: {tot}, EXPLICIT_NOTIFY: {expl}")

# 4. remove a widget from its parent inside its own handler
box = Gtk.Box()
b = Gtk.Button(label="b")
box.append(b)
b.connect("clicked", lambda btn: box.remove(btn))
b.emit("clicked")
print("remove-in-own-clicked: box child after =", box.get_first_child())
