#!/usr/bin/env python3
"""Generate the QtWidgets svelters backend (C++17) plus verification tests and typings.

  python3 tools/metadata/qt/gen_cpp.py --config gen.toml --out <dir>

Inputs are only the config, metadata/qt/*.json, metadata/overrides/qt/*.yaml and the files in
tools/metadata/qt/ (this script and templates/). Output is byte-identical on rerun: no clocks,
no environment, no absolute paths in the output.

Config (TOML; relative paths resolve against the config file's directory):

  [input]
  metadata  = "metadata/qt"
  overrides = "metadata/overrides/qt"
  [output]
  backend = "svl_qt_backend.cpp"
  verify  = "svl_qt_verify.cpp"
  dts     = "qt.d.ts"
  [elements]
  tags = []          # restrict to these element tags (empty: every annotated element)

Elements come from the `elements:` annotation lists in the overrides (schema extension, see
docs/notes/p4-qt-lane.md). Every annotated fact is validated against the ingested metadata, and
the verification file holds one test per slot/ownership/signal/property/method fact.
"""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "common"))
import svmeta  # noqa: E402


class GenError(Exception):
    pass


# --------------------------------------------------------------------------------------------
# model

class Model:
    def __init__(self, meta_dir: Path, ov_dir: Path, only: list[str]):
        self.meta_dir, self.ov_dir = meta_dir, ov_dir
        self.read_files: list[Path] = []
        self._kinds: dict[str, dict] = {}
        self.elements: list[dict] = []
        for p in sorted(ov_dir.glob("*.yaml")):
            cls = p.stem
            self.read_files.append(p)
            ov = svmeta.load_yaml(p)
            for el in ov.get("elements", []):
                if only and el["tag"] not in only:
                    continue
                el = dict(el)
                el["cls"] = el["construct"]["class"]
                if el["cls"] != cls:
                    raise GenError(f"{p.name}: element {el['tag']} constructs {el['cls']}, expected {cls}")
                self.elements.append(el)
        self.elements.sort(key=lambda e: e["tag"])
        tags = [e["tag"] for e in self.elements]
        if len(set(tags)) != len(tags):
            raise GenError("duplicate element tags")
        self.by_tag = {e["tag"]: e for e in self.elements}
        for e in self.elements:
            self._resolve(e)
        self._assign_ids()

    # metadata access ---------------------------------------------------------------------
    def kind(self, cls: str) -> dict | None:
        if cls not in self._kinds:
            p = self.meta_dir / f"{cls}.json"
            if not p.exists():
                self._kinds[cls] = None
            else:
                self.read_files.append(p)
                self._kinds[cls] = svmeta.load_json(p)
        return self._kinds[cls]

    def chain(self, cls: str) -> list[str]:
        k = self.kind(cls)
        if k is None:
            raise GenError(f"no metadata for class {cls}")
        return [cls] + [b for b in k["bases"] if self.kind(b)]

    def derives(self, cls: str, base: str) -> bool:
        return base == cls or base in self.kind(cls)["bases"]

    def find(self, cls: str, section: str, pred):
        for c in self.chain(cls):
            for r in self.kind(c)[section]:
                if pred(r):
                    return c, r
        return None, None

    # resolution --------------------------------------------------------------------------
    def _resolve(self, el: dict) -> None:
        tag, cls = el["tag"], el["cls"]
        comp = el["construct"].get("composite", {})
        el["layout_cls"] = comp.get("layout", {}).get("class")
        if el["layout_cls"] and not self.kind(el["layout_cls"]):
            raise GenError(f"{tag}: no metadata for layout {el['layout_cls']}")
        for p in el.get("props", []):
            self._resolve_prop(el, p)
        for m in el.get("methods", []):
            self._resolve_method(el, m)
        for s in el.get("signals", []):
            self._resolve_signal(el, s)
        ch = el.get("children")
        if ch:
            for s in ch["slots"]:
                self._resolve_slot(el, s)
            if ch["default_slot"] not in [s["name"] for s in ch["slots"]]:
                raise GenError(f"{tag}: default_slot {ch['default_slot']} not in slots")

    def target_cls(self, el: dict, target: str | None) -> str:
        if target == "layout":
            return el["layout_cls"]
        return el["cls"]

    def _resolve_prop(self, el: dict, p: dict) -> None:
        tag = el["tag"]
        tcls = self.target_cls(el, p.get("target"))
        if p.get("type") == "enum":
            for key in ("getter", "setter"):
                self._need_method(tag, tcls, p[key], None)
            p["decl_ref"] = "enum"
            return
        dcls, prop = self.find(tcls, "properties", lambda r: r["name"] == p["native"])
        if not prop:
            raise GenError(f"{tag}: property {p['native']} not found on {tcls}")
        acc = prop["accessors"]
        p["decl_cls"], p["prop"] = dcls, prop
        p["decl_ref"] = prop["type"]["ref"]
        p["getter"], p["setter"] = acc.get("get"), acc.get("set")
        p["qprop"] = prop.get("metaobject") is not None
        direct = p["decl_ref"] in ("string", "bool", "i32", "u32", "i64", "f32", "f64") and p["getter"] and p["setter"]
        if direct:
            self._need_method(tag, tcls, p["getter"], 0)
            self._need_method(tag, tcls, p["setter"], 1)
        elif not p["qprop"]:
            raise GenError(f"{tag}: property {p['native']} has no direct accessors and no QMetaProperty")
        p["direct"] = bool(direct)

    def _need_method(self, tag: str, cls: str, name: str, nparams):
        c, m = self.find(cls, "methods", lambda r: r["name"] == name and (nparams is None or len(r["params"]) >= nparams
                                                                          and sum(1 for q in r["params"] if not q["default"]) <= nparams))
        if not m:
            raise GenError(f"{tag}: method {name}({nparams}) not found on {cls}")
        return c, m

    def _resolve_method(self, el: dict, m: dict) -> None:
        tag, cls = el["tag"], el["cls"]
        want = m.get("params")
        def match(r):
            if r["name"] != m["native"] or r["static"]:
                return False
            if want is None:
                return not [q for q in r["params"] if not q["default"]] or len(r["params"]) == 0
            return [q["type"]["ref"] for q in r["params"]] == want
        c, rec = self.find(cls, "methods", match)
        if not rec:
            raise GenError(f"{tag}: method {m['native']}{want or ''} not found on {cls}")
        m["rec"], m["decl_cls"] = rec, c
        m["ret"] = rec["returns"]["ref"]
        m["argrefs"] = want or []
        if m["ret"] not in ("void", "bool", "i32", "u32", "i64", "string", "f64"):
            raise GenError(f"{tag}: method {m['native']} returns unsupported {m['ret']}")

    def _resolve_signal(self, el: dict, s: dict) -> None:
        tag, cls = el["tag"], el["cls"]
        if s.get("synthetic"):
            # Raised by the backend itself, not by a QObject signal. Only `close_request` exists.
            if s["synthetic"] != "close_request":
                raise GenError(f"{tag}: unknown synthetic signal {s['synthetic']}")
            s.setdefault("args", [])
            s["decl_cls"], s["rec"], s["qparams"] = None, None, []
            return
        c, rec = self.find(cls, "signals", lambda r: r["name"] == s["native"])
        if not rec:
            raise GenError(f"{tag}: signal {s['native']} not found on {cls}")
        if s["connect"] not in rec["metaobject_signatures"]:
            raise GenError(f"{tag}: {s['connect']} not among metaobject signatures {rec['metaobject_signatures']}")
        for sig in s.get("collapses", []):
            if sig not in rec["metaobject_signatures"]:
                raise GenError(f"{tag}: collapsed overload {sig} not in metaobject")
        nconn = s["connect"].count(",") + 1 if s["connect"].split("(")[1] != ")" else 0
        if nconn != len(rec["params"]):
            raise GenError(f"{tag}: connect signature {s['connect']} arity differs from qdoc {rec['signature']}")
        s["decl_cls"], s["rec"] = c, rec
        s["qparams"] = [(q["name"], q["type"]["native"]) for q in rec["params"]]
        names = [n for n, _ in s["qparams"]]
        for a in s["args"]:
            if a["name"] not in names:
                raise GenError(f"{tag}: signal arg {a['name']} not a parameter of {rec['signature']}")

    _NATIVE = re.compile(r"^(\w+)::([\w<>*:, ]+?)\((.*)\)(?:\s+const)?$")

    def parse_native(self, where: str, native: str) -> tuple[str, str, int]:
        mt = self._NATIVE.match(native.strip())
        if not mt:
            raise GenError(f"{where}: cannot parse native symbol {native!r}")
        cls, name, params = mt.group(1), mt.group(2), mt.group(3).strip()
        arity = 0 if not params else params.count(",") + 1
        base = name.split("<")[0]
        if self.kind(cls) is not None and "<" not in name:
            self._need_method(where, cls, base, arity)
        return cls, name, arity

    def _op(self, where: str, host_cls: str, d: dict, arity_args: bool = True) -> dict:
        """Normalise a section 2.5.2 op descriptor into {method, args, on, then}."""
        cls, name, arity = self.parse_native(where, d["native"])
        on = d.get("receiver")
        recv = host_cls if on is None else "QWidget"
        if self.kind(cls) is not None and cls not in self.chain(recv) and not (on == "child"):
            raise GenError(f"{where}: {cls} is not a base of {recv}")
        args = d.get("args", [])
        if arity and "args" not in d:
            raise GenError(f"{where}: native {d['native']} takes {arity} argument(s); list them in args")
        return {"method": name, "args": args, "receiver": on,
                "then": [self._op(where + ".then", host_cls, t) for t in d.get("then", [])]}

    def _resolve_slot(self, el: dict, s: dict) -> None:
        tag = el["tag"]
        where = f"{tag}.{s['name']}"
        host_cls = el["layout_cls"] if s["host"] == "layout" else el["cls"]
        s["host_cls"] = host_cls
        for a in s["accepts"]:
            if not self.kind(a):
                raise GenError(f"{where}: unknown accepted class {a}")
        raw, ops = s["ops"], {}
        if s["cardinality"] == "one":
            for need in ("get", "set", "remove"):
                if need not in raw:
                    raise GenError(f"{where}: missing op {need}")
            ops["get"] = self._op(where + ".get", host_cls, raw["get"])
            ops["set"] = self._op(where + ".set", host_cls, raw["set"])
            rm = self._op(where + ".remove", host_cls, raw["remove"])
            ops["clear" if None in rm["args"] else "take"] = rm
            old = raw["set"].get("effects", {}).get("old")
            s["replace"] = {"deleted": "deletes_old", "unparented": "releases_old"}.get(old)
            rc = raw["remove"].get("effects", {}).get("child")
            s["remove_effect"] = {"deleted": "deletes_child", "unparented": "released"}.get(rc)
            if s["replace"] is None or s["remove_effect"] is None:
                raise GenError(f"{where}: set.effects.old must be deleted|unparented and remove.effects.child deleted|unparented")
            if (s.get("on_remove") == "destroys_child") != (s["remove_effect"] == "deletes_child"):
                raise GenError(f"{where}: on_remove disagrees with remove.effects.child")
            hits = [o for o in self.kind(el["cls"])["ownership"]
                    if o["fact"] == "replace_child" and o["method"].startswith(ops["set"]["method"] + "(")]
            s["typesystem_replace_child"] = hits[0]["get_old"] if hits else None
        else:
            for need in ("insert", "remove", "index_of", "list"):
                if need not in raw:
                    raise GenError(f"{where}: missing op {need}")
            ins = raw["insert"]
            ops["append"] = self._op(where + ".insert", host_cls, ins)
            var = [v for v in ins.get("variants", []) if v.get("when") == "before != null"]
            if not var:
                raise GenError(f"{where}: insert needs a `before != null` variant")
            ops["insert"] = self._op(where + ".insert.variant", host_cls, var[0])
            ops["remove"] = self._op(where + ".remove", host_cls, raw["remove"])
            ops["index_of"] = self._op(where + ".index_of", host_cls, raw["index_of"])
            lst = raw["list"]
            ops["count"] = self._op(where + ".list", host_cls, lst)
            _, item, _ = self.parse_native(where + ".list.item", lst["item"])
            ops["at"] = {"method": item, "args": ["index"], "receiver": None, "then": []}
            rc = raw["remove"].get("effects", {}).get("child")
            s["remove_effect"] = {"unparented": "released"}.get(rc)
            if s["remove_effect"] is None:
                raise GenError(f"{where}: remove.effects.child must be unparented")
            s["replace"] = None
        s["ops"] = ops

    def accepted_child_tag(self, el: dict, s: dict) -> str:
        for e in self.elements:
            if e.get("construct", {}).get("composite"):
                continue
            if all(self.derives(e["cls"], a) for a in s["accepts"]):
                return e["tag"]
        for e in self.elements:
            if any(self.derives(e["cls"], a) for a in s["accepts"]):
                return e["tag"]
        raise GenError(f"no element accepted by {el['tag']}.{s['name']}")

    def wrong_child_tag(self, s: dict) -> str | None:
        for e in self.elements:
            if not any(self.derives(e["cls"], a) for a in s["accepts"]):
                return e["tag"]
        return None

    # ids ---------------------------------------------------------------------------------
    def _assign_ids(self) -> None:
        self.kind_id = {e["tag"]: i + 1 for i, e in enumerate(self.elements)}
        self.ids = {"prop": {}, "signal": {}, "method": {}, "slot": {}}
        for e in self.elements:
            e["props_by_name"] = {p["js"]: p for p in e.get("props", [])}
            for key, items, field in (("prop", e.get("props", []), "js"), ("signal", e.get("signals", []), "js"),
                                      ("method", e.get("methods", []), "js"),
                                      ("slot", (e.get("children") or {}).get("slots", []), "name")):
                for it in items:
                    it["id"] = len(self.ids[key]) + 1
                    self.ids[key][(e["tag"], it[field])] = it["id"]

    def digest(self, extra: list[Path]) -> str:
        h = hashlib.sha256()
        for p in sorted(set(self.read_files) | set(extra), key=lambda p: (p.parent.name, p.name)):
            h.update(p.name.encode())
            h.update(p.read_bytes())
        return h.hexdigest()


