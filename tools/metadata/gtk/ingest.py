#!/usr/bin/env python3
"""Ingest the Gtk-4.0 namespace into svelters-element-kind/v0 metadata.

Sources: Gtk-4.0.gir (+ transitive includes) and a PyGObject live pass (GParamSpec / signal
query on the installed libgtk). Hand/agent annotations from metadata/overrides/gtk/<Kind>.yaml
are merged over the ingested facts. Output: metadata/gtk/<Kind>.json, one file per type.

Deterministic: inputs are the GIR, the live type registry (no display needed), the overrides
and the constants below. No clocks, env or network. Run from anywhere:

    python3 tools/metadata/gtk/ingest.py [--girdir /usr/share/gir-1.0]
"""
from __future__ import annotations

import argparse
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import naming  # noqa: E402
import svmeta  # noqa: E402
import gir as G  # noqa: E402

META_DIR = ROOT / "metadata/gtk"
OVERRIDE_DIR = ROOT / "metadata/overrides/gtk"
SCHEMA_PATH = Path(__file__).resolve().parent / "override.schema.json"
CHILD_RE = re.compile(r"^(append|prepend|insert.*|remove|pack_.*|add_.*|set_.*_widget|set_child|reorder.*|put|attach.*)$")

LIVE_PRIM = {
    "gboolean": "bool", "gchar": "i32", "guchar": "u32", "gint": "i32", "guint": "u32", "glong": "i64",
    "gulong": "u64", "gint64": "i64", "guint64": "u64", "gfloat": "f32", "gdouble": "f64",
    "gchararray": "string", "GVariant": "variant", "GStrv": "list<string>", "void": "void",
    "gpointer": "pointer", "GType": "u64",
}
TAG_PREFIX = {"class": "object", "interface": "object", "enumeration": "enum", "bitfield": "flags",
              "record": "boxed", "union": "value", "callback": "callback"}


def finite(v):
    if isinstance(v, float) and not math.isfinite(v):
        return None
    return v


