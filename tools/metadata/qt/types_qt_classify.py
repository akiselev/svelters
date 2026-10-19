#!/usr/bin/env python3
"""Qt non-GUI types (plans/research/NON-GUI-TYPES.md, N2): classification, binding scope and facts.

Promoted from docs/notes/scripts/types_qt_classify.py; the Qt counterpart of tools/metadata/gtk/types_facts.py.
Reads only the checked-in ingest (`metadata/qt/types/ingested/*.json`, written by ingest_types.py from
qdoc + PySide6 + moc) and `metadata/overrides/qt/types/qt-types.yaml`, so it runs without a Qt install, and
writes per module `metadata/qt/types/<Module>.json` (one record per bound class) and `report.txt`.

Classification (section 3.1), rules only:
  QWidget-derived                          -> widget (elements; not in this taxonomy)
  QAbstractItemModel-derived               -> model
  QObject-derived (qdoc bases / moc)       -> object
  PySide value-type, record shape known    -> record (plain JS object)
  PySide value-type                        -> value (VALUE handle, non-const members hidden: decision Q3)
  only static members, no public ctor      -> statics
  PySide object-type, not a QObject:
     QEvent / paint / style option         -> transient (not exposed)
     *Item, *Command (or such bases)       -> item (ownership moves into a container: PySide parent facts)
     public constructor                    -> unique (JS-owned, deleted on dispose)
     otherwise                             -> needs-override
  C++ machinery (MACHINERY regex)          -> excluded
  in no typesystem                         -> unclassified

Members: public qdoc functions of bound classes. Excluded by rule: operators (equality is the core's
`equals`), deprecated, removed by PySide, templates and platform types, signals of non-QObjects. A member is
bound when every slot has a converter in the hand-written core (templates/types_core.hpp.in, serdes.hpp.in);
the others are listed under `blocked` with a reason. Overloads keep one JS name (decision Q4): the generated
thunk tries them in the fixed order of `overload_key` and takes the first whose arity and JS types match; an
overload whose JS signature equals an earlier one's can never be chosen and needs a `renames:` entry.

Usage:
  python3 tools/metadata/qt/types_qt_classify.py --report    # tables of the classification
  python3 tools/metadata/qt/types_qt_classify.py --write     # metadata/qt/types/*.json + report.txt
  python3 tools/metadata/qt/types_qt_classify.py --check     # the checked-in facts are current
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import yaml  # noqa: E402

INGEST = ROOT / "metadata/qt/types/ingested"
FACTS_DIR = ROOT / "metadata/qt/types"
OVERRIDES = ROOT / "metadata/overrides/qt/types/qt-types.yaml"
MODULES = ["QtCore", "QtGui", "QtWidgets"]
JS_MODULE = {"QtCore": "core", "QtGui": "gui", "QtWidgets": "widgets"}

# JS shape decision (portable field names), not a classification fact: the records the core converts.
RECORDS = {"QPoint": "Point", "QPointF": "Point", "QSize": "Size", "QSizeF": "Size", "QRect": "Rect", "QRectF": "Rect",
           "QMargins": "Insets", "QMarginsF": "Insets", "QColor": "Color"}
DATES = {"QDate": "date", "QTime": "time", "QDateTime": "datetime"}
INTS = {"int", "uint", "unsigned int", "short", "unsigned short", "long", "unsigned long", "long long",
        "unsigned long long", "size_t", "char", "unsigned char", "signed char", "int64_t", "uint64_t", "int32_t",
        "uint32_t"}
FLOATS = {"double", "float"}
TEMPLATE = {"T", "auto", "Callable", "Predicate", "Args ...", "Args...", "InputIterator", "Container", "Key", "U",
            "Functor", "Func", "Duration", "Rep", "Period", "Clock", "Fn", "Function", "Receiver"}
PLATFORM = re.compile(r"^(emscripten|CF|CG|NS|UI|HWND|HICON|HBITMAP|HRGN|HMONITOR|HDC|Vk|IUnknown|jobject|JNIEnv|wl_|xcb_|"
                      r"Display|EGL|GL|MSG|LPARAM|WPARAM|id\b|QNativeInterface|QPlatform|std::)")
MACHINERY = re.compile(r"Iterator|Iterable|Atomic|Pointer$|Pointer<|^QScoped|^QShared|^QWeak|^QExplicitly|Mutex|Locker|"
                       r"Semaphore|RWLock|ReadWriteLock|WaitCondition|ThreadStorage|Propert|Bindable|Binding|^QJni|"
                       r"^QAndroid|^QMac|^QWindows|^QVulkan|^QRhi|^QOpenGL|^QAccessible|^QDebug|^QNoDebug|^QMessageLog|"
                       r"^QLoggingCategory|^QMeta|Endian|^QTypeInfo|^QSpan|^QVarLength|^QContiguous|^QCache|Matcher|"
                       r"^QFlag|^QIncompatibleFlag|^QInternal|^QPlatform|DialogOptions|^QBasic|^QArgument|^QReturnArgument|"
                       r"^QVariant(Ref|ConstPointer|Pointer)|^QScopeGuard|^QSpecialInteger|^QLEInteger|^QBEInteger|^qfloat16|"
                       r"^QPartialOrdering|^QException|^QUnhandledException|^QSignalBlocker|^QPainterStateGuard|"
                       r"^QEventLoopLocker|^QNtfs|^QSupportedWritingSystems|^QTypeRevision|^QSharedData|^QObjectData|"
                       r"^QEnableShared|^QMapData|^QGenericPluginFactory|^QStringTokenizer|^QVector$|^QByteArrayList$|"
                       r"^QLatin1StringView$|^QUtf8StringView$|^QRgbaFloat|^QShaderDescription|^QTextFrameLayoutData|"
                       r"^QRangeModelAdapter|Converter$|^QStringDecoder|^QStringEncoder|^QFuture|^QPromise|^QtFuture|"
                       r"^QChronoTimer|^QPermission|Permission$|^QNative|^QGenericMatrix|^QOperatingSystemVersion|^QStaticPlugin")
PAINT = {"QPainter", "QPaintDevice", "QPaintEngine", "QPaintEngineState", "QBackingStore", "QOpenGLContext", "QStyleOption",
         "QStylePainter", "QRhi", "QTextItem", "QTextLine", "QTextFragment", "QTextBlock", "QTextFrame", "QTextObject"}
BINDABLE = {"record", "value", "object", "model", "statics", "unique", "item"}
# Members the hand-written core owns (lifetime, identity, reflection): never generated.
CORE_NAMES = {"deleteLater", "destroyed", "setParent", "parent", "children", "findChild", "findChildren", "installEventFilter",
              "removeEventFilter", "event", "eventFilter", "metaObject", "connect", "disconnect", "blockSignals", "signalsBlocked",
              "thread", "moveToThread", "startTimer", "killTimer", "dumpObjectTree", "dumpObjectInfo", "setProperty", "property",
              "dynamicPropertyNames", "objectNameChanged", "inherits", "isWidgetType", "isWindowType", "isQuickItemType",
              "tr", "trUtf8", "staticMetaObject", "sender", "senderSignalIndex", "receivers", "isSignalConnected", "swap",
              "detach", "isDetached", "data_ptr", "bindingStorage", "bindableObjectName", "isQmlExposed",
              "d_func", "customEvent", "childEvent", "timerEvent", "connectNotify", "disconnectNotify"}


class Blocked(Exception):
    pass


def load_overrides() -> dict:
    return yaml.safe_load(OVERRIDES.read_text()) or {}


def norm_spelling(t: str) -> tuple[str, bool, bool, bool]:
    """(core spelling, is pointer, is const, is non-const reference)."""
    t = (t or "void").strip()
    const = bool(re.search(r"\bconst\b", t))
    rvalue = "&&" in t
    ref = "&" in t and not rvalue
    ptr = t.rstrip().endswith("*")
    core = re.sub(r"\b(const|volatile|typename|struct|class|enum)\b", "", t).replace("&&", "").replace("&", "")
    core = core.replace("*", "").strip()
    core = re.sub(r"\s+", " ", core).replace("< ", "<").replace(" >", ">")
    return core, ptr, const, (ref and not const)


def split_args(inner: str) -> list[str]:
    parts, depth, cur = [], 0, ""
    for ch in inner:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur.strip())
    return parts


class Ingest:
    def __init__(self) -> None:
        self.classes: dict[str, dict] = {}
        self.sources: dict = {}
        self.enums: dict[str, dict] = {}      # fullname -> {owner, members, flags typedef}
        self.flag_alias: dict[str, str] = {}  # QFlags typedef spelling -> enum fullname
        for mod in MODULES:
            d = json.loads((INGEST / f"{mod}.json").read_text())
            self.sources = d["sources"]
            for n, c in d["classes"].items():
                self.classes[n] = c
            if mod == "QtCore":
                self.scoped = set(d.get("scoped_enums", []))
                for ns in d.get("namespaces", {}).values():
                    self._enums(ns["name"], ns)
        for n, c in self.classes.items():
            self._enums(n, c)

    def _enums(self, owner: str, c: dict) -> None:
        for e in c.get("enums", []):
            full = e["fullname"]
            scoped = full in self.scoped
            self.enums[full] = {"owner": owner, "name": e["name"], "scoped": scoped,
                                "members": [{"name": m["name"], "value": m["value"],
                                             "cpp": f"{full}::{m['name']}" if scoped else f"{owner}::{m['name']}"}
                                            for m in e["members"]]}
            if e.get("flags"):
                self.flag_alias[e["flags"]] = full
        for t in c.get("flag_typedefs", []):
            self.flag_alias.setdefault(t["name"], t["enum"])

    def chain(self, name: str) -> list[str]:
        out: list[str] = []
        todo = list(self.classes.get(name, {}).get("bases", []))
        while todo:
            b = todo.pop(0)
            if b in out:
                continue
            out.append(b)
            todo += self.classes.get(b, {}).get("bases", [])
        return out


def statics_only(c: dict) -> bool:
    fs = [f for f in c.get("functions", []) if f["meta"] in ("plain", "constructor", "slot", "signal")]
    return bool(fs) and all(f.get("static") for f in fs if f["meta"] != "constructor") and not any(
        f["meta"] == "constructor" for f in fs)


def classify(ing: Ingest, name: str, ov: dict) -> str:
    over = (ov.get("classes") or {}).get(name)
    if over:
        return over
    c = ing.classes[name]
    if c.get("widget"):
        return "widget"
    if MACHINERY.search(name) or "::" in name:
        return "excluded"
    ch = ing.chain(name)
    if name == "QAbstractItemModel" or "QAbstractItemModel" in ch:
        return "model"
    if name == "QObject" or "QObject" in ch or (c.get("moc") or {}).get("object"):
        return "object"
    k = (c.get("pyside") or {}).get("kind")
    if k == "value-type":
        return "record" if name in RECORDS or name in DATES else "value"
    if k in ("object-type", None) and statics_only(c):
        return "statics"
    if k == "object-type":
        if name == "QEvent" or "QEvent" in ch or name in PAINT or any(b in PAINT for b in ch) or name.startswith("QStyleOption"):
            return "transient"
        if name.endswith("Item") or name.endswith("Command") or any(b.endswith("Item") for b in ch):
            return "item"
        if any(f["meta"] == "constructor" for f in c.get("functions", [])):
            return "unique"
        return "needs-override"
    if k in ("primitive-type", "container-type"):
        return "primitive"
    if k == "namespace-type":
        return "namespace"
    if k == "rejected":
        return "excluded"
    if name in RECORDS or name in DATES:
        return "record"
    return "unclassified"


class Types:
    """Resolves C++ spellings to slot types the core converts, in the context of a class."""

    def __init__(self, ing: Ingest, cats: dict[str, str], bound: set[str], ov: dict) -> None:
        self.ing, self.cats, self.bound, self.ov = ing, cats, bound, ov
        self.aliases = ov.get("aliases") or {}
        self.block = ov.get("block_types") or {}

    def enum(self, spelling: str, ctx: list[str]):
        m = re.match(r"^QFlags<(.+)>$", spelling)
        if m:
            r = self.enum(m.group(1).strip(), ctx)
            return ("flags", r[1]) if r else None
        cands = [spelling] + [f"{c}::{spelling}" for c in ctx] + ([f"Qt::{spelling}"] if "::" not in spelling else [])
        for cand in cands:
            if cand in self.ing.flag_alias:
                return ("flags", self.ing.flag_alias[cand])
            if cand in self.ing.enums:
                return ("enum", cand)
        return None

    def cls(self, spelling: str, ctx: list[str]) -> str | None:
        if spelling in self.cats:
            return spelling
        for c in ctx:
            if f"{c}::{spelling}" in self.cats:
                return f"{c}::{spelling}"
        return None

    def slot(self, spelling: str, ctx: list[str], ret: bool = False) -> dict:
        core, ptr, const, mref = norm_spelling(spelling)
        if core in self.block:
            raise Blocked(f"blocked-type:{core}")
        if core == "void" and not ptr:
            return {"k": "void"}
        if "(" in core or core in TEMPLATE or re.search(r"\b(Functor|Func|Callable|Duration)\b", core):
            raise Blocked("template" if core in TEMPLATE else "callback")
        if core.startswith("std::function"):
            raise Blocked("callback")
        if core.startswith("QFuture<"):
            if not ret:
                raise Blocked("type:QFuture param")
            inner = self.slot(core[8:-1], ctx)
            return {"k": "async", "of": inner, "cpp": core}
        a = self.aliases.get(core)
        if a and a.get("as"):
            # A typedef of a spelling the rules know (QModelIndexList -> QList<QModelIndex>).
            return self.slot(spelling.replace(core, a["as"]), ctx, ret)
        if a and not ptr:
            out = {"k": a["k"], "cpp": a.get("cpp") or core}
            return self._outify(out, ptr, const, mref, ret)
        if PLATFORM.match(core):
            raise Blocked("platform")
        if core == "char" and ptr:
            if const:
                return {"k": "string", "cpp": "const char *", "cstring": True}
            raise Blocked("char buffer")
        if ptr and core in ("void",):
            raise Blocked("gpointer")
        if core == "bool":
            return self._outify({"k": "bool", "cpp": "bool"}, ptr, const, mref, ret)
        if core in INTS:
            return self._outify({"k": "int", "cpp": core}, ptr, const, mref, ret)
        if core in FLOATS:
            return self._outify({"k": "float", "cpp": core}, ptr, const, mref, ret)
        if core == "QString":
            return self._outify({"k": "string", "cpp": "QString"}, ptr, const, mref, ret)
        if core == "QByteArray":
            return self._outify({"k": "bytes", "cpp": "QByteArray"}, ptr, const, mref, ret)
        if core == "QStringList":
            return self._outify({"k": "list", "of": {"k": "string", "cpp": "QString"}, "cpp": "QStringList"}, ptr, const, mref, ret)
        if core == "QVariant":
            return self._outify({"k": "variant", "cpp": "QVariant"}, ptr, const, mref, ret)
        en = None if ptr else self.enum(core, ctx)
        if en:
            kind, full = en
            cpp = full if kind == "enum" else f"QFlags<{full}>"
            return self._outify({"k": kind, "t": full, "cpp": cpp}, ptr, const, mref, ret)
        if core in RECORDS:
            return self._outify({"k": "record", "t": RECORDS[core], "cpp": core}, ptr, const, mref, ret)
        if core in DATES:
            return self._outify({"k": DATES[core], "cpp": core}, ptr, const, mref, ret)
        # The element spelling keeps its `*` (QList<QStandardItem *>): parse the outer spelling, not `core`.
        m = re.match(r"^(?:const\s+)?(QList|QVector)<(.+)>\s*&?$", spelling.strip())
        if m and not ptr:
            inner = self.slot(m.group(2), ctx)
            if inner["k"] in ("void", "async") or inner.get("out") or inner["k"] == "list":
                raise Blocked("nested list")
            if inner.get("deref"):
                raise Blocked("list of references")
            return self._outify({"k": "list", "of": inner, "cpp": f"QList<{inner['cpp']}>"}, ptr, const, mref, ret)
        if re.match(r"^(QMap|QHash|QSet|QMultiMap|QMultiHash|QPair|std::pair|QSpan)\b", core):
            raise Blocked("container")
        c = self.cls(core, ctx)
        if c is None:
            raise Blocked(f"type:unknown:{core}")
        cat = self.cats[c]
        if cat in ("object", "model", "widget"):
            if not ptr:
                raise Blocked(f"QObject by value:{c}")
            if cat != "widget" and c not in self.bound:
                raise Blocked(f"unbound:{c}")
            return {"k": "object", "t": c, "cpp": f"{c} *", "widget": cat == "widget"}
        if cat in ("value", "record"):
            if c not in self.bound:
                raise Blocked(f"unbound:{c}")
            if ptr:
                if const:
                    raise Blocked(f"const pointer to value:{c}")
                return {"k": "value", "t": c, "cpp": c, "out": True}
            if mref and not ret:
                return {"k": "value", "t": c, "cpp": c, "out": "inout"}
            return {"k": "value", "t": c, "cpp": c}
        if cat in ("unique", "item"):
            if c not in self.bound:
                raise Blocked(f"unbound:{c}")
            if not ptr and not (mref or (const and "&" in spelling)):
                raise Blocked(f"{cat} by value:{c}")
            return {"k": cat, "t": c, "cpp": f"{c} *", "deref": not ptr}
        raise Blocked(f"type:{cat}:{c}")

    @staticmethod
    def _outify(s: dict, ptr: bool, const: bool, mref: bool, ret: bool) -> dict:
        """The sip rule: a non-const `T *` of a basic or value type is an out parameter; a non-const `T &` an in-out."""
        if ptr:
            if const or ret:
                raise Blocked("pointer to " + s["k"])
            return {**s, "out": True}
        if mref and not ret:
            return {**s, "out": "inout"}
        return s


def norm_sig(name: str, params: list[dict]) -> str:
    return re.sub(r"\s+", "", f"{name}({','.join(p['type'] for p in params)})")


JS_CLASS = {"bool": "bool", "int": "number", "float": "number", "string": "string", "bytes": "bytes", "variant": "any",
            "date": "date", "time": "time", "datetime": "datetime"}


def js_class(t: dict) -> str:
    """What the runtime dispatch can tell apart: overloads with equal tuples of these are ambiguous."""
    k = t["k"]
    if k in JS_CLASS:
        return JS_CLASS[k] + ("/int" if k == "int" else "")
    if k in ("enum", "flags"):
        return f"{k}:{t['t']}"
    if k == "record":
        return f"record:{t['t']}" + ("/int" if not t["cpp"].endswith("F") and t["t"] != "Color" else "")
    if k == "list":
        return "list<" + js_class(t["of"]) + ">"
    return f"{k}:{t['t']}"


RANK = {"object": 0, "unique": 0, "item": 0, "value": 1, "enum": 2, "flags": 3, "record": 4, "date": 4, "time": 4,
        "datetime": 4, "bool": 5, "int": 6, "float": 7, "string": 8, "bytes": 9, "list": 10, "variant": 11}


def overload_key(o: dict) -> tuple:
    """The fixed dispatch order (decision Q4): fewer required JS arguments first, then more specific JS types
    first (handles, values, enums, records, bool, integers before floats, strings, bytes, lists, variants),
    then declaration order. Integer forms come before float forms so `1` picks `int` and `1.5` the `double`."""
    ins = [p for p in o["params"] if not p["type"].get("out") or p["type"].get("out") == "inout"]
    req = sum(1 for p in ins if not p.get("default"))
    return (req, len(ins), tuple(RANK.get(p["type"]["k"], 12) for p in ins), o["decl"])


def member_name(f: dict) -> str:
    return f["name"]


def build_members(ing: Ingest, name: str, cat: str, ty: Types, ov: dict) -> dict:
    c = ing.classes[name]
    ctx = [name] + ing.chain(name)
    removed = set((c.get("pyside") or {}).get("removed", []))
    ownership = (c.get("pyside") or {}).get("ownership", [])
    renames = ov.get("renames") or {}
    keep = set((ov.get("keep") or {}).get(name, []))
    hide_over = set((ov.get("hide") or {}).get(name, []))
    blocked: dict[str, str] = {}
    excluded: collections.Counter = collections.Counter()
    hidden: list[str] = []
    groups: dict[tuple[str, str], list[dict]] = collections.defaultdict(list)   # (section, js) -> overloads
    signals: list[dict] = []
    is_qobject = cat in ("object", "model")
    for decl, f in enumerate(c.get("functions", [])):
        meta = f["meta"]
        if meta not in ("plain", "slot", "signal", "constructor"):
            excluded["special"] += 1
            continue
        sig = norm_sig(f["name"], f["params"])
        key = f"{name}.{f['name']}({', '.join(p['type'] for p in f['params'])})"
        if f.get("status") in ("deprecated", "obsolete", "internal", "ignored"):
            excluded["deprecated"] += 1
            continue
        if f["name"].startswith("operator") or f["name"].startswith("~"):
            excluded["operator"] += 1
            continue
        if f["name"] in CORE_NAMES:
            excluded["core"] += 1
            continue
        if sig in removed or f"{f['name']}()" in removed and not f["params"]:
            excluded["removed by PySide"] += 1
            continue
        if any(PLATFORM.match(norm_spelling(t)[0]) or norm_spelling(t)[0] in TEMPLATE
               for t in [f["type"]] + [p["type"] for p in f["params"]]) or "template" in (f["signature"] or ""):
            excluded["template/platform"] += 1
            continue
        if meta == "signal":
            if not is_qobject:
                excluded["special"] += 1
                continue
            try:
                args = [{"name": p["name"] or f"arg{i}", "type": ty.slot(p["type"], ctx)} for i, p in enumerate(f["params"])]
                if any(a["type"].get("out") for a in args):
                    raise Blocked("out param in signal")
                signals.append({"name": f["name"], "args": args, "sig": sig, **({"since": f["since"]} if f.get("since") else {})})
            except Blocked as b:
                blocked[key] = str(b)
            continue
        try:
            params = []
            for i, p in enumerate(f["params"]):
                try:
                    t = ty.slot(p["type"], ctx)
                except Blocked:
                    # A blocked parameter in a defaulted tail: bind the shorter call (C++ fills the defaults).
                    if all(q.get("default") for q in f["params"][i:]):
                        break
                    raise
                if t["k"] == "async":
                    raise Blocked("async param")
                pr = {"name": p["name"] or f"arg{i}", "type": t}
                if p.get("default"):
                    pr["default"] = True
                params.append(pr)
            if meta == "constructor":
                if any(p["type"].get("out") for p in params):
                    raise Blocked("out param in constructor")
                ret = {"k": "self"}
            else:
                ret = ty.slot(f["type"], ctx, ret=True)
        except Blocked as b:
            blocked[key] = str(b)
            continue
        if meta == "constructor":
            section = "constructors"
        elif f.get("static"):
            section = "statics"
        else:
            section = "methods"
        if (cat == "value" and section == "methods" and not f.get("const") and f["name"] not in keep) or f["name"] in hide_over:
            hidden.append(f["name"])
            continue
        own = []
        for o in ownership:
            if o["sig"] == re.sub(r"\s+", "", norm_sig(f["name"], f["params"]).replace(" ", "")) or o["sig"].split("(")[0] == f["name"] and \
                    o["sig"].replace("const", "") == sig.replace("const", ""):
                own.append({k: o[k] for k in ("arg", "fact", "action", "owner", "index", "class") if o.get(k)})
        rec = {"cpp": f["name"], "params": params, "ret": ret, "decl": decl, "key": key}
        if f.get("const"):
            rec["const"] = True
        if f.get("since"):
            rec["since"] = f["since"]
        if f.get("virtual") == "pure":
            rec["pure"] = True
        if own:
            rec["ownership"] = sorted(own, key=lambda x: json.dumps(x, sort_keys=True))
        if meta == "slot":
            rec["slot"] = True
        js = renames.get(key, f["name"])
        groups[(section, js)].append(rec)
    out: dict = {"constructors": [], "statics": {}, "methods": {}}
    ambiguous: list[dict] = []
    merged: list[dict] = []
    twins = 0
    for (section, js), ovs in sorted(groups.items()):
        ovs.sort(key=overload_key)
        kept: list[dict] = []
        seen: dict[tuple, dict] = {}
        for o in ovs:
            ins = tuple(js_class(p["type"]) for p in o["params"] if not p["type"].get("out") or p["type"].get("out") == "inout")
            reqs = sum(1 for p in o["params"] if not p.get("default") and (not p["type"].get("out") or p["type"]["out"] == "inout"))
            sig_key = (ins, reqs)
            if sig_key in seen:
                prev = seen[sig_key]
                if prev.get("const") != o.get("const") and [p["type"] for p in prev["params"]] == [p["type"] for p in o["params"]]:
                    twins += 1  # `T &x()` next to `const T &x() const`: the const one stays
                    continue
                outs = lambda x: tuple(js_class(p["type"]) for p in x["params"] if p["type"].get("out") is True)  # noqa: E731
                if outs(prev) == outs(o) and (prev["ret"]["k"] == "void") == (o["ret"]["k"] == "void"):
                    # Same JS arguments and result shape: a C++ spelling variant (QStringView next to QString,
                    # short/long next to int, float next to double). The earlier one in the order serves both.
                    merged.append({"key": o["key"], "served_by": prev["key"]})
                    continue
                ambiguous.append({"key": o["key"], "shadowed_by": prev["key"]})
                blocked[o["key"]] = "ambiguous overload (decision Q4: add a `renames:` entry)"
                continue
            seen[sig_key] = o
            kept.append(o)
        if section == "constructors":
            out["constructors"] = kept
        else:
            out[section][js] = kept
    # Properties (Q_PROPERTY of QObject classes): read and written through the metaobject by the core.
    props = []
    moc_props = ((c.get("moc") or {}).get("properties") or {})
    if is_qobject:
        for p in c.get("properties", []):
            if p.get("status") in ("deprecated", "obsolete"):
                continue
            mp = moc_props.get(p["name"])
            if mp is None:
                continue
            try:
                t = ty.slot(mp.get("type") or p["type"], ctx, ret=True)
            except Blocked as b:
                blocked[f"{name}.{p['name']} (property)"] = str(b)
                continue
            props.append({"name": p["name"], "type": t, "writable": bool(mp.get("write") or mp.get("member")) and p["writable"],
                          **({"notify": mp["notify"]} if mp.get("notify") else {}),
                          **({"constant": True} if mp.get("constant") else {}),
                          **({"since": p["since"]} if p.get("since") else {})})
    # A method that reads a property is reached as the property accessor (JS `obj.x`, not `obj.x()`).
    pnames = {p["name"] for p in props}
    for pn in pnames:
        if pn in out["methods"]:
            hidden.append(pn)
            del out["methods"][pn]
    return {**out, "properties": props, "signals": sorted(signals, key=lambda s: (s["name"], s["sig"])),
            "blocked": dict(sorted(blocked.items())), "hidden": sorted(set(hidden)), "excluded": dict(excluded),
            "ambiguous": ambiguous, "merged": len(merged), "const_twins": twins}


def refs_of(t: dict) -> set[str]:
    out: set[str] = set()
    if t.get("k") in ("object", "value", "unique", "item") and not t.get("widget"):
        out.add(t["t"])
    if t.get("k") in ("list", "async"):
        out |= refs_of(t["of"])
    return out


def enums_of(t: dict) -> set[str]:
    if t.get("k") in ("enum", "flags"):
        return {t["t"]}
    if t.get("k") in ("list", "async"):
        return enums_of(t["of"])
    return set()


def slot_types(rec: dict):
    for o in rec["constructors"]:
        for p in o["params"]:
            yield p["type"]
    for sec in ("statics", "methods"):
        for ovs in rec[sec].values():
            for o in ovs:
                yield o["ret"]
                for p in o["params"]:
                    yield p["type"]
    for p in rec["properties"]:
        yield p["type"]
    for s in rec["signals"]:
        for a in s["args"]:
            yield a["type"]


def build() -> dict:
    ov = load_overrides()
    ing = Ingest()
    for key in ("curated", "roots", "exclude"):
        for n in ov.get(key) or []:
            if n not in ing.classes:
                raise SystemExit(f"types_qt_classify: {OVERRIDES.name}: {key}: no class {n} in the ingest")
    for n in list(ov.get("classes") or {}) + list(ov.get("aliases") or {}) * 0:
        if n not in ing.classes:
            raise SystemExit(f"types_qt_classify: {OVERRIDES.name}: classes: no class {n} in the ingest")
    cats = {n: classify(ing, n, ov) for n in ing.classes}
    exclude = set(ov.get("exclude") or [])
    # Scope: the roots, then the closure of every bindable class their bound members mention, plus ancestors.
    bindable = {n for n, c in cats.items() if c in BINDABLE and n not in exclude}
    bound: set[str] = set()
    why: dict[str, str] = {}
    todo = [(r, "root") for r in ov.get("roots") or []]
    while todo:
        n, reason = todo.pop(0)
        if n in bound or n not in bindable:
            continue
        bound.add(n)
        why[n] = reason
        for b in ing.chain(n):
            if b in bindable and b not in bound:
                todo.append((b, "ancestor"))
        # Mentioned types: optimistic resolution (everything bindable counts as bound) to find the closure.
        ty = Types(ing, cats, bindable, ov)
        rec = build_members(ing, n, cats[n], ty, ov)
        for t in slot_types(rec):
            for r in refs_of(t):
                if r not in bound:
                    todo.append((r, "closure"))
    ty = Types(ing, cats, bound, ov)
    curated = set(ov.get("curated") or [])
    types: dict[str, dict] = {}
    for n in sorted(bound):
        c = ing.classes[n]
        cat = cats[n]
        rec = build_members(ing, n, cat, ty, ov)
        parent = next((b for b in ing.chain(n) if b in bound), None)
        refs: set[str] = set()
        enums: set[str] = set()
        for t in slot_types(rec):
            refs |= refs_of(t)
            enums |= enums_of(t)
        abstract = any(f.get("virtual") == "pure" for f in c.get("functions", []))
        default_ctor = any(not [p for p in o["params"] if not p.get("default")] for o in rec["constructors"])
        types[n] = {
            "name": n, "module": c["module"], "js_module": f"svelters:qt/{JS_MODULE[c['module']]}", "category": cat,
            "lifetime": {"object": "shared", "model": "shared", "value": "value", "record": "record", "unique": "owned",
                         "item": "item", "statics": "none"}[cat],
            "header": c.get("header"), "since": c.get("since"), "scope": why.get(n, "closure"), "curated": n in curated,
            "parent": parent, "bases": [b for b in ing.chain(n) if b in bound], "abstract": abstract,
            "qobject": cat in ("object", "model"), "constructible": default_ctor and not abstract and cat in ("object", "model"),
            **rec, "refs": sorted(r for r in refs if r != n), "enums": sorted(enums)}
        if abstract:
            types[n]["constructors"] = []
    enum_tables = {}
    for n, rec in types.items():
        for e in rec["enums"]:
            if e in ing.enums:
                enum_tables[e] = ing.enums[e]
    return {"sources": ing.sources, "types": types, "categories": cats, "enums": enum_tables, "modules": MODULES}


def split_by_module(data: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for mod in MODULES:
        out[mod] = {"schema": "svelters-types/v1", "toolkit": "qt", "module": mod, "js_module": f"svelters:qt/{JS_MODULE[mod]}",
                    "version": data["sources"].get("qdoc_version"), "types": {}, "enums": {}}
    for n, rec in data["types"].items():
        out[rec["module"]]["types"][n] = rec
    for full, e in data["enums"].items():
        mod = data["types"][e["owner"]]["module"] if e["owner"] in data["types"] else (
            "QtCore" if e["owner"] == "Qt" else next((r["module"] for r in data["types"].values() if full in r["enums"]), "QtCore"))
        out[mod]["enums"][full] = e
    return out


def serialise(d: dict) -> str:
    return json.dumps(d, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def load_facts() -> dict:
    """The checked-in facts as the generators read them: {types, enums} over every module."""
    types, enums = {}, {}
    for mod in MODULES:
        p = FACTS_DIR / f"{mod}.json"
        if p.exists():
            d = json.loads(p.read_text())
            types.update(d["types"])
            enums.update(d["enums"])
    return {"types": types, "enums": enums}


def write(data: dict) -> int:
    FACTS_DIR.mkdir(parents=True, exist_ok=True)
    files = split_by_module(data)
    for mod, d in files.items():
        (FACTS_DIR / f"{mod}.json").write_text(serialise(d))
    (FACTS_DIR / "report.txt").write_text(report(data))
    return len(files)


def check(data: dict) -> int:
    bad = []
    for mod, d in split_by_module(data).items():
        p = FACTS_DIR / f"{mod}.json"
        if not p.exists() or p.read_text() != serialise(d):
            bad.append(f"{mod}.json")
    rp = FACTS_DIR / "report.txt"
    if not rp.exists() or rp.read_text() != report(data):
        bad.append("report.txt")
    if bad:
        print("types_qt_classify: stale facts: " + ", ".join(bad) + " (run `python3 tools/metadata/qt/types_qt_classify.py --write`)",
              file=sys.stderr)
        return 1
    print(f"types_qt_classify: {len(data['types'])} types in {len(MODULES)} modules are current")
    return 0


def n_members(rec: dict) -> int:
    return (len(rec["constructors"]) + sum(len(v) for v in rec["statics"].values()) + sum(len(v) for v in rec["methods"].values())
            + len(rec["properties"]) + len(rec["signals"]))


def report(data: dict) -> str:
    cats = data["categories"]
    order = ["record", "value", "object", "model", "statics", "unique", "item", "transient", "needs-override", "unclassified",
             "excluded", "primitive", "namespace", "widget"]
    lines = [f"== classes by category (all public classes; qdoc {data['sources'].get('qdoc_version')}, PySide6 typesystems, moc metatypes)"]
    lines.append(f"{'module':10}" + "".join(f"{o[:8]:>9}" for o in order))
    ing_mod = {n: r["module"] for n, r in data["types"].items()}
    per: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for n, c in cats.items():
        per[data.get("module_of", {}).get(n) or ing_mod.get(n) or MOD_OF.get(n, "?")][c] += 1
    tot: collections.Counter = collections.Counter()
    for mod in MODULES:
        lines.append(f"{mod:10}" + "".join(f"{per[mod][o]:9}" for o in order))
        tot.update(per[mod])
    lines.append(f"{'total':10}" + "".join(f"{tot[o]:9}" for o in order))
    lines.append("")
    lines.append("== bound scope (roots + signature closure + ancestors), by module and category")
    types = data["types"]
    by: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for r in types.values():
        by[r["module"]][r["category"]] += 1
    for mod in MODULES:
        lines.append(f"  {mod:10} {sum(by[mod].values()):4}  " + ", ".join(f"{k} {v}" for k, v in sorted(by[mod].items())))
    lines.append(f"  total      {len(types):4}  (roots {sum(1 for r in types.values() if r['scope'] == 'root')})")
    lines.append("")
    lines.append("== members of bound types: bound / blocked")
    mem: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    reasons: collections.Counter = collections.Counter()
    excluded: collections.Counter = collections.Counter()
    hidden = twins = overloaded = names = merged = 0
    amb: list[dict] = []
    for r in types.values():
        mem[r["module"]]["bound"] += n_members(r)
        mem[r["module"]]["blocked"] += len(r["blocked"])
        hidden += len(r["hidden"])
        twins += r["const_twins"]
        merged += r["merged"]
        amb += r["ambiguous"]
        excluded.update(r["excluded"])
        for sec in ("statics", "methods"):
            for ovs in r[sec].values():
                names += 1
                overloaded += len(ovs) > 1
        for why in r["blocked"].values():
            reasons[why if not why.startswith(("unbound:", "blocked-type:")) else why.split(":")[0]] += 1
    for mod in MODULES:
        m = mem[mod]
        n = m["bound"] + m["blocked"]
        if n:
            lines.append(f"  {mod:10} members {n:5}  bound {m['bound']:5} ({100 * m['bound'] // n:3}%)  blocked {m['blocked']:4}")
    lines.append("  excluded by rule: " + ", ".join(f"{k} {v}" for k, v in sorted(excluded.items())))
    lines.append(f"  VALUE mutators hidden (non-const members of value classes, decision Q3) and property readers: {hidden}")
    lines.append("  blocking reasons: " + ", ".join(f"{k} {v}" for k, v in reasons.most_common(16)))
    lines.append("")
    lines.append("== overloads (decision Q4: runtime dispatch in a fixed order)")
    lines.append(f"  JS method names {names}, overloaded {overloaded}; const/non-const twins merged {twins}; C++ spelling variants merged {merged} (same JS arguments and result: QStringView/QString, short/int, float/double)")
    lines.append(f"  ambiguous (an earlier overload has the same JS signature; blocked unless renamed): {len(amb)}")
    for a in amb:
        lines.append(f"    {a['key']}  <- {a['shadowed_by']}")
    lines.append("")
    lines.append("== curated set")
    for n, r in sorted(types.items()):
        if r["curated"]:
            lines.append(f"  {n:22} {r['category']:7} bound {n_members(r):4} blocked {len(r['blocked']):3} hidden {len(r['hidden']):3}")
    return "\n".join(lines) + "\n"


MOD_OF: dict[str, str] = {}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    for mod in MODULES:
        for n in json.loads((INGEST / f"{mod}.json").read_text())["classes"]:
            MOD_OF[n] = mod
    data = build()
    if a.report:
        sys.stdout.write(report(data))
    if a.write:
        print(f"types_qt_classify: wrote {write(data)} module files to {FACTS_DIR.relative_to(ROOT)}")
    if a.check:
        return check(data)
    return 0


if __name__ == "__main__":
    sys.exit(main())
