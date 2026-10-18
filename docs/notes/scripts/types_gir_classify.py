#!/usr/bin/env python3
"""Prototype auto-classifier: GIR types of the GTK stack -> the svelters non-GUI taxonomy.

Rules (plans/research/NON-GUI-TYPES.md section 3.1), applied to the GIR only, no hand lists except
RECORD_SHAPES (the JS shape decision for value structs, not a classification fact):

  enumeration/bitfield                         -> enum / flags
  callback                                     -> callback
  class/interface deriving from GObject        -> object (model if it is or implements Gio.ListModel)
    ... Gio.AsyncResult/Gio.Cancellable/Gio.Task -> async-infra (hidden behind Promise + AbortSignal)
  fundamental class with ref/unref funcs       -> value (refcounted, immutable: GskRenderNode, GdkEvent)
  record that is a class/iface struct          -> skip (glib:is-gtype-struct-for)
  record, all public fields plain numbers      -> record (by value)
  record with GType and ref/unref methods      -> value (refcounted boxed)
  record with GType (copy/free)                -> value (boxed copy)
  record ...Iter / used as caller-allocated cursor -> transient (excluded; offsets/persistent forms)
  record without GType, not plain              -> needs-override
  union                                        -> needs-override

Members of bindable types (object/model/value/record) are then checked slot by slot against what
the generic GI invoker can marshal once it has out params, statics, callbacks by scope, async
pairs and arrays/lists (section 3.2). Automatic exclusions: deprecated, introspectable="0",
`*_finish` (folded into its async method), ref/unref/copy/free/get_type (the core).

Also computes the binding scope policy: the closure of types reachable from the Gtk API.

Usage: python3 -I types_gir_classify.py [--girdir /usr/share/gir-1.0] [--json out.json] [--show CATEGORY]
"""
import argparse
import collections
import json
import xml.etree.ElementTree as ET

C = "{http://www.gtk.org/introspection/core/1.0}"
GL = "{http://www.gtk.org/introspection/glib/1.0}"
FILES = ["GLib-2.0", "GObject-2.0", "Gio-2.0", "Gdk-4.0", "Gsk-4.0", "Pango-1.0", "Graphene-1.0",
         "GdkPixbuf-2.0", "cairo-1.0", "Gtk-4.0"]
TARGET = ["GLib", "GObject", "Gio", "Gdk", "Gsk", "Pango", "Graphene", "Gtk"]
NUM = {"gboolean", "gint", "guint", "gint8", "guint8", "gint16", "guint16", "gint32", "guint32", "gint64",
       "guint64", "glong", "gulong", "gsize", "gssize", "gchar", "guchar", "gshort", "gushort", "goffset",
       "gintptr", "guintptr", "gdouble", "gfloat", "gunichar"}
STR = {"utf8", "filename"}
ASYNC_INFRA = {"Gio.AsyncResult", "Gio.Cancellable", "Gio.Task"}
CORE_NAMES = {"ref", "unref", "copy", "free", "get_type", "ref_sink", "sink", "force_floating", "is_floating",
              "ref_count", "weak_ref", "weak_unref", "add_weak_pointer", "remove_weak_pointer",
              "add_toggle_ref", "remove_toggle_ref", "get_qdata", "set_qdata", "steal_qdata", "get_data",
              "set_data", "steal_data", "set_data_full", "set_qdata_full", "dup_data", "dup_qdata",
              "replace_data", "replace_qdata", "watch_closure"}


def skip(el):
    return el.get("introspectable") == "0" or el.get("deprecated") == "1" or el.get("moved-to")


class Gir:
    def __init__(self, girdir):
        self.t = {}
        for f in FILES:
            ns = ET.parse(f"{girdir}/{f}.gir").getroot().find(C + "namespace")
            for el in ns:
                if el.get("name"):
                    self.t[f"{ns.get('name')}.{el.get('name')}"] = (el.tag.split("}")[1], el, ns.get("name"))

    def q(self, n, ns):
        if not n or n in NUM or n in STR or "." in n or n in ("none", "gpointer", "gconstpointer", "GType", "va_list"):
            return n
        return f"{ns}.{n}"

    def chain(self, full):
        out = []
        while full in self.t and full not in out:
            out.append(full)
            tag, el, ns = self.t[full]
            full = self.q(el.get("parent"), ns) if tag == "class" else None
        return out

    def implements(self, full, iface):
        for c in self.chain(full):
            tag, el, ns = self.t[c]
            if c == iface or any(self.q(i.get("name"), ns) == iface for i in el.findall(C + "implements")):
                return True
            if tag == "interface" and any(self.q(p.get("name"), ns) == iface for p in el.findall(C + "prerequisite")):
                return True
        return False