# --------------------------------------------------------------------------------------------
# emit helpers

def cident(s: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in s)


def inc(model: Model, classes) -> str:
    seen = sorted({model.kind(c)["native"]["header"] for c in classes if model.kind(c)})
    return "\n".join(f"#include <QtWidgets/{h}>" for h in seen)


def host_expr(el: dict, s_host: str, obj: str = "o") -> str:
    if s_host == "layout":
        return f"static_cast<{el['layout_cls']} *>(static_cast<QWidget *>({obj})->layout())"
    return f"static_cast<{el['cls']} *>({obj})"


def target_expr(el: dict, p: dict, obj: str = "o") -> str:
    return host_expr(el, "layout" if p.get("target") == "layout" else "self", obj)


def conv_in(ref: str, src: str, var: str, what: str) -> tuple[str, str]:
    """(declaration+check, expression) converting svl_value* `src` to a C++ value."""
    if ref == "string":
        return f"QString {var}; if (!to_qstring({src}, {var})) return bad_type(\"{what}\", \"string\");", var
    if ref == "bool":
        return f"bool {var}; if (!to_bool({src}, {var})) return bad_type(\"{what}\", \"bool\");", var
    if ref in ("i32", "u32"):
        return f"int {var}; if (!to_i32({src}, {var})) return bad_type(\"{what}\", \"number\");", var
    if ref == "i64":
        return f"qint64 {var}; if (!to_i64({src}, {var})) return bad_type(\"{what}\", \"number\");", var
    if ref in ("f64", "f32"):
        return f"double {var}; if (!to_f64({src}, {var})) return bad_type(\"{what}\", \"number\");", var
    raise GenError(f"no input conversion for {ref}")


