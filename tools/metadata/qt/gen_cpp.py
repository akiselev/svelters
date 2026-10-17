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
import copy
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "common"))
import svmeta  # noqa: E402
from tsvalues import write_values_dts  # noqa: E402


class GenError(Exception):
    pass



# --------------------------------------------------------------------------------------------
# value model: classification of metadata type refs, enum member naming, C++ spellings

INT_REFS = {"i32", "u32", "i64", "u64"}
UNSUPPORTED_REFS = {"opaque", "list", "void"}
GUI_HEADERS = {"QIcon": "QtGui/QIcon", "QFont": "QtGui/QFont", "QPixmap": "QtGui/QPixmap", "QImage": "QtGui/QImage",
               "QBrush": "QtGui/QBrush", "QPalette": "QtGui/QPalette", "QCursor": "QtGui/QCursor",
               "QKeySequence": "QtGui/QKeySequence", "QRegion": "QtGui/QRegion", "QPicture": "QtGui/QPicture",
               "QPainterPath": "QtGui/QPainterPath", "QTextCharFormat": "QtGui/QTextCharFormat",
               "QTextFormat": "QtGui/QTextFormat", "QTextCursor": "QtGui/QTextCursor", "QTextOption": "QtGui/QTextOption",
               "QPen": "QtGui/QPen", "QTransform": "QtGui/QTransform", "QBitmap": "QtGui/QBitmap",
               "QUrl": "QtCore/QUrl", "QLocale": "QtCore/QLocale", "QSizePolicy": "QtWidgets/QSizePolicy",
               "Qt": "QtCore/Qt"}


def classify(ref: str):
    """(category, payload) for a neutral type ref; category 'unsupported' has no converter yet."""
    if ref == "bool":
        return ("bool", None)
    if ref in INT_REFS:
        return ("int", ref)
    if ref in ("f32", "f64"):
        return ("float", ref)
    if ref in ("string", "bytes", "date", "time", "datetime", "cstring", "variant"):
        return (ref, None)
    head, _, rest = ref.partition("<")
    inner = rest[:-1] if rest.endswith(">") else rest
    if head in ("enum", "flags", "record", "boxed", "object"):
        return (head, inner)
    if head == "list":
        if inner == "string":
            return ("list", "string")
        sub = classify(inner)
        return ("list", inner) if sub[0] != "unsupported" else ("unsupported", ref)
    return ("unsupported", ref)


def kebab_words(name: str) -> list[str]:
    spaced = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", " ", name.replace("_", " "))
    parts = spaced.split()
    out, i = [], 0
    while i < len(parts):
        w = parts[i]
        if len(w) == 1 and i + 1 < len(parts):  # AlignHCenter -> hcenter, not h-center
            w += parts[i + 1]
            i += 1
        out.append(w.lower())
        i += 1
    return out


def enum_member_names(members: list[dict]) -> dict[str, str]:
    """JS member name -> C++ expression: kebab-case, with the common leading words stripped only when
    every name stays unique and non-empty (QLineEdit::PasswordEchoOnEdit -> password-echo-on-edit,
    Qt::ScrollBarAsNeeded -> as-needed). Aliases that collide keep the first member."""
    words = [kebab_words(m["name"]) for m in members]
    names = ["-".join(w) for w in words]
    pre = 0
    if len(members) > 1:
        while all(len(w) > pre + 1 and w[pre] == words[0][pre] for w in words):
            pre += 1
    if pre:
        stripped = ["-".join(w[pre:]) for w in words]
        if all(stripped) and len(set(stripped)) == len(stripped):
            names = stripped
    out: dict[str, str] = {}
    for n, m in zip(names, members):
        out.setdefault(n, m["cpp"])
    return out


def strip_cv(native: str) -> str:
    return re.sub(r"\bconst\b", "", native).replace("&", "").strip()


def ts_leaf(cat: str, payload, m) -> str:
    """The TypeScript type for a classified ref (see packages/renderer/types/values.d.ts)."""
    if cat == "bool":
        return "boolean"
    if cat in ("int", "float"):
        return "number"
    if cat in ("string", "cstring"):
        return "string"
    if cat == "variant":
        return "Variant"
    if cat == "bytes":
        return "Uint8Array"
    if cat == "date":
        return "DateParts"
    if cat == "time":
        return "TimeParts"
    if cat == "datetime":
        return "Date"
    if cat == "record":
        return payload + (" | string" if payload == "Color" else "")
    if cat == "enum":
        names = sorted(m.enums_used[payload]) if payload in m.enums_used else []
        return " | ".join(f'"{n}"' for n in names) or "string"
    if cat == "flags":
        names = sorted(m.enums_used[payload]) if payload in m.enums_used else []
        return "Array<" + (" | ".join(f'"{n}"' for n in names) or "string") + ">"
    if cat == "boxed":
        return f"Boxed<\"{payload}\">" + (" | string" if payload in ("QIcon", "QKeySequence", "QUrl", "QFont") else "")
    if cat == "object":
        return "number"
    if cat == "list":
        inner = classify(payload)
        leaf = ts_leaf(inner[0], inner[1], m) if payload != "string" else "string"
        return f"Array<{leaf}>"
    return "unknown"


