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
import naming  # noqa: E402
import svmeta  # noqa: E402
from tsvalues import write_values_dts  # noqa: E402


class GenError(Exception):
    pass


OBJ_REF = re.compile(r"object<(\w+)>")

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
    if ref in ("string", "bytes", "date", "time", "datetime", "cstring", "variant", "callback"):
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


def element_tag(el: dict) -> str:
    """The tag of an annotated element: its class's tag, or for a plain QWidget carrying a layout,
    the layout's (`QWidget` + `QGridLayout` -> `grid-layout`)."""
    c = el["construct"]
    lay = (c.get("composite") or {}).get("layout", {}).get("class")
    return naming.tag_for("qt", lay if c["class"] == "QWidget" and lay else c["class"])


def native_js(r: dict, key: str) -> str | None:
    """The JS name a prop or signal record must have: kebab(native); an accessor prop is named after its
    setter (`setSeparator` -> `separator`). None for synthetic records (dynamic props, event-filter signals)."""
    if r.get("native"):
        return naming.kebab(r["native"])
    if key == "props" and r.get("setter", "").startswith("set"):
        return naming.kebab(r["setter"][3:])
    return None


def check_names(file: str, el: dict, block: bool = False) -> list[str]:
    """Names follow the shared rule: tags and prop/signal/child-attribute names are derived from the native
    API; a synthetic name must still be kebab-case."""
    errs = []
    if not block:
        want = element_tag(el)
        if el.get("tag") != want and not el.get("tag_reason"):
            errs.append(f"{file}: element {el.get('tag')!r} must be tagged {want!r} (or give a tag_reason)")
    for key in ("props", "signals"):
        for r in el.get(key, []) or []:
            want = native_js(r, key) or naming.kebab(r["js"])
            if r["js"] != want:
                errs.append(f"{file}: {el.get('tag')} {key[:-1]} {r['js']!r} must be named {want!r}")
    for sl in (el.get("children") or {}).get("slots", []) or []:
        for a in sl.get("attrs", []) or []:
            if a["name"] != naming.kebab(a["name"]):
                errs.append(f"{file}: {el.get('tag')} attr {a['name']!r} must be kebab-case")
    return errs


# Parameter types an `out:` / `inout:` annotation may name: the C++ core type -> neutral ref.
OUT_PRIM = {"bool": "bool", "int": "i32", "uint": "u32", "unsigned int": "u32", "qint64": "i64", "qreal": "f64",
            "double": "f64", "float": "f32", "QString": "string"}