def conv_out(ref: str, expr: str, out: str = "out") -> str:
    return {"string": f"out_str({out}, {expr});", "bool": f"out_bool({out}, {expr});",
            "i32": f"out_i64({out}, {expr});", "u32": f"out_i64({out}, {expr});", "i64": f"out_i64({out}, {expr});",
            "f64": f"out_f64({out}, {expr});", "f32": f"out_f64({out}, {expr});"}[ref]


# --------------------------------------------------------------------------------------------
# backend emission

def emit_ids(m: Model) -> str:
    out = []
    for e in m.elements:
        out.append(f"constexpr svl_id K_{cident(e['tag'])} = {m.kind_id[e['tag']]};")
    for key, pre in (("prop", "P"), ("signal", "G"), ("method", "M"), ("slot", "S")):
        for (tag, name), i in sorted(m.ids[key].items(), key=lambda kv: kv[1]):
            out.append(f"constexpr svl_id {pre}_{cident(tag)}_{cident(name)} = {i};")
    return "\n".join(out)


def emit_tables(m: Model) -> str:
    out = []
    kinds = ", ".join(f'{{"{e["tag"]}", K_{cident(e["tag"])}}}' for e in m.elements)
    out.append(f"const NameId kKinds[] = {{{kinds}}};")
    names = []
    for e in m.elements:
        t = cident(e["tag"])
        for key, pre, items, f in (("prop", "P", e.get("props", []), "js"), ("signal", "G", e.get("signals", []), "js"),
                                   ("method", "M", e.get("methods", []), "js"),
                                   ("slot", "S", (e.get("children") or {}).get("slots", []), "name")):
            rows = ", ".join(f'{{"{it[f]}", {pre}_{t}_{cident(it[f])}}}' for it in items)
            out.append(f"const NameId k{pre}_{t}[] = {{{rows}{', ' if rows else ''}{{\"\", 0}}}};")
            names.append((key, e["tag"], f"k{pre}_{t}", len(items)))
    out.append("Table table_for(uint32_t what, svl_id kind) {")
    out.append(f"  if (what == SVL_KIND) return {{kKinds, {len(m.elements)}}};")
    out.append("  switch (what) {")
    for key, what in (("prop", "SVL_PROP"), ("signal", "SVL_SIGNAL"), ("method", "SVL_METHOD"), ("slot", "SVL_SLOT")):
        out.append(f"    case {what}:")
        out.append("      switch (kind) {")
        for k, tag, arr, n in names:
            if k == key:
                out.append(f"        case K_{cident(tag)}: return {{{arr}, {n}}};")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }\n  return {nullptr, 0};\n}")
    return "\n".join(out)


