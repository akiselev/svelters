#!/usr/bin/env python3
"""Proof-of-concept: live GType/GParamSpec/signal introspection via PyGObject.

This is the *runtime* source of truth (what the linked libgtk actually registers),
complementing GIR. It needs no display: class introspection does not require gtk_init.

Usage: python3 -I gobject_live_dump.py Button [--ns Gtk --ver 4.0]
"""
import argparse
import json

import gi


def pspec_info(ps, GObject):
    flags = ps.flags
    d = {
        "name": ps.name,
        "value_type": ps.value_type.name,
        "owner": ps.owner_type.name,
        "readable": bool(flags & GObject.ParamFlags.READABLE),
        "writable": bool(flags & GObject.ParamFlags.WRITABLE),
        "construct": bool(flags & GObject.ParamFlags.CONSTRUCT),
        "construct_only": bool(flags & GObject.ParamFlags.CONSTRUCT_ONLY),
        "explicit_notify": bool(flags & GObject.ParamFlags.EXPLICIT_NOTIFY),
        "deprecated": bool(flags & GObject.ParamFlags.DEPRECATED),
    }
    # Default value straight from the GParamSpec (typed, not a C literal string).
    try:
        dv = ps.get_default_value()
        if hasattr(dv, "value_nick"):
            dv = dv.value_nick
        elif hasattr(dv, "first_value_nick"):  # flags
            dv = int(dv)
        elif dv is not None and not isinstance(dv, (bool, int, float, str)):
            dv = repr(dv)
        d["default"] = dv
    except Exception as e:  # noqa: BLE001
        d["default_error"] = str(e)
    for k in ("minimum", "maximum"):
        if hasattr(ps, k):
            d[k] = getattr(ps, k)
    return d


def signal_info(sid, GObject):
    q = GObject.signal_query(sid)
    F = GObject.SignalFlags
    return {
        "name": q.signal_name,
        "owner": q.itype.name,
        "params": [t.name for t in q.param_types],
        "return": q.return_type.name,
        "flags": [n for n in ("RUN_FIRST", "RUN_LAST", "RUN_CLEANUP", "NO_RECURSE", "DETAILED",
                              "ACTION", "NO_HOOKS", "DEPRECATED") if q.signal_flags & getattr(F, n)],
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("klass")
    ap.add_argument("--ns", default="Gtk")
    ap.add_argument("--ver", default="4.0")
    a = ap.parse_args()
    gi.require_version(a.ns, a.ver)
    from gi.repository import GObject
    mod = __import__("gi.repository." + a.ns, fromlist=[a.ns])
    cls = getattr(mod, a.klass)
    gtype = cls.__gtype__

    chain, t = [], gtype
    while t.name != "invalid" and t != GObject.TYPE_INVALID:
        chain.append(t.name)
        t = t.parent
    props = [pspec_info(p, GObject) for p in GObject.list_properties(gtype)]
    sigs = []
    t = gtype
    seen = set()
    while t != GObject.TYPE_INVALID:
        for sid in GObject.signal_list_ids(t):
            if sid not in seen:
                seen.add(sid)
                sigs.append(signal_info(sid, GObject))
        t = t.parent
    for iface in gtype.interfaces:
        for sid in GObject.signal_list_ids(iface):
            if sid not in seen:
                seen.add(sid)
                sigs.append(signal_info(sid, GObject))
    print(json.dumps({
        "gtype": gtype.name,
        "abstract": gtype.is_abstract(),
        "inheritance": chain,
        "interfaces": [i.name for i in gtype.interfaces],
        "properties": props,
        "signals": sigs,
    }, indent=1, default=str))


if __name__ == "__main__":
    main()