# --------------------------------------------------------------------------------------------
# model

class Model:
    def __init__(self, meta_dir: Path, ov_dir: Path, only: list[str]):
        self.meta_dir, self.ov_dir = meta_dir, ov_dir
        self.read_files: list[Path] = []
        self._kinds: dict[str, dict] = {}
        self.elements: list[dict] = []
        self.inherit: dict[str, dict] = {}
        errs: list[str] = []
        for p in sorted(ov_dir.glob("*.yaml")):
            errs += svmeta.check_override(p, HERE / "override.schema.json")
        if errs:
            raise GenError("override schema errors:\n  " + "\n  ".join(errs))
        for p in sorted(ov_dir.glob("*.yaml")):
            cls = p.stem
            self.read_files.append(p)
            ov = svmeta.load_yaml(p)
            if self.kind(cls) is None:
                raise GenError(f"{p.name}: no metadata/qt/{cls}.json (file names are class names)")
            if ov.get("inherit"):
                self.inherit[cls] = ov["inherit"]
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
        # Enum definitions (every enum a slot type mentions) live in the `Qt` namespace record.
        self.enum_defs = {}
        table = meta_dir / "_enums" / "Qt.json"
        if table.exists():
            self.read_files.append(table)
            self.enum_defs = {e["fullname"]: e for e in svmeta.load_json(table)["enums"]}
        self.enums_used: dict[str, dict[str, str]] = {}  # enum full name -> JS name -> C++ expression
        self.enum_override: set[str] = set()
        self.boxed_used: set[str] = set()
        self.owners_used: set[str] = set()
        for e in self.elements:
            self._apply_inherit(e)
            for key in ("props", "signals", "methods"):
                for r in e.get(key, []):
                    r["_raw"] = copy.deepcopy({k: v for k, v in r.items() if k != "_raw"})
            for sl in (e.get("children") or {}).get("slots", []):
                sl["_raw"] = copy.deepcopy(sl)
            for b in e.get("bindings", []):
                b["_raw"] = copy.deepcopy(b)
        for e in self.elements:
            self._resolve(e)
        self._assign_ids()

    def _apply_inherit(self, el: dict) -> None:
        """Copy `inherit:` facts of the element's class and its bases (nearest first) into the
        element, unless it declares the same JS name or lists it under `exclude`."""
        el["exclude"] = list(el.get("exclude", []))
        for c in self.chain(el["cls"]):
            block = self.inherit.get(c)
            if not block:
                continue
            for key in ("props", "signals", "methods"):
                have = {r["js"] for r in el.get(key, [])}
                for r in block.get(key, []):
                    if r["js"] in have or r["js"] in el["exclude"]:
                        continue
                    el.setdefault(key, []).append({**copy.deepcopy(r), "inherited_from": c})
                    have.add(r["js"])

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


    # value types ---------------------------------------------------------------------------
    def use(self, ref: str, where: str, table: dict | None = None) -> tuple:
        """Registers what a slot of type `ref` needs (enum tables, boxed converters) and classifies it."""
        cat, pay = classify(ref)
        if cat == "unsupported":
            raise GenError(f"{where}: type {ref} has no converter yet")
        if cat in ("enum", "flags"):
            if table is not None:
                self.enums_used[pay] = dict(table)
                self.enum_override.add(pay)
            elif pay not in self.enum_override and pay not in self.enums_used:
                d = self.enum_defs.get(pay)
                if not d:
                    raise GenError(f"{where}: enum {pay} is not in the enum table (rerun the ingest)")
                self.enums_used[pay] = enum_member_names(d["members"])
            self.owners_used.add(pay.split("::")[0])
        elif cat == "boxed":
            self.boxed_used.add(pay)
            self.owners_used.add(pay)
        elif cat == "list" and pay != "string":
            self.use(pay, where)
        return cat, pay

    def cpp_type(self, ref: str, native: str) -> str:
        """The C++ spelling of a parameter, return or signal-argument type (cv/ref stripped)."""
        cat, pay = classify(ref)
        if cat == "enum":
            return pay
        if cat == "flags":
            return f"QFlags<{pay}>"
        if cat == "cstring":
            return "const char *"
        return strip_cv(native)

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
        p["readonly"] = False
        if p.get("type") == "enum":
            # An override that narrows or renames an enum's members (QBoxLayout::Direction -> orientation).
            for key in ("getter", "setter"):
                self._need_method(tag, tcls, p[key], None)
            _, g = self._need_method(tag, tcls, p["getter"], 0)
            ref = g["returns"]["ref"]
            cat, pay = classify(ref)
            if cat != "enum":
                raise GenError(f"{tag}: enum prop {p['js']} getter returns {ref}")
            self.use(ref, f"{tag}.{p['js']}", p["values"])
            p["decl_ref"], p["ref"], p["cat"] = "enum", ref, "enum"
            p["direct"] = True
            return
        dcls, prop = self.find(tcls, "properties", lambda r: r["name"] == p["native"])
        if not prop:
            raise GenError(f"{tag}: property {p['native']} not found on {tcls}")
        acc = prop["accessors"]
        p["decl_cls"], p["prop"] = dcls, prop
        p["decl_ref"] = prop["type"]["ref"]
        p["ref"] = prop["type"]["ref"]
        p["getter"], p["setter"] = acc.get("get"), acc.get("set")
        p["qprop"] = prop.get("metaobject") is not None
        cat, _ = classify(p["ref"])
        scalar = cat in ("bool", "int", "float", "string")
        # Without a direct setter a scalar can still be written through its QMetaProperty; any other
        # category is read-only.
        direct = cat != "unsupported" and p["getter"] and (p["setter"] or not prop["access"]["write"] or not scalar)
        if direct:
            self.use(p["ref"], f"{tag}.{p['js']}")
            self._need_method(tag, tcls, p["getter"], 0)
            if p["setter"]:
                self._need_method(tag, tcls, p["setter"], 1)
            else:
                p["readonly"] = True
        elif not p["qprop"]:
            raise GenError(f"{tag}: property {p['native']} has no direct accessors and no QMetaProperty")
        p["cat"] = cat
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
        m["ret_native"] = rec["returns"]["native"]
        m["argrefs"] = want or []
        m["argtypes"] = []
        for i, q in enumerate(rec["params"][:len(m["argrefs"])]):
            self.use(q["type"]["ref"], f"{tag}.{m['js']} arg {i}")
            m["argtypes"].append(self.cpp_type(q["type"]["ref"], q["type"]["native"]))
        m["argnames"] = [q["name"] or f"arg{i}" for i, q in enumerate(rec["params"][:len(m["argrefs"])])]
        if m["ret"] != "void":
            self.use(m["ret"], f"{tag}.{m['js']} return")

    def _resolve_signal(self, el: dict, s: dict) -> None:
        tag, cls = el["tag"], el["cls"]
        if s.get("synthetic"):
            # Raised by the backend itself (an event filter), not by a QObject signal. Both are
            # sync signals: the JS handler's result decides (close_request: veto, key_press: consume).
            if s["synthetic"] not in ("close_request", "key_press"):
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
        s["argrefs"] = {}
        for a in s["args"]:
            if a["name"] not in names:
                raise GenError(f"{tag}: signal arg {a['name']} not a parameter of {rec['signature']}")
            q = rec["params"][names.index(a["name"])]
            self.use(q["type"]["ref"], f"{tag}.{s['js']}.{a['name']}")
            a["ref"] = q["type"]["ref"]

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
            icls, item, _ = self.parse_native(where + ".list.item", lst["item"])
            ops["at"] = {"method": item, "args": ["index"], "receiver": None, "then": []}
            _, irec = self.find(icls, "methods", lambda r: r["name"] == item)
            ops["at_layout_item"] = bool(irec) and "QLayoutItem" in (irec["returns"].get("native") or "")
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
    seen = {f"QtWidgets/{model.kind(c)['native']['header']}" for c in classes if model.kind(c)}
    for o in model.owners_used:
        if model.kind(o) and o != "Qt":
            seen.add(f"QtWidgets/{model.kind(o)['native']['header']}")
        elif o in GUI_HEADERS:
            seen.add(GUI_HEADERS[o])
    return "\n".join(f"#include <{h}>" for h in sorted(seen))