class Ingest:
    def __init__(self, girdir: str):
        import gi

        gi.require_version("Gtk", "4.0")
        from gi.repository import GObject, Gtk

        self.gi, self.GObject, self.Gtk = gi, GObject, Gtk
        self.repo = G.Repo(girdir)
        self.repo.load("Gtk-4.0")
        self.ns = self.repo.ns["Gtk"]
        self.gtk_version = f"{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}"
        self.warnings: list[str] = []

    # ---- type helpers -------------------------------------------------
    def live_ref(self, gt) -> str:
        n = gt.name
        if n in LIVE_PRIM:
            return LIVE_PRIM[n]
        hit = self.repo.by_gtype.get(n)
        if hit:
            ns, name, tag = hit
            return f"{TAG_PREFIX[tag]}<{G.type_id(ns, name)}>"
        GO = self.GObject
        for base, prefix in ((GO.TYPE_ENUM, "enum"), (GO.TYPE_FLAGS, "flags"), (GO.TYPE_BOXED, "boxed"),
                             (GO.TYPE_OBJECT, "object"), (GO.TYPE_INTERFACE, "object")):
            if gt.is_a(base):
                return f"{prefix}<svelters-api:v1:type:gtk:?:{n}>"
        return f"native:{n}"

    def py_gtype(self, name: str):
        cls = getattr(self.Gtk, name, None)
        gt = getattr(cls, "__gtype__", None)
        return gt

    # ---- member builders ---------------------------------------------
    def method_c(self, els, name):
        for el in els:
            m = el.find(f"core:method[@name='{name}']", G.NS)
            if m is not None:
                return m.get(G.CN + "identifier")
        return None

    def params_of(self, m, ns):
        out = []
        for p in m.findall("core:parameters/core:parameter", G.NS):
            ref, native = self.repo.type_ref(p, ns)
            out.append({"name": p.get("name"), "type": ref, "native": native,
                        "nullable": p.get("nullable") == "1" or p.get("allow-none") == "1",
                        "ownership": {"none": "borrowed", "full": "owned", "container": "container"}.get(
                            p.get("transfer-ownership"), "borrowed")})
        return out

    def callable_rec(self, m, ns, ident: str):
        ret = m.find("core:return-value", G.NS)
        rref, rnative = self.repo.type_ref(ret, ns) if ret is not None else ("void", None)
        rec = {
            "id": ident, "name": m.get("name"), "native": m.get(G.CN + "identifier"),
            "params": self.params_of(m, ns),
            "returns": {"type": rref, "native": rnative, "nullable": ret is not None and ret.get("nullable") == "1",
                        "ownership": {"none": "borrowed", "full": "owned"}.get(
                            ret.get("transfer-ownership") if ret is not None else None, "borrowed")},
            "invocation": {"dev": "introspected", "release": "generated_thunk"},
            "unsupported_reason": None,
            "origin": "gir",
        }
        if m.get("throws") == "1":
            rec["throws"] = True
        if m.get("deprecated") == "1":
            rec["deprecated"] = True
        reason = None
        if m.get("introspectable") == "0":
            reason = "non_introspectable"
        elif any(p["type"] == "varargs" for p in rec["params"]):
            reason = "varargs"
        elif any(p["type"].startswith("callback<") for p in rec["params"]):
            reason = "callback_scope"
        rec["unsupported_reason"] = reason
        return rec

    def build_props(self, el, gtype_name, live_gt, ns, tid, accessor_els):
        gir = {p.get("name"): p for p in el.findall("core:property", G.NS)}
        live = {}
        if live_gt is not None and (live_gt.is_a(self.GObject.TYPE_OBJECT) or live_gt.is_interface()):
            for ps in self.GObject.list_properties(live_gt):
                if ps.owner_type.name == gtype_name:
                    live[ps.name] = ps
        out = []
        F = self.GObject.ParamFlags
        for name in sorted(set(gir) | set(live)):
            g, ps = gir.get(name), live.get(name)
            rec = {"id": f"{tid}:prop:{name}", "name": name, "js_name": name, "origin": "gir" if g is not None else "runtime"}
            if g is not None:
                ref, native = self.repo.type_ref(g, ns)
                rec["access"] = {"read": g.get("readable", "1") == "1", "write": g.get("writable") == "1",
                                 "construct": g.get("construct") == "1", "construct_only": g.get("construct-only") == "1"}
                get, setr = g.get("getter"), g.get("setter")
                rec["accessors"] = {"get": self.method_c(accessor_els, get) if get else None,
                                    "set": self.method_c(accessor_els, setr) if setr else None}
                if g.get("version"):
                    rec["since"] = g.get("version")
                if g.get("deprecated") == "1":
                    rec["deprecated"] = True
            else:
                ref, native = self.live_ref(ps.value_type), None
                rec["access"] = {"read": bool(ps.flags & F.READABLE), "write": bool(ps.flags & F.WRITABLE),
                                 "construct": bool(ps.flags & F.CONSTRUCT), "construct_only": bool(ps.flags & F.CONSTRUCT_ONLY)}
                rec["accessors"] = {"get": None, "set": None}
            t = {"ref": ref, "nullable": ref.split("<")[0] in ("string", "object", "boxed", "variant"), "native": native}
            rec["default"], rec["range"] = None, None
            rec["notify"] = {"signal": f"notify::{name}", "explicit": False}
            rec["invocation"] = "reflected"
            rec["role"] = "value"
            if ps is not None:
                t["gtype"] = ps.value_type.name
                dv = None
                try:
                    # get_default_value() crashes PyGObject on object/boxed/param pspecs: scalars only.
                    if not (ref in LIVE_PRIM.values() or ref.startswith(("enum<", "flags<"))):
                        raise ValueError("no scalar default")
                    if ref in ("variant", "pointer", "u64") and ps.value_type.name != "guint64":
                        raise ValueError("no scalar default")
                    dv = ps.get_default_value()
                    if hasattr(dv, "value_nick"):
                        dv = dv.value_nick
                    elif dv is not None and not isinstance(dv, (bool, int, float, str)):
                        dv = int(dv) if hasattr(dv, "first_value_nick") else str(dv)
                except Exception:  # noqa: BLE001 - pspecs without a default (object, boxed, ...)
                    dv = None
                rec["default"] = finite(dv)
                if hasattr(ps, "minimum") and hasattr(ps, "maximum"):
                    rec["range"] = {"min": finite(ps.minimum), "max": finite(ps.maximum)}
                rec["notify"]["explicit"] = bool(ps.flags & F.EXPLICIT_NOTIFY)
                rec["runtime"] = {"origin": "runtime", "owner": ps.owner_type.name, "gtype": ps.value_type.name,
                                  "flags": [n for n in ("READABLE", "WRITABLE", "CONSTRUCT", "CONSTRUCT_ONLY",
                                                        "EXPLICIT_NOTIFY", "DEPRECATED") if ps.flags & getattr(F, n)]}
                if g is not None:  # cross-check GIR against the installed library
                    w = bool(ps.flags & F.WRITABLE)
                    if w != (g.get("writable") == "1"):
                        self.warnings.append(f"{tid}:{name}: writable differs between GIR and runtime")
            # Enums from other namespaces (Pango, Gdk) have no kind file: keep their nicks on the
            # property so tests and typings can use them (A1-GAPS G11).
            if ref.startswith(("enum<", "flags<")) and ":Gtk:" not in ref:
                ens, ename = ref[ref.index("<") + 1:-1].rsplit(":", 2)[-2:]
                tag, el_e = self.repo.find(ens, ename)
                if el_e is not None and tag in ("enumeration", "bitfield"):
                    t["nicks"] = [m.get(G.GL + "nick") for m in el_e.findall("core:member", G.NS)]
            rec["type"] = t
            out.append(rec)
        return out

    def build_signals(self, el, live_gt, ns, tid):
        gir = {s.get("name"): s for s in el.findall("glib:signal", G.NS)}
        live = {}
        GO = self.GObject
        if live_gt is not None and (live_gt.is_a(GO.TYPE_OBJECT) or live_gt.is_interface()):
            for sid in GO.signal_list_ids(live_gt):
                q = GO.signal_query(sid)
                live[q.signal_name] = q
        out = []
        SF = GO.SignalFlags
        for name in sorted(set(gir) | set(live)):
            g, q = gir.get(name), live.get(name)
            rec = {"id": f"{tid}:signal:{name}", "name": name, "js_event": name, "origin": "gir" if g is not None else "runtime"}
            gparams = g.findall("core:parameters/core:parameter", G.NS) if g is not None else []
            if q is not None:
                params = [{"name": gparams[i].get("name") if i < len(gparams) else f"arg{i}", "type": self.live_ref(t),
                           "gtype": t.name} for i, t in enumerate(q.param_types)]
                ret = self.live_ref(q.return_type)
                flags = [n.lower() for n in ("ACTION", "DETAILED", "NO_RECURSE", "NO_HOOKS", "DEPRECATED") if q.signal_flags & getattr(SF, n)]
                phase = "first" if q.signal_flags & SF.RUN_FIRST else "last" if q.signal_flags & SF.RUN_LAST else "cleanup"
                rec["runtime"] = {"origin": "runtime", "return_gtype": q.return_type.name}
            else:
                params = [{"name": p.get("name"), "type": self.repo.type_ref(p, ns)[0]} for p in gparams]
                ret = self.repo.type_ref(g.find("core:return-value", G.NS), ns)[0]
                flags, phase = [], g.get("when", "last")
            rec["params"] = params
            rec["returns"] = {"type": ret}
            rec["handler_result"] = "bool_stop" if ret == "bool" else "none"
            rec["sync_required"] = False
            rec["phase"] = phase
            rec["flags"] = flags
            rec["connection"] = {"disconnect_on": ["handle_drop", "native_destroy"]}
            out.append(rec)
        return out

    # ---- kinds ------------------------------------------------------------
    def class_chain(self, el):
        chain = []
        ns = "Gtk"
        while el.get("parent"):
            ns, n = self.repo.qualify(el.get("parent"), ns)
            chain.append(f"{ns}.{n}")
            _, el = self.repo.find(ns, n)
            if el is None:
                break
        return chain

    def source(self, name, has_override):
        return {"gir": {"file": "Gtk-4.0.gir", "sha256": G.sha256(self.repo.files["Gtk"]), "version": self.gtk_version},
                "runtime": {"pygobject": self.gi.__version__, "gtk": self.gtk_version},
                "overrides": f"overrides/gtk/{name}.yaml" if has_override else None}

    def object_kind(self, tag, el, overrides):
        name = el.get("name")
        tid = G.type_id("Gtk", name)
        gtype_name = el.get(G.GL + "type-name")
        live_gt = self.py_gtype(name)
        if live_gt is None:
            self.warnings.append(f"{tid}: no live GType")
        elif live_gt.name != gtype_name:
            self.warnings.append(f"{tid}: GType differs ({live_gt.name} vs {gtype_name})")
        chain = self.class_chain(el) if tag == "class" else []
        iface_els = []
        ifaces = []
        if live_gt is not None:
            for i in live_gt.interfaces:
                hit = self.repo.by_gtype.get(i.name)
                if hit:
                    ifaces.append(f"{hit[0]}.{hit[1]}")
                    iface_els.append(self.repo.find(hit[0], hit[1])[1])
        else:
            for i in el.findall("core:implements", G.NS):
                ifaces.append(i.get("name") if "." in i.get("name") else f"Gtk.{i.get('name')}")
        abstract = el.get("abstract") == "1" or (live_gt is not None and live_gt.is_abstract())
        props = self.build_props(el, gtype_name, live_gt, "Gtk", tid, [el] + iface_els)
        signals = self.build_signals(el, live_gt, "Gtk", tid)
        methods = [self.callable_rec(m, "Gtk", f"{tid}:method:{m.get('name')}") for m in
                   sorted(el.findall("core:method", G.NS), key=lambda m: m.get("name"))]
        ctors = [self.callable_rec(c, "Gtk", f"{tid}:ctor:{c.get('name')}") for c in
                 sorted(el.findall("core:constructor", G.NS), key=lambda c: c.get("name"))]
        for c in ctors:
            c["returns"] = {"type": "self", "ownership": "floating_sink"}
        if tag == "class" and not abstract:
            ctors.append({"id": f"{tid}:ctor:generic", "name": "generic", "invocation": {"dev": "reflected"}, "origin": "runtime"})
        is_widget = "Gtk.Widget" in chain or name == "Widget"
        widget_ref = f"object<{G.type_id('Gtk', 'Widget')}>"
        candidates = {"widget_props": [], "child_ops": []}
        if is_widget:
            candidates["widget_props"] = [p["name"] for p in props if p["type"]["ref"] == widget_ref and p["access"]["write"]]
            candidates["child_ops"] = [m["name"] for m in methods
                                       if CHILD_RE.match(m["name"]) and any(p["type"] == widget_ref for p in m["params"])]
        avail = {"since": el.get("version"), "deprecated": el.get("deprecated-version") if el.get("deprecated") == "1" else None}
        doc = {
            "schema": svmeta.SCHEMA, "toolkit": "gtk", "id": tid,
            "native": {"name": el.get(G.CN + "type"), "gtype": gtype_name, "c_type": el.get(G.CN + "type"),
                       "get_type": el.get(G.GL + "get-type")},
            "kind": "interface" if tag == "interface" else "object",
            "abstract": bool(abstract), "bases": chain, "interfaces": sorted(ifaces),
            "availability": avail, "constructors": ctors, "properties": props, "signals": signals, "methods": methods,
            "enums": [],
            "children": {"policy": "unknown", "default_slot": None, "slots": [], "candidates": candidates},
            "origin": "gir",
        }
        if is_widget and not abstract and tag == "class":
            doc["element"] = {"tag": naming.tag_for("gtk", name), "origin": "gir"}
        return doc

    def enum_kind(self, tag, el):
        name = el.get("name")
        return {
            "schema": svmeta.SCHEMA, "toolkit": "gtk", "id": G.type_id("Gtk", name),
            "native": {"name": el.get(G.CN + "type"), "gtype": el.get(G.GL + "type-name"), "c_type": el.get(G.CN + "type")},
            "kind": "enum" if tag == "enumeration" else "flags", "origin": "gir",
            "availability": {"since": el.get("version"), "deprecated": None},
            "members": [{"name": m.get("name"), "nick": m.get(G.GL + "nick"), "value": int(m.get("value")),
                         "native": m.get(G.CN + "identifier")} for m in el.findall("core:member", G.NS)],
        }

    def boxed_kind(self, el):
        name = el.get("name")
        tid = G.type_id("Gtk", name)
        return {
            "schema": svmeta.SCHEMA, "toolkit": "gtk", "id": tid,
            "native": {"name": el.get(G.CN + "type"), "gtype": el.get(G.GL + "type-name"), "c_type": el.get(G.CN + "type")},
            "kind": "boxed", "origin": "gir", "availability": {"since": el.get("version"), "deprecated": None},
            "constructors": [self.callable_rec(c, "Gtk", f"{tid}:ctor:{c.get('name')}") for c in
                             sorted(el.findall("core:constructor", G.NS), key=lambda c: c.get("name"))],
            "methods": [self.callable_rec(m, "Gtk", f"{tid}:method:{m.get('name')}") for m in
                        sorted(el.findall("core:method", G.NS), key=lambda m: m.get("name"))],
        }

    def all_kinds(self, overrides):
        kinds = {}
        for tag in ("class", "interface", "enumeration", "bitfield", "record"):
            for el in sorted(self.ns.findall(f"core:{tag}", G.NS), key=lambda e: e.get("name")):
                if not el.get(G.GL + "type-name"):
                    continue  # plain structs and non-GObject types have no GType
                if tag == "record" and el.get(G.GL + "is-gtype-struct-for"):
                    continue
                name = el.get("name")
                if tag in ("class", "interface"):
                    kinds[name] = self.object_kind(tag, el, overrides)
                elif tag in ("enumeration", "bitfield"):
                    kinds[name] = self.enum_kind(tag, el)
                else:
                    kinds[name] = self.boxed_kind(el)
        return kinds