def emit_create(m: Model) -> str:
    out = ["QObject *gen_create(svl_id kind) {", "  switch (kind) {"]
    for e in m.elements:
        out.append(f"    case K_{cident(e['tag'])}: {{")
        out.append(f"      auto *w = new {e['cls']}();")
        for ini in e["construct"].get("init", []):
            iargs = ", ".join({True: "true", False: "false"}.get(a, str(a)) if isinstance(a, bool) else str(a)
                              for a in ini.get("args", []))
            out.append(f"      w->{ini['method']}({iargs});")
        lay = e["construct"].get("composite", {}).get("layout")
        if lay:
            args = ", ".join(lay.get("args", []) + ["w"])
            out.append(f"      new {lay['class']}({args});")
        out.append("      return w;\n    }")
    out.append("    default: return nullptr;\n  }\n}")
    return "\n".join(out)


def prop_cases(m: Model, setter: bool) -> str:
    sig = "int gen_set_prop(Entry &e, svl_id prop, const svl_value *v)" if setter else \
        "int gen_get_prop(Entry &e, svl_id prop, svl_value *out)"
    out = [sig + " {", "  QObject *o = e.obj.data();", "  (void)o;", "  switch (e.kind) {"]
    for e in m.elements:
        t = cident(e["tag"])
        out.append(f"    case K_{t}:\n      switch (prop) {{")
        for p in e.get("props", []):
            pid = f"P_{t}_{cident(p['js'])}"
            tgt = target_expr(e, p)
            out.append(f"        case {pid}: {{")
            if p["decl_ref"] == "enum":
                if setter:
                    out.append(f"          QString x; if (!to_qstring(v, x)) return bad_type(\"{p['js']}\", \"string\");")
                    out.append(f"          auto *t = {tgt};")
                    first = True
                    for name, cexpr in sorted(p["values"].items()):
                        out.append(f"          {'' if first else 'else '}if (x == QLatin1String(\"{name}\")) t->{p['setter']}({cexpr});")
                        first = False
                    allowed = "|".join(sorted(p["values"]))
                    out.append(f"          else return fail(\"{p['js']}: expected one of {allowed}\");")
                    out.append("          return 0;")
                else:
                    out.append(f"          auto *t = {tgt};")
                    for name, cexpr in sorted(p["values"].items()):
                        out.append(f"          if (t->{p['getter']}() == {cexpr}) {{ out_str(out, QStringLiteral(\"{name}\")); return 0; }}")
                    out.append("          return fail(\"" + p["js"] + ": unmapped enum value\");")
            elif p["direct"]:
                if setter:
                    decl, expr = conv_in(p["decl_ref"], "v", "x", p["js"])
                    out.append(f"          {decl}")
                    if p.get("suppress_signals"):
                        out.append(f"          QSignalBlocker blk({tgt});  // programmatic set must not echo back as an event")
                    out.append(f"          {tgt}->{p['setter']}({expr});")
                    out.append("          return 0;")
                else:
                    out.append(f"          {conv_out(p['decl_ref'], tgt + '->' + p['getter'] + '()')}")
                    out.append("          return 0;")
            else:  # QMetaProperty fallback
                if setter:
                    out.append(f"          return {tgt}->setProperty(\"{p['native']}\", to_variant(v)) ? 0 : fail(\"{p['js']}: setProperty failed\");")
                else:
                    out.append(f"          out_variant(out, {tgt}->property(\"{p['native']}\")); return 0;")
            out.append("        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("' + ("set_prop" if setter else "get_prop") + ': unknown property id for this kind");\n}')
    return "\n".join(out)


def emit_call(m: Model) -> str:
    out = ["int gen_call(Entry &e, svl_id method, const svl_value *args, size_t n, svl_value *out) {",
           "  QObject *o = e.obj.data();", "  (void)args; (void)out;", "  switch (e.kind) {"]
    for e in m.elements:
        t = cident(e["tag"])
        if not e.get("methods"):
            continue
        out.append(f"    case K_{t}:\n      switch (method) {{")
        for me in e["methods"]:
            n = len(me["argrefs"])
            out.append(f"        case M_{t}_{cident(me['js'])}: {{")
            out.append(f"          if (n != {n}) return fail(\"{me['js']}: expected {n} argument(s)\");")
            names = []
            for i, ref in enumerate(me["argrefs"]):
                decl, expr = conv_in(ref, f"&args[{i}]", f"a{i}", f"{me['js']} arg {i}")
                out.append(f"          {decl}")
                names.append(expr)
            call = f"static_cast<{e['cls']} *>(o)->{me['native']}({', '.join(names)})"
            if me["ret"] == "void":
                out.append(f"          {call};")
            else:
                out.append(f"          auto r = {call};")
                out.append(f"          {conv_out(me['ret'], 'r')}")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("call: unknown method id for this kind");\n}')
    return "\n".join(out)


def op_call(host: str, op: dict, child: str | None = None, extra: dict | None = None) -> str:
    """C++ statement(s) for a normalised op: the call plus its `then` steps (joined with `; `)."""
    def one(o: dict) -> str:
        args = []
        for a in o["args"]:
            if a == "child":
                args.append(child)
            elif a == "before":
                args.append((extra or {})["before"])
            elif a is None or a == "null":
                args.append("nullptr")
            elif a == "index":
                args.append((extra or {})["index"])
            elif isinstance(a, str) and a.startswith("const:"):
                args.append(a[6:])
            else:
                raise GenError(f"unknown op arg {a}")
        recv = child if o.get("receiver") == "child" else host
        return f"{recv}->{o['method']}({', '.join(args)})"
    return "; ".join([one(op)] + [one(t) for t in op["then"]])


