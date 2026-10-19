#!/usr/bin/env python3
"""Automated GTK4 prober: candidate facts with evidence for every concrete widget kind.

The probe is an offline maintainer step (like ingestion): its output is checked in as
metadata/probe/gtk/<toolkit version>.json and read by tools/metadata/gtk/infer.py. CI does not
re-probe; it checks that the checked-in probe matches the toolkit version in the ledger
(`tools/metadata/ledger.py check`). The output is deterministic: sorted keys, rounded floats, no
machine paths or addresses.

For each kind (from metadata/gtk/*.json: widget subclasses, not abstract, not deprecated):
  props    set a typed sample, read back, record the outcome (exact | normalized | ignored | error),
           whether notify::<prop> fired and which other signals fired (trigger/echo/binding
           evidence). Non-exact outcomes are retried with precondition recipes (range, populate,
           text) on a fresh instance; the recipe that makes the round trip exact is the evidence.
  methods  accessor methods (set_X/get_X/is_X of a prop) are checked against the property; pure
           zero-arg getters are called; zero-arg actions are called and diffed (props + signals).
  slots    widget-typed writable props: is a set child parented here (slot) or not (reference),
           what happens to the old child on replace, does set(None) unparent; many-slot families
           (one-widget-arg insert + remove) are exercised the same way.
  signals  return type -> handler_result/sync; zero-param action signals are emitted.
GLib criticals/warnings during an operation make it `error` (with the message as evidence).

Each kind runs in a worker subprocess; a crash or timeout bisects the batch so one bad kind
cannot hide the others. Output: one JSON document (kinds -> facts), deterministic ordering.

Usage:
  xvfb-run -a python3 tools/metadata/gtk/probe.py [--out metadata/probe/gtk/<version>.json] [--kinds Button,Entry] [-j 4]
  xvfb-run -a python3 tools/metadata/gtk/probe.py --ns Adw --out adw.json     (a pack with no ingested metadata)
"""
from __future__ import annotations

import argparse
import concurrent.futures as cf
import gc
import json
import os
import re
import subprocess
import sys
import weakref
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SAMPLE_PNG = next((str(p) for p in sorted(Path("/usr/share/icons/hicolor/48x48/apps").glob("*.png"))), "/dev/null")
# Machine-specific values are replaced by tokens so the checked-in probe is the same on every machine.
PNG_TOKEN = "<sample-png>"
ADDR = re.compile(r"0x[0-9a-fA-F]{4,}")
# Parameter types a synthetic emission can fill, with the Python sample (infer.py has the Rust twin).
EMIT_ARGS = {"gboolean": True, "gint": 1, "guint": 1, "glong": 1, "gulong": 1, "gint64": 1, "guint64": 1,
             "gdouble": 1.5, "gfloat": 1.5, "gchararray": "x"}
DANGEROUS = re.compile(r"^(run|.*_run|main|block.*|grab_add|dispose|destroy|realize|unrealize|map|unmap|show|present|"
                       r"measure|size_allocate|allocate|snapshot.*|queue_\w+|init_template|dispose_template|"
                       r"unparent|insert_after|insert_before|.*_with_time|compute_\w+|pick|get_allocat\w+|"
                       r"activate_default|activate|close|destroy|fullscreen|maximize|minimize|.*_dialog|"
                       r"error_bell|keynav_failed|mnemonic_activate|popup|popdown|emit_\w+|spin|update|hide|grab_focus|child_focus|"
                       r"trigger_tooltip_query)$")


def kinds_from_metadata() -> list[str]:
    """Concrete widget kinds, then the abstract kinds and interfaces of metadata/rules/gtk.yaml `subjects`
    as `Kind=Subject`: their facts are probed (and later tested) on an instance of a concrete subject."""
    import yaml
    out = []
    for p in sorted((ROOT / "metadata/gtk").glob("*.json")):
        d = json.loads(p.read_text())
        if d.get("kind") != "object" or d.get("abstract"):
            continue
        if "Gtk.Widget" not in d.get("bases", []) or (d.get("availability") or {}).get("deprecated"):
            continue
        out.append(p.stem)
    subjects = yaml.safe_load((ROOT / "metadata/rules/gtk.yaml").read_text()).get("subjects") or {}
    return out + [f"{k}={v}" for k, v in sorted(subjects.items())]