def plain_record(g, el, ns, seen=()):
    fields = [f for f in el.findall(C + "field") if f.get("private") != "1"]
    if not fields or el.get("disguised") == "1" or el.get("opaque") == "1":
        return False
    if any(f.get("private") == "1" for f in el.findall(C + "field")):
        return False
    for f in fields:
        t = f.find(C + "type")
        if t is None:
            return False  # array or callback field
        n = g.q(t.get("name"), ns)
        if n in NUM:
            continue
        tag, sub, sns = g.t.get(n, ("?", None, None))
        if tag in ("enumeration", "bitfield"):
            continue
        if tag == "record" and n not in seen and plain_record(g, sub, sns, seen + (n,)) and t.get("c:type", "").find("*") < 0:
            continue
        return False
    return True


def classify(g, full):
    tag, el, ns = g.t[full]
    name = full.split(".", 1)[1]
    if tag == "enumeration":
        return "enum"
    if tag == "bitfield":
        return "flags"
    if tag == "callback":
        return "callback"
    if tag == "alias":
        return "alias"
    if tag in ("constant", "function", "docsection", "boxed"):
        return None
    if tag in ("class", "interface"):
        if full in ASYNC_INFRA:
            return "async-infra"
        if tag == "class" and "GObject.Object" not in g.chain(full) and full != "GObject.Object":
            # fundamental types (GskRenderNode, GdkEvent, GtkExpression, GParamSpec) and their
            # subclasses: a ref-func anywhere up the chain makes them refcounted values
            if any(g.t[c][1].get(GL + "ref-func") for c in g.chain(full)):
                return "value"
            return "needs-override"
        if g.implements(full, "Gio.ListModel"):
            return "model"
        return "object"
    if tag == "record":
        if el.get(GL + "is-gtype-struct-for"):
            return None  # class struct: never a JS type
        if name.endswith("Private") or name.endswith("Iface") or name.endswith("Interface") and not el.get(GL + "get-type"):
            return None
        if name.endswith("Iter") or name in ("TreePath", "TreeRowReference", "BitsetIter", "HashTableIter", "VariantIter"):
            return "transient"
        if plain_record(g, el, ns):
            return "record"
        if el.get(GL + "get-type"):
            meths = {m.get("name") for m in el.findall(C + "method")}
            return "value"
        return "needs-override"
    if tag == "union":
        return "needs-override"
    return None


BINDABLE = {"object", "model", "value", "record"}


def slot_ok(g, node, ns, cats, scope_ok=True):
    """None if the generic invoker can marshal the slot, else a reason."""
    t = node.find(C + "type")
    a = node.find(C + "array")
    if a is not None:
        inner = a.find(C + "type")
        if inner is None:
            return "nested array"
        if a.get("name") in ("GLib.ByteArray",) or (inner.get("name") in ("guint8", "gint8", "guchar")):
            return None  # bytes
        r = elem_ok(g, inner.get("name"), ns, cats)
        return r
    if t is None:
        return "varargs"
    n = g.q(t.get("name"), ns)
    if n in ("GLib.List", "GLib.SList", "GLib.PtrArray", "GLib.Array"):
        inner = t.find(C + "type")
        return elem_ok(g, inner.get("name") if inner is not None else None, ns, cats)
    if n == "GLib.HashTable":
        inner = t.findall(C + "type")
        if len(inner) == 2 and all(i.get("name") in STR for i in inner):
            return None  # string map -> object
        return "hash table"
    return elem_ok(g, n, ns, cats)


def elem_ok(g, n, ns, cats):
    n = g.q(n, ns)
    if n is None:
        return "untyped"
    if n in NUM or n in STR or n == "none":
        return None
    if n == "GType":
        return None  # SVL_T_TYPE
    if n in ("gpointer", "gconstpointer", "va_list"):
        return "gpointer"
    if n in ("GLib.Variant", "GObject.Value", "GLib.DateTime", "GLib.Bytes", "GLib.Error", "GLib.Strv"):
        return None
    c = cats.get(n)
    if c in ("enum", "flags", "callback", "async-infra", "widget") or c in BINDABLE:
        return None
    if c == "alias":
        tag, el, ans = g.t[n]
        inner = el.find(C + "type")
        return elem_ok(g, inner.get("name") if inner is not None else None, ans, cats)
    return f"type:{c or 'unknown'}:{n}"