def emit_insert(m: Model) -> str:
    out = ["int gen_insert(Entry &pe, QObject *child, QObject *before, svl_id slot) {",
           "  QObject *o = pe.obj.data();", "  (void)before;", "  switch (pe.kind) {"]
    for e in m.elements:
        ch = e.get("children")
        if not ch:
            continue
        t = cident(e["tag"])
        out.append(f"    case K_{t}:\n      if (slot == 0) slot = S_{t}_{cident(ch['default_slot'])};\n      switch (slot) {{")
        for s in ch["slots"]:
            ops = s["ops"]
            ctype = s["accepts"][0]
            out.append(f"        case S_{t}_{cident(s['name'])}: {{")
            out.append(f"          auto *ch = qobject_cast<{ctype} *>(child);")
            out.append(f"          if (!ch) return fail(\"slot {s['name']} of <{e['tag']}> does not accept this child\");")
            out.append(f"          auto *h = {host_expr(e, s['host'])};")
            if s["cardinality"] == "one":
                if s["replace"] == "releases_old" and "take" in ops:
                    out.append(f"          if (auto *old = {op_call('h', ops['get'])}; old && old != ch) {op_call('h', ops['take'])};")
                out.append(f"          {op_call('h', ops['set'], 'ch')};")
            else:
                out.append("          if (before == child) return 0;")
                out.append(f"          if ({op_call('h', ops['index_of'], 'ch')} >= 0) {{ {op_call('h', ops['remove'], 'ch')}; }}")
                out.append("          if (before) {")
                out.append(f"            auto *bw = qobject_cast<QWidget *>(before);")
                out.append(f"            int i = bw ? {op_call('h', ops['index_of'], 'bw')} : -1;")
                out.append(f"            if (i < 0) return fail(\"insert: `before` is not a child of this slot\");")
                out.append(f"            {op_call('h', ops['insert'], 'ch', {'index': 'i'})};")
                out.append(f"          }} else {{ {op_call('h', ops['append'], 'ch')}; }}")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("insert: unknown slot for this parent kind");\n}')
    return "\n".join(out)


def emit_remove(m: Model) -> str:
    out = ["int gen_remove(Entry &pe, QObject *child) {", "  QObject *o = pe.obj.data();", "  switch (pe.kind) {"]
    for e in m.elements:
        ch = e.get("children")
        if not ch:
            continue
        out.append(f"    case K_{cident(e['tag'])}: {{")
        for s in ch["slots"]:
            ops = s["ops"]
            ctype = s["accepts"][0]
            out.append(f"      if (auto *ch = qobject_cast<{ctype} *>(child)) {{")
            out.append(f"        auto *h = {host_expr(e, s['host'])};")
            if s["cardinality"] == "one":
                out.append(f"        if (static_cast<QWidget *>(o)->isAncestorOf(ch) && {op_call('h', ops['get'])} == ch) {{")
                if "take" in ops:
                    out.append(f"          {op_call('h', ops['take'])};")
                else:
                    out.append(f"          {op_call('h', ops['clear'])};")
                out.append("          return 0;\n        }")
            else:
                out.append(f"        if ({op_call('h', ops['index_of'], 'ch')} >= 0) {{")
                out.append(f"          {op_call('h', ops['remove'], 'ch')};")
                out.append("          return 0;\n        }")
            out.append("      }")
        out.append("      break;\n    }")
    out.append("    default: break;\n  }")
    out.append('  return fail("remove: child is not in this parent");\n}')
    return "\n".join(out)


def emit_listen(m: Model) -> str:
    out = ["int gen_listen(svl_handle h, Entry &e, svl_id sig, bool on) {", "  QObject *o = e.obj.data();",
           "  auto key = std::make_pair(h, sig);", "  switch (e.kind) {"]
    for e in m.elements:
        if not e.get("signals"):
            continue
        t = cident(e["tag"])
        out.append(f"    case K_{t}:\n      switch (sig) {{")
        for s in e["signals"]:
            sid = f"G_{t}_{cident(s['js'])}"
            if s.get("synthetic"):
                # onRequestClose: while listened, the backend vetoes the native close and posts it.
                out.append(f"        case {sid}: {{ e.close_listen = on; e.close_sig = sig; return 0; }}")
                continue
            ptypes = ", ".join(tn for _, tn in s["qparams"])
            lam_params = ", ".join(f"{tn} a{i}" for i, (_, tn) in enumerate(s["qparams"]))
            out.append(f"        case {sid}: {{")
            out.append("          if (!on) {")
            out.append("            auto it = g.listeners.find(key);")
            out.append("            if (it != g.listeners.end()) { QObject::disconnect(it->second); g.listeners.erase(it); }")
            out.append("            return 0;\n          }")
            out.append("          if (g.listeners.count(key)) return 0;")
            out.append(f"          auto *w = static_cast<{s['decl_cls']} *>(o);")
            body = []
            if s["args"]:
                body.append("svl_value vals[%d]; QByteArray bufs[%d];" % (len(s["args"]), len(s["args"])))
                names = [n for n, _ in s["qparams"]]
                for i, a in enumerate(s["args"]):
                    pi = names.index(a["name"])
                    if a["type"] != "string":
                        raise GenError(f"signal arg type {a['type']} unsupported")
                    body.append(f"bufs[{i}] = a{pi}.toUtf8(); vals[{i}].tag = SVL_STR; vals[{i}].u.s.ptr = bufs[{i}].constData(); vals[{i}].u.s.len = static_cast<size_t>(bufs[{i}].size());")
                body.append(f"post(h, {sid}, vals, {len(s['args'])});")
            else:
                body.append(f"post(h, {sid}, nullptr, 0);")
            out.append(f"          g.listeners[key] = QObject::connect(w, static_cast<void ({s['decl_cls']}::*)({ptypes})>(&{s['decl_cls']}::{s['native']}), w,")
            out.append(f"              [h]({lam_params}) {{ {' '.join(body)} }});")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("listen: unknown signal id for this kind");\n}')
    return "\n".join(out)