def out_core(native: str) -> str:
    """`int *` / `QString &` / `const int &` -> `int` / `QString`."""
    return re.sub(r"\bconst\b", "", native).replace("&", "").replace("*", "").strip()


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
                errs += check_names(p.name, {"tag": f"inherit:{cls}", **ov["inherit"]}, block=True)
                self.inherit[cls] = ov["inherit"]
            for el in ov.get("elements", []):
                errs += check_names(p.name, el)
                if only and el["tag"] not in only:
                    continue
                el = dict(el)
                el["cls"] = el["construct"]["class"]
                if el["cls"] != cls:
                    raise GenError(f"{p.name}: element {el['tag']} constructs {el['cls']}, expected {cls}")
                self.elements.append(el)
        if errs:
            raise GenError("naming rule (tools/metadata/common/naming.py):\n  " + "\n  ".join(errs))
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
                sl["_raw"] = copy.deepcopy({k: v for k, v in sl.items() if k != "attrs"})
                for a in sl.get("attrs", []):
                    a["_raw"] = copy.deepcopy(a)
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
        if p.get("dynamic"):
            # A dynamic QObject property (QObject::setProperty with an undeclared name): not part of the
            # class's API, so the backend (template glue) gives it meaning through `after_set`.
            p.update({"native": p["dynamic"], "decl_cls": tcls, "prop": {"access": {"write": True}}, "decl_ref": "string",
                      "ref": "string", "getter": None, "setter": None, "qprop": True, "cat": "string", "direct": False})
            return
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
        if not p.get("native") and p.get("getter") and p.get("setter"):
            # Accessors without a Q_PROPERTY (QAction::isSeparator / setSeparator): typed by the getter.
            self._need_method(tag, tcls, p["setter"], 1)
            _, g = self._need_method(tag, tcls, p["getter"], 0)
            ref = g["returns"]["ref"]
            cat, _ = classify(ref)
            if cat not in ("bool", "int", "float", "string", "enum"):
                raise GenError(f"{tag}: accessor prop {p['js']} getter returns {ref}")
            self.use(ref, f"{tag}.{p['js']}")
            p.update({"native": None, "decl_cls": tcls, "prop": {"access": {"write": True}}, "decl_ref": ref, "ref": ref,
                      "qprop": False, "cat": cat, "direct": True})
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
        tag, cls = el["tag"], self.target_cls(el, m.get("target"))
        want = m.get("params")
        # `out:` / `inout:` name C++ parameters (`int *left`, `QString &input`) that the call fills in.
        # JS passes only the in / inout ones; the result is the object {ret?, <out name>...}.
        modes = {n: "out" for n in m.get("out", [])}
        modes.update({n: "inout" for n in m.get("inout", [])})
        if len(modes) != len(m.get("out", [])) + len(m.get("inout", [])):
            raise GenError(f"{tag}.{m['js']}: a parameter is listed twice in out / inout")

        # `callbacks:` name Functor / std::function parameters that become JS functions (CALLBACK values).
        cbmap = {c["param"]: c for c in m.get("callbacks", [])}
        asyn = m.get("async")

        def eff_ref(q):
            if q["name"] in cbmap:
                return "callback"
            if q["name"] in modes:
                core = out_core(q["type"]["native"])
                ref = OUT_PRIM.get(core)
                if ref is None:
                    raise GenError(f"{tag}.{m['js']}: out parameter {q['name']} of type {core} has no converter yet")
                return ref
            return q["type"]["ref"]

        def match(r):
            if r["name"] != m["native"] or r["static"] or r.get("access", "public") != "public":
                return False
            if modes or cbmap:
                return set(modes) | set(cbmap) <= {q["name"] for q in r["params"]} and (want is None or [eff_ref(q) for q in r["params"]] == want)
            if want is None:
                return not [q for q in r["params"] if not q["default"]] or len(r["params"]) == 0
            return [q["type"]["ref"] for q in r["params"]] == want
        c, rec = self.find(cls, "methods", match)
        if not rec:
            raise GenError(f"{tag}: method {m['native']}{want or ''} not found on {cls}")
        m["rec"], m["decl_cls"] = rec, c
        m["ret"] = rec["returns"]["ref"]
        m["ret_native"] = rec["returns"]["native"]
        if m.get("discard_return"):
            m["ret"] = "void"  # e.g. QMenu::addAction returns the QAction, which JS has no use for
        whole = bool(modes or cbmap)
        params = rec["params"] if whole else rec["params"][:len(want or [])]
        m["argrefs"] = [eff_ref(q) for q in params] if whole else (want or [])
        m["argtypes"] = []
        m["modes"] = []
        m["ptrs"] = []
        m["cbs"] = []
        for i, q in enumerate(params):
            ref = m["argrefs"][i]
            m["cbs"].append(cbmap.get(q["name"]))
            if q["name"] in cbmap:
                self._resolve_callback(f"{tag}.{m['js']}", cbmap[q["name"]])
                m["argtypes"].append(None)
                m["ptrs"].append(False)
                m["modes"].append("in")
                continue
            self.use(ref, f"{tag}.{m['js']} arg {i}")
            if q["name"] in modes:
                m["argtypes"].append(out_core(q["type"]["native"]))
                m["ptrs"].append(out_core(q["type"]["native"]) != strip_cv(q["type"]["native"]))
            else:
                m["argtypes"].append(self.cpp_type(ref, q["type"]["native"]))
                m["ptrs"].append(False)
            m["modes"].append(modes.get(q["name"], "in"))
        m["argnames"] = [q["name"] or f"arg{i}" for i, q in enumerate(params)]
        m["nin"] = sum(1 for x in m["modes"] if x != "out")
        if asyn:
            self._resolve_async(el, m, cls, asyn)
        elif m["ret"] != "void":
            self.use(m["ret"], f"{tag}.{m['js']} return")

    def _resolve_callback(self, where: str, c: dict) -> None:
        """A callback spec: `{param, scope, args: [{name, cpp, ref}], returns: {cpp, ref}}`."""
        if c.get("scope") not in ("call", "notified"):
            raise GenError(f"{where}: callback {c['param']}: scope must be call or notified (async methods use `async:`)")
        for a in c.get("args", []):
            self.use(a["ref"], f"{where} callback {c['param']}.{a['name']}")
        if c.get("returns"):
            self.use(c["returns"]["ref"], f"{where} callback {c['param']} return")

    def _resolve_async(self, el: dict, m: dict, cls: str, a: dict) -> None:
        """`async: {signal, connect, result?, cancel?}`: the method starts an operation that ends with a signal.

        The generated call connects the signal (once), calls the method, and completes the host's
        one-shot callback with the named signal argument. `cancel` is a zero-argument method of the
        same object that stops the operation (the Promise then rejects with a NativeError).
        """
        tag = el["tag"]
        c, rec = self.find(cls, "signals", lambda r: r["name"] == a["signal"])
        if not rec:
            raise GenError(f"{tag}.{m['js']}: async signal {a['signal']} not found on {cls}")
        if a["connect"] not in rec["metaobject_signatures"]:
            raise GenError(f"{tag}.{m['js']}: {a['connect']} not among {rec['metaobject_signatures']}")
        a["decl_cls"] = c
        a["qparams"] = [(q["name"], q["type"]["native"]) for q in rec["params"]]
        a["result_ref"] = None
        if a.get("result"):
            names = [n for n, _ in a["qparams"]]
            if a["result"] not in names:
                raise GenError(f"{tag}.{m['js']}: async result {a['result']} is not an argument of {rec['signature']}")
            q = rec["params"][names.index(a["result"])]
            a["result_ref"] = q["type"]["ref"]
            a["result_idx"] = names.index(a["result"])
            self.use(q["type"]["ref"], f"{tag}.{m['js']} async result")
        if a.get("cancel"):
            a["cancel_cls"], _ = self._need_method(f"{tag}.{m['js']} cancel", cls, a["cancel"], 0)

    def _resolve_signal(self, el: dict, s: dict) -> None:
        tag, cls = el["tag"], el["cls"]
        if s.get("synthetic"):
            # Raised by the backend itself (an event filter), not by a QObject signal. Both are
            # sync signals: the JS handler's result decides (close_request: veto, key_press: consume).
            if s["synthetic"] not in ("close_request", "key_press") and s["synthetic"] not in EVENT_FAMILIES:
                raise GenError(f"{tag}: unknown synthetic signal {s['synthetic']}")
            s.setdefault("args", [])
            for a in s["args"]:
                a.setdefault("ref", {"int": "i32", "string": "string", "Point": "record<Point>", "Size": "record<Size>"}[a["type"]])
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
        if s.get("order") == "actions":
            # Menu items (X4): QWidget::insertAction/removeAction on a menu, menu bar or tool bar. A QAction child
            # is the item; a QMenu child is its menuAction(). The glue is `menu_insert` / `menu_remove` (menus.hpp.in).
            if s["cardinality"] != "many" or s["host"] != "self":
                raise GenError(f"{where}: order actions needs a many slot on the element itself")
            s.update({"actions": True, "ops": {}, "replace": None, "remove_effect": "released", "append_only": False})
            return
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
            # `order: append_only` slots on containers without an index/list API (QStatusBar,
            # QDialogButtonBox): insert + remove only, membership by QWidget::isAncestorOf (A1-GAPS G13).
            s["append_only"] = s.get("order") == "append_only" and "index_of" not in raw and "list" not in raw
            for need in (("insert", "remove") if s["append_only"] else ("insert", "remove", "index_of", "list")):
                if need not in raw:
                    raise GenError(f"{where}: missing op {need}")
            ins = raw["insert"]
            ops["append"] = self._op(where + ".insert", host_cls, ins)
            if s["append_only"]:
                ops["remove"] = self._op(where + ".remove", host_cls, raw["remove"])
                if s.get("id_of"):
                    _, lm, _ = self.parse_native(where + ".id_of.list", s["id_of"]["list"])
                    _, im, _ = self.parse_native(where + ".id_of.item", s["id_of"]["item"])
                    s["id_of"] = {**s["id_of"], "list_method": lm, "item_method": im}
                rc = raw["remove"].get("effects", {}).get("child")
                s["remove_effect"] = {"unparented": "released"}.get(rc)
                if s["remove_effect"] is None:
                    raise GenError(f"{where}: remove.effects.child must be unparented")
                s["replace"] = None
                s["ops"] = ops
                for a in s.get("attrs", []):
                    self._resolve_attr(el, s, a, f"{where}.{a['name']}", host_cls)
                return
            var = [v for v in ins.get("variants", []) if v.get("when") == "before != null"]
            if s.get("order") == "unordered":
                # Position is not the order (a grid): `before` is ignored, so there is no variant.
                ops["insert"] = ops["append"]
            elif not var:
                raise GenError(f"{where}: insert needs a `before != null` variant")
            else:
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
        if s.get("attrs") and s["cardinality"] != "many":
            raise GenError(f"{where}: child attributes need a `many` slot")
        for a in s.get("attrs", []):
            self._resolve_attr(el, s, a, f"{where}.{a['name']}", host_cls)

    VALUE_CPP = {"bool": "bool", "i32": "int", "f64": "double", "string": "QString", "enum": "int"}

    def native_params(self, where: str, native: str) -> tuple[str, str, list[str]]:
        """Class, method and the declared parameter types of a native symbol (checked against the metadata)."""
        cls, name, arity = self.parse_native(where, native)
        params = self._NATIVE.match(native.strip()).group(3).strip()
        types, depth, cur = [], 0, ""
        for ch in params:
            depth += ch in "<(" 
            depth -= ch in ">)"
            if ch == "," and depth == 0:
                types.append(cur.strip())
                cur = ""
            else:
                cur += ch
        if cur.strip():
            types.append(cur.strip())
        return cls, name, [strip_cv(t) for t in types]

    def _resolve_attr(self, el: dict, s: dict, a: dict, where: str, host_cls: str) -> None:
        sibs = {x["name"] for x in s["attrs"]}
        if a["type"] == "enum" and not a.get("values"):
            raise GenError(f"{where}: an enum attribute needs `values`")
        if a["type"] == "enum" and a.get("default") is not None and a["default"] not in a["values"]:
            raise GenError(f"{where}: default {a['default']!r} is not a key of `values`")
        if "default" not in a:
            raise GenError(f"{where}: needs a `default` (what `null` resets to)")
        a["vcpp"] = self.VALUE_CPP[a["type"]]
        for key in ("set", "get"):
            op = a[key]
            if "expr" in op:
                if "native" in op:
                    raise GenError(f"{where}.{key}: give `native` or `expr`, not both")
                op.update({"args": [], "ptypes": [], "out_params": []})
                continue
            if "native" not in op:
                raise GenError(f"{where}.{key}: needs `native`")
            for arg in op["args"]:
                if arg.startswith("attr.") and arg[5:] not in sibs:
                    raise GenError(f"{where}.{key}: {arg} names no attribute of slot {s['name']}")
            cls, name, types = self.native_params(f"{where}.{key}", op["native"])
            if cls not in self.chain(host_cls):
                raise GenError(f"{where}.{key}: {cls} is not a base of {host_cls}")
            op["cls"], op["method"], op["ptypes"] = cls, name, types
            if op.get("member"):
                _, rec = self.find(cls, "methods", lambda r: r["name"] == name and len(r["params"]) >= len(types))
                rcls = re.sub(r"[\s*]|\bconst\b", "", rec["returns"]["native"]) if rec else ""
                if not rec or not self.kind(rcls):
                    raise GenError(f"{where}.{key}: cannot resolve the class of the value `member` is called on")
                self._need_method(where, rcls, op["member"], 0)
            # pointer parameters the caller does not supply are out parameters; one the args name (the child) is not
            ptr = [i for i, t in enumerate(types) if t.endswith("*") and t != "QWidget*"] if key == "get" and len(op["args"]) < len(types) else []
            op["out_params"] = ptr
            if key == "get":
                if ptr and op.get("out") not in ptr:
                    raise GenError(f"{where}.get: `out` must name one of the pointer parameters {ptr}")
                if not ptr and "out" in op:
                    raise GenError(f"{where}.get: `out` given but {op['native']} has no pointer parameter")
                if len(op["args"]) != len(types) - len(ptr) and not any(
                        q["default"] for c in self.chain(cls) for r in self.kind(c)["methods"] if r["name"] == name for q in r["params"]):
                    raise GenError(f"{where}.get: {len(op['args'])} args for {len(types) - len(ptr)} in-parameters")
            elif "value" not in op["args"]:
                raise GenError(f"{where}.set: args must contain `value`")
            elif len(op["args"]) > len(types):
                raise GenError(f"{where}.{key}: {len(op['args'])} args for {len(types)} parameters")
        a["value_ptype"] = a["set"]["ptypes"][a["set"]["args"].index("value")] if "expr" not in a["set"] else a["vcpp"]
        a["child_cls"] = s["accepts"][0]

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
        self.ids = {"prop": {}, "signal": {}, "method": {}, "slot": {}, "attr": {}}
        for e in self.elements:
            e["props_by_name"] = {p["js"]: p for p in e.get("props", [])}
            for key, items, field in (("prop", e.get("props", []), "js"), ("signal", e.get("signals", []), "js"),
                                      ("method", e.get("methods", []), "js"),
                                      ("slot", (e.get("children") or {}).get("slots", []), "name")):
                for it in items:
                    it["id"] = len(self.ids[key]) + 1
                    self.ids[key][(e["tag"], it[field])] = it["id"]
            # One id per (element, attribute name); slots of one element may share a name.
            e["attr_names"] = []
            for sl in (e.get("children") or {}).get("slots", []):
                for a in sl.get("attrs", []):
                    if a["name"] not in e["attr_names"]:
                        e["attr_names"].append(a["name"])
                        self.ids["attr"][(e["tag"], a["name"])] = len(self.ids["attr"]) + 1

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
    def header(c):
        n = model.kind(c)["native"]
        return f"{n.get('module', 'QtWidgets')}/{n['header']}"
    seen = {header(c) for c in classes if model.kind(c)}
    for o in model.owners_used:
        if model.kind(o) and o != "Qt":
            seen.add(header(o))
        elif o in GUI_HEADERS:
            seen.add(GUI_HEADERS[o])
    return "\n".join(f"#include <{h}>" for h in sorted(seen))


def host_expr(el: dict, s_host: str, obj: str = "o") -> str:
    if s_host == "layout":
        return f"static_cast<{el['layout_cls']} *>(static_cast<QWidget *>({obj})->layout())"
    return f"static_cast<{el['cls']} *>({obj})"


def self_cls(el: dict, p: dict) -> str:
    return el["layout_cls"] if p.get("target") == "layout" else el["cls"]


def target_expr(el: dict, p: dict, obj: str = "o") -> str:
    return host_expr(el, "layout" if p.get("target") == "layout" else "self", obj)








# --------------------------------------------------------------------------------------------
# backend emission

