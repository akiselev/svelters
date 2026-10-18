#!/usr/bin/env python3
"""Hide list for VALUE-type mutators, derived from metadata; reports the residual hand list.

Decision (user, 2026-10-17): VALUE handles hide their mutating methods. This script derives the
hide list per toolkit from what the metadata says, and counts what is left undecided:

GTK (GIR, value types from types_gir_classify.py)
  keep   instance-parameter `const T *`, or a reading name (get_/is_/has_/to_/equal/hash/compare/
         contains/lookup/find/n_/copy/dup/print/serialize/ref-less query)
  hide   a mutating name (set_/append/prepend/insert/remove/add/clear/reset/change/merge/splice/
         take/steal/push/pop/update/init/unset/replace/filter/sort/scale/translate/rotate/...)
  rest   non-const self, no telling name                 -> residual hand list
Qt (qdoc, value types from types_qt_classify.py)
  keep   `... const` member functions, statics
  hide   non-const member functions (C++ const-correctness; non-const overloads that have a const
         twin are dropped as duplicates)
  rest   non-const functions with a reading name (qdoc lists a few non-const getters) -> residual
Apple (objc2, value classes from types_objc2_classify.py)
  keep   everything on the immutable half of an NSMutable* pair (immutable by API design)
  hide   on other value classes: property setters ("Setter for"), and set*/add*/remove*/insert*/
         replace*/append* selectors
  rest   none by construction; misclassified classes are an override of the class, not of members

Usage: python3 -I types_value_mutators.py [--show-residual]
"""
import argparse
import collections
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import types_gir_classify as G  # noqa: E402
import types_qt_classify as Q  # noqa: E402
import types_objc2_classify as A  # noqa: E402

MUTATE = re.compile(r"^(set|append|prepend|insert|remove|add|clear|reset|change|merge|splice|take|steal|push|pop|"
                    r"update|init|unset|replace|filter|sort|scale|translate|rotate|transform|multiply|invert|"
                    r"normalize|negate|truncate|resize|fill|assign|swap|move|shrink|grow|union|intersect|subtract|"
                    r"offset|expand|inset|round|detach|squeeze|reserve|chop|prepend|erase|emplace)(_|$|[A-Z])")
READ = re.compile(r"^(get|is|has|to|equal|equals|hash|compare|contains|lookup|find|n|count|copy|dup|print|serialize|"
                  r"query|peek|iter|foreach|ref|match|parse|load|save|write|dump|as|can|bounds|area|distance|"
                  r"length|size|width|height|interpolate|intersects|near|dot|cross|is_|has_|format|matches|difference|equivalent|"
                  r"in|index|x|evaluate|includes|triggers|exists|generate|classify|check|range|enumerate|last|"
                  r"create|draw)(_|$|[A-Z])")


def gtk():
    g = G.Gir("/usr/share/gir-1.0")
    cats = {}
    for full, (tag, el, ns) in g.t.items():
        if not G.skip(el):
            c = G.classify(g, full)
            if c:
                cats[full] = c
    # scope: Gtk plus the closure of non-Gtk types reachable from the Gtk API (as in types_gir_classify)
    def refs(full):
        tag, el, ns = g.t[full]
        return {g.q(n.get("name"), ns) for n in el.iter(G.C + "type")} & cats.keys()
    closure = set()
    todo = [r for f in cats if f.startswith("Gtk.") for r in refs(f) if not r.startswith("Gtk.")]
    while todo:
        f = todo.pop()
        if f in closure:
            continue
        closure.add(f)
        if cats.get(f) in G.BINDABLE:
            todo += [r for r in refs(f) if not r.startswith("Gtk.")]
    keep = hide = 0
    residual = collections.defaultdict(list)
    builders = 0
    for full, c in cats.items():
        if c != "value" or full.split(".")[0] not in G.TARGET:
            continue
        if not (full.startswith("Gtk.") or full in closure):
            continue
        if full.endswith("Builder"):
            builders += 1  # builders exist to be mutated: a local OWNED object, mutators kept
            continue
        el = g.t[full][1]
        for m in el.findall(G.C + "method"):
            nm = m.get("name") or ""
            if G.skip(m) or nm in G.CORE_NAMES or nm.endswith("_finish"):
                continue
            ip = m.find(G.C + "parameters/" + G.C + "instance-parameter")
            ct = (ip.find(G.C + "type").get(G.CNS + "type") if ip is not None and ip.find(G.C + "type") is not None else "") or ""
            if ip is not None and ip.get("transfer-ownership") == "full":
                keep += 1  # consumes self and returns a new value (GskTransform chaining): the core refs first
            elif "const" in ct or READ.match(nm):
                keep += 1
            elif MUTATE.match(nm):
                hide += 1
            else:
                residual[full].append(nm)
    print(f"GTK  (in scope: Gtk + closure) builders kept mutable: {builders}")
    return keep, hide, residual