def load_overrides() -> dict[str, dict]:
    out = {}
    for p in sorted(OVERRIDE_DIR.glob("*.yaml")):
        out[p.stem] = svmeta.load_yaml(p)
    return out


def split_provenance(rec: dict) -> tuple[dict, dict]:
    rec = dict(rec)
    prov = rec.pop("provenance", None)
    return rec, {"origin": "manual", "provenance": prov, "fields": sorted(k for k in rec if k != "name")}


EXPLICIT_NEW = ("new", "virtual", "derived_from")  # markers that let an override add a record GTK does not have


def apply_override(doc: dict, ov: dict, errors: list[str]) -> dict:
    """Strict merge: every property/signal/method record must match an ingested record by name,
    unless it is explicitly new (`new: true`, `virtual: {...}`, `derived_from: {...}`). Slots,
    bindings and the lifetime block are annotation-only and always new."""
    ov = dict(ov)
    marks: list[tuple[list, str, dict]] = []  # (path, record name, annotation)
    clean: dict = {}
    for key, val in ov.items():
        if key in ("properties", "signals", "methods"):
            have = {r["name"] for r in doc.get(key, [])}
            recs = []
            for r in val:
                r2, ann = split_provenance(r)
                if not ann["provenance"]:
                    errors.append(f"{doc['id']}: {key} {r.get('name')} lacks provenance")
                if r["name"] not in have and not any(m in r for m in EXPLICIT_NEW):
                    errors.append(f"{doc['id']}: {key} {r['name']!r} matches no ingested record "
                                  f"(typo? mark it `new: true` if it really is additional)")
                if r["name"] in have and r.get("new"):
                    errors.append(f"{doc['id']}: {key} {r['name']!r} is marked new but exists upstream")
                recs.append(r2)
                marks.append(([key], r["name"], ann))
            clean[key] = recs
        elif key == "children":
            ch = dict(val)
            recs = []
            for r in ch.get("slots", []):
                r2, ann = split_provenance(r)
                if not ann["provenance"]:
                    errors.append(f"{doc['id']}: slot {r.get('name')} lacks provenance")
                r2["id"] = f"{doc['id']}:slot:{r['name']}"
                for a in r.get("attrs", []):
                    # Child attribute names are native: the layout-child property, else the kebab-cased name.
                    want = a.get("property") or naming.kebab(a["name"])
                    if a["name"] != want:
                        errors.append(f"{doc['id']}: slot {r['name']} attr {a['name']!r} must be named {want!r}")
                recs.append(r2)
                marks.append((["children", "slots"], r["name"], ann))
            ch["slots"] = recs
            clean[key] = ch
        elif key == "bindings":
            clean[key] = [{**r, "id": f"{doc['id']}:binding:{r['name']}", "origin": "manual"} for r in val]
        elif key == "lifetime":
            clean[key] = {**val, "id": f"{doc['id']}:lifetime", "origin": "manual"}
        elif key == "text_prop":
            continue  # read from the override by tools/metadata/semantics/gen_semantics.py, not merged
        elif key == "unexposed":
            clean[key] = {**val, "origin": "manual"}
        else:
            clean[key] = val
    if "lifetime" in clean:
        doc = {k: v for k, v in doc.items() if k != "lifetime"}  # replaced, not merged
    merged = svmeta.merge(doc, clean)
    for path, name, ann in marks:
        lst = merged
        for p in path:
            lst = lst[p]
        for rec in lst:
            if rec["name"] == name:
                rec["annotated"] = ann
                rec["origin"] = rec.get("origin") or "manual"
    if "element" in ov:
        # Tags are derived (tools/metadata/common/naming.py); an override may restate the tag, or
        # differ only with a `tag_reason`.
        want = naming.tag_for("gtk", doc["id"].rsplit(":", 1)[-1])
        tag = ov["element"].get("tag", want)
        if tag != want and not ov["element"].get("tag_reason"):
            errors.append(f"{doc['id']}: element tag {tag!r} breaks the naming rule ({want!r}) without tag_reason")
        merged["element"]["tag"] = tag
        merged["element"]["origin"] = "manual"
    if "unexposed" in ov:
        merged.pop("element", None)
    if "verify_subject" in ov:
        merged["verify_subject"] = ov["verify_subject"]
    return merged