def emit_ids(m: Model) -> str:
    out = []
    for e in m.elements:
        out.append(f"constexpr svl_id K_{cident(e['tag'])} = {m.kind_id[e['tag']]};")
    for key, pre in (("prop", "P"), ("signal", "G"), ("method", "M"), ("slot", "S"), ("attr", "A")):
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
                                   ("slot", "S", (e.get("children") or {}).get("slots", []), "name"),
                                   ("attr", "A", [{"name": n} for n in e["attr_names"]], "name")):
            rows = ", ".join(f'{{"{it[f]}", {pre}_{t}_{cident(it[f])}}}' for it in items)
            out.append(f"const NameId k{pre}_{t}[] = {{{rows}{', ' if rows else ''}{{\"\", 0}}}};")
            names.append((key, e["tag"], f"k{pre}_{t}", len(items)))
    out.append("Table table_for(uint32_t what, svl_id kind) {")
    out.append(f"  if (what == SVL_KIND) return {{kKinds, {len(m.elements)}}};")
    out.append("  switch (what) {")
    for key, what in (("prop", "SVL_PROP"), ("signal", "SVL_SIGNAL"), ("method", "SVL_METHOD"), ("slot", "SVL_SLOT"), ("attr", "SVL_ATTR")):
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
        out.append(f"      auto *w = new {e['cls']}({', '.join(e['construct'].get('args', []))});")
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
                    if p.get("dynamic"):
                        # setProperty() returns false when it creates (or updates) a dynamic property: not an error.
                        out.append(f"          {tgt}->setProperty(\"{p['native']}\", to_variant(v));")
                        out.append(f"          return {p['after_set']}(e, o);" if p.get("after_set") else "          return 0;")
                    else:
                        out.append(f"          return {tgt}->setProperty(\"{p['native']}\", to_variant(v)) ? 0 : fail(\"{p['js']}: setProperty failed\");")
                else:
                    out.append(f"          out_variant(out, {tgt}->property(\"{p['native']}\")); return 0;")
            out.append("        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("' + ("set_prop" if setter else "get_prop") + ': unknown property id for this kind");\n}')
    return "\n".join(out)


def emit_async_start(recv: str, me: dict, a: dict, nin: int, call: str) -> list[str]:
    """Connect the completion signal, remember the operation for `cancel`, start it."""
    dc = a["decl_cls"]
    ptypes = ", ".join(tn for _, tn in a["qparams"])
    lam = ", ".join(f"{tn} a{i}" for i, (_, tn) in enumerate(a["qparams"]))
    if a["result_ref"]:
        i = a["result_idx"]
        done = f"Conv<Plain<decltype(a{i})>>::out(a{i}, &v[0]); async_complete(done, v, 1);"
    else:
        done = "async_complete(done, nullptr, 0);"
    cancel = f"[w] {{ static_cast<{a['cancel_cls']} *>(w)->{a['cancel']}(); }}" if a.get("cancel") else "{}"
    return [
        f'          if (args[{nin}].tag != SVL_CALLBACK) return fail("{me["js"]}: missing completion callback");',
        f"          const uint64_t done = args[{nin}].u.u;",
        f"          auto *w = static_cast<{dc} *>({recv});",
        "          g.asyncs.erase(done);",
        f"          QMetaObject::Connection conn = QObject::connect(w, static_cast<void ({dc}::*)({ptypes})>(&{dc}::{a['signal']}), w,",
        f"              [done]({lam}) {{ svl_value v[1]; (void)v; {done} }});",
        f"          g.asyncs[done] = AsyncOp{{QPointer<QObject>(w), conn, {cancel}}};",
        "          try {",
        f"            {call};",
        "          } catch (...) {",
        "            QObject::disconnect(conn);",
        "            g.asyncs.erase(done);",
        "            throw;",
        "          }",
    ]


def emit_call(m: Model) -> str:
    out = ["int gen_call(Entry &e, svl_id method, const svl_value *args, size_t n, svl_value *out) {",
           "  QObject *o = e.obj.data();", "  (void)args; (void)out; std::string er;", "  switch (e.kind) {"]
    for e in m.elements:
        t = cident(e["tag"])
        if not e.get("methods"):
            continue
        out.append(f"    case K_{t}:\n      switch (method) {{")
        for me in e["methods"]:
            n = me["nin"]
            asyn = me.get("async")
            out.append(f"        case M_{t}_{cident(me['js'])}: {{")
            out.append(f"          if (n != {n + (1 if asyn else 0)}) return fail(\"{me['js']}: expected {n + (1 if asyn else 0)} argument(s)\");")
            names, j = [], 0
            for i, (ty, an, mode, ptr, cb) in enumerate(zip(me["argtypes"], me["argnames"], me["modes"], me["ptrs"], me["cbs"])):
                if cb:
                    out.append(f'          if (args[{j}].tag != SVL_CALLBACK) return fail("{me["js"]}({an}): expected a function");')
                    out.append(f"          const uint64_t cb{i} = args[{j}].u.u;")
                    params = ", ".join(f"{a['cpp']} a{k}" for k, a in enumerate(cb.get("args", [])))
                    rcpp = cb["returns"]["cpp"] if cb.get("returns") else "void"
                    call_args = ", ".join(["cb%d" % i] + [f"a{k}" for k in range(len(cb.get("args", [])))])
                    body = f"js_invoke<{rcpp}>({call_args});" if rcpp == "void" else f"return js_invoke<{rcpp}>({call_args});"
                    names.append(f"[cb{i}]({params}) -> {rcpp} {{ {body} }}")
                    j += 1
                    continue
                out.append(f"          {ty} a{i}{{}};")
                if mode != "out":
                    out.append(f'          if (!conv_in(&args[{j}], a{i}, "{me["js"]}({an})", er)) return fail(er);')
                    j += 1
                names.append(f"&a{i}" if ptr else f"a{i}")
            # Through the declaring class: an override may make the method protected in the element's class.
            recv = target_expr(e, me)
            if me["decl_cls"] != self_cls(e, me):
                recv = f"static_cast<{me['decl_cls']} *>({recv})"
            call = f"{recv}->{me['native']}({', '.join(names)})"
            outs = [i for i, mode in enumerate(me["modes"]) if mode != "in"]
            if asyn:
                out.extend(emit_async_start(target_expr(e, me), me, asyn, n, call))
                out.append("          return 0;\n        }")
                continue
            if me["ret"] == "void":
                out.append(f"          {call};")
            else:
                out.append(f"          auto r = {call};")
            if outs:
                # [ret, outs...] in parameter order; the host shapes the object.
                out.append(f"          if (out) {{\n            g_out.lists.emplace_back({len(outs) + 1}, mkv(SVL_NULL));\n            auto &vec = g_out.lists.back();")
                if me["ret"] != "void":
                    out.append("            Conv<Plain<decltype(r)>>::out(r, &vec[0]);")
                for k, i in enumerate(outs):
                    out.append(f"            Conv<Plain<decltype(a{i})>>::out(a{i}, &vec[{k + 1}]);")
                out.append("            *out = mkv(SVL_LIST);\n            out->u.l.ptr = vec.data();\n            out->u.l.len = vec.size();\n          }")
            elif me["ret"] != "void":
                out.append("          if (out) Conv<Plain<decltype(r)>>::out(r, out);")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("call: unknown method id for this kind");\n}')
    return "\n".join(out)


def cb_slots(m: Model, args: list[dict]) -> str:
    return ", ".join(f'{{"{a["name"]}", Conv<{a["cpp"]}>::slot("{a["name"]}")}}' for a in args)


def cb_type_text(m: Model, cb: dict) -> str:
    args = ", ".join(f"{a['name']}: {ts_type(m, a['ref'])}" for a in cb.get("args", []))
    ret = ts_type(m, cb["returns"]["ref"]) if cb.get("returns") else "void"
    return f"({args}) => {ret}"


def cb_type_expr(m: Model, cb: dict, scope: str, name: str) -> str:
    """A function-local static that registers the callback's TypeDesc on first describe()."""
    sc = {"call": "SVL_CB_CALL", "notified": "SVL_CB_NOTIFIED"}[scope]
    ret = f'Conv<{cb["returns"]["cpp"]}>::slot(nullptr)' if cb.get("returns") else "slot_desc(nullptr, SVL_NULL)"
    return (f'([]{{ static svl_id t = new_callback_type("{cb_type_text(m, cb)}", {sc}, '
            f'{{{cb_slots(m, cb.get("args", []))}}}, {ret}); return t; }})()')


def async_type_expr(m: Model, me: dict) -> str:
    a = me["async"]
    if a["result_ref"]:
        idx = a["result_idx"]
        cpp = strip_cv(a["qparams"][idx][1])
        fields = f'{{"result", Conv<{cpp}>::slot("result")}}'
        text = f"(result: {ts_type(m, a['result_ref'])}) => void"
    else:
        fields, text = "", "() => void"
    return (f'([]{{ static svl_id t = new_callback_type("{text}", SVL_CB_ASYNC, {{{fields}}}, slot_desc(nullptr, SVL_NULL)); '
            f'return t; }})()')


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
                    flag = {"out": "SVL_D_OUT", "inout": "SVL_D_INOUT"}
                    parts = []
                    for ty, an, mode, cb in zip(it["argtypes"], it["argnames"], it["modes"], it["cbs"]):
                        if cb:
                            parts.append(f'slot_desc("{an}", SVL_CALLBACK, 0, {cb_type_expr(m, cb, cb["scope"], an)})')
                        elif mode != "in":
                            parts.append(f'with_flags(Conv<{ty}>::slot("{an}"), {flag[mode]})')
                        else:
                            parts.append(f'Conv<{ty}>::slot("{an}")')
                    mflags = "0"
                    if it.get("async"):
                        parts.append(f'slot_desc("done", SVL_CALLBACK, 0, {async_type_expr(m, it)})')
                        n += 1
                        mflags = "SVL_M_ASYNC"
                    slots = ", ".join(parts) or "slot_desc(nullptr, SVL_NULL)"
                    out.append(f"              static svl_slot_desc ps[{max(n, 1)}] = {{{slots}}};")
                    if it["ret"] == "void" or it.get("async"):
                        ret = "slot_desc(nullptr, SVL_NULL)"
                    else:
                        ret = f'Conv<{m.cpp_type(it["ret"], it["ret_native"])}>::slot(nullptr)'
                    out.append(f'              static svl_member_desc d = {{"{it["js"]}", ps, {n}, {ret}, {mflags}}};')
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
            elif a == "id":
                args.append((extra or {})["id"])
            elif isinstance(a, str) and a.startswith("const:"):
                args.append(a[6:])
            else:
                raise GenError(f"unknown op arg {a}")
        recv = child if o.get("receiver") == "child" else host
        return f"{recv}->{o['method']}({', '.join(args)})"
    return "; ".join([one(op)] + [one(t) for t in op["then"]])


