#!/usr/bin/env python3
"""Probe GTK4/libadwaita child slots: defaults, replace semantics, auto-wrapping, attached props.

GTK has no QObject-style ownership: a parent holds one reference on its child. Replacing a
single-slot child unparents the old one; it is finalized only if nobody else (here: the
svelters host handle) holds a reference. This probe checks that with weakrefs, and records
slots that a *convenience property* fills with an internal default widget
(Expander:label -> label-widget, MenuButton:menu-model -> popover, Notebook default tab label).

Usage: xvfb-run -a python3 -I gtk_slot_probe.py
"""
import gc
import weakref

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, Gtk

Adw.init()


def name(w):
    return "None" if w is None else type(w).__name__


def single(host, prop, make=Gtk.Label):
    a = make()
    ra = weakref.ref(a)
    host.set_property(prop, a)
    del a
    b = make()
    host.set_property(prop, b)
    gc.collect()
    old = ra()
    state = "finalized" if old is None else f"alive parent={name(old.get_parent())}"
    host.set_property(prop, None)
    print(f"{type(host).__name__}:{prop:14} replace -> old {state}; set(None) -> {name(host.get_property(prop))}; "
          f"B parent={name(b.get_parent())}")


print("== replace semantics (host holds no extra ref) ==")
single(Gtk.Window(), "child")
single(Gtk.Window(), "titlebar")
single(Gtk.Paned(), "start-child")
single(Gtk.HeaderBar(), "title-widget")
single(Gtk.CenterBox(), "center-widget")
single(Gtk.Expander(), "label-widget")
single(Gtk.Frame(), "label-widget")
single(Gtk.MenuButton(), "popover", Gtk.Popover)
single(Adw.ToolbarView(), "content")
single(Adw.ApplicationWindow(), "content")

print("== implicit defaults / wrapping ==")
e = Gtk.Expander(label="x")
print("Expander(label=x).label-widget =", name(e.get_label_widget()))
e.set_label_widget(Gtk.Button()); e.set_label("y")
print("  after set_label_widget(Button) then set_label(y): label-widget =", name(e.get_label_widget()))
f = Gtk.Frame(label="x"); print("Frame(label=x).label-widget =", name(f.get_label_widget()))
m = Gtk.MenuButton(); m.set_menu_model(Gio.Menu())
print("MenuButton.set_menu_model -> popover =", name(m.get_popover()))
m.set_popover(Gtk.Popover()); print("  then set_popover(Popover) -> menu-model =", m.get_menu_model())
mb = Gtk.MenuButton(label="L"); print("MenuButton(label=L).child =", name(mb.get_child()))
n = Gtk.Notebook(); c = Gtk.Label(); n.append_page(c, None)
print("Notebook.append_page(c, None): tab_label =", name(n.get_tab_label(c)), repr(n.get_tab_label_text(c)),
      "; menu_label =", name(n.get_menu_label(c)))
pg = n.get_page(c); print("  NotebookPage props:", [p.name for p in pg.list_properties()])
sw = Gtk.ScrolledWindow(); lab = Gtk.Label(); sw.set_child(lab)
print("ScrolledWindow.set_child(Label) -> child =", name(sw.get_child()), "; label parent =", name(lab.get_parent()))
lb = Gtk.ListBox(); lab = Gtk.Label(); lb.append(lab); print("ListBox.append(Label) -> label parent =", name(lab.get_parent()))
fb = Gtk.FlowBox(); lab = Gtk.Label(); fb.append(lab); print("FlowBox.append(Label) -> label parent =", name(lab.get_parent()))
st = Gtk.Stack(); p = st.add_titled(Gtk.Label(), "a", "A")
print("Stack.add_titled -> returns", name(p), [q.name for q in p.list_properties()])
hb = Gtk.HeaderBar(); print("HeaderBar title-widget default =", name(hb.get_title_widget()))
w = Gtk.Window(); print("Window titlebar default =", name(w.get_titlebar()))
g = Gtk.Grid(); x = Gtk.Label(); g.attach(x, 2, 1, 3, 2)
lc = g.get_layout_manager().get_layout_child(x)
print("Grid.attach(x,2,1,3,2) -> GridLayoutChild", (lc.get_column(), lc.get_row(), lc.get_column_span(), lc.get_row_span()))
lc.set_column(0); print("  GridLayoutChild.set_column(0) -> query_child", g.query_child(x))
o = Gtk.Overlay(); ov = Gtk.Label(); o.add_overlay(ov); o.set_measure_overlay(ov, True)
print("Overlay attached per-child flags via methods: measure =", o.get_measure_overlay(ov), "clip =", o.get_clip_overlay(ov))
hb = Gtk.HeaderBar(); a, b = Gtk.Button(), Gtk.Button(); hb.pack_start(a); hb.pack_start(b)
print("HeaderBar has insert-at-index for pack_start?", any(hasattr(hb, n) for n in ("insert_start", "reorder_start")))
w = Gtk.Label(); ctl = Gtk.EventControllerKey(); w.add_controller(ctl)
print("add_controller: controller widget =", name(ctl.get_widget()), "; observe_controllers n =", w.observe_controllers().get_n_items())
tv = Gtk.TextView(); buf = tv.get_buffer(); anc = buf.create_child_anchor(buf.get_end_iter())
btn = Gtk.Button(); tv.add_child_at_anchor(btn, anc); print("TextView.add_child_at_anchor -> anchor widgets =", len(anc.get_widgets()))
tb = Adw.ToolbarView(); tb.add_top_bar(Adw.HeaderBar()); print("Adw.ToolbarView top bars: has remove:", hasattr(tb, "remove"))
ar = Adw.ActionRow(); ar.add_prefix(Gtk.Label()); ar.add_suffix(Gtk.Label()); print("Adw.ActionRow prefix/suffix ok; remove:", hasattr(ar, "remove"))
tvw = Adw.TabView(); tp = tvw.append(Gtk.Label()); print("Adw.TabView.append -> ", name(tp), "title prop:", tp.find_property("title") is not None)