def gen_backend(m: Model, digest: str, tmpl: str) -> str:
    classes = set()
    for e in m.elements:
        classes.add(e["cls"])
        if e["layout_cls"]:
            classes.add(e["layout_cls"])
        for s in e.get("signals", []):
            if s["decl_cls"]:
                classes.add(s["decl_cls"])
        for s in (e.get("children") or {}).get("slots", []):
            classes.update(s["accepts"])
    parts = {
        "DIGEST": digest, "INCLUDES": inc(m, classes), "IDS": emit_ids(m), "TABLES": emit_tables(m),
        "CREATE": emit_create(m), "SET_PROP": prop_cases(m, True), "GET_PROP": prop_cases(m, False),
        "CALL": emit_call(m), "INSERT": emit_insert(m), "REMOVE": emit_remove(m), "LISTEN": emit_listen(m),
    }
    for k, v in parts.items():
        tmpl = tmpl.replace(f"@@{k}@@" if k == "DIGEST" else f"//@@{k}@@", v)
    return tmpl


# --------------------------------------------------------------------------------------------
# verification tests

class Tests:
    def __init__(self, m: Model):
        self.m = m
        self.fns: list[tuple[str, str]] = []
        self.src: list[str] = []

    def add(self, name: str, body: str) -> None:
        ident = "t_" + cident(name)
        self.fns.append((name, ident))
        self.src.append(f"bool {ident}(std::string &why) {{\n{body}\n  return true;\n}}\n")

    def build(self) -> tuple[str, str]:
        m = self.m
        for e in m.elements:
            self.props(e)
            self.methods(e)
            self.signals(e)
            ch = e.get("children")
            if ch:
                for s in ch["slots"]:
                    if s["cardinality"] == "one":
                        self.slot_one(e, s)
                    else:
                        self.slot_many(e, s)
        return "\n".join(self.src), "\n".join(f'    {{"{n}", {i}}},' for n, i in self.fns)

    # helpers
    def mkparent(self, e): return f'  svl_handle p = mk("{e["tag"]}");'

    def props(self, e):
        t = e["tag"]
        for p in e.get("props", []):
            pn = p["js"]
            tgt = target_expr(e, p, "O(h)")
            if p["decl_ref"] == "enum":
                lines = [f'  svl_handle h = mk("{t}");']
                for name, cexpr in sorted(p["values"].items()):
                    lines += [f'  {{ std::string s = "{name}"; svl_value v = vstr(s);',
                              f'    CHECK(B->set_prop(B->self, h, P("{t}", "{pn}"), &v) == 0);',
                              f'    CHECK({tgt}->{p["getter"]}() == {cexpr});',
                              f'    CHECK(getstr(h, P("{t}", "{pn}")) == "{name}"); }}']
                lines += [f'  {{ std::string s = "bogus"; svl_value v = vstr(s); CHECK(B->set_prop(B->self, h, P("{t}", "{pn}"), &v) < 0); }}',
                          "  B->destroy(B->self, h);"]
            elif p["decl_ref"] == "string":
                lines = [f'  svl_handle h = mk("{t}");', '  std::string s = "h\\xc3\\xa9llo";', "  svl_value v = vstr(s);",
                         f'  CHECK(B->set_prop(B->self, h, P("{t}", "{pn}"), &v) == 0);',
                         f'  CHECK({tgt}->{p["getter"]}().toUtf8().toStdString() == s);',
                         f'  CHECK(getstr(h, P("{t}", "{pn}")) == s);', "  B->destroy(B->self, h);"]
            else:
                lines = [f'  svl_handle h = mk("{t}");', "  svl_value v = vi64(7);",
                         f'  CHECK(B->set_prop(B->self, h, P("{t}", "{pn}"), &v) == 0);',
                         f'  CHECK({tgt}->{p["getter"]}() == 7);',
                         '  svl_value out{};',
                         f'  CHECK(B->get_prop(B->self, h, P("{t}", "{pn}"), &out) == 0 && out.u.i == 7);', "  B->destroy(B->self, h);"]
            self.add(f"prop/{t}.{pn}", "\n".join(lines))
            if p.get("suppress_signals") and e.get("signals"):
                lines = [f'  svl_handle h = mk("{t}");', '  std::string s = "echo"; svl_value v = vstr(s);']
                for sg in e["signals"]:
                    lines.append(f'  CHECK(B->listen(B->self, h, G("{t}", "{sg["js"]}"), true) == 0);')
                lines += [f'  CHECK(B->set_prop(B->self, h, P("{t}", "{pn}"), &v) == 0);',
                          f'  CHECK({tgt}->{p["getter"]}().toStdString() == s);',
                          "  CHECK(rec.events.empty());  // no echo for programmatic sets", "  B->destroy(B->self, h);"]
                self.add(f"signal/{t}.{pn}/no_echo_on_set_prop", "\n".join(lines))

    def methods(self, e):
        t = e["tag"]
        for me in e.get("methods", []):
            args = ", ".join(f"vi64({i + 10})" for i, _ in enumerate(me["argrefs"]))
            lines = [f'  svl_handle h = mk("{t}");']
            if me["argrefs"]:
                lines.append(f"  svl_value a[] = {{{args}}};")
            lines += [f'  CHECK(B->call(B->self, h, M("{t}", "{me["js"]}"), {"a" if me["argrefs"] else "nullptr"}, {len(me["argrefs"])}, nullptr) == 0);',
                      f'  CHECK(B->call(B->self, h, M("{t}", "{me["js"]}"), nullptr, {len(me["argrefs"]) + 1}, nullptr) < 0);',
                      "  B->destroy(B->self, h);"]
            self.add(f"method/{t}.{me['js']}", "\n".join(lines))

    def signals(self, e):
        t = e["tag"]
        for s in e.get("signals", []):
            if s.get("synthetic"):
                continue  # covered by the hand-written close tests in verify.cpp.in
            trig = lambda i: s["trigger"].replace("$w", f"static_cast<{e['cls']} *>(O(h))").replace("$i", str(i))
            expect = s.get("expect_args", [])
            lines = [f'  svl_handle h = mk("{t}");', f'  svl_id sig = G("{t}", "{s["js"]}");',
                     "  CHECK(sig != 0);",
                     "  CHECK(B->listen(B->self, h, sig, true) == 0);",
                     "  CHECK(B->listen(B->self, h, sig, true) == 0);  // idempotent: must not double-connect",
                     f"  {trig(0)};",
                     "  CHECK(rec.events.size() == 1);  // delivered exactly once",
                     "  CHECK(rec.events[0].h == h && rec.events[0].sig == sig);",
                     f"  CHECK(rec.events[0].n == {len(s['args'])});"]
            for i, v in enumerate(expect):
                lines.append(f'  CHECK(rec.events[0].strs[{i}] == "{v}");')
            lines += ["  CHECK(B->listen(B->self, h, sig, false) == 0);", f"  {trig(1)};",
                      "  CHECK(rec.events.size() == 1);  // no delivery after listen(off)",
                      "  CHECK(B->listen(B->self, h, sig, true) == 0);", f"  {trig(2)};",
                      "  CHECK(rec.events.size() == 2);",
                      "  B->destroy(B->self, h);"]
            self.add(f"signal/{t}.{s['js']}/delivered_once", "\n".join(lines))

    def slot_one(self, e, s):
        m, t, sn = self.m, e["tag"], s["name"]
        child = m.accepted_child_tag(e, s)
        ops, sid = s["ops"], f'S("{t}", "{sn}")'
        get = lambda p: op_call(host_expr(e, s["host"], f"O({p})"), ops["get"])
        base = [f'  svl_handle p = mk("{t}"); svl_handle a = mk("{child}"); svl_handle b = mk("{child}");']
        ts = s.get("typesystem_replace_child")
        note = f" (typesystem replace_child via {ts})" if ts else " (no typesystem entry)"
        self.add(f"slot/{t}.{sn}/set_get", "\n".join(base + [
            f"  // set then read back through {ops['get']['method']}{note}",
            f"  CHECK(B->insert(B->self, p, a, 0, {sid}) == 0);",
            f"  CHECK({get('p')} == O(a));",
            f"  CHECK(static_cast<QWidget *>(O(p))->isAncestorOf(static_cast<QWidget *>(O(a))));",
            "  B->destroy(B->self, p); B->destroy(B->self, a); B->destroy(B->self, b);"]))
        if sn == ((e.get("children") or {}).get("default_slot")):
            self.add(f"slot/{t}.{sn}/default_slot_is_slot_0", "\n".join(base + [
                "  CHECK(B->insert(B->self, p, a, 0, 0) == 0);", f"  CHECK({get('p')} == O(a));",
                "  B->destroy(B->self, p); B->destroy(B->self, a); B->destroy(B->self, b);"]))
        old_alive = "!pa.isNull() && pa->parent() == nullptr" if s["replace"] == "releases_old" else "pa.isNull()"
        extra = ["  CHECK(was_destroyed(a));"] if s["replace"] == "deletes_old" else []
        self.add(f"ownership/{t}.{sn}/replace_{s['replace']}", "\n".join(base + [
            f"  CHECK(B->insert(B->self, p, a, 0, {sid}) == 0);",
            "  QPointer<QObject> pa(O(a));",
            f"  CHECK(B->insert(B->self, p, b, 0, {sid}) == 0);",
            "  flush();",
            f"  CHECK({old_alive});"] + extra + [
            f"  CHECK({get('p')} == O(b));",
            "  B->destroy(B->self, p); B->destroy(B->self, a); B->destroy(B->self, b);"]))
        if s["remove_effect"] == "released":
            check = ["  CHECK(!pa.isNull());", "  CHECK(O(a)->parent() == nullptr);", "  CHECK(!was_destroyed(a));"]
        else:
            check = ["  CHECK(pa.isNull());", "  CHECK(was_destroyed(a));"]
        self.add(f"ownership/{t}.{sn}/remove_{s['remove_effect']}", "\n".join(base + [
            f"  CHECK(B->insert(B->self, p, a, 0, {sid}) == 0);",
            "  QPointer<QObject> pa(O(a));",
            "  QObject *ra = O(a);",
            "  CHECK(B->remove(B->self, p, a) == 0);",
            "  flush();",
            f"  CHECK({get('p')} != ra);"] + check + [
            "  CHECK(B->remove(B->self, p, a) < 0);  // second remove: not a child any more",
            "  B->destroy(B->self, p); B->destroy(B->self, a); B->destroy(B->self, b);"]))
        wrong = m.wrong_child_tag(s)
        if wrong:
            self.add(f"slot/{t}.{sn}/rejects_{wrong}", "\n".join([
                f'  svl_handle p = mk("{t}"); svl_handle w = mk("{wrong}");',
                f"  CHECK(B->insert(B->self, p, w, 0, {sid}) < 0);",
                "  B->destroy(B->self, p); B->destroy(B->self, w);"]))

    def slot_many(self, e, s):
        m, t, sn = self.m, e["tag"], s["name"]
        child = m.accepted_child_tag(e, s)
        ops, sid = s["ops"], f'S("{t}", "{sn}")'
        host = lambda p: host_expr(e, s["host"], f"O({p})")
        at = lambda p, i: f"{host(p)}->{ops['at']['method']}({i})->widget()"
        cnt = lambda p: op_call(host(p), ops['count'])
        mk3 = [f'  svl_handle p = mk("{t}"); svl_handle c0 = mk("{child}"); svl_handle c1 = mk("{child}"); svl_handle c2 = mk("{child}");']
        clean = "  B->destroy(B->self, p); B->destroy(B->self, c0); B->destroy(B->self, c1); B->destroy(B->self, c2);"
        self.add(f"slot/{t}.{sn}/order", "\n".join(mk3 + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c2, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c1, c2, {sid}) == 0);  // before=c2",
            f"  CHECK({cnt('p')} == 3);",
            f"  CHECK({at('p', 0)} == O(c0) && {at('p', 1)} == O(c1) && {at('p', 2)} == O(c2));",
            f"  CHECK(O(c1)->parent() == O(p));",
            f"  CHECK(B->insert(B->self, p, c2, c0, {sid}) == 0);  // move existing child to the front",
            f"  CHECK({cnt('p')} == 3);",
            f"  CHECK({at('p', 0)} == O(c2) && {at('p', 1)} == O(c0) && {at('p', 2)} == O(c1));",
            f"  CHECK(B->insert(B->self, p, c0, c0, {sid}) == 0);  // before itself: unchanged",
            f"  CHECK({at('p', 1)} == O(c0));",
            clean]))
        self.add(f"slot/{t}.{sn}/default_slot_is_slot_0", "\n".join(mk3 + [
            "  CHECK(B->insert(B->self, p, c0, 0, 0) == 0);", f"  CHECK({at('p', 0)} == O(c0));", clean]))
        self.add(f"ownership/{t}.{sn}/remove_{s['remove_effect']}", "\n".join(mk3 + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c1, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c2, 0, {sid}) == 0);",
            "  QPointer<QObject> p1(O(c1));",
            "  CHECK(B->remove(B->self, p, c1) == 0);",
            "  flush();",
            f"  CHECK({cnt('p')} == 2);",
            f"  CHECK({at('p', 0)} == O(c0) && {at('p', 1)} == O(c2));",
            "  CHECK(!p1.isNull() && p1->parent() == nullptr);  // released, not deleted",
            "  CHECK(!was_destroyed(c1));",
            "  CHECK(B->remove(B->self, p, c1) < 0);",
            f"  CHECK(B->insert(B->self, p, c1, 0, {sid}) == 0);  // a released child can be inserted again",
            f"  CHECK({cnt('p')} == 3 && {at('p', 2)} == O(c1));",
            clean]))
        self.add(f"ownership/{t}.{sn}/parent_destroy_deletes_children", "\n".join(mk3 + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            "  QPointer<QObject> pc(O(c0));",
            "  B->destroy(B->self, p);",
            "  flush();",
            "  CHECK(pc.isNull());",
            "  CHECK(was_destroyed(c0));",
            clean]))