def derived_lifetime(doc: dict) -> dict | None:
    """Per-kind lifetime block (docs/notes/ownership-and-lifetimes.md 2.3), derived from the type
    hierarchy; an override's `lifetime:` replaces it."""
    if doc["kind"] != "object":
        return None
    chain = ["Gtk." + doc["id"].rsplit(":", 1)[-1]] + doc["bases"]
    floating = "GObject.InitiallyUnowned" in chain
    out = {"id": f"{doc['id']}:lifetime", "origin": "derived",
           "native_refcount": "gobject_floating" if floating else "gobject"}
    if doc["abstract"]:
        out.update({"class": "borrowed_only", "provenance": "derived: abstract type, JS cannot create it"})
    elif "Gtk.Window" in chain:
        out.update({"class": "toplevel", "destroy": {"native": "gtk_window_destroy"},
                    "close": {"request_signal": "close-request", "veto": "return_stop", "default": "destroy"},
                    "provenance": "derived: GtkWindow subclass; ABI v2 toplevel class (plans/architecture/backend-abi.md)"})
    elif "Gtk.Widget" in chain:
        out.update({"class": "owned", "destroy": {"native": "unref"},
                    "provenance": "derived: GtkWidget (floating ref sunk by the handle table; the parent holds a ref while inserted)"})
    else:
        out.update({"class": "owned", "destroy": {"native": "unref"}, "provenance": "derived: GObject"})
    return out