def callable_reason(g, m, ns, cats):
    rv = m.find(C + "return-value")
    ps = m.find(C + "parameters")
    skip_idx = set()
    params = ps.findall(C + "parameter") if ps is not None else []
    for p in params:
        for at in ("closure", "destroy"):
            if p.get(at) is not None:
                skip_idx.add(int(p.get(at)))
        a = p.find(C + "array")
        if a is not None and a.get("length") is not None:
            skip_idx.add(int(a.get("length")))
    if rv is not None:
        a = rv.find(C + "array")
        if a is not None and a.get("length") is not None:
            skip_idx.add(int(a.get("length")))
        r = slot_ok(g, rv, ns, cats)
        if r:
            return r
    for i, p in enumerate(params):
        if i in skip_idx:
            continue
        if p.get("name") == "..." or p.find(C + "varargs") is not None:
            return "varargs"
        tn = p.find(C + "type")
        if tn is not None and g.q(tn.get("name"), ns) in ("gpointer", "gconstpointer") and p.get("closure") is None:
            # user_data of a callback names it with closure=; a bare gpointer is a real blocker
            if not any(q.get("closure") == str(i) for q in params):
                return "gpointer"
            continue
        r = slot_ok(g, p, ns, cats)
        if r:
            return r
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--json")
    ap.add_argument("--show")
    a = ap.parse_args()
    g = Gir(a.girdir)
    cats = {}
    for full, (tag, el, ns) in g.t.items():
        if skip(el):
            continue
        c = classify(g, full)
        if c:
            cats[full] = c
    # widgets are elements, not part of this taxonomy
    for full in list(cats):
        if "Gtk.Widget" in g.chain(full):
            cats[full] = "widget"

    # --- type counts per namespace and category
    per = collections.defaultdict(collections.Counter)
    for full, c in cats.items():
        ns = full.split(".")[0]
        if ns in TARGET:
            per[ns][c] += 1
    order = ["record", "value", "object", "model", "async-infra", "transient", "needs-override", "enum", "flags",
             "callback", "alias", "widget"]
    print("== types by category (auto-classified from GIR; no hand list except record shapes)")
    print(f"{'ns':9}" + "".join(f"{o[:9]:>10}" for o in order))
    tot = collections.Counter()
    for ns in TARGET:
        print(f"{ns:9}" + "".join(f"{per[ns][o]:10}" for o in order))
        tot.update(per[ns])
    print(f"{'total':9}" + "".join(f"{tot[o]:10}" for o in order))

    # --- member coverage of bindable types
    mem = collections.defaultdict(collections.Counter)
    reasons = collections.Counter()
    excluded = collections.Counter()
    for full, c in cats.items():
        if c not in BINDABLE:
            continue
        tag, el, ns = g.t[full]
        if ns not in TARGET:
            continue
        for kind in ("method", "constructor", "function", "virtual-method"):
            for m in el.findall(C + kind):
                if kind == "virtual-method":
                    continue  # subclassing is out of scope for non-GUI types
                if skip(m):
                    excluded["deprecated/non-introspectable"] += 1
                    continue
                nm = m.get("name") or ""
                if nm in CORE_NAMES:
                    excluded["core (ref/unref/copy/free/qdata)"] += 1
                    continue
                if nm.endswith("_finish") and m.get(GL + "async-func") or nm.endswith("_finish"):
                    excluded["async finish (folded)"] += 1
                    continue
                r = callable_reason(g, m, ns, cats)
                bucket = "auto" if r is None else "blocked"
                mem[ns][bucket] += 1
                if r:
                    reasons[r.split(":")[0] + (":" + r.split(":")[1] if r.startswith("type:") else "")] += 1
        for p in el.findall(C + "property"):
            if skip(p):
                continue
            r = slot_ok(g, p, ns, cats)
            mem[ns]["auto" if r is None else "blocked"] += 1
            mem[ns]["props"] += 1
        for s in el.findall(GL + "signal"):
            if skip(s):
                continue
            ps = s.find(C + "parameters")
            r = None
            for p in (ps.findall(C + "parameter") if ps is not None else []):
                r = r or slot_ok(g, p, ns, cats)
            rv = s.find(C + "return-value")
            r = r or (slot_ok(g, rv, ns, cats) if rv is not None else None)
            mem[ns]["auto" if r is None else "blocked"] += 1
            mem[ns]["signals"] += 1
        if c == "record":
            mem[ns]["fields"] += len([f for f in el.findall(C + "field") if f.get("private") != "1"])
    print("\n== members of bindable types (record/value/object/model): auto = all slots marshal generically")
    allc = collections.Counter()
    for ns in TARGET:
        m = mem[ns]
        n = m["auto"] + m["blocked"]
        allc.update(m)
        if n:
            print(f"  {ns:9} members {n:5}  auto {m['auto']:5} ({100 * m['auto'] // n:3}%)  blocked {m['blocked']:4}   (props {m['props']}, signals {m['signals']}, record fields {m['fields']})")
    n = allc["auto"] + allc["blocked"]
    print(f"  {'all':9} members {n:5}  auto {allc['auto']:5} ({100 * allc['auto'] // n:3}%)  blocked {allc['blocked']:4}")
    print("  excluded automatically:", dict(excluded))
    print("  top blocking reasons:")
    for r, k in reasons.most_common(15):
        print(f"    {k:5}  {r}")

    # --- scope policy: closure of types reachable from the Gtk API
    def refs(full):
        tag, el, ns = g.t[full]
        out = set()
        for node in el.iter():
            if node.tag in (C + "type",):
                n = g.q(node.get("name"), ns)
                if n in cats:
                    out.add(n)
        return out

    gtk_api = {f for f, c in cats.items() if f.startswith("Gtk.")}
    direct = set()
    for f in gtk_api:
        direct |= {r for r in refs(f) if not r.startswith("Gtk.")}
    closure, todo = set(direct), list(direct)
    while todo:
        f = todo.pop()
        if cats.get(f) not in BINDABLE:
            continue
        for r in refs(f):
            if r not in closure and not r.startswith("Gtk."):
                closure.add(r)
                todo.append(r)
    print("\n== scope: non-Gtk types reachable from the Gtk API (bindable categories only)")
    for ns in TARGET[:-1]:
        allb = sum(1 for f, c in cats.items() if f.startswith(ns + ".") and c in BINDABLE)
        d = sum(1 for f in direct if f.startswith(ns + ".") and cats.get(f) in BINDABLE)
        cl = sum(1 for f in closure if f.startswith(ns + ".") and cats.get(f) in BINDABLE)
        print(f"  {ns:9} bindable {allb:4}  directly referenced {d:4}  transitive closure {cl:4}")

    print("\n== in-scope (closure) types per category, and the overrides they need")
    inscope = collections.Counter(cats[f] for f in closure)
    print("  ", dict(inscope))
    blockers = collections.Counter()
    for f in closure | gtk_api:
        if cats.get(f) not in BINDABLE:
            continue
        tag, el, ns = g.t[f]
        for kind in ("method", "constructor", "function"):
            for m in el.findall(C + kind):
                if skip(m) or (m.get("name") or "") in CORE_NAMES or (m.get("name") or "").endswith("_finish"):
                    continue
                r = callable_reason(g, m, ns, cats)
                if r and r.startswith("type:"):
                    blockers[r.split(":", 2)[2]] += 1
    print("   types blocking in-scope members:", len(blockers))
    print("  ", ", ".join(f"{k} {v}" for k, v in blockers.most_common(25)))

    # value types: immutable (every method takes `const T *self`) vs mutable boxed (builders, AttrList)
    imm, mut = [], []
    for f, c in cats.items():
        if c != "value" or f.split(".")[0] not in TARGET:
            continue
        el = g.t[f][1]
        selfs = [m.find(C + "parameters/" + C + "instance-parameter") for m in el.findall(C + "method") if not skip(m)
                 and (m.get("name") or "") not in CORE_NAMES]
        selfs = [x for x in selfs if x is not None]
        consts = [("const" in ((x.find(C + "type").get("c:type") or "") if x.find(C + "type") is not None else "")) for x in selfs]
        (imm if all(consts) else mut).append(f)
    print(f"\n== value types: {len(imm)} take `const self` everywhere, {len(mut)} do not (C const-ness is NOT a reliable mutability signal: GBytes, GskRenderNode are immutable yet non-const)")
    print("   in scope with mutators:", " ".join(sorted(x for x in mut if x in closure or x.startswith("Gtk."))))

    if a.show:
        print(f"\n== {a.show}:")
        print("  " + " ".join(sorted(f for f, c in cats.items() if c == a.show and f.split('.')[0] in TARGET)))
    if a.json:
        json.dump({"categories": cats}, open(a.json, "w"), indent=1, sort_keys=True)


if __name__ == "__main__":
    main()