def gen_verify(m: Model, digest: str, tmpl: str) -> str:
    tests, table = Tests(m).build()
    classes = {e["cls"] for e in m.elements} | {e["layout_cls"] for e in m.elements if e["layout_cls"]}
    simple = [e for e in m.elements if not e.get("construct", {}).get("composite")]
    parent = next((e for e in m.elements if e.get("children") and e["children"]["default_slot"]
                   and any(s["name"] == e["children"]["default_slot"] and "QWidget" in s["accepts"]
                           for s in e["children"]["slots"])), None)
    if not parent:
        raise GenError("need an element with a QWidget default slot")
    val = next((e for e in simple for p in e.get("props", []) if p["decl_ref"] == "string"), None)
    if not val:
        raise GenError("need an element with a string prop")
    any_tag = next(e["tag"] for e in simple if e["tag"] == "label") if any(e["tag"] == "label" for e in simple) else simple[0]["tag"]
    child_tag = m.accepted_child_tag(parent, next(s for s in parent["children"]["slots"] if s["name"] == parent["children"]["default_slot"]))
    prop = next(p for p in val["props"] if p["decl_ref"] == "string")
    subs = {"@@ANY_TAG@@": any_tag, "@@PARENT_TAG@@": parent["tag"], "@@CHILD_TAG@@": child_tag,
            "@@VALUE_TAG@@": val["tag"], "@@VALUE_PROP@@": prop["js"], "@@DIGEST@@": digest}
    out = tmpl.replace("//@@INCLUDES@@", inc(m, classes)).replace("//@@TESTS@@", tests).replace("//@@TEST_TABLE@@", table)
    for k, v in subs.items():
        out = out.replace(k, v)
    return out