def host_expr(el: dict, s_host: str, obj: str = "o") -> str:
    if s_host == "layout":
        return f"static_cast<{el['layout_cls']} *>(static_cast<QWidget *>({obj})->layout())"
    return f"static_cast<{el['cls']} *>({obj})"


def target_expr(el: dict, p: dict, obj: str = "o") -> str:
    return host_expr(el, "layout" if p.get("target") == "layout" else "self", obj)








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
            if p["direct"]:
                out.append(f"          auto *t = {tgt};")
                out.append(f"          using T = Plain<decltype(t->{p['getter']}())>;")
                if setter and p["readonly"]:
                    out.append(f'          return fail("{p["js"]}: property is read-only");')
                elif setter:
                    out.append("          T x{};")
                    out.append("          std::string er;")
                    out.append(f'          if (!conv_in(v, x, "{p["js"]}", er)) return fail(er);')
                    if p.get("suppress_signals"):
                        out.append("          Suppress quiet(e);  // programmatic set must not echo back as an event (per-handle flag, never QSignalBlocker)")
                    out.append(f"          t->{p['setter']}(x);")
                    out.append("          return 0;")
                else:
                    out.append(f"          Conv<T>::out(t->{p['getter']}(), out);")
                    out.append("          return 0;")
            else:  # QMetaProperty fallback
                if setter:
                    if p.get("suppress_signals"):
                        out.append("          Suppress quiet(e);")
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
           "  QObject *o = e.obj.data();", "  (void)args; (void)out; std::string er;", "  switch (e.kind) {"]
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
            for i, (ty, an) in enumerate(zip(me["argtypes"], me["argnames"])):
                out.append(f"          {ty} a{i}{{}};")
                out.append(f'          if (!conv_in(&args[{i}], a{i}, "{me["js"]}({an})", er)) return fail(er);')
                names.append(f"a{i}")
            call = f"static_cast<{e['cls']} *>(o)->{me['native']}({', '.join(names)})"
            if me["ret"] == "void":
                out.append(f"          {call};")
            else:
                out.append(f"          auto r = {call};")
                out.append("          if (out) Conv<Plain<decltype(r)>>::out(r, out);")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("call: unknown method id for this kind");\n}')
    return "\n".join(out)