INHERIT_SECTIONS = ("properties", "signals", "methods", "bindings")


def inherited_facts(kinds: dict[str, dict]) -> None:
    """Annotations on base classes and interfaces apply to subclasses unless the subclass annotates
    the same name. Records where each inherited fact comes from (`inherited.<section>[]`)."""
    by_gir = {f"Gtk.{n}": d for n, d in kinds.items()}

    def annotated(d: dict, sec: str) -> list:
        if sec == "slots":
            return d.get("children", {}).get("slots", [])
        return [r for r in d.get(sec, []) if "annotated" in r or sec == "bindings"]

    for doc in kinds.values():
        if doc.get("kind") not in ("object", "interface"):
            continue
        sections = INHERIT_SECTIONS + ("slots",)
        own = {sec: {r["name"] for r in annotated(doc, sec)} for sec in sections}
        inh: dict[str, list] = {}
        for src_name in doc.get("bases", []) + doc.get("interfaces", []):
            src = by_gir.get(src_name)
            if src is None:
                continue
            for sec in sections:
                if sec == "slots" and src_name not in doc.get("bases", []):
                    continue
                for r in annotated(src, sec):
                    if r["name"] in own[sec] or any(x["name"] == r["name"] for x in inh.get(sec, [])):
                        continue
                    inh.setdefault(sec, []).append({"name": r["name"], "from": src["id"], "fact": r.get("id"), "origin": "manual"})
        if inh:
            doc["inherited"] = {k: sorted(v, key=lambda r: r["name"]) for k, v in sorted(inh.items())}
        else:
            doc.pop("inherited", None)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    args = ap.parse_args()
    ing = Ingest(args.girdir)
    overrides = load_overrides()
    errors: list[str] = []
    for p in sorted(OVERRIDE_DIR.glob("*.yaml")):
        errors += svmeta.check_override(p, SCHEMA_PATH)
    kinds = ing.all_kinds(overrides)
    for name in overrides:
        if name not in kinds:
            errors.append(f"override for unknown kind {name}")

    for name, doc in sorted(kinds.items()):
        doc["source"] = ing.source(name, name in overrides)
        life = derived_lifetime(doc)
        if life:
            doc["lifetime"] = life
        if name in overrides:
            doc = apply_override(doc, overrides[name], errors)
            kinds[name] = doc
        for p in doc.get("properties", []):
            if naming.kebab(p["name"]) != p["name"]:
                errors.append(f"{name}: property {p['name']!r} must be kebab-case ({naming.kebab(p['name'])!r})")
            p["js_name"] = p["name"]  # GObject property names are already kebab-case (naming.py)
        refs = sorted({r for p in doc.get("properties", []) for r in [(p.get("type") or {}).get("ref", "")] if r.startswith(("enum<", "flags<"))})
        doc["enums"] = [r[r.index("<") + 1:-1] for r in refs]
        errs = svmeta.validate(doc)
        errors += [f"{name}: {e}" for e in errs]
    inherited_facts(kinds)
    tags: dict[str, str] = {}
    for name, doc in kinds.items():
        if "element" in doc:
            t = doc["element"]["tag"]
            if t in tags:
                errors.append(f"duplicate element tag {t}: {tags[t]} / {name}")
            tags[t] = name
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1

    wanted = set()
    changed = 0
    for name, doc in kinds.items():
        wanted.add(f"{name}.json")
        changed += svmeta.write_json(META_DIR / f"{name}.json", doc)
    for p in sorted(META_DIR.glob("*.json")):
        if p.name not in wanted:
            p.unlink()
    for w in sorted(set(ing.warnings)):
        print("warning:", w, file=sys.stderr)
    print(f"{len(kinds)} kinds, {changed} files written", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