def emit_insert(m: Model) -> str:
    out = ["int gen_insert(Entry &pe, QObject *child, QObject *before, svl_id &slot) {",
           "  QObject *o = pe.obj.data();", "  (void)before;", "  switch (pe.kind) {"]
    for e in m.elements:
        ch = e.get("children")
        if not ch:
            continue
        t = cident(e["tag"])
        routes = "".join(f"      if (slot == 0 && qobject_cast<{x['accepts'][0]} *>(child)) slot = S_{t}_{cident(x['name'])};\n"
                         for x in ch["slots"] if x.get("route_by_kind"))
        out.append(f"    case K_{t}:\n{routes}      if (slot == 0) slot = S_{t}_{cident(ch['default_slot'])};\n      switch (slot) {{")
        for s in ch["slots"]:
            ops = s["ops"]
            ctype = s["accepts"][0]
            if s.get("actions"):
                out.append(f"        case S_{t}_{cident(s['name'])}: return menu_insert(o, child, before);")
                continue
            out.append(f"        case S_{t}_{cident(s['name'])}: {{")
            out.append(f"          auto *ch = qobject_cast<{ctype} *>(child);")
            out.append(f"          if (!ch) return fail(\"slot {s['name']} of <{e['tag']}> does not accept this child\");")
            out.append(f"          auto *h = {host_expr(e, s['host'])};")
            if s["cardinality"] == "one":
                # Release the old child first when the setter would not (takeCentralWidget, or a clearing
                # remove with `then` steps such as QDockWidget::setWidget(nullptr) + setParent(nullptr)).
                rm = ops.get("take") or (ops["clear"] if ops.get("clear", {}).get("then") else None)
                if s["replace"] == "releases_old" and rm:
                    out.append(f"          if (auto *old = {op_call('h', ops['get'])}; old && old != ch) {{ {op_call('h', rm, 'old')}; }}")
                out.append(f"          {op_call('h', ops['set'], 'ch')};")
            elif s.get("append_only"):
                out.append(f"          if (before) return fail(\"slot {s['name']} of <{e['tag']}> is append-only (no insert-before)\");")
                out.append(f"          if (static_cast<QWidget *>(o)->isAncestorOf(ch)) return 0;")
                out.append(f"          {op_call('h', ops['append'], 'ch')};")
            elif s.get("order") == "unordered":
                # No order: `before` is ignored, and a child already in the host stays where it is (with its attributes).
                out.append(f"          if ({op_call('h', ops['index_of'], 'ch')} >= 0) return 0;")
                out.append(f"          {op_call('h', ops['append'], 'ch')};")
            else:
                out.append("          if (before == child) return 0;")
                out.append(f"          if (int i = {op_call('h', ops['index_of'], 'ch')}; i >= 0) {{ {op_call('h', ops['remove'], 'ch', {'index': 'i'})}; }}")
                out.append("          if (before) {")
                out.append(f"            auto *bw = qobject_cast<QWidget *>(before);")
                out.append(f"            int i = bw ? {op_call('h', ops['index_of'], 'bw')} : -1;")
                out.append(f"            if (i < 0) return fail(\"insert: `before` is not a child of this slot\");")
                out.append(f"            {op_call('h', ops['insert'], 'ch', {'index': 'i'})};")
                out.append(f"          }} else {{ {op_call('h', ops['append'], 'ch')}; }}")
                for a in s.get("attrs", []):
                    if a.get("reset"):
                        out.append(f"          a_set_{attr_fn(e, s, a)}(h, ch, {attr_default_cpp(a)});  // state that outlives the slot")
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
            if s.get("actions"):
                out.append("      if (menu_is_item(child)) return menu_remove(o, child);")
                continue
            out.append(f"      if (auto *ch = qobject_cast<{ctype} *>(child)) {{")
            out.append(f"        auto *h = {host_expr(e, s['host'])};")
            if s["cardinality"] == "one":
                out.append(f"        if (static_cast<QWidget *>(o)->isAncestorOf(ch) && {op_call('h', ops['get'])} == ch) {{")
                if "take" in ops:
                    out.append(f"          {op_call('h', ops['take'], 'ch')};")
                else:
                    out.append(f"          {op_call('h', ops['clear'], 'ch')};")
                out.append("          return 0;\n        }")
            elif s.get("append_only"):
                out.append(f"        if (static_cast<QWidget *>(o)->isAncestorOf(ch)) {{")
                if s.get("id_of"):
                    # Removal by an id the child does not carry (QWizard::removePage): find it (A1-GAPS G18).
                    lst, item = s["id_of"]["list_method"], s["id_of"]["item_method"]
                    out.append(f"          int id = -1; for (int k : h->{lst}()) if (h->{item}(k) == ch) id = k;")
                    out.append("          if (id < 0) return fail(\"remove: child has no id in this slot\");")
                out.append(f"          {op_call('h', ops['remove'], 'ch', {'id': 'id'})};")
                out.append("          return 0;\n        }")
            else:
                out.append(f"        if (int i = {op_call('h', ops['index_of'], 'ch')}; i >= 0) {{")
                for a in s.get("attrs", []):
                    if a.get("reset"):
                        out.append(f"          a_set_{attr_fn(e, s, a)}(h, ch, {attr_default_cpp(a)});  // state that outlives the slot")
                out.append(f"          {op_call('h', ops['remove'], 'ch', {'index': 'i'})};")
                out.append("          return 0;\n        }")
            out.append("      }")
        out.append("      break;\n    }")
    out.append("    default: break;\n  }")
    out.append('  return fail("remove: child is not in this parent");\n}')
    return "\n".join(out)


# ---- child attributes (X3) ---------------------------------------------------------------------
#
# Per (element, slot, attribute) the generator emits `a_get_*` / `a_set_*` helpers that wrap the
# annotated native calls on the slot host, and two dispatchers keyed by parent kind, attribute id and
# the slot the child was inserted through (`Entry::slot`). State lives in Qt: nothing is cached.

def attr_fn(e: dict, s: dict, a: dict) -> str:
    return f"{cident(e['tag'])}_{cident(s['name'])}_{cident(a['name'])}"


def attr_default_cpp(a: dict) -> str:
    d, t = a["default"], a["type"]
    if t == "bool":
        return "true" if d else "false"
    if t == "i32":
        return str(int(d))
    if t == "f64":
        return repr(float(d))
    if t == "string":
        return 'QString::fromUtf8("%s")' % str(d).replace("\\", "\\\\").replace('"', '\\"')
    return f"static_cast<int>({a['values'][d]})"


def attr_arg(e: dict, s: dict, a: dict, arg: str, ptype: str) -> str:
    if arg == "child":
        return "ch" if ptype.replace(" ", "") == "QWidget*" else f"static_cast<{ptype.replace('*', ' *').replace('  ', ' ')}>(ch)"
    if arg == "value":
        return f"from_int<{ptype}>(x)" if a["type"] == "enum" else "x"
    if arg == "child_index":
        return "ci"
    if arg.startswith("attr."):
        o = next(x for x in s["attrs"] if x["name"] == arg[5:])
        call = f"a_get_{attr_fn(e, s, o)}(h, ch)"
        return f"from_int<{ptype}>({call})" if o["type"] == "enum" else call
    if arg.startswith("const:"):
        return arg[6:]
    raise GenError(f"unknown attribute argument {arg}")


def emit_attr_helpers(m: Model) -> list[str]:
    dget, dset, tabs = [], [], []
    for e in m.elements:
        for s in (e.get("children") or {}).get("slots", []):
            if not s.get("attrs"):
                continue
            ops = s["ops"]
            hostt = e["layout_cls"] if s["host"] == "layout" else e["cls"]
            idx = "0" if s.get("append_only") else op_call("h", ops["index_of"], "ch")
            for a in s.get("attrs", []):
                fn = attr_fn(e, s, a)
                vt = a["vcpp"]
                if a["type"] == "enum":
                    rows = ", ".join(f'{{"{k}", static_cast<int>({v})}}' for k, v in sorted(a["values"].items()))
                    tabs.append(f"const std::pair<const char *, int> kAE_{fn}[] = {{{rows}}};")
                g = a["get"]
                gargs = iter(g["args"])
                call_args = []
                for i, pt in enumerate(g["ptypes"]):
                    call_args.append(f"&o{i}" if i in g["out_params"] else attr_arg(e, s, a, next(gargs), pt))
                body = [f"{vt} a_get_{fn}({hostt} *h, QWidget *ch) {{", f"  int ci = {idx};", "  (void)ci; (void)h;"]
                for i in g["out_params"]:
                    body.append(f"  {g['ptypes'][i][:-1].strip()} o{i} = 0;")
                call = g["expr"].replace("$h", "h").replace("$c", f"static_cast<{a['child_cls']} *>(ch)") if "expr" in g else f"h->{g['method']}({', '.join(call_args)})"
                if g.get("member"):
                    call += f"->{g['member']}()"
                if g["out_params"]:
                    body += [f"  {call};", f"  return static_cast<{vt}>(o{g['out']});"]
                else:
                    body += [f"  return static_cast<{vt}>({call});"]
                body.append("}")
                dget.append("\n".join(body))
                st = a["set"]
                sargs = [attr_arg(e, s, a, x, pt) for x, pt in zip(st["args"], st["ptypes"])]
                body = [f"void a_set_{fn}({hostt} *h, QWidget *ch, {vt} x) {{", f"  int ci = {idx};", "  (void)ci;"]
                if st.get("readd"):
                    # Read every argument first: removing the widget drops the layout item that holds them.
                    for i, x in enumerate(sargs):
                        body.append(f"  auto r{i} = {x};")
                    body.append(f"  {op_call('h', {**ops['remove'], 'then': []}, 'ch', {'index': 'ci'})};")
                    sargs = [f"r{i}" for i in range(len(sargs))]
                if "expr" in st:
                    expr = st["expr"].replace("$h", "h").replace("$v", "x").replace("$c", f"static_cast<{a['child_cls']} *>(ch)")
                    body += [f"  {expr};", "}"]
                else:
                    body += [f"  h->{st['method']}({', '.join(sargs)});", "}"]
                dset.append("\n".join(body))
    return tabs + dget + dset