def emit_describe(m: Model) -> str:
    """describe(): one static descriptor per prop and method, built on first request from Conv<T>::slot."""
    out = ["const svl_member_desc *gen_describe(uint32_t what, svl_id kind, svl_id member) {", "  switch (what) {"]
    for what, key in (("SVL_PROP", "props"), ("SVL_METHOD", "methods")):
        out.append(f"    case {what}:\n      switch (kind) {{")
        for e in m.elements:
            t = cident(e["tag"])
            items = e.get(key, [])
            if not items:
                continue
            out.append(f"        case K_{t}:\n          switch (member) {{")
            for it in items:
                if key == "props":
                    pid = f"P_{t}_{cident(it['js'])}"
                    cls = e["layout_cls"] if it.get("target") == "layout" else e["cls"]
                    out.append(f"            case {pid}: {{")
                    if it["direct"]:
                        out.append(f"              using T = Plain<decltype(std::declval<{cls} *>()->{it['getter']}())>;")
                        out.append(f'              static svl_slot_desc ps[1] = {{Conv<T>::slot("{it["js"]}")}};')
                        out.append(f'              static svl_member_desc d = {{"{it["js"]}", ps, {0 if it["readonly"] else 1}, ps[0], 0}};')
                    else:
                        out.append(f'              static svl_slot_desc ps[1] = {{slot_desc("{it["js"]}", SVL_NULL, SVL_D_NULLABLE)}};')
                        out.append(f'              static svl_member_desc d = {{"{it["js"]}", ps, 1, ps[0], 0}};')
                    out.append("              return &d;\n            }")
                else:
                    mid = f"M_{t}_{cident(it['js'])}"
                    n = len(it["argrefs"])
                    out.append(f"            case {mid}: {{")
                    slots = ", ".join(f'Conv<{ty}>::slot("{an}")' for ty, an in zip(it["argtypes"], it["argnames"])) or "slot_desc(nullptr, SVL_NULL)"
                    out.append(f"              static svl_slot_desc ps[{max(n, 1)}] = {{{slots}}};")
                    if it["ret"] == "void":
                        ret = "slot_desc(nullptr, SVL_NULL)"
                    else:
                        ret = f'Conv<{m.cpp_type(it["ret"], it["ret_native"])}>::slot(nullptr)'
                    out.append(f'              static svl_member_desc d = {{"{it["js"]}", ps, {n}, {ret}, 0}};')
                    out.append("              return &d;\n            }")
            out.append("            default: break;\n          }\n          break;")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }\n  return nullptr;\n}")
    return "\n".join(out)


def emit_enums(m: Model) -> str:
    """EnumInfo<E> for every enum a slot mentions, and BoxedConv for the value classes."""
    out = []
    for full in sorted(m.enums_used):
        mem = ", ".join(f'{{"{name}", static_cast<int64_t>({cpp})}}' for name, cpp in sorted(m.enums_used[full].items()))
        out.append(f"template <>\nstruct EnumInfo<{full}> {{\n  static const char *name() {{ return \"{full}\"; }}\n"
                   f"  static const std::vector<std::pair<std::string, int64_t>> &members() {{\n"
                   f"    static const std::vector<std::pair<std::string, int64_t>> v = {{{mem}}};\n    return v;\n  }}\n}};")
    for b in sorted(m.boxed_used):
        out.append(f"template <>\nstruct Conv<{b}> : BoxedConv<{b}> {{}};")
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
                out.append(f"          if (int i = {op_call('h', ops['index_of'], 'ch')}; i >= 0) {{ {op_call('h', ops['remove'], 'ch', {'index': 'i'})}; }}")
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
                out.append(f"        if (int i = {op_call('h', ops['index_of'], 'ch')}; i >= 0) {{")
                out.append(f"          {op_call('h', ops['remove'], 'ch', {'index': 'i'})};")
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
                if s["synthetic"] == "key_press":
                    out.append(f"        case {sid}: return listen_key(e, sig, on);")
                else:
                    # onRequestClose: while listened, the JS handler decides each native close.
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
                body.append("g_out.clear(); svl_value vals[%d];" % len(s["args"]))
                names = [n for n, _ in s["qparams"]]
                for i, a in enumerate(s["args"]):
                    pi = names.index(a["name"])
                    body.append(f"Conv<Plain<decltype(a{pi})>>::out(a{pi}, &vals[{i}]);")
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


def emit_toplevel(m: Model) -> str:
    """Kinds of the toplevel lifetime class: windows and dialogs. A parentless QWidget is a Qt
    window too (until it is inserted somewhere), so the class cannot be read off the object."""
    tops = [e for e in m.elements if m.derives(e["cls"], "QMainWindow") or m.derives(e["cls"], "QDialog")]
    out = ["bool gen_is_toplevel(svl_id kind) {", "  switch (kind) {"]
    out += [f"    case K_{cident(e['tag'])}: return true;" for e in tops]
    out += ["    default: return false;", "  }", "}"]
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
        "CALL": emit_call(m), "DESCRIBE": emit_describe(m), "ENUMS": emit_enums(m), "INSERT": emit_insert(m), "REMOVE": emit_remove(m), "LISTEN": emit_listen(m),
        "TOPLEVEL": emit_toplevel(m),
    }
    parts["SERDES"] = (HERE / "templates" / "serdes.hpp.in").read_text()
    for k, v in parts.items():
        tmpl = tmpl.replace(f"@@{k}@@" if k == "DIGEST" else f"//@@{k}@@", v)
    return tmpl