# --------------------------------------------------------------------------------------------
# typings

def ts_type(p: dict) -> str:
    r = p["decl_ref"]
    if r == "enum":
        return " | ".join(f'"{k}"' for k in sorted(p["values"]))
    return {"string": "string", "bool": "boolean"}.get(r, "number")


def gen_dts(m: Model) -> str:
    out = ["// GENERATED by tools/metadata/qt/gen_cpp.py -- do not edit.", "",
           "export interface QtElementDescriptors {"]
    for e in m.elements:
        out.append(f"  {e['tag']}: {{")
        out.append("    props: {")
        for p in e.get("props", []):
            out.append(f"      {p['js']}?: {ts_type(p)};")
        out.append("    };")
        out.append("    events: {")
        for s in e.get("signals", []):
            args = ", ".join(f"{a['name']}: {ts_type({'decl_ref': a['type']})}" for a in s["args"])
            out.append(f"      {s['js']}: ({args}) => void;")
        out.append("    };")
        out.append("    methods: {")
        for me in e.get("methods", []):
            args = ", ".join(f"arg{i}: number" for i, _ in enumerate(me["argrefs"]))
            out.append(f"      {me['js']}({args}): void;")
        out.append("    };")
        ch = e.get("children")
        out.append("    slots: " + (" | ".join(f'"{s["name"]}"' for s in ch["slots"]) if ch else "never") + ";")
        out.append("  };")
    out.append("}")
    out.append("")
    out.append("export type QtElementTag = keyof QtElementDescriptors;")
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------------------------

def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_text() == text:
        return
    path.write_text(text)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--dts", help="also write the .d.ts to this stable path (only rewritten when it changes)")
    a = ap.parse_args()
    cfgp = Path(a.config).resolve()
    cfg = tomllib.loads(cfgp.read_text())
    base = cfgp.parent
    res = lambda s: (base / s).resolve()
    out = Path(a.out)
    try:
        m = Model(res(cfg["input"]["metadata"]), res(cfg["input"]["overrides"]), cfg.get("elements", {}).get("tags", []))
        templ = HERE / "templates"
        digest = m.digest([HERE / "gen_cpp.py", templ / "backend.cpp.in", templ / "verify.cpp.in"])
        o = cfg["output"]
        write_text(out / o["backend"], gen_backend(m, digest, (templ / "backend.cpp.in").read_text()))
        write_text(out / o["verify"], gen_verify(m, digest, (templ / "verify.cpp.in").read_text()))
        write_text(out / o["dts"], gen_dts(m))
        if a.dts:
            write_text(Path(a.dts), gen_dts(m))
    except GenError as ex:
        print(f"gen_cpp: {ex}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