def qt():
    classes, enums, funcs, props = Q.load_qdoc()
    ts, removed, rejected, ownership = Q.load_typesystem()
    moc_objs, gadgets = Q.load_moc()
    keep = hide = dup = 0
    residual = collections.defaultdict(list)
    for n in classes:
        if "::" in n or Q.classify(n, classes, ts, moc_objs, funcs) not in ("value", "record"):
            continue
        fs = [f for f in funcs.get(n, []) if f.get("meta") in ("plain", "slot") and f.get("status") != "deprecated"
              and not (f.get("name") or "").startswith("operator")]
        const_names = {f.get("name") for f in fs if (f.get("signature") or "").rstrip().endswith("const")}
        for f in fs:
            sig = (f.get("signature") or "").rstrip()
            nm = f.get("name") or ""
            if f.get("static") == "true" or sig.endswith("const"):
                keep += 1
            elif nm in const_names and not MUTATE.match(nm):
                dup += 1  # non-const twin of a const accessor (begin/end/data/first ...)
            elif READ.match(nm) and not MUTATE.match(nm):
                residual[n].append(nm)
            else:
                hide += 1
    return keep, hide, dup, residual


def apple():
    import glob
    reg = os.path.expanduser("~/.cargo/registry/src")
    classes, copying, setters, sels = {}, set(), collections.Counter(), collections.defaultdict(list)
    for crate in A.CRATES:
        root = glob.glob(f"{reg}/*/{crate}/src/generated")[0]
        for f in glob.glob(f"{root}/*.rs"):
            src = open(f).read()
            for m in A.CLASS_RE.finditer(src):
                classes[m.group(3)] = ([s.strip() for s in m.group(1).split(",")], "MainThreadOnly" in m.group(2))
            copying |= set(re.findall(r"unsafe impl NSCopying for (\w+)", src))
            impls = [(m.start(), m.group(1)) for m in A.IMPL_RE.finditer(src)]
            for m in A.METHOD_RE.finditer(src):
                owner = None
                for pos, name in impls:
                    if pos < m.start():
                        owner = name
                    else:
                        break
                if owner:
                    is_setter = "Setter for" in src[max(0, m.start() - 300):m.start()].split("#[unsafe(method(")[-1]
                    sels[owner].append((m.group(1), is_setter))
                    setters[owner] += is_setter
    keep = hide = 0
    pair_values = 0
    for n, (sup, main_only) in classes.items():
        if n in A.WIDGET_ROOTS or any(s in A.WIDGET_ROOTS for s in sup) or n.startswith("NSMutable"):
            continue
        paired = f"NSMutable{n[2:]}" in classes
        value = paired or (n in copying and setters[n] == 0 and not main_only)
        if not value:
            continue
        pair_values += paired
        for sel, is_setter in sels[n]:
            if not paired and (is_setter or re.match(r"(set|add|remove|insert|replace|append)[A-Z]", sel)):
                hide += 1
            else:
                keep += 1
    return keep, hide, pair_values


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--show-residual", action="store_true")
    a = ap.parse_args()
    k, h, res = gtk()
    nres = sum(len(v) for v in res.values())
    print(f"GTK  value methods: keep {k}, hide {h}, residual {nres} in {len(res)} types")
    if a.show_residual:
        for t, ms in sorted(res.items(), key=lambda x: -len(x[1])):
            print(f"   {t}: {' '.join(ms)}")
    k, h, d, res = qt()
    nres = sum(len(v) for v in res.values())
    print(f"Qt   value methods: keep {k} (const/static), hide {h} (non-const), non-const twins dropped {d}, residual {nres} in {len(res)} types")
    if a.show_residual:
        for t, ms in sorted(res.items(), key=lambda x: -len(x[1])):
            print(f"   {t}: {' '.join(ms)}")
    k, h, p = apple()
    print(f"Apple value methods: keep {k}, hide {h}; {p} value classes are the immutable half of an NSMutable pair (no member decisions)")


if __name__ == "__main__":
    main()