# --------------------------------------------------------------------------------------------
# verification tests

PREFIX = "svelters-api:v1:type:qt:QtWidgets"
SCALAR = ("string", "bool", "int", "float")


def fact_id(e: dict, section: str, name: str, rec: dict | None = None) -> str:
    """Stable fact id. Inherited facts are keyed by the class that declares them, so one fact is
    verified on every element that inherits it."""
    if rec is not None and rec.get("inherited_from"):
        return f"{PREFIX}:{rec['inherited_from']}:inherit:{section}:{name}"
    return f"{PREFIX}:{e['cls']}:element:{e['tag']}:{section}:{name}"


def strip(rec):
    if isinstance(rec, dict):
        return {k: strip(v) for k, v in rec.items() if k not in ("_raw", "rec", "prop", "decl_cls", "inherited_from")}
    if isinstance(rec, list):
        return [strip(x) for x in rec]
    return rec


class Tests:
    """One test (or a few) per annotated fact; a fact the generator cannot test is an error.
    Every test is attributed to its fact for the verified ledger (`--facts`)."""

    def __init__(self, m: Model):
        self.m = m
        self.fns: list[tuple[str, str]] = []
        self.src: list[str] = []
        self.facts: dict[str, dict] = {}
        self.errors: list[str] = []
        self._fact: tuple[str, dict] | None = None

    def add(self, name: str, body: str) -> None:
        ident = "t_" + cident(name)
        src = f"bool {ident}(std::string &why) {{\n{body}\n  return true;\n}}\n"
        self.fns.append((name, ident))
        self.src.append(src)
        if self._fact:
            fid, raw = self._fact
            f = self.facts.setdefault(fid, {"fact": fid, "record": raw, "tests": [], "srcs": []})
            f["tests"].append(name)
            f["srcs"].append(src)

    def fact(self, fid: str, raw, section: str, e: dict, name: str, fn) -> None:
        self._fact = (fid, {"raw": raw, "section": section, "name": name})
        n0 = len(self.fns)
        try:
            fn()
        except GenError as ex:
            self.errors.append(f"{fid} (<{e['tag']}>): {ex}")
            del self.fns[n0:]
            del self.src[n0:]
        self._fact = None
        f = self.facts.get(fid)
        if f is not None:
            f.setdefault("elements", []).append(e["tag"])
            f["section"], f["name"] = section, name

    def build(self) -> tuple[str, str]:
        m = self.m
        for e in m.elements:
            for p in e.get("props", []):
                self.fact(fact_id(e, "prop", p["js"], p), [p["_raw"], strip(p.get("prop"))], "props", e, p["js"], lambda e=e, p=p: self.prop(e, p))
            for me in e.get("methods", []):
                self.fact(fact_id(e, "method", me["js"], me), [me["_raw"], strip(me.get("rec"))], "methods", e, me["js"], lambda e=e, me=me: self.method(e, me))
            for sg in e.get("signals", []):
                self.fact(fact_id(e, "signal", sg["js"], sg), [sg["_raw"], strip(sg.get("rec"))], "signals", e, sg["js"], lambda e=e, sg=sg: self.signal(e, sg))
            ch = e.get("children")
            if ch:
                for sl in ch["slots"]:
                    fn = (lambda e=e, sl=sl: self.slot_one(e, sl)) if sl["cardinality"] == "one" else (lambda e=e, sl=sl: self.slot_many(e, sl))
                    self.fact(fact_id(e, "slot", sl["name"]), sl["_raw"], "slots", e, sl["name"], fn)
            for b in e.get("bindings", []):
                self.fact(fact_id(e, "binding", b["name"]), b["_raw"], "bindings", e, b["name"], lambda e=e, b=b: self.binding(e, b))
        return "\n".join(self.src), "\n".join(f'    {{"{n}", {i}}},' for n, i in self.fns)

    def manifest(self, version: str) -> dict:
        facts = []
        for fid in sorted(self.facts):
            f = self.facts[fid]
            facts.append({"fact": fid, "section": f["section"], "name": f["name"], "elements": sorted(set(f["elements"])),
                          "tests": f["tests"], "fingerprint": svmeta.fingerprint({"record": f["record"], "tests": f["srcs"]})})
        return {"toolkit": "qt", "version": version, "facts": facts}

    # helpers
    def mkparent(self, e): return f'  svl_handle p = mk("{e["tag"]}");'

    def native_get(self, e, p, obj="O(h)") -> str:
        """C++ expression reading the property natively."""
        tgt = target_expr(e, p, obj)
        if p.get("getter"):
            return f"{tgt}->{p['getter']}()"
        if p.get("native"):
            conv = {"string": ".toString()", "bool": ".toBool()", "float": ".toDouble()"}.get(p["cat"], ".toLongLong()")
            return f'{tgt}->property("{p["native"]}"){conv}'
        raise GenError(f"{p['js']}: no native getter to verify against")

    def sample(self, p) -> tuple[str, str | None, str]:
        """(svl_value declaration of `v`, C++ native comparison rhs, expected backend output check on `out`)."""
        cat, smp = p["cat"], p.get("sample")
        if cat == "string":
            val = "h\\xc3\\xa9llo" if smp is None else str(smp).replace('"', '\\"')
            return (f'std::string s = "{val}"; svl_value v = vstr(s);', None, "out.tag == SVL_STR && valstr(out) == s")
        if cat == "bool":
            b = True if smp is None else bool(smp)
            lit = "true" if b else "false"
            return (f"svl_value v = vbool({lit});", lit, f"out.tag == SVL_BOOL && out.u.b == {lit}")
        if cat == "float":
            x = 0.5 if smp is None else float(smp)
            return (f"svl_value v = vf64({x!r});", repr(x), f"out.tag == SVL_F64 && out.u.f == {x!r}")
        if cat == "int":
            x = 7 if smp is None else int(smp)
            return (f"svl_value v = vi64({x});", str(x), f"out.u.i == {x}")
        raise GenError(f"{p['js']}: no scalar sample for category {cat}")

    def prop(self, e, p):
        t = e["tag"]
        pn = p["js"]
        cat = p["cat"]
        tgt = target_expr(e, p, "O(h)")
        P_ = f'P("{t}", "{pn}")'
        if p["readonly"] or not ((p.get("prop") or {}).get("access") or {}).get("write", True):
            lines = [f'  svl_handle h = mk("{t}");', "  svl_value out{};",
                     f"  CHECK(B->get_prop(B->self, h, {P_}, &out) == 0);",
                     f"  CHECK(B->set_prop(B->self, h, {P_}, &out) < 0);  // read-only",
                     "  B->destroy(B->self, h);"]
            self.add(f"prop/{t}.{pn}/read_only", "\n".join(lines))
        elif cat == "enum" and p["direct"]:
            members = sorted(self.m.enums_used[classify(p["ref"])[1]].items())
            lines = [f'  svl_handle h = mk("{t}");',
                     f'  const svl_member_desc *d = B->describe(B->self, SVL_PROP, K("{t}"), {P_});',
                     '  CHECK(d && d->n_params == 1 && d->params[0].tag == SVL_ENUM);',
                     '  svl_id tid = d->params[0].type;',
                     '  const svl_type_desc *td = B->describe_type(B->self, tid);',
                     f'  CHECK(td && td->kind == SVL_ENUM && td->n_members == {len(members)});']
            for name, cexpr in members:
                lines += [f'  {{ svl_value v = venum(tid, static_cast<int64_t>({cexpr}));',
                          f'    CHECK(B->set_prop(B->self, h, {P_}, &v) == 0);',
                          f'    CHECK({tgt}->{p["getter"]}() == {cexpr});',
                          '    svl_value out{};',
                          f'    CHECK(B->get_prop(B->self, h, {P_}, &out) == 0 && out.tag == SVL_ENUM && out.type == tid && out.u.i == static_cast<int64_t>({cexpr}));',
                          f'    CHECK(enum_member_value(td, "{name}") == static_cast<int64_t>({cexpr})); }}']
            lines += [f'  {{ svl_value v = venum(tid, 987654); CHECK(B->set_prop(B->self, h, {P_}, &v) < 0); }}',
                      '  B->destroy(B->self, h);']
            self.add(f"prop/{t}.{pn}", "\n".join(lines))
        elif cat in SCALAR:
            decl, rhs, outcheck = self.sample(p)
            native = self.native_get(e, p)
            ncheck = (f"  CHECK(QString({native}).toUtf8().toStdString() == s);" if cat == "string" else f"  CHECK({native} == {rhs});")
            lines = [f'  svl_handle h = mk("{t}");', f"  {decl}",
                     f"  CHECK(B->set_prop(B->self, h, {P_}, &v) == 0);", ncheck,
                     "  svl_value out{};",
                     f"  CHECK(B->get_prop(B->self, h, {P_}, &out) == 0 && {outcheck});", "  B->destroy(B->self, h);"]
            self.add(f"prop/{t}.{pn}", "\n".join(lines))
        else:
            # records, flags, boxed, dates, lists: write back the value just read (the converters'
            # value round trips are in examples/counter-qt/tests/serdes.rs).
            lines = [f'  svl_handle h = mk("{t}");', "  svl_value out{};",
                     f"  CHECK(B->get_prop(B->self, h, {P_}, &out) == 0);",
                     "  uint32_t tag = out.tag;",
                     f"  CHECK(B->set_prop(B->self, h, {P_}, &out) == 0);",
                     "  svl_value again{};",
                     f"  CHECK(B->get_prop(B->self, h, {P_}, &again) == 0 && again.tag == tag);",
                     "  B->destroy(B->self, h);"]
            self.add(f"prop/{t}.{pn}/round_trip", "\n".join(lines))
        if p.get("suppress_signals"):
            sigs = [sg for sg in e.get("signals", []) if not sg.get("synthetic")]
            if not sigs or not sigs[0].get("trigger"):
                raise GenError(f"{pn}: suppress_signals needs a signal with a trigger on the element")
            decl, _, _ = self.sample(p)
            lines = [f'  svl_handle h = mk("{t}");', f"  {decl}"]
            for sg in sigs:
                lines.append(f'  CHECK(B->listen(B->self, h, G("{t}", "{sg["js"]}"), true) == 0);')
            trig = sigs[0]["trigger"].replace("$w", f"static_cast<{e['cls']} *>(O(h))").replace("$i", "1")
            lines += [f"  CHECK(B->set_prop(B->self, h, {P_}, &v) == 0);",
                      "  CHECK(rec.events.empty());  // no echo for programmatic sets",
                      f"  {trig};",
                      "  CHECK(!rec.events.empty());  // native changes are still delivered",
                      "  rec.events.clear();",
                      f"  CHECK(B->set_prop(B->self, h, {P_}, &v) == 0);",
                      "  CHECK(rec.events.empty());  // suppressed again on the next programmatic set",
                      "  B->destroy(B->self, h);"]
            self.add(f"signal/{t}.{pn}/no_echo_on_set_prop", "\n".join(lines))

    def method(self, e, me):
        t = e["tag"]
        vals, synth = [], True
        given = me.get("args") or [None] * len(me["argrefs"])
        for i, ref in enumerate(me["argrefs"]):
            cat = classify(ref)[0]
            a = given[i] if i < len(given) else None
            if cat == "int":
                vals.append(f"vi64({10 + i if a is None else int(a)})")
            elif cat == "bool":
                vals.append(f"vbool({'true' if a is None or a else 'false'})")
            elif cat == "float":
                vals.append(f"vf64({0.5 if a is None else float(a)!r})")
            elif cat == "string":
                vals.append(f'vstr(std::string("{"x" if a is None else a}"))')
            else:
                synth = False
        lines = [f'  svl_handle h = mk("{t}");', f'  CHECK(M("{t}", "{me["js"]}") != 0);']
        if synth:
            if vals:
                lines.append(f"  svl_value a[] = {{{', '.join(vals)}}};")
            lines.append(f'  CHECK(B->call(B->self, h, M("{t}", "{me["js"]}"), {"a" if vals else "nullptr"}, {len(vals)}, nullptr) == 0);')
            if me.get("expect"):
                w = f"static_cast<{e['cls']} *>(O(h))"
                lines.append(f"  CHECK({me['expect'].replace('$w', w)});")
        elif me.get("expect"):
            raise GenError(f"{me['js']}: `expect` needs test arguments the generator can build (int, bool, float, string)")
        # non-scalar arguments: the call itself is exercised by examples/counter-qt/tests/serdes.rs
        lines += [f'  CHECK(B->call(B->self, h, M("{t}", "{me["js"]}"), nullptr, {len(me["argrefs"]) + 1}, nullptr) < 0);',
                  "  B->destroy(B->self, h);"]
        self.add(f"method/{t}.{me['js']}", "\n".join(lines))

    def signal(self, e, s):
        t = e["tag"]
        if s.get("synthetic"):
            self.add(f"signal/{t}.{s['js']}/synthetic_listen", "\n".join([
                f'  svl_handle h = mk("{t}");', f'  svl_id sig = G("{t}", "{s["js"]}");', "  CHECK(sig != 0);",
                "  CHECK(B->listen(B->self, h, sig, true) == 0);", "  CHECK(B->listen(B->self, h, sig, false) == 0);",
                "  B->destroy(B->self, h);  // close behaviour: hand-written close tests in verify.cpp.in"]))
            return
        if not s.get("trigger"):
            raise GenError(f"signal {s['js']} has no `trigger` (C++ statement that makes Qt emit it)")
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

    def binding(self, e, b):
        """The change signal's first argument is the new property value (what the renderer stores)."""
        t = e["tag"]
        sg = next((x for x in e.get("signals", []) if x["js"] == b["change"]["signal"]), None)
        p = e["props_by_name"].get(b["property"])
        if sg is None or p is None or not sg.get("trigger"):
            raise GenError(f"bind:{b['name']}: needs prop {b['property']} and a triggerable signal {b['change']['signal']}")
        trig = sg["trigger"].replace("$w", f"static_cast<{e['cls']} *>(O(h))").replace("$i", "1")
        value = b["change"].get("value", "arg0")
        lines = [f'  svl_handle h = mk("{t}");', f'  CHECK(B->listen(B->self, h, G("{t}", "{sg["js"]}"), true) == 0);',
                 f"  {trig};", "  CHECK(!rec.events.empty());", "  svl_value out{};",
                 f'  CHECK(B->get_prop(B->self, h, P("{t}", "{p["js"]}"), &out) == 0);']
        if value == "arg0":
            if not sg["args"]:
                raise GenError(f"bind:{b['name']}: change.value arg0 needs a signal with args")
            lines.append("  CHECK(rec.events.back().strs[0] == valstr(out));  // first arg == property value")
        lines.append("  B->destroy(B->self, h);")
        self.add(f"binding/{t}.{b['name']}", "\n".join(lines))

    def item_widget(self, ops, expr: str) -> str:
        """`list.item` may return a QLayoutItem* (layouts) or the QWidget* itself (QTabWidget::widget)."""
        return f"{expr}->widget()" if ops.get("at_layout_item", True) else expr

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
        at = lambda p, i: self.item_widget(ops, f"{host(p)}->{ops['at']['method']}({i})")
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