def kinds_from_gi(ns: str) -> list[str]:
    """Concrete, non-deprecated widget classes of a GI namespace (for packs without ingested metadata)."""
    import gi
    gi.require_version("Gtk", "4.0")
    gi.require_version(ns, "1")
    from gi.repository import GObject, Gtk
    mod = __import__("gi.repository." + ns, fromlist=[ns])
    out = []
    for n in sorted(dir(mod)):
        c = getattr(mod, n, None)
        gt = getattr(c, "__gtype__", None)
        if gt is None or not isinstance(c, type) or not GObject.type_is_a(gt, Gtk.Widget.__gtype__):
            continue
        if GObject.type_test_flags(gt, GObject.TypeFlags.ABSTRACT) or getattr(c, "__deprecated__", False):
            continue
        out.append(n)
    return out


# ----------------------------------------------------------------------------- worker side

def worker(kinds: list[str]) -> None:
    import faulthandler
    faulthandler.enable()
    skip = set(filter(None, os.environ.get("SVL_PROBE_SKIP", "").split(";")))
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import GLib

    only = os.environ.get("SVL_PROBE_ONLY")
    counter = [0]

    def op(kind, section, name):
        """Announce an operation (the driver blames the last one on a crash); False = skip it.
        With SVL_PROBE_ONLY=i only the i-th operation runs (isolated mode, one op per process)."""
        key = f"{kind}:{section}:{name}"
        if only is not None:
            i, counter[0] = counter[0], counter[0] + 1
            if i != int(only):
                return False
        if key in skip:
            return False
        # settle the previous op first (finalizers, idles), so a deferred crash is blamed on it
        gc.collect()
        ctx = GLib.MainContext.default()
        for _ in range(20):
            if not ctx.iteration(False):
                break
        print("#op " + key, flush=True)
        return True
    os.environ.setdefault("GSK_RENDERER", "cairo")
    os.environ.setdefault("GDK_DISABLE", "gl,vulkan")
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import GLib, GObject, Gtk
    NS = Gtk
    if os.environ.get("SVL_PROBE_NS", "Gtk") == "Adw":
        gi.require_version("Adw", "1")
        from gi.repository import Adw
        Adw.init()
        NS = Adw

    logs: list[str] = []

    def on_log(dom, lvl, msg, *_):
        logs.append(ADDR.sub("0x...", f"{dom}: {msg}"))
    for d in ("Gtk", "GLib-GObject", "GLib", "Gdk", "Gsk", "Pango", None):
        GLib.log_set_handler(d, GLib.LogLevelFlags.LEVEL_WARNING | GLib.LogLevelFlags.LEVEL_CRITICAL, on_log, None)

    WIDGET = Gtk.Widget.__gtype__
    from gi.repository import Gio
    OBJ_FACTORY = {
        "GtkAdjustment": lambda: Gtk.Adjustment(value=5, lower=0, upper=100, step_increment=1, page_increment=10),
        "GMenuModel": Gio.Menu,
        "GListModel": lambda: Gtk.StringList.new(["a", "b", "c"]),
        "GFile": lambda: Gio.File.new_for_path(SAMPLE_PNG),
        "GtkTextBuffer": Gtk.TextBuffer,
        "GtkEntryBuffer": Gtk.EntryBuffer,
        "GtkStringList": lambda: Gtk.StringList.new(["a", "b", "c"]),
        "GtkSelectionModel": lambda: Gtk.SingleSelection.new(Gtk.StringList.new(["a", "b", "c"])),
        "GtkListItemFactory": Gtk.SignalListItemFactory,
        "GtkFilter": Gtk.CustomFilter,
        "GtkSorter": Gtk.CustomSorter,
        "GtkPopover": Gtk.Popover,
        "GtkEventController": Gtk.EventControllerKey,
        "GtkLayoutManager": Gtk.BoxLayout,
        "GtkColorDialog": Gtk.ColorDialog,
        "GtkFontDialog": Gtk.FontDialog,
        "GtkFileDialog": Gtk.FileDialog,
        "GtkTextTagTable": Gtk.TextTagTable,
    }

    def pump():
        ctx = GLib.MainContext.default()
        for _ in range(20):
            if not ctx.iteration(False):
                break

    def jsonable(v):
        # enums and flags are int subclasses: test them before the plain int case, or a sample
        # becomes the enum's number where the harness compares nicks
        if isinstance(v, GObject.GEnum) or (hasattr(v, "value_nick") and not isinstance(v, GObject.GFlags)):
            return getattr(v, "value_nick", None) or str(int(v))
        if isinstance(v, GObject.GFlags):
            return sorted(getattr(v, "value_nicks", []) or [str(int(v))])
        if v is None or isinstance(v, (bool, int)):
            return v
        if isinstance(v, str):
            return PNG_TOKEN if v == SAMPLE_PNG else ADDR.sub("0x...", v)
        if isinstance(v, float):
            return round(v, 6)
        return f"<{type(v).__name__}>"

    def all_signals(gtype):
        out = {}
        t = gtype
        while t and t.name not in ("invalid", ""):
            for sid in GObject.signal_list_ids(t):
                q = GObject.signal_query(sid)
                out.setdefault(q.signal_name, q)
            t = t.parent
        for it in gtype.interfaces:
            for sid in GObject.signal_list_ids(it):
                q = GObject.signal_query(sid)
                out.setdefault(q.signal_name, q)
        return out

    class Rec:
        """Records every signal emission on one instance."""

        def __init__(self, w, sigs):
            self.ev: list[str] = []
            self.w = w
            for name, q in sigs.items():
                rt = q.return_type.name
                if rt not in ("void", "gboolean"):
                    continue
                if len(q.param_types) > 6:
                    continue
                ret = False if rt == "gboolean" else None
                try:
                    w.connect(name, self._mk(name, ret))
                except Exception:
                    pass

        def _mk(self, name, ret):
            def cb(*a):
                if name == "notify":
                    self.ev.append("notify::" + a[1].name)
                else:
                    self.ev.append(name)
                return ret
            return cb

        def take(self):
            e, self.ev = self.ev, []
            return e

    subject_of: dict[str, str] = {}

    def new(kind):
        cls = getattr(NS, subject_of.get(kind, kind))
        return GObject.new(cls.__gtype__)

    def samples(pspec, cur):
        t = pspec.value_type
        f = t.fundamental.name
        if f == "gboolean":
            return [not cur]
        if f in ("gint", "guint", "glong", "gulong", "gint64", "guint64", "gchar", "guchar"):
            lo, hi = pspec.minimum, pspec.maximum
            cands = [cur + 1, cur - 1, (lo + hi) // 2 if hi - lo < 2**40 else 7, lo, hi]
            return [c for c in dict.fromkeys(cands) if lo <= c <= hi and c != cur][:2]
        if f in ("gdouble", "gfloat"):
            lo, hi = pspec.minimum, pspec.maximum
            cands = [cur + 1.0, 0.5 if lo <= 0.5 <= hi else lo, (lo + hi) / 2 if abs(hi - lo) < 1e30 else 7.0, cur - 1.0]
            return [c for c in dict.fromkeys(cands) if lo <= c <= hi and c != cur][:2]
        if pspec.value_type.name == "gunichar":
            return ["x"] if cur != "x" else ["y"]
        if f == "gchararray":
            n = pspec.name
            if re.search(r"uri|url", n):
                return ["https://example.org/"]
            if n in ("file", "filename", "path"):
                return [SAMPLE_PNG]
            if n == "icon-name":
                return ["edit-copy"]
            return ["svl probe"]
        if f in ("GObject", "GInterface") and pspec.value_type.name in OBJ_FACTORY:
            return [OBJ_FACTORY[pspec.value_type.name]()]
        if f == "GEnum":
            vals = [v for v in pspec.enum_class.__enum_values__.values() if v != cur]
            return vals[:1]
        return []

    def eq(a, b):
        if isinstance(a, float) or isinstance(b, float):
            try:
                return abs(float(a) - float(b)) < 1e-6
            except Exception:
                return False
        return a == b

    def recipe_range(w):
        if hasattr(w, "set_range"):
            w.set_range(0, 100)
        elif "adjustment" in [p.name for p in GObject.list_properties(w)]:
            w.set_property("adjustment", Gtk.Adjustment(value=0, lower=0, upper=100, step_increment=1,
                                                        page_increment=10, page_size=0))

    def recipe_populate(w):
        for m in ("append_page", "append", "add_child", "add"):
            if hasattr(w, m):
                for i in range(3):
                    try:
                        getattr(w, m)(Gtk.Label(label=f"c{i}"))
                    except TypeError:
                        getattr(w, m)(Gtk.Label(label=f"c{i}"), None)
                return
        raise RuntimeError("no populate op")

    def recipe_text(w):
        for p in ("text", "label"):
            if p in [x.name for x in GObject.list_properties(w)]:
                w.set_property(p, "hello world")
                return
        raise RuntimeError("no text prop")

    def recipe_window(w):
        win = Gtk.Window()
        if not isinstance(w, Gtk.Root):
            win.set_child(w)
        (w if isinstance(w, Gtk.Root) else win).present()
        pump()
        w._svl_win = win

    RECIPES = [("range", recipe_range), ("populate", recipe_populate), ("text", recipe_text), ("realized", recipe_window)]

    def probe_prop(kind, sigs, pspec):
        name = pspec.name
        res = {}
        for recipe_name, recipe in [(None, None)] + RECIPES:
            w = new(kind)
            if recipe:
                try:
                    recipe(w)
                except Exception:
                    continue
            rec = Rec(w, sigs)
            logs.clear()
            cur = w.get_property(name)
            ss = samples(pspec, cur)
            if not ss:
                # opaque types: identity round trip (read, write back, read)
                try:
                    w.set_property(name, cur)
                    res = {"outcome": "identity" if not logs else "error", "type": pspec.value_type.name,
                           "log": logs[:1]}
                except Exception as e:
                    res = {"outcome": "error", "type": pspec.value_type.name, "log": [str(e)[:120]]}
                return res
            v = ss[0]
            rec.take()
            try:
                w.set_property(name, v)
            except Exception as e:
                res = {"outcome": "error", "sample": jsonable(v), "log": [str(e)[:120]]}
                continue
            got = w.get_property(name)
            ev = rec.take()
            set_logs = list(logs)
            logs.clear()
            # second change back, to show the trigger works repeatedly (and whether the default is re-settable)
            w.set_property(name, cur)
            ev2 = rec.take()
            reset_logs = list(logs)
            logs[:] = set_logs
            outcome = ("error" if logs else "exact" if eq(got, v) else "ignored" if eq(got, cur) else "normalized")
            res = {"outcome": outcome, "sample": jsonable(v), "read": jsonable(got), "default": jsonable(cur),
                   "notify": f"notify::{name}" in ev, "signals": sorted(set(x for x in ev if x != f"notify::{name}")),
                   "repeat": sorted(set(ev2)) == sorted(set(ev)) if ev else None, "recipe": recipe_name,
                   "log": logs[:1], "reset_log": reset_logs[:1]}
            if outcome == "exact":
                return res
            if outcome == "error" and recipe_name is None:
                continue
        return res

    def own_dict(cls):
        """The attributes the kind itself defines. PyGObject overrides (Gtk.Widget, ...) are a subclass of the
        introspected class with the same GType, so the methods live further down the MRO."""
        out = {}
        for c in cls.__mro__:
            if getattr(c, "__gtype__", None) != cls.__gtype__:
                break
            for n, v in c.__dict__.items():
                out.setdefault(n, v)
        return out

    def gi_methods(kind):
        out = {}
        for n, fi in own_dict(getattr(NS, kind)).items():
            if hasattr(fi, "get_arguments") and hasattr(fi, "is_method") and fi.is_method():
                out[n] = fi
        return out

    def probe_methods(kind, sigs, props, recipes):
        out = {}
        meths = gi_methods(kind)
        for n, fi in sorted(meths.items()):
            if not op(kind, "method", n):
                out[n] = {"class": "crash"}
                continue
            if fi.is_deprecated():
                continue
            args = list(fi.get_arguments())
            m = re.match(r"(get|set|is)_(\w+)$", n)
            stem = m.group(2).replace("_", "-") if m else None
            w = new(kind)
            logs.clear()
            try:
                if m and stem in props and m.group(1) in ("get", "is") and len(args) == 0:
                    r = getattr(w, n)()
                    pv = w.get_property(stem)
                    out[n] = {"class": "accessor_get", "prop": stem,
                              "outcome": "error" if logs else "exact" if eq(r, pv) or (isinstance(r, GObject.Object) and r == pv) else "differs",
                              "ret": jsonable(r)}
                elif m and stem in props and m.group(1) == "set" and len(args) == 1:
                    ps = props[stem]
                    rn = recipes.get(stem)
                    if rn:
                        dict(RECIPES)[rn](w)
                    ss = samples(ps, w.get_property(stem))
                    if not ss:
                        out[n] = {"class": "accessor_set", "prop": stem, "outcome": "no_sample"}
                        continue
                    getattr(w, n)(ss[0])
                    got = w.get_property(stem)
                    out[n] = {"class": "accessor_set", "prop": stem, "sample": jsonable(ss[0]), "recipe": rn,
                              "outcome": "error" if logs else "exact" if eq(got, ss[0]) else "differs"}
                elif len(args) == 0 and DANGEROUS.match(n) is None:
                    rt = fi.get_return_type().get_tag_as_string()
                    if rt != "void" and re.match(r"(get|is|has|n|count)_?", n):
                        r = getattr(w, n)()
                        # a value that depends on global state (window ids, counters) is no snapshot: ask a second instance
                        again = getattr(new(kind), n)()
                        out[n] = {"class": "pure_getter", "outcome": "error" if logs else "ok", "ret": jsonable(r),
                                  "stable": jsonable(r) == jsonable(again)}
                    elif rt == "void":
                        rec = Rec(w, sigs)
                        before = snapshot(w)
                        getattr(w, n)()
                        pump()
                        after = snapshot(w)
                        diff = {k: [before[k], after[k]] for k in before if before[k] != after.get(k)}
                        out[n] = {"class": "action", "outcome": "error" if logs else "ok", "changed": diff,
                                  "signals": sorted(set(rec.take()))}
                elif DANGEROUS.match(n):
                    out[n] = {"class": "dangerous"}
            except Exception as e:
                out[n] = {"class": "exception", "outcome": "error", "log": [str(e)[:120]]}
            if logs and n in out:
                out[n]["log"] = logs[:1]
        return out

    def snapshot(w):
        s = {}
        for p in GObject.list_properties(w):
            if not p.flags & GObject.ParamFlags.READABLE:
                continue
            if p.value_type.fundamental.name in ("GObject", "GBoxed", "GPointer", "GParam", "GVariant", "GInterface"):
                continue
            try:
                s[p.name] = jsonable(w.get_property(p.name))
            except Exception:
                pass
        return s

    def probe_slots(kind, props):
        out = {}
        for name, p in props.items():
            if not op(kind, "slot", name):
                out[name] = {"role": "crash"}
                continue
            if not GObject.type_is_a(p.value_type, WIDGET) or not p.flags & GObject.ParamFlags.WRITABLE or \
                    not p.flags & GObject.ParamFlags.READABLE:
                continue
            if p.flags & GObject.ParamFlags.CONSTRUCT_ONLY:
                continue
            w = new(kind)
            logs.clear()
            child_t = p.value_type if p.value_type != WIDGET else Gtk.Label.__gtype__
            try:
                a = GObject.new(child_t) if not GObject.type_test_flags(child_t, GObject.TypeFlags.ABSTRACT) else Gtk.Label()
                wa = weakref.ref(a)
                w.set_property(name, a)
                parent = a.get_parent()
                direct = parent == w
                anc = a.is_ancestor(w)
                revealed = None
                if not anc:
                    # children of collapsed/hidden hosts are parented only when shown: reveal and look again
                    for rp in ("expanded", "reveal-child", "search-mode-enabled", "reveal-flap"):
                        if rp in [x.name for x in GObject.list_properties(w)]:
                            w.set_property(rp, True)
                            revealed = rp
                    direct, anc = a.get_parent() == w, a.is_ancestor(w)
                readback = w.get_property(name) == a
                b = GObject.new(child_t) if not GObject.type_test_flags(child_t, GObject.TypeFlags.ABSTRACT) else Gtk.Label()
                w.set_property(name, b)
                old_parent = a.get_parent()
                del a
                gc.collect()
                old = "finalized" if wa() is None else ("unparented" if old_parent is None else "still_parented")
                w.set_property(name, None)
                cleared = b.get_parent() is None
                role = "slot" if (direct or anc) else "reference"
                out[name] = {"role": role, "parent": "direct" if direct else "ancestor" if anc else "none",
                             "readback": readback, "old_on_replace": old, "revealed_by": revealed if anc else None, "clear_unparents": cleared,
                             "accepts": child_t.name, "log": logs[:1]}
            except Exception as e:
                out[name] = {"role": "error", "log": [str(e)[:120]]}
        # many-slot families: one widget argument insert + remove(widget)
        cls = getattr(NS, kind)
        for ins in ("append", "prepend", "add_child", "add_overlay", "pack_start", "pack_end", "add_named",
                    "append_page", "insert", "put", "attach"):
            fi = own_dict(cls).get(ins)
            if not fi or not hasattr(fi, "get_arguments"):
                continue
            if not op(kind, "slot", f"<{ins}>"):
                out[f"<{ins}>"] = {"role": "crash"}
                continue
            rem = next((r for r in ("remove", "remove_overlay") if own_dict(cls).get(r) or hasattr(cls, r)), None)
            w = new(kind)
            logs.clear()
            try:
                nargs = len(fi.get_arguments())
                kids = [Gtk.Label(label=f"c{i}") for i in range(2)]
                for k in kids:
                    if ins == "insert":
                        getattr(w, ins)(k, -1)
                    elif ins == "add_named":
                        getattr(w, ins)(k, None)
                    elif ins == "put":
                        getattr(w, ins)(k, 1.0, 2.0)
                    elif ins == "attach":
                        getattr(w, ins)(k, kids.index(k), 0, 1, 1)
                    elif ins == "add_child" and nargs == 2:
                        getattr(w, ins)(k, f"id{kids.index(k)}")
                    elif nargs == 1:
                        getattr(w, ins)(k)
                    elif ins == "append_page":
                        getattr(w, ins)(k, None)
                    else:
                        raise RuntimeError(f"{nargs} args")
                par = [("direct" if k.get_parent() == w else "ancestor" if k.is_ancestor(w) else "none") for k in kids]
                order = None
                fc = w.get_first_child()
                seq = []
                while fc is not None:
                    seq.append(fc)
                    fc = fc.get_next_sibling()
                if all(x == "direct" for x in par):
                    order = [seq.index(k) for k in kids]
                removed = None
                if rem:
                    if ins == "append_page":
                        w.remove_page(0)
                    else:
                        getattr(w, rem)(kids[0])
                    removed = kids[0].get_parent() is None
                out[f"<{ins}>"] = {"role": "many_slot", "insert": ins, "remove": rem if ins != "append_page" else "remove_page",
                                   "parent": par, "order": order, "remove_unparents": removed, "log": logs[:1]}
            except Exception as e:
                out[f"<{ins}>"] = {"role": "error", "log": [str(e)[:120]]}
        return out

    def probe_signals(kind, sigs, own_type):
        out = {}
        for name, q in sorted(sigs.items()):
            if q.itype != own_type:
                continue
            rt = q.return_type.name
            r = {"returns": rt, "handler_result": "none" if rt == "void" else "bool_stop" if rt == "gboolean" else "value",
                 "sync_required": rt != "void", "action": bool(q.signal_flags & GObject.SignalFlags.ACTION),
                 "run": "last" if q.signal_flags & GObject.SignalFlags.RUN_LAST else "first",
                 "n_params": len(q.param_types), "params": [t.name for t in q.param_types]}
            plain = not DANGEROUS.match(name.replace("-", "_"))
            if r["action"] and len(q.param_types) == 0 and rt == "void" and plain \
                    and not op(kind, "signal", name):
                r["emit"] = "crash"
            elif r["action"] and len(q.param_types) == 0 and rt == "void" and plain:
                w = new(kind)
                rec = Rec(w, {name: q})
                logs.clear()
                try:
                    w.emit(name)
                    r["emit"] = "delivered" if name in rec.take() else "not_delivered"
                except Exception as e:
                    r["emit"] = "error: " + str(e)[:80]
                if logs:
                    r["log"] = logs[:1]
            elif plain and rt in ("void", "gboolean") and all(t.name in EMIT_ARGS for t in q.param_types) \
                    and not (q.signal_flags & GObject.SignalFlags.DETAILED):
                # synthetic emission with sampled arguments: evidence for a delivery-only fact
                args = [EMIT_ARGS[t.name] for t in q.param_types]
                if not op(kind, "signal", name):
                    r["synthetic"] = {"args": args, "emit": "crash"}
                else:
                    w = new(kind)
                    rec = Rec(w, {name: q})
                    logs.clear()
                    try:
                        w.emit(name, *args)
                        r["synthetic"] = {"args": args, "emit": "delivered" if name in rec.take() else "not_delivered"}
                    except Exception as e:
                        r["synthetic"] = {"args": args, "emit": "error: " + str(e)[:80]}
                    if logs:
                        r["synthetic"]["log"] = logs[:1]
            out[name] = r
        return out

    for spec in kinds:
        kind, _, subject = spec.partition("=")
        if subject:
            subject_of[kind] = subject
        res: dict = {"kind": kind}
        if subject:
            res["subject"] = subject
        try:
            w = new(kind)
            gt = getattr(NS, kind).__gtype__          # props are the kind's own, instances are the subject's
            sigs = all_signals(w.__gtype__)
            own = {p.name: p for p in GObject.list_properties(w) if p.owner_type == gt}
            allp = {p.name: p for p in GObject.list_properties(w)}
            res["props"] = {}
            for name, p in sorted(own.items()):
                if not (p.flags & GObject.ParamFlags.READABLE and p.flags & GObject.ParamFlags.WRITABLE) or \
                        p.flags & GObject.ParamFlags.CONSTRUCT_ONLY or p.flags & GObject.ParamFlags.DEPRECATED:
                    continue
                if GObject.type_is_a(p.value_type, WIDGET):
                    continue
                if not op(kind, "prop", name):
                    res["props"][name] = {"outcome": "crash"}
                    continue
                try:
                    res["props"][name] = probe_prop(kind, sigs, p)
                except Exception as e:
                    res["props"][name] = {"outcome": "error", "log": [f"probe: {e}"[:120]]}
            res["methods"] = probe_methods(kind, sigs, allp, {k: v.get("recipe") for k, v in res["props"].items()})
            res["slots"] = probe_slots(kind, own)
            res["signals"] = {} if subject else probe_signals(kind, sigs, gt)
        except Exception as e:
            res["error"] = str(e)[:200]
        print(json.dumps(res, sort_keys=True), flush=True)


# ----------------------------------------------------------------------------- driver side

def run_batch(kinds: list[str], timeout: int, skip: tuple[str, ...] = ()) -> list[dict]:
    env = dict(os.environ, SVL_PROBE_SKIP=";".join(skip))
    try:
        p = subprocess.run([sys.executable, __file__, "--worker", ",".join(kinds)], capture_output=True, text=True,
                           timeout=timeout, env=env)
        done = [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
        ops = [l[4:] for l in p.stdout.splitlines() if l.startswith("#op ")]
        crashed = p.returncode != 0
        err = p.stderr[-2000:] + f"\nexit {p.returncode}"
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        done = [json.loads(l) for l in out.splitlines() if l.startswith("{")]
        ops = [l[4:] for l in out.splitlines() if l.startswith("#op ")]
        crashed, err = True, "timeout"
    if not crashed:
        return done
    seen = {d["kind"] for d in done}
    rest = [k for k in kinds if k.partition("=")[0] not in seen]
    if not rest:
        return done
    bad = rest[0]                           # the first unfinished kind is the one that died
    # Heap corruption is often detected one or more ops after the culprit, so the last announced op
    # is not trustworthy: rerun the crashed kind with one operation per process.
    done.append(run_isolated(bad, timeout))
    return done + (run_batch(rest[1:], timeout, skip) if rest[1:] else [])


SECTION = {"prop": "props", "method": "methods", "slot": "slots", "signal": "signals"}


def run_isolated(kind: str, timeout: int) -> dict:
    merged: dict = {"kind": kind.partition("=")[0], "props": {}, "methods": {}, "slots": {}, "signals": {}, "isolated": True}
    crashes = []
    i = 0
    while i < 2000:
        env = dict(os.environ, SVL_PROBE_ONLY=str(i))
        try:
            p = subprocess.run([sys.executable, __file__, "--worker", kind], capture_output=True, text=True,
                               timeout=timeout, env=env)
            out, rc, err = p.stdout, p.returncode, p.stderr
        except subprocess.TimeoutExpired as e:
            out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
            rc, err = -999, "timeout"
        ops = [l[4:] for l in out.splitlines() if l.startswith("#op ")]
        docs = [json.loads(l) for l in out.splitlines() if l.startswith("{")]
        if not ops:
            if rc != 0:
                merged["crash"] = f"before first op (exit {rc})"
            if docs and not merged["signals"]:
                merged["signals"] = docs[0].get("signals", {})
            break
        key = ops[0]
        _, section, name = key.split(":", 2)
        if rc != 0:
            crashes.append({"op": key, "why": "timeout" if rc == -999 else f"exit {rc}", "isolated": True})
        elif docs:
            d = docs[0]
            if not merged["signals"]:
                merged["signals"] = d.get("signals", {})
            src = d.get(SECTION[section], {})
            if name in src:
                merged[SECTION[section]][name] = src[name]
        i += 1
    if crashes:
        merged["crashes"] = crashes
        for c in crashes:
            _, section, name = c["op"].split(":", 2)
            entry = merged[SECTION[section]].setdefault(name, {})
            entry.update({"outcome": "crash"} if section == "prop" else {"class": "crash"} if section == "method"
                         else {"role": "crash"} if section == "slot" else {"emit": "crash"})
    return merged


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--worker")
    ap.add_argument("--ns", default="Gtk", help="Gtk (kinds from metadata/gtk) or Adw (kinds from GI: a pack, no metadata yet)")
    ap.add_argument("--kinds")
    ap.add_argument("--out", help="default: metadata/probe/gtk/<toolkit version>.json (Gtk) or probe-<ns>.json")
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--batch", type=int, default=6)
    ap.add_argument("--timeout", type=int, default=120)
    a = ap.parse_args()
    if a.worker:
        worker(a.worker.split(","))
        return 0
    if a.ns != "Gtk":
        os.environ["SVL_PROBE_NS"] = a.ns
    kinds = a.kinds.split(",") if a.kinds else kinds_from_metadata() if a.ns == "Gtk" else kinds_from_gi(a.ns)
    batches = [kinds[i:i + a.batch] for i in range(0, len(kinds), a.batch)]
    results = []
    with cf.ThreadPoolExecutor(a.j) as ex:
        for r in ex.map(lambda b: run_batch(b, a.timeout), batches):
            results.extend(r)
    results.sort(key=lambda d: d["kind"])
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import Gtk
    version = f"{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}"
    doc = {"schema": "svelters-probe/v0", "toolkit": "gtk", "namespace": a.ns, "version": version,
           "kinds": {d["kind"]: scrub(d) for d in results}}
    out = Path(a.out) if a.out else (ROOT / f"metadata/probe/gtk/{version}.json" if a.ns == "Gtk" else Path(f"probe-{a.ns}.json"))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")
    print(f"probed {len(results)} kinds -> {out}")
    return 0


def scrub(o):
    """Remove what differs between machines and runs (heap addresses in messages)."""
    if isinstance(o, str):
        return ADDR.sub("0x...", o)
    if isinstance(o, list):
        return [scrub(x) for x in o]
    if isinstance(o, dict):
        return {k: scrub(v) for k, v in o.items()}
    return o


if __name__ == "__main__":
    sys.exit(main())