def emit_attrs(m: Model) -> str:
    out = emit_attr_helpers(m)
    for setter in (True, False):
        w = "set_child_attr" if setter else "get_child_attr"
        out += [("int gen_set_child_attr(Entry &pe, Entry &ce, svl_id attr, const svl_value *v) {" if setter else
                 "int gen_get_child_attr(Entry &pe, Entry &ce, svl_id attr, svl_value *out) {"),
                "  QObject *o = pe.obj.data();", "  QObject *co = ce.obj.data();", "  (void)o; (void)co;",
                "  std::string er; (void)er;", "  switch (pe.kind) {"]
        for e in m.elements:
            if not e["attr_names"]:
                continue
            t = cident(e["tag"])
            out += [f"    case K_{t}:", "      switch (attr) {"]
            for an in e["attr_names"]:
                out += [f"        case A_{t}_{cident(an)}:", "          switch (ce.slot) {"]
                for s in e["children"]["slots"]:
                    a = next((x for x in s.get("attrs", []) if x["name"] == an), None)
                    if not a:
                        continue
                    fn = attr_fn(e, s, a)
                    out += [f"            case S_{t}_{cident(s['name'])}: {{",
                            f"              auto *h = {host_expr(e, s['host'])};",
                            "              auto *ch = qobject_cast<QWidget *>(co);",
                            f"              if (!ch || !({'static_cast<QWidget *>(o)->isAncestorOf(ch)' if s.get('append_only') else op_call('h', s['ops']['index_of'], 'ch') + ' >= 0'})) return fail(\"{w}: child attributes need an inserted child\");"]
                    if setter:
                        out += [f"              {a['vcpp']} x{{}};", f"              if (v->tag == SVL_NULL) x = {attr_default_cpp(a)};", "              else {"]
                        if a["type"] == "enum":
                            out += ["                QString q;", f'                if (!conv_in(v, q, "{an}", er)) return fail(er);',
                                    "                bool found = false;",
                                    f"                for (const auto &kv : kAE_{fn}) if (q == QLatin1String(kv.first)) {{ x = kv.second; found = true; break; }}",
                                    f'                if (!found) return fail("{an}: unknown value \\"" + q.toStdString() + "\\"");']
                        else:
                            out.append(f'                if (!conv_in(v, x, "{an}", er)) return fail(er);')
                        out += ["              }", f"              a_set_{fn}(h, ch, x);", "              return 0;", "            }"]
                    else:
                        out.append(f"              auto r = a_get_{fn}(h, ch);")
                        if a["type"] == "enum":
                            out += [f"              for (const auto &kv : kAE_{fn}) if (kv.second == r) {{ out_str(out, QString::fromUtf8(kv.first)); return 0; }}",
                                    f'              return fail("{an}: the native value " + std::to_string(r) + " has no name");']
                        else:
                            out += ["              Conv<Plain<decltype(r)>>::out(r, out);", "              return 0;"]
                        out.append("            }")
                out += ["            default: break;", "          }",
                        f'          return fail("{w}: attribute {an} does not apply to the slot the child is in");']
            out += ["        default: break;", "      }", "      break;"]
        out += ["    default: break;", "  }", f'  return fail("{w}: unknown attribute id for this parent kind");', "}"]
    return "\n".join(out)


# Synthetic signals backed by the event-family filter of backend.cpp.in (ABI v2.4): family -> EvFam enumerator.
EVENT_FAMILIES = {
    "key_release": "EV_KEY_RELEASE", "mouse_press": "EV_MOUSE_PRESS", "mouse_release": "EV_MOUSE_RELEASE",
    "mouse_move": "EV_MOUSE_MOVE", "mouse_double_click": "EV_DOUBLE_CLICK", "wheel": "EV_WHEEL",
    "focus_in": "EV_FOCUS_IN", "focus_out": "EV_FOCUS_OUT", "enter": "EV_ENTER", "leave": "EV_LEAVE",
    "resize": "EV_RESIZE", "context_menu": "EV_CONTEXT_MENU",
}


def emit_listen(m: Model) -> str:
    out = ["int gen_listen(svl_handle h, Entry &e, svl_id sig, bool on, uint32_t flags) {", "  QObject *o = e.obj.data();",
           "  const ListenKey key{h, sig, flags};",
           "  const svl_id wire = sig | (flags << SVL_SIGNAL_FLAGS_SHIFT);  // what post_event / call_sync carry",
           "  switch (e.kind) {"]
    for e in m.elements:
        if not e.get("signals"):
            continue
        t = cident(e["tag"])
        out.append(f"    case K_{t}:\n      switch (sig) {{")
        for s in e["signals"]:
            sid = f"G_{t}_{cident(s['js'])}"
            if s.get("synthetic"):
                if s["synthetic"] == "key_press":
                    out.append(f"        case {sid}: return listen_key(h, e, sig, on, flags);")
                elif s["synthetic"] in EVENT_FAMILIES:
                    out.append(f"        case {sid}: return listen_event(h, e, sig, on, flags, {EVENT_FAMILIES[s['synthetic']]});")
                else:
                    # onRequestClose: while listened, the JS handler decides each native close.
                    out.append(f"        case {sid}: {{")
                    out.append('          if (flags && on) return fail("listen: no capture or after phase for event-filter signals (the close handler decides through call_sync)");')
                    out.append("          if (!flags) { e.close_listen = on; e.close_sig = sig; }")
                    out.append("          return 0;\n        }")
                continue
            ptypes = ", ".join(tn for _, tn in s["qparams"])
            lam_params = ", ".join(f"{tn} a{i}" for i, (_, tn) in enumerate(s["qparams"]))
            out.append(f"        case {sid}: {{")
            out.append("          if (!on) {")
            out.append("            auto it = g.listeners.find(key);")
            out.append("            if (it != g.listeners.end()) { QObject::disconnect(it->second); g.listeners.erase(it); }")
            out.append("            g.after_make.erase(key);")
            out.append("            return 0;\n          }")
            out.append('          if (flags & SVL_LISTEN_CAPTURE) return fail("listen: signals of QObjects have no capture phase (only key-press, an event filter, can be captured)");')
            out.append("          if (g.listeners.count(key)) return 0;")
            out.append(f"          auto *w = static_cast<{s['decl_cls']} *>(o);")
            body = []
            if s["args"]:
                body.append("g_out.clear(); svl_value vals[%d];" % len(s["args"]))
                names = [n for n, _ in s["qparams"]]
                for i, a in enumerate(s["args"]):
                    pi = names.index(a["name"])
                    body.append(f"Conv<Plain<decltype(a{pi})>>::out(a{pi}, &vals[{i}]);")
                body.append(f"post(h, wire, vals, {len(s['args'])});")
            else:
                body.append("post(h, wire, nullptr, 0);")
            # `after`: Qt runs slots in connection order, so an after-connection is kept behind every plain one
            # (keep_after_last re-makes it whenever a plain connection is added).
            out.append(f"          auto make = [w, h, wire]() {{ return QObject::connect(w, static_cast<void ({s['decl_cls']}::*)({ptypes})>(&{s['decl_cls']}::{s['native']}), w,")
            out.append(f"              [h, wire]({lam_params}) {{ {' '.join(body)} }}); }};")
            out.append("          g.listeners[key] = make();")
            out.append("          if (flags & SVL_LISTEN_AFTER) g.after_make[key] = make;")
            out.append("          else keep_after_last(h, sig);")
            out.append("          return 0;\n        }")
        out.append("        default: break;\n      }\n      break;")
    out.append("    default: break;\n  }")
    out.append('  return fail("listen: unknown signal id for this kind");\n}')
    return "\n".join(out)


GETTER_BASE = 1 << 20
_GETTER_SKIP = re.compile(r"^(take|create|new|clone|grab|make|release|detach)")


def getter_table(m: Model) -> dict[str, list[tuple[str, str]]]:
    """Object getters from the qdoc metadata of every QObject-derived class: name -> [(receiver
    class, returned class)]. Public, non-static methods without required parameters that return a
    pointer to a QObject-derived class we have metadata (and so a header) for. `take*`/`create*`
    and friends are left out: the caller owns what they return."""
    table: dict[str, list[tuple[str, str]]] = {}
    for p in sorted(m.meta_dir.glob("*.json")):
        k = m.kind(p.stem)
        if not k or k.get("kind") != "object" or not k.get("qobject"):
            continue
        for me in k["methods"]:
            if me["static"] or me["access"] != "public" or me["deprecated"] or _GETTER_SKIP.match(me["name"]):
                continue
            mo = OBJ_REF.fullmatch(me["returns"]["ref"])
            if not mo or any(not q["default"] for q in me["params"]):
                continue
            rk = m.kind(mo.group(1))
            if not rk or not rk.get("qobject") or rk.get("kind") != "object":
                continue
            row = (p.stem, mo.group(1))
            if row not in table.setdefault(me["name"], []):
                table[me["name"]].append(row)
    return table


def emit_getters(m: Model) -> str:
    """`gen_kind_of` (kind of a borrowed object: the nearest annotated class) and the getter
    dispatch behind the ids from GETTER_BASE."""
    out = ["svl_id gen_kind_of(QObject *o) {",
           "  for (const QMetaObject *mo = o->metaObject(); mo; mo = mo->superClass()) {",
           "    const char *n = mo->className();"]
    seen = set()
    for e in m.elements:
        # Composite elements (a QWidget with a layout) cannot be told from a plain QWidget.
        if e["layout_cls"] or e["cls"] in seen:
            continue
        seen.add(e["cls"])
        out.append(f'    if (!std::strcmp(n, "{e["cls"]}")) return K_{cident(e["tag"])};')
    out += ["  }", "  return 0;", "}", ""]
    tab = getter_table(m)
    names = ["parent"] + sorted(n for n in tab if n != "parent")
    out.append(f"constexpr svl_id kGetterBase = {GETTER_BASE};")
    out.append(f"constexpr size_t kGetterCount = {len(names)};")
    out.append("const std::string_view kGetterNames[] = {" + ", ".join(f'"{n}"' for n in names) + "};")
    out.append("int gen_getter(Entry &e, svl_id id, QObject **out) {")
    out.append("  QObject *o = e.obj.data();")
    out.append("  switch (id - kGetterBase) {")
    for i, n in enumerate(names):
        out.append(f"    case {i}: {{  // {n}")
        if n == "parent":
            out.append("      *out = o->parent();")
            out.append("      return 0;")
        else:
            for recv, ret in sorted(tab[n]):
                out.append(f"      if (auto *t = qobject_cast<{recv} *>(o)) {{ *out = const_cast<QObject *>(static_cast<const QObject *>(t->{n}())); return 0; }}")
            out.append(f'      return fail("{n}: does not apply to a " + std::string(o->metaObject()->className()));')
        out.append("    }")
    out.append("    default: break;")
    out.append("  }")
    out.append('  return fail("call: unknown method id");')
    out.append("}")
    return "\n".join(out)