def gen_verify(m: Model, digest: str, tmpl: str) -> tuple[str, dict]:
    T = Tests(m)
    tests, table = T.build()
    if T.errors:
        raise GenError("manual facts without a working generated test (fix or drop them):\n  " + "\n  ".join(T.errors))
    version = (m.kind("QWidget") or {}).get("source", {}).get("qdoc", {}).get("version", "?")
    classes = {e["cls"] for e in m.elements} | {e["layout_cls"] for e in m.elements if e["layout_cls"]}
    simple = [e for e in m.elements if not e.get("construct", {}).get("composite")]
    parent = next((e for e in m.elements if e.get("children") and e["children"]["default_slot"]
                   and any(s["name"] == e["children"]["default_slot"] and "QWidget" in s["accepts"]
                           for s in e["children"]["slots"])), None)
    if not parent:
        raise GenError("need an element with a QWidget default slot")
    val = next((e for e in simple for p in e.get("props", []) if p.get("cat") == "string"), None)
    if not val:
        raise GenError("need an element with a string prop")
    any_tag = next(e["tag"] for e in simple if e["tag"] == "label") if any(e["tag"] == "label" for e in simple) else simple[0]["tag"]
    child_tag = m.accepted_child_tag(parent, next(s for s in parent["children"]["slots"] if s["name"] == parent["children"]["default_slot"]))
    prop = next(p for p in val["props"] if p.get("cat") == "string")
    subs = {"@@ANY_TAG@@": any_tag, "@@PARENT_TAG@@": parent["tag"], "@@CHILD_TAG@@": child_tag,
            "@@VALUE_TAG@@": val["tag"], "@@VALUE_PROP@@": prop["js"], "@@DIGEST@@": digest}
    out = tmpl.replace("//@@INCLUDES@@", inc(m, classes)).replace("//@@TESTS@@", tests).replace("//@@TEST_TABLE@@", table)
    for k, v in subs.items():
        out = out.replace(k, v)
    return out, T.manifest(version)


# --------------------------------------------------------------------------------------------
# typings

def ts_type(m: Model, ref: str) -> str:
    cat, pay = classify(ref)
    return ts_leaf(cat, pay, m)


def gen_dts(m: Model) -> str:
    out = ["// GENERATED by tools/metadata/qt/gen_cpp.py -- do not edit.", "",
           "import type { Boxed, DateParts, Insets, Point, Rect, Size, TimeParts, Color, Variant } from './values';", "",
           "export interface QtElementDescriptors {"]
    for e in m.elements:
        out.append(f"  {e['tag']}: {{")
        out.append("    props: {")
        for p in e.get("props", []):
            if p["direct"]:
                ty = ts_type(m, p["ref"])
            else:
                ty = {"string": "string", "bool": "boolean"}.get(p["decl_ref"], "number")
            ro = "readonly " if p.get("readonly") else ""
            out.append(f"      {ro}{p['js']}?: {ty};")
        out.append("    };")
        out.append("    events: {")
        for s in e.get("signals", []):
            args = ", ".join(f"{a['name']}: {ts_type(m, a['ref'])}" for a in s["args"])
            key = s["js"] if s["js"].isidentifier() else '"' + s["js"] + '"'
            out.append(f"      {key}: ({args}) => void;")
        out.append("    };")
        out.append("    methods: {")
        for me in e.get("methods", []):
            args = ", ".join(f"{an}: {ts_type(m, r)}" for an, r in zip(me["argnames"], me["argrefs"]))
            ret = "void" if me["ret"] == "void" else ts_type(m, me["ret"])
            out.append(f"      {me['js']}({args}): {ret};")
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
    ap.add_argument("--facts", help="write the fact manifest (fact id -> test names + fingerprint) to this path")
    a = ap.parse_args()
    cfgp = Path(a.config).resolve()
    cfg = tomllib.loads(cfgp.read_text())
    base = cfgp.parent
    res = lambda s: (base / s).resolve()
    out = Path(a.out)
    try:
        m = Model(res(cfg["input"]["metadata"]), res(cfg["input"]["overrides"]), cfg.get("elements", {}).get("tags", []))
        templ = HERE / "templates"
        digest = m.digest([HERE / "gen_cpp.py", templ / "backend.cpp.in", templ / "verify.cpp.in", templ / "serdes.hpp.in"])
        o = cfg["output"]
        write_text(out / o["backend"], gen_backend(m, digest, (templ / "backend.cpp.in").read_text()))
        verify, facts = gen_verify(m, digest, (templ / "verify.cpp.in").read_text())
        write_text(out / o["verify"], verify)
        if a.facts:
            write_text(Path(a.facts), json.dumps(facts, indent=1, sort_keys=True) + "\n")
        write_text(out / o["dts"], gen_dts(m))
        if a.dts:
            write_text(Path(a.dts), gen_dts(m))
            write_values_dts(Path(a.dts).parent / "values.d.ts")
    except GenError as ex:
        print(f"gen_cpp: {ex}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