def getter_classes(m: Model) -> set[str]:
    out = set()
    for recv_ret in getter_table(m).values():
        for recv, ret in recv_ret:
            out.update((recv, ret))
    return out


def emit_toplevel(m: Model) -> str:
    """Kinds of the toplevel lifetime class: windows and dialogs. A parentless QWidget is a Qt
    window too (until it is inserted somewhere), so the class cannot be read off the object."""
    tops = [e for e in m.elements if m.derives(e["cls"], "QMainWindow") or m.derives(e["cls"], "QDialog")]
    out = ["bool gen_is_toplevel(svl_id kind) {", "  switch (kind) {"]
    out += [f"    case K_{cident(e['tag'])}: return true;" for e in tops]
    out += ["    default: return false;", "  }", "}"]
    return "\n".join(out)


def emit_context(m: Model) -> str:
    """Kinds that attach to the widget they are inserted into and pop up on its context-menu request."""
    out = ["bool gen_is_context(svl_id kind) {", "  switch (kind) {"]
    out += [f"    case K_{cident(e['tag'])}: return true;" for e in m.elements if e["construct"].get("context_menu")]
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
    classes |= getter_classes(m)
    parts = {
        "DIGEST": digest, "INCLUDES": inc(m, classes), "IDS": emit_ids(m), "TABLES": emit_tables(m),
        "CREATE": emit_create(m), "SET_PROP": prop_cases(m, True), "GET_PROP": prop_cases(m, False),
        "CALL": emit_call(m), "DESCRIBE": emit_describe(m), "ENUMS": emit_enums(m), "ATTRS": emit_attrs(m), "INSERT": emit_insert(m), "REMOVE": emit_remove(m), "LISTEN": emit_listen(m),
        "TOPLEVEL": emit_toplevel(m), "GETTERS": emit_getters(m), "CONTEXT": emit_context(m),
    }
    parts["SERDES"] = (HERE / "templates" / "serdes.hpp.in").read_text()
    parts["VLIST"] = (HERE / "templates" / "vlist.hpp.in").read_text()
    parts["MENUS"] = (HERE / "templates" / "menus.hpp.in").read_text()
    parts["CANVAS"] = (HERE / "templates" / "canvas.hpp.in").read_text()
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
        self.synthetic_done: set[str] = set()
        self.src: list[str] = []
        self.facts: dict[str, dict] = {}
        self.errors: list[str] = []
        self._fact: tuple[str, dict] | None = None

    def add(self, name: str, body: str) -> None:
        if getattr(self, "_setup", None):
            mk, stmt = self._setup
            body = body.replace(mk, mk + "\n" + stmt, 1)
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
                    for a in sl.get("attrs", []):
                        self.fact(fact_id(e, "slot", f"{sl['name']}:attr:{a['name']}"), a["_raw"], "attrs", e, f"{sl['name']}.{a['name']}",
                                  lambda e=e, sl=sl, a=a: self.attr(e, sl, a))
            for b in e.get("bindings", []):
                self.fact(fact_id(e, "binding", b["name"]), b["_raw"], "bindings", e, b["name"], lambda e=e, b=b: self.binding(e, b))
            if e["construct"].get("context_menu"):
                self.fact(fact_id(e, "construct", "context_menu"), e["construct"], "construct", e, "context_menu", lambda e=e: self.context_menu(e))
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
        """Prop tests; `setup` (C++, $w = widget) runs right after the element is created."""
        if p.get("setup"):
            self._setup = (f'  svl_handle h = mk("{e["tag"]}");', "  " + p["setup"].replace("$w", f"static_cast<{e['cls']} *>(O(h))") + ";")
        try:
            self._prop(e, p)
        finally:
            self._setup = None

    def _prop(self, e, p):
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
            if cat not in SCALAR:
                # Non-scalar values (dates, records): no literal sample. Read the value, change it natively
                # with nothing listening (no backend output is produced, so `out` stays valid), then write the
                # first value back through the backend while listening: nothing may be posted (A1-GAPS G16).
                trig = lambda i: sigs[0]["trigger"].replace("$w", f"static_cast<{e['cls']} *>(O(h))").replace("$i", str(i))
                lines = [f'  svl_handle h = mk("{t}");', f"  {trig(0)};", "  svl_value out{};",
                         f"  CHECK(B->get_prop(B->self, h, {P_}, &out) == 0);", f"  {trig(1)};"]
                for sg in sigs:
                    lines.append(f'  CHECK(B->listen(B->self, h, G("{t}", "{sg["js"]}"), true) == 0);')
                lines += [f"  CHECK(B->set_prop(B->self, h, {P_}, &out) == 0);",
                          "  CHECK(rec.events.empty());  // no echo for programmatic sets",
                          f"  {trig(2)};",
                          "  CHECK(!rec.events.empty());  // native changes are still delivered",
                          "  B->destroy(B->self, h);"]
                self.add(f"signal/{t}.{pn}/no_echo_on_set_prop", "\n".join(lines))
                return
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
        if me.get("async") or any(me["cbs"]):
            synth = False  # completion callbacks and JS functions are exercised by examples/counter-qt/tests
        for i, ref in enumerate(me["argrefs"]):
            if me["modes"][i] == "out":
                continue  # filled in by the callee, not sent
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
        lines += [f'  CHECK(B->call(B->self, h, M("{t}", "{me["js"]}"), nullptr, {me["nin"] + (1 if me.get("async") else 0) + 1}, nullptr) < 0);',
                  "  B->destroy(B->self, h);"]
        self.add(f"method/{t}.{me['js']}", "\n".join(lines))

    def signal(self, e, s):
        t = e["tag"]
        if s.get("synthetic"):
            if s.get("inherited_from") and s["js"] in self.synthetic_done:
                return  # an inherited event family is verified on the first element that has it
            self.synthetic_done.add(s["js"])
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

    def context_menu(self, e):
        """`context_menu` elements attach to the widget they are inserted into (menus.hpp.in)."""
        t = e["tag"]
        self.add(f"construct/{t}.context_menu", "\n".join([
            f'  svl_handle w = mk("box-layout"); svl_handle m = mk("{t}");',
            "  auto *wd = static_cast<QWidget *>(O(w)); auto *menu = qobject_cast<QMenu *>(O(m));",
            "  CHECK(B->insert(B->self, w, m, 0, 0) == 0);",
            "  CHECK(wd->contextMenuPolicy() == Qt::CustomContextMenu && menu->parentWidget() == wd);  // owned by the widget",
            f'  svl_handle it = mk("action"); CHECK(B->insert(B->self, m, it, 0, S("{t}", "items")) == 0);  // an empty menu does not open',
            "  Q_EMIT wd->customContextMenuRequested(QPoint(3, 4));",
            "  CHECK(menu->isVisible());  // popup(), not exec()",
            "  menu->hide();",
            "  CHECK(B->insert(B->self, w, m, 0, 0) < 0);  // already attached",
            "  CHECK(B->remove(B->self, w, m) == 0);",
            "  CHECK(wd->contextMenuPolicy() == Qt::DefaultContextMenu && menu->parent() == nullptr);  // released",
            "  CHECK(B->remove(B->self, w, m) < 0);",
            "  CHECK(B->insert(B->self, w, m, 0, 0) == 0);  // can be attached again",
            "  QPointer<QObject> pm(O(m));",
            "  B->destroy(B->self, w);",
            "  flush();",
            "  CHECK(pm.isNull());  // deleting the widget deletes its context menu",
            "  B->destroy(B->self, m);"]))

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

    def slot_append_only(self, e, s):
        m, t, sn = self.m, e["tag"], s["name"]
        child = m.accepted_child_tag(e, s)
        sid = f'S("{t}", "{sn}")'
        anc = lambda c: f"static_cast<QWidget *>(O(p))->isAncestorOf(static_cast<QWidget *>(O({c})))"
        mk = [f'  svl_handle p = mk("{t}"); svl_handle c0 = mk("{child}"); svl_handle c1 = mk("{child}");']
        clean = "  B->destroy(B->self, p); B->destroy(B->self, c0); B->destroy(B->self, c1);"
        self.add(f"slot/{t}.{sn}/append", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c1, 0, {sid}) == 0);",
            f"  CHECK({anc('c0')} && {anc('c1')});",
            f"  CHECK(B->insert(B->self, p, c1, c0, {sid}) < 0);  // append-only: no insert-before",
            clean]))
        self.add(f"ownership/{t}.{sn}/remove_released", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            "  QPointer<QObject> p0(O(c0));",
            "  CHECK(B->remove(B->self, p, c0) == 0);",
            "  flush();",
            f"  CHECK(!p0.isNull() && !{anc('c0')});  // released, not deleted",
            "  CHECK(!was_destroyed(c0));",
            "  CHECK(B->remove(B->self, p, c0) < 0);",
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0 && {anc('c0')});  // can be inserted again",
            clean]))
        self.add(f"ownership/{t}.{sn}/parent_destroy_deletes_children", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            "  QPointer<QObject> pc(O(c0));",
            "  B->destroy(B->self, p);",
            "  flush();",
            "  CHECK(pc.isNull());",
            "  CHECK(was_destroyed(c0));",
            clean]))

    @staticmethod
    def attr_value(a: dict, v, name: str) -> tuple[str, str]:
        """(C++ declaration of an svl_value `name`, C++ string literal `valstr` gives for it)."""
        ty = a["type"]
        if ty == "bool":
            return f"svl_value {name} = vbool({'true' if v else 'false'});", '"true"' if v else '"false"'
        if ty == "i32":
            return f"svl_value {name} = vi64({int(v)});", f'"{int(v)}"'
        if ty == "f64":
            return f"svl_value {name} = vf64({float(v)!r});", f'"{float(v):g}"'
        lit = json.dumps(str(v))
        return f"std::string s_{name} = {lit}; svl_value {name} = vstr(s_{name});", lit

    def attr(self, e, s, a):
        t, sn, an = e["tag"], s["name"], a["name"]
        if len(a.get("samples", [])) != 2:
            raise GenError(f"attribute {an} needs two `samples` (non-default, different)")
        if a["samples"][0] == a["samples"][1] or a["samples"][0] == a["default"] or (a["default"] == a["samples"][1] and a["type"] != "bool"):
            raise GenError(f"attribute {an}: samples must differ from each other and from the default (a bool's second sample may be the default)")
        if a["type"] == "enum" and any(x not in a["values"] for x in a["samples"]):
            raise GenError(f"attribute {an}: samples must be keys of `values`")
        child = self.m.accepted_child_tag(e, s)
        d1, e1 = self.attr_value(a, a["samples"][0], "v1")
        d2, e2 = self.attr_value(a, a["samples"][1], "v2")
        if a["type"] == "enum":
            dd, ed = self.attr_value({"type": "string"}, a["default"], "dv")
        else:
            dd, ed = self.attr_value(a, a["default"], "dv")
        sid, aid = f'S("{t}", "{sn}")', f'A("{t}", "{an}")'
        setc = lambda who, v: f"B->set_child_attr(B->self, p, {who}, {aid}, &{v})"
        getc = lambda who: f"B->get_child_attr(B->self, p, {who}, {aid}, &out)"
        lines = [f'  svl_handle p = mk("{t}"); svl_handle c = mk("{child}"); svl_handle stray = mk("{child}");',
                 f"  CHECK({aid} != 0);", f"  {d1}", f"  {d2}", f"  {dd}", "  svl_value out{}; svl_value nul{};  // nul: SVL_NULL",
                 f"  CHECK(B->insert(B->self, p, c, 0, {sid}) == 0);"]
        if a.get("setup"):
            # state the attribute needs to be observable (a toolbar break needs a toolbar before it)
            lines.append(f"  {{ auto *pw = static_cast<{e['cls']} *>(O(p)); auto *w = static_cast<QWidget *>(O(c)); (void)pw; (void)w; {a['setup']} }}")
        lines += [f"  CHECK({setc('stray', 'v1')} < 0);  // a child that is not inserted has no attributes",
                 '  CHECK(std::strstr(B->last_error(B->self), "inserted child") != nullptr);',
                 f"  CHECK({getc('stray')} < 0);",
                 f"  CHECK({setc('c', 'v1')} == 0);",
                 f"  CHECK({getc('c')} == 0 && valstr(out) == {e1});"]
        if a.get("expect"):
            ex = a["expect"].replace("$w", "w").replace("$p", "pw")
            lines += [f"  {{ auto *pw = static_cast<{e['cls']} *>(O(p)); auto *w = static_cast<QWidget *>(O(c)); (void)pw; (void)w;",
                      f"    CHECK(({ex})); }}  // the toolkit really holds it"]
        lines += [f"  CHECK({setc('c', 'v2')} == 0);",
                  f"  CHECK({getc('c')} == 0 && valstr(out) == {e2});",
                  f"  CHECK({setc('c', 'nul')} == 0);  // null resets",
                  f"  CHECK({getc('c')} == 0 && valstr(out) == {ed});",
                  f"  CHECK({setc('c', 'v1')} == 0);",
                  "  CHECK(B->remove(B->self, p, c) == 0);",
                  f"  CHECK({setc('c', 'v1')} < 0);  // a removed child has no attributes",
                  f"  CHECK({getc('c')} < 0);",
                  f"  CHECK(B->insert(B->self, p, c, 0, {sid}) == 0);",
                  f"  CHECK({getc('c')} == 0 && valstr(out) == {ed});  // remove + insert: back to the default",
                  "  B->destroy(B->self, p); B->destroy(B->self, c); B->destroy(B->self, stray);"]
        if a["type"] == "enum":
            lines.insert(-1, f'  {{ std::string bad = "no-such-value"; svl_value vb = vstr(bad); CHECK({setc("c", "vb")} < 0); }}')
        self.add(f"attr/{t}.{sn}.{an}", "\n".join(lines))

    def slot_actions(self, e, s):
        """`order: actions` slots: QWidget::actions() order, submenus, radio sets and ownership (menus.hpp.in)."""
        m, t, sn = self.m, e["tag"], s["name"]
        sid = f'S("{t}", "{sn}")'
        A = lambda h: f"qobject_cast<QAction *>(O({h}))"
        acts = lambda p: f"static_cast<QWidget *>(O({p}))->actions()"
        lst = lambda *hs: "(QList<QAction *>{" + ", ".join(A(h) for h in hs) + "})"
        mk = [f'  svl_handle p = mk("{t}"); svl_handle a0 = mk("action"); svl_handle a1 = mk("action"); svl_handle a2 = mk("action");']
        clean = "  B->destroy(B->self, p); B->destroy(B->self, a0); B->destroy(B->self, a1); B->destroy(B->self, a2);"
        self.add(f"slot/{t}.{sn}/order", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, a0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, a2, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, a1, a2, {sid}) == 0);  // before=a2",
            f"  CHECK({acts('p')} == {lst('a0', 'a1', 'a2')});",
            f"  CHECK(O(a1)->parent() == O(p));  // owned by the holder",
            f"  CHECK(B->insert(B->self, p, a2, a0, {sid}) == 0);  // move an existing item to the front",
            f"  CHECK({acts('p')} == {lst('a2', 'a0', 'a1')});",
            f"  CHECK(B->insert(B->self, p, a0, a0, {sid}) == 0);  // before itself: unchanged",
            f"  CHECK({acts('p')} == {lst('a2', 'a0', 'a1')});",
            f'  {{ svl_handle x = mk("action"); CHECK(B->insert(B->self, p, x, a0 + 1000, {sid}) < 0); B->destroy(B->self, x); }}  // unknown before',
            clean]))
        self.add(f"slot/{t}.{sn}/default_slot_is_slot_0", "\n".join(mk + [
            "  CHECK(B->insert(B->self, p, a0, 0, 0) == 0);", f"  CHECK({acts('p')} == {lst('a0')});", clean]))
        self.add(f"slot/{t}.{sn}/separator_and_submenu", "\n".join(mk + [
            '  svl_handle sep = mk("action"); svl_handle sub = mk("menu");',
            '  { svl_value on = vbool(true); CHECK(B->set_prop(B->self, sep, P("action", "separator"), &on) == 0); }  // separator={true}',
            f"  CHECK(B->insert(B->self, p, a0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, sep, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, sub, 0, {sid}) == 0);",
            "  auto *menu = qobject_cast<QMenu *>(O(sub));",
            f"  CHECK({acts('p')}.size() == 3 && {acts('p')}[1]->isSeparator() && {acts('p')}[2] == menu->menuAction());",
            "  CHECK(menu->parentWidget() == static_cast<QWidget *>(O(p)));  // the holder owns the submenu",
            '  CHECK(B->insert(B->self, sub, a1, 0, S("menu", "items")) == 0);  // submenus hold items too',
            f"  CHECK(menu->actions() == {lst('a1')});",
            f"  CHECK(B->remove(B->self, p, sub) == 0 && !{acts('p')}.contains(menu->menuAction()) && menu->parent() == nullptr);",
            "  B->destroy(B->self, sep); B->destroy(B->self, sub);",
            clean]))
        wrong = m.wrong_child_tag(s)
        if wrong:
            self.add(f"slot/{t}.{sn}/rejects_{wrong}", "\n".join([
                f'  svl_handle p = mk("{t}"); svl_handle w = mk("{wrong}");',
                f"  CHECK(B->insert(B->self, p, w, 0, {sid}) < 0);",
                "  B->destroy(B->self, p); B->destroy(B->self, w);"]))
        self.add(f"ownership/{t}.{sn}/remove_released", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, a0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, a1, 0, {sid}) == 0);",
            "  QPointer<QObject> p1(O(a1));",
            "  CHECK(B->remove(B->self, p, a1) == 0);",
            "  flush();",
            f"  CHECK({acts('p')} == {lst('a0')});",
            "  CHECK(!p1.isNull() && p1->parent() == nullptr);  // released, not deleted",
            "  CHECK(!was_destroyed(a1));",
            "  CHECK(B->remove(B->self, p, a1) < 0);",
            f"  CHECK(B->insert(B->self, p, a1, 0, {sid}) == 0);  // a released item can be inserted again",
            f"  CHECK({acts('p')} == {lst('a0', 'a1')});",
            clean]))
        self.add(f"ownership/{t}.{sn}/parent_destroy_deletes_children", "\n".join(mk + [
            f"  CHECK(B->insert(B->self, p, a0, 0, {sid}) == 0);",
            "  QPointer<QObject> pc(O(a0));",
            "  B->destroy(B->self, p);",
            "  flush();",
            "  CHECK(pc.isNull());",
            "  CHECK(was_destroyed(a0));",
            clean]))
        self.add(f"slot/{t}.{sn}/radio_group", "\n".join(mk + [
            '  std::string g = "size"; svl_value gv = vstr(g);',
            '  CHECK(B->set_prop(B->self, a0, P("action", "group"), &gv) == 0);',
            '  CHECK(B->set_prop(B->self, a1, P("action", "group"), &gv) == 0);',
            f"  CHECK(B->insert(B->self, p, a0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, a1, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, a2, 0, {sid}) == 0);  // no group",
            f"  CHECK({A('a0')}->actionGroup() && {A('a0')}->actionGroup() == {A('a1')}->actionGroup() && !{A('a2')}->actionGroup());",
            f"  CHECK({A('a0')}->isCheckable() && {A('a0')}->actionGroup()->isExclusive());",
            f"  {A('a0')}->setChecked(true);",
            f"  {A('a1')}->trigger();  // the user picks the other one",
            f"  CHECK(!{A('a0')}->isChecked() && {A('a1')}->isChecked());",
            '  std::string none = ""; svl_value nv = vstr(none);',
            '  CHECK(B->set_prop(B->self, a1, P("action", "group"), &nv) == 0);  // leaving the set',
            f"  CHECK(!{A('a1')}->actionGroup());",
            f"  CHECK(B->remove(B->self, p, a0) == 0 && !{A('a0')}->actionGroup());",
            clean]))

    def slot_many(self, e, s):
        if s.get("actions"):
            return self.slot_actions(e, s)
        if s.get("append_only"):
            return self.slot_append_only(e, s)
        m, t, sn = self.m, e["tag"], s["name"]
        child = m.accepted_child_tag(e, s)
        ops, sid = s["ops"], f'S("{t}", "{sn}")'
        host = lambda p: host_expr(e, s["host"], f"O({p})")
        at = lambda p, i: self.item_widget(ops, f"{host(p)}->{ops['at']['method']}({i})")
        cnt = lambda p: op_call(host(p), ops['count'])
        mk3 = [f'  svl_handle p = mk("{t}"); svl_handle c0 = mk("{child}"); svl_handle c1 = mk("{child}"); svl_handle c2 = mk("{child}");']
        clean = "  B->destroy(B->self, p); B->destroy(B->self, c0); B->destroy(B->self, c1); B->destroy(B->self, c2);"
        if s.get("order") == "unordered":
            self.add(f"slot/{t}.{sn}/unordered_ignores_before", "\n".join(mk3 + [
                f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
                f"  CHECK(B->insert(B->self, p, c2, 0, {sid}) == 0);",
                f"  CHECK(B->insert(B->self, p, c1, c2, {sid}) == 0);  // before=c2 is ignored: c1 is appended",
                f"  CHECK({cnt('p')} == 3);",
                f"  CHECK({at('p', 0)} == O(c0) && {at('p', 1)} == O(c2) && {at('p', 2)} == O(c1));",
                f"  CHECK(B->insert(B->self, p, c2, c0, {sid}) == 0);  // a child already in the host stays where it is",
                f"  CHECK({cnt('p')} == 3);",
                f"  CHECK({at('p', 0)} == O(c0) && {at('p', 1)} == O(c2) && {at('p', 2)} == O(c1));",
                "  CHECK(static_cast<QWidget *>(O(p))->isAncestorOf(static_cast<QWidget *>(O(c1))));  // the page may sit in an inner container (QTabWidget)",
                clean]))
        else:
          self.add(f"slot/{t}.{sn}/order", "\n".join(mk3 + [
            f"  CHECK(B->insert(B->self, p, c0, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c2, 0, {sid}) == 0);",
            f"  CHECK(B->insert(B->self, p, c1, c2, {sid}) == 0);  // before=c2",
            f"  CHECK({cnt('p')} == 3);",
            f"  CHECK({at('p', 0)} == O(c0) && {at('p', 1)} == O(c1) && {at('p', 2)} == O(c2));",
            (f"  CHECK(static_cast<QWidget *>(O(p))->isAncestorOf(static_cast<QWidget *>(O(c1))));  // nested: reparented into an internal widget"
             if s.get("nested") else "  CHECK(O(c1)->parent() == O(p));"),
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
    # A plain QObject signal with a trigger, for the listener-phase tests.
    sig = next(((e, sg) for e in simple for sg in e.get("signals", []) if not sg.get("synthetic") and sg.get("trigger")), None)
    if not sig:
        raise GenError("need an element with a triggerable signal")
    sig_e, sig_s = sig
    subs = {"@@SIG_TAG@@": sig_e["tag"], "@@SIG_NAME@@": sig_s["js"], "@@SIG_CLASS@@": sig_e["cls"],
            "@@SIG_TRIGGER@@": sig_s["trigger"].replace("$w", "w").replace("$i", "i"), "@@ANY_TAG@@": any_tag, "@@PARENT_TAG@@": parent["tag"], "@@CHILD_TAG@@": child_tag,
            "@@VALUE_TAG@@": val["tag"], "@@VALUE_PROP@@": prop["js"], "@@DIGEST@@": digest}
    out = tmpl.replace("//@@INCLUDES@@", inc(m, classes)).replace("//@@TESTS@@", tests).replace("//@@TEST_TABLE@@", table)
    for k, v in subs.items():
        out = out.replace(k, v)
    return out, T.manifest(version)


# --------------------------------------------------------------------------------------------
# typings

def ts_key(name: str) -> str:
    """A TS property key: bare when it is an identifier, else quoted (`"minimum-width"`)."""
    return name if name.isidentifier() else '"' + name + '"'


def ts_type(m: Model, ref: str) -> str:
    cat, pay = classify(ref)
    return ts_leaf(cat, pay, m)


def gen_dts(m: Model) -> str:
    out = ["// GENERATED by tools/metadata/qt/gen_cpp.py -- do not edit.", "",
           "import type { Boxed, DateParts, Insets, Point, Rect, Size, TimeParts, Color, Variant } from './values';", "",
           "export interface QtElementDescriptors {"]
    for e in m.elements:
        out.append(f"  {ts_key(e['tag'])}: {{")
        out.append("    props: {")
        for p in e.get("props", []):
            if p["direct"]:
                ty = ts_type(m, p["ref"])
            else:
                ty = {"string": "string", "bool": "boolean"}.get(p["decl_ref"], "number")
            ro = "readonly " if p.get("readonly") else ""
            out.append(f"      {ro}{ts_key(p['js'])}?: {ty};")
        out.append("    };")
        out.append("    events: {")
        for s in e.get("signals", []):
            args = ", ".join(f"{a['name']}: {ts_type(m, a['ref'])}" for a in s["args"])
            key = ts_key(s["js"])
            out.append(f"      {key}: ({args}) => void;")
        out.append("    };")
        out.append("    methods: {")
        for me in e.get("methods", []):
            args = ", ".join(
                f"{an}: {cb_type_text(m, cb) if cb else ts_type(m, r)}"
                for an, r, mode, cb in zip(me["argnames"], me["argrefs"], me["modes"], me["cbs"]) if mode != "out")
            ret = "void" if me["ret"] == "void" else ts_type(m, me["ret"])
            if me.get("async"):
                a = me["async"]
                inner = ts_type(m, a["result_ref"]) if a["result_ref"] else "void"
                args = (args + ", " if args else "") + "options?: { signal?: AbortSignal }"
                out.append(f"      {me['js']}({args}): Promise<{inner}>;")
                continue
            if any(mode != "in" for mode in me["modes"]):
                fields = ([] if me["ret"] == "void" else [f"ret: {ret}"]) + [
                    f"{an}: {ts_type(m, r)}" for an, r, mode in zip(me["argnames"], me["argrefs"], me["modes"]) if mode != "in"]
                ret = "{ " + "; ".join(fields) + " }"
            out.append(f"      {me['js']}({args}): {ret};")
        out.append("    };")
        ch = e.get("children")
        out.append("    slots: " + (" | ".join(f'"{s["name"]}"' for s in ch["slots"]) if ch else "never") + ";")
        out.append("  };")
    out.append("}")
    out.append("")
    out.append("export type QtElementTag = keyof QtElementDescriptors;")
    out.append("")
    out.append("/** `parent:<name>` attributes any child element can carry; which apply depends on the parent and the slot. */")
    out.append("export interface QtChildAttrs {")
    types: dict[str, list[str]] = {}
    for e in m.elements:
        for s in (e.get("children") or {}).get("slots", []):
            for a in s.get("attrs", []):
                ty = ("(" + " | ".join(f'"{k}"' for k in sorted(a["values"])) + ")") if a["type"] == "enum" else \
                    {"bool": "boolean", "i32": "number", "f64": "number", "string": "string"}[a["type"]]
                if ty not in types.setdefault(a["name"], []):
                    types[a["name"]].append(ty)
    for n in sorted(types):
        out.append(f"  'parent:{n}'?: {' | '.join(types[n])};")
    out.append("}")
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------------------------

def gen_schema(m: Model) -> str:
    """Element tag -> every settable prop -> coarse type for hosts without a backend (packages/testhost),
    like the GTK schema.json; a host that knows a kind resolves only these props."""
    schema = {}
    for e in m.elements:
        props = {}
        for p in e.get("props", []):
            if p.get("readonly"):
                continue
            cat = p.get("cat")
            props[p["js"]] = {"bool": "boolean", "int": "integer", "float": "number", "string": "string",
                              "enum": "enum", "flags": "flags"}.get(cat, "value")
        schema[e["tag"]] = dict(sorted(props.items()))
    return json.dumps(schema, indent=2, sort_keys=True) + "\n"


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
    ap.add_argument("--schema", help="write the TestHost coarse-type schema (packages/testhost/src/schema.qt.json) to this path")
    a = ap.parse_args()
    cfgp = Path(a.config).resolve()
    cfg = tomllib.loads(cfgp.read_text())
    base = cfgp.parent
    res = lambda s: (base / s).resolve()
    out = Path(a.out)
    try:
        m = Model(res(cfg["input"]["metadata"]), res(cfg["input"]["overrides"]), cfg.get("elements", {}).get("tags", []))
        templ = HERE / "templates"
        digest = m.digest([HERE / "gen_cpp.py", templ / "backend.cpp.in", templ / "verify.cpp.in", templ / "serdes.hpp.in", templ / "vlist.hpp.in", templ / "menus.hpp.in", templ / "canvas.hpp.in"])
        o = cfg["output"]
        write_text(out / o["backend"], gen_backend(m, digest, (templ / "backend.cpp.in").read_text()))
        verify, facts = gen_verify(m, digest, (templ / "verify.cpp.in").read_text())
        write_text(out / o["verify"], verify)
        if a.facts:
            write_text(Path(a.facts), json.dumps(facts, indent=1, sort_keys=True) + "\n")
        write_text(out / o["dts"], gen_dts(m))
        if a.schema:
            write_text(Path(a.schema), gen_schema(m))
        if a.dts:
            write_text(Path(a.dts), gen_dts(m))
            write_values_dts(Path(a.dts).parent / "values.d.ts")
    except GenError as ex:
        print(f"gen_cpp: {ex}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
