#!/usr/bin/env python3
"""Ingest QtWidgets metadata into svelters-element-kind/v0 JSON (one file per class).

Sources (see plans/architecture/widget-metadata.md):
  qdoc    <docdir>/{qtcore,qtgui,qtwidgets}/*.index   full public API, signatures, defaults, virtuals
  meta    PySide6 QMetaObject dump                    properties, signals, slots, enums (flags, notify)
  tsys    PySide6 typesystem_widgets_common.xml       parent ownership, replace_child, keep-alive

Deterministic: output depends only on those inputs; sorted keys and sorted records.

  python3 -I tools/metadata/qt/ingest.py [--out metadata/qt] [--docdir /usr/share/doc/qt6]
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "common"))
import svmeta  # noqa: E402

PRIM = {
    "bool": "bool", "int": "i32", "uint": "u32", "unsigned int": "u32", "short": "i32", "long": "i64",
    "qint64": "i64", "qlonglong": "i64", "quint64": "u64", "qulonglong": "u64", "double": "f64",
    "qreal": "f64", "float": "f32", "QString": "string", "QStringView": "string", "QLatin1StringView": "string",
    "void": "void", "char": "i32", "QVariant": "variant",
}


def sha(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


# Value classes that map onto the neutral record vocabulary (portable field names, see
# docs/notes/js-native-serdes.md section 2.2) and the opaque value classes JS holds as boxed handles.
RECORDS = {"QRect": "Rect", "QRectF": "Rect", "QSize": "Size", "QSizeF": "Size", "QPoint": "Point",
           "QPointF": "Point", "QMargins": "Insets", "QMarginsF": "Insets", "QColor": "Color"}
BOXED = {"QIcon", "QFont", "QPixmap", "QImage", "QBrush", "QPalette", "QCursor", "QKeySequence", "QUrl",
         "QLocale", "QPainterPath", "QRegion", "QPicture", "QSizePolicy", "QTextCharFormat", "QTextFormat",
         "QTextCursor", "QTextOption", "QPen", "QTransform", "QBitmap"}
DATES = {"QDate": "date", "QTime": "time", "QDateTime": "datetime"}


class EnumIndex:
    """Enums and flags typedefs from the qdoc indexes, by fully qualified name.

    Resolves the spellings qdoc uses for a property or parameter type: `Qt::TextFormat`,
    `EchoMode` (relative to the class being documented), `Qt::Alignment` / `StandardButtons`
    (a QFlags typedef) and `QFlags<Qt::AlignmentFlag>`.
    """

    def __init__(self, owners):
        self.enums: dict = {}
        self.alias: dict = {}
        for owner, node in owners:
            for e in node.findall("enum"):
                full = e.get("fullname") or f"{owner}::{e.get('name')}"
                self.enums[full] = {"owner": owner, "name": e.get("name"),
                                    "members": [{"name": v.get("name"), "value": v.get("value")}
                                                for v in e.findall("value")]}
                if e.get("typedef"):
                    self.alias[e.get("typedef")] = full
            for t in node.findall("typedef"):
                if t.get("enum"):
                    self.alias[t.get("fullname") or f"{owner}::{t.get('name')}"] = t.get("enum")

    def resolve(self, spelling: str, ctx):
        m = re.match(r"^QFlags<(.+)>$", spelling)
        if m:
            r = self.resolve(m.group(1).strip(), ctx)
            return ("flags", r[1]) if r else None
        for cand in [spelling] + [f"{c}::{spelling}" for c in ctx]:
            if cand in self.alias:
                return ("flags", self.alias[cand])
            if cand in self.enums:
                return ("enum", cand)
        return None


def neutral_type(native, qobjects, enums, ctx=()) -> dict:
    """Map a C++ type spelling onto the neutral vocabulary; `native` keeps the original.

    `ctx` is the class being documented followed by its bases: it scopes unqualified enum names.
    """
    n = (native or "void").strip()
    core = re.sub(r"\bconst\b", "", n).replace("&", "").strip()
    ptr = core.endswith("*")
    core = core.rstrip("*").strip()
    out: dict = {"native": n, "origin": "qdoc"}
    en = None if ptr else enums.resolve(core, ctx)
    if ptr and core in qobjects:
        out.update(ref=f"object<{core}>", nullable=True)
    elif core in PRIM and not ptr:
        out.update(ref=PRIM[core], nullable=False)
    elif en and not ptr:
        out.update(ref=f"{en[0]}<{en[1]}>", nullable=False)
    elif core in RECORDS and not ptr:
        out.update(ref=f"record<{RECORDS[core]}>", nullable=False)
    elif core in DATES and not ptr:
        out.update(ref=DATES[core], nullable=False)
    elif core == "QByteArray" and not ptr:
        out.update(ref="bytes", nullable=False)
    elif core == "QStringList" and not ptr:
        out.update(ref="list<string>", nullable=False)
    elif core in BOXED and not ptr:
        out.update(ref=f"boxed<{core}>", nullable=False)
    elif core.startswith("QList<") and core.endswith(">") and not ptr:
        inner = neutral_type(core[6:-1], qobjects, enums, ctx)
        out.update(ref=f"list<{inner['ref']}>" if inner["ref"] != "opaque" else "list", nullable=False)
    elif core.startswith("QList<") or core.startswith("QStringList"):
        out.update(ref="list", nullable=False)
    else:
        out.update(ref="opaque", nullable=ptr)
    return out


def parse_qdoc(docdir: Path):
    classes, files, namespaces = {}, {}, {}
    for mod in ("qtcore", "qtgui", "qtwidgets"):
        p = docdir / mod / f"{mod}.index"
        files[mod] = p
        root = ET.parse(p).getroot()
        for c in root.iter("class"):
            classes.setdefault(c.get("name"), (mod, c))
        for ns in root.iter("namespace"):
            if ns.get("name") == "Qt":
                namespaces.setdefault("Qt", (mod, ns))
    return classes, files, namespaces


def direct_bases(name, classes):
    ent = classes.get(name)
    if not ent:
        return []
    return [x.strip().split("<")[0] for x in (ent[1].get("bases") or "").split(",") if x.strip()]


def base_chain(name, classes):
    out: list = []

    def walk(n):
        for b in direct_bases(n, classes):
            if b not in out:
                out.append(b)
                walk(b)

    walk(name)
    return out


def parse_typesystem(path: Path):
    """object-type name -> ownership facts (origin typesystem)."""
    root = ET.parse(path).getroot()
    facts: dict = {}
    for ot in root.iter("object-type"):
        cls = ot.get("name")
        out = facts.setdefault(cls, [])
        for mf in ot.findall("modify-function"):
            base = {"method": mf.get("signature") or "", "origin": "typesystem"}
            for arg in mf.findall("modify-argument"):
                idx = arg.get("index")
                for par in arg.findall("parent"):
                    out.append({**base, "fact": "parent", "arg": idx, "parent_index": par.get("index"),
                                "action": par.get("action")})
                for own in arg.findall("define-ownership"):
                    out.append({**base, "fact": "define_ownership", "arg": idx, "owner": own.get("owner"),
                                "class": own.get("class")})
                for rc in arg.findall("reference-count"):
                    out.append({**base, "fact": "reference_count", "arg": idx, "action": rc.get("action"),
                                "variable": rc.get("variable-name")})
            for ic in mf.iter("insert-template"):
                rep = {r.get("from"): r.get("to") for r in ic.findall("replace")}
                out.append({**base, "fact": ic.get("name"), "get_old": rep.get("$FUNCTION_GET_OLD"),
                            "child_type": rep.get("$CHILD_TYPE")})
            for ic in mf.iter("inject-code"):
                if ic.get("snippet"):
                    out.append({**base, "fact": "snippet", "snippet": ic.get("snippet")})
            if mf.get("remove"):
                out.append({**base, "fact": "removed", "scope": mf.get("remove")})
    for k in list(facts):
        facts[k] = sorted(facts[k], key=lambda f: (f["method"], f["fact"], str(f.get("arg")), str(f.get("action")),
                                                   str(f.get("snippet"))))
    return facts


def metaobject_dump(names):
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6 import QtCore, QtWidgets, __version__ as pyside

    mt = QtCore.QMetaMethod.MethodType

    def b(x):
        return bytes(x).decode() if not isinstance(x, str) else x

    dumps = {}
    for n in names:
        cls = getattr(QtWidgets, n, None)
        if not (isinstance(cls, type) and issubclass(cls, QtCore.QObject)):
            continue
        mo = cls.staticMetaObject
        d = {"properties": {}, "signals": {}, "slots": {}, "invokables": {}, "enums": {}}
        for i in range(mo.propertyOffset(), mo.propertyCount()):
            p = mo.property(i)
            d["properties"][p.name()] = {
                "type": p.typeName(), "readable": p.isReadable(), "writable": p.isWritable(),
                "resettable": p.isResettable(), "designable": p.isDesignable(), "stored": p.isStored(),
                "user": p.isUser(), "constant": p.isConstant(), "final": p.isFinal(), "required": p.isRequired(),
                "enum": p.isEnumType(), "flag": p.isFlagType(),
                "notify": b(p.notifySignal().methodSignature()) if p.hasNotifySignal() else None}
        for i in range(mo.methodOffset(), mo.methodCount()):
            m = mo.method(i)
            kind = {mt.Signal: "signals", mt.Slot: "slots", mt.Method: "invokables"}.get(m.methodType())
            if kind is None:
                continue
            sig = b(m.methodSignature())
            d[kind].setdefault(sig.split("(")[0], []).append(
                {"sig": sig, "return": m.typeName(), "params": [b(x) for x in m.parameterNames()]})
        for i in range(mo.enumeratorOffset(), mo.enumeratorCount()):
            e = mo.enumerator(i)
            d["enums"][e.name()] = {"flag": e.isFlag(), "scoped": e.isScoped(),
                                    "keys": {e.key(k): e.value(k) for k in range(e.keyCount())}}
        dumps[n] = d
    return dumps, {"pyside6": pyside, "qt": QtCore.qVersion()}


def build_kind(name, c, classes, qobjects, enums, meta, tsys, src):
    prefix = src["id_prefix"]
    ctx = [name] + base_chain(name, classes)
    ctors, methods, signals, props = [], [], [], []
    func_prop: dict = {}
    for f in c.findall("function"):
        m = f.get("meta")
        acc = f.get("access")
        if m in ("destructor", "copy-assign", "copy-constructor", "macrowithoutparams", "macrowithparams"):
            continue
        if acc not in ("public", "protected") or f.get("status") == "obsolete":
            continue
        params = [{"name": p.get("name") or "", "type": neutral_type(p.get("type"), qobjects, enums, ctx),
                   "default": p.get("default") or None} for p in f.findall("parameter")]
        sig = f.get("signature") or f.get("name")
        rec = {"name": f.get("name"), "signature": sig, "params": params, "access": acc, "origin": "qdoc",
               "since": f.get("since"), "deprecated": f.get("status") == "deprecated"}
        if m == "constructor":
            ctors.append({"id": f"{prefix}:{name}:ctor:{sig}", "native": sig, "params": params,
                          "returns": {"type": "self", "ownership": "parent_owned"},
                          "invocation": {"dev": "generated_thunk"}, "origin": "qdoc"})
            continue
        rec["returns"] = neutral_type(f.get("type"), qobjects, enums, ctx)
        rec["virtual"] = f.get("virtual")
        rec["static"] = f.get("static") == "true"
        rec["const"] = f.get("const") == "true"
        rec["override"] = f.get("override") == "true"
        rec["associated_property"] = f.get("associated-property")
        if m == "signal":
            signals.append(rec)
            continue
        rec["meta"] = "slot" if m == "slot" else "plain"
        rec["id"] = f"{prefix}:{name}:method:{sig}"
        methods.append(rec)
        if f.get("associated-property"):
            func_prop.setdefault(f.get("associated-property"), []).append(rec)
    md = meta.get(name)
    for p in c.findall("property"):
        pn = p.get("name")
        accs = func_prop.get(pn, [])
        getter = next((a["name"] for a in accs if not a["params"]), None)
        setter = next((a["name"] for a in accs if len(a["params"]) == 1 and a["name"].startswith("set")), None)
        reset = next((a["name"] for a in accs if a["name"].startswith("reset")), None)
        rec = {"name": pn, "js_name": pn, "type": neutral_type(p.get("dataType"), qobjects, enums, ctx),
               "access": {"read": True, "write": p.get("writable") != "false"},
               "accessors": {"get": getter, "set": setter, "reset": reset, "origin": "qdoc"},
               "brief": p.get("brief"), "role": "value", "origin": "qdoc"}
        mp = (md or {}).get("properties", {}).get(pn)
        if mp:
            rec["metaobject"] = {**mp, "origin": "metaobject"}
            rec["notify"] = {"signal": mp["notify"], "origin": "metaobject"} if mp["notify"] else None
            rec["access"]["write"] = mp["writable"]
        props.append(rec)
    msigs = (md or {}).get("signals", {})
    for s in signals:
        s["id"] = f"{prefix}:{name}:signal:{s['signature']}"
        s["js_event"] = s["name"]
        s["metaobject_signatures"] = sorted(x["sig"] for x in msigs.get(s["name"], []))
        s["returns"] = {"type": "void"}
        s["handler_result"] = "none"
        s["sync_required"] = False
    mmeta = {**(md or {}).get("slots", {}), **(md or {}).get("invokables", {})}
    for m in methods:
        refl = m["name"] in mmeta
        m["invocation"] = {"dev": "reflected" if refl else "generated_thunk", "origin": "metaobject" if refl else "qdoc"}
    enum_recs = []
    for e in c.findall("enum"):
        mo = (md or {}).get("enums", {}).get(e.get("name"))
        scoped = bool(mo and mo.get("scoped"))
        full = e.get("fullname") or f"{name}::{e.get('name')}"
        enum_recs.append({"name": e.get("name"), "fullname": full, "flags_alias": e.get("typedef"),
                          "scoped": scoped,
                          "members": [{"name": v.get("name"), "value": v.get("value"),
                                       "cpp": f"{full}::{v.get('name')}" if scoped else f"{name}::{v.get('name')}"}
                                      for v in e.findall("value")],
                          "origin": "qdoc", "metaobject": mo})

    def order(r):
        return (str(r.get("name") or r.get("id")), str(r.get("signature") or r.get("native") or ""))

    return {
        "schema": svmeta.SCHEMA, "toolkit": "qt",
        "id": f"{prefix}:{name}",
        "native": {"name": name, "header": c.get("location"), "module": "QtWidgets", "brief": c.get("brief")},
        "kind": "object", "abstract": any(f.get("virtual") == "pure" for f in c.findall("function")),
        "qobject": md is not None,
        "bases": base_chain(name, classes), "interfaces": [],
        "availability": {"since": None, "deprecated": c.get("status") == "deprecated"},
        "constructors": sorted(ctors, key=order), "properties": sorted(props, key=order),
        "signals": sorted(signals, key=order), "methods": sorted(methods, key=order),
        "enums": sorted(enum_recs, key=order),
        "ownership": tsys.get(name, []),
        "children": {"policy": "none", "default_slot": None, "slots": []},
        "source": src["source"],
    }


def build_enum_table(name, node, enums, kinds, src):
    """`_enums/Qt.json` (not a kind: the docs and the kind loaders read only `metadata/qt/*.json`): member tables for every enum and flags type that a slot type in the metadata mentions,
    whichever class (QtCore, QtGui or QtWidgets) declares it. The Qt backend generator reads member
    names and C++ expressions from here; the Qt namespace record itself carries no members of its own."""
    import json
    blob = json.dumps(kinds, sort_keys=True)
    used = sorted(set(re.findall(r"(?:enum|flags)<([A-Za-z0-9_:]+)>", blob)))
    recs = []
    for full in used:
        e = enums.enums.get(full)
        if e is None:
            continue
        owner = e["owner"]
        recs.append({"name": e["name"], "fullname": full, "owner": owner, "scoped": False,
                     "flags_alias": next((a for a, t in sorted(enums.alias.items()) if t == full), None),
                     "members": [{"name": v["name"], "value": v["value"], "cpp": f"{owner}::{v['name']}"}
                                 for v in e["members"]],
                     "origin": "qdoc", "metaobject": None})
    return {
        "schema": svmeta.SCHEMA, "toolkit": "qt", "id": f"{src['id_prefix']}:{name}",
        "native": {"name": name, "header": node.get("location"), "module": "QtCore", "brief": node.get("brief")},
        "kind": "namespace", "abstract": False, "qobject": False, "bases": [], "interfaces": [],
        "availability": {"since": None, "deprecated": False},
        "constructors": [], "properties": [], "signals": [], "methods": [],
        "enums": recs, "ownership": [],
        "children": {"policy": "none", "default_slot": None, "slots": []}, "source": src["source"],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--docdir", default="/usr/share/doc/qt6")
    ap.add_argument("--typesystem", default="/usr/share/PySide6/typesystems/typesystem_widgets_common.xml")
    ap.add_argument("--out", default=str(HERE.parents[2] / "metadata" / "qt"))
    a = ap.parse_args()
    docdir = Path(a.docdir)
    classes, files, namespaces = parse_qdoc(docdir)
    widget_classes = sorted(
        n for n, (m, c) in classes.items()
        if m == "qtwidgets" and c.get("access") == "public" and c.get("status") in ("active", "deprecated")
        and c.get("location") and "::" not in n and "<" not in n)
    qobjects = {n for n in classes if n == "QObject" or "QObject" in base_chain(n, classes)}
    owners = [(n, c) for n, (_, c) in sorted(classes.items())] + [(n, c) for n, (_, c) in sorted(namespaces.items())]
    enums = EnumIndex(owners)
    meta, vers = metaobject_dump(widget_classes)
    tsys = parse_typesystem(Path(a.typesystem))
    source = {
        "qdoc": {"files": {m: {"file": f"{m}.index", "sha256": sha(p)} for m, p in sorted(files.items())},
                 "version": ET.parse(files["qtwidgets"]).getroot().get("version")},
        "metaobject": {"origin": "metaobject", **vers},
        "typesystem": {"file": Path(a.typesystem).name, "sha256": sha(Path(a.typesystem))},
    }
    src = {"id_prefix": "svelters-api:v1:type:qt:QtWidgets", "source": source}
    out = Path(a.out)
    wrote = 0
    built = []
    for n in widget_classes:
        kind = build_kind(n, classes[n][1], classes, qobjects, enums, meta, tsys, src)
        errs = svmeta.validate(kind)
        if errs:
            print(f"{n}: {errs}", file=sys.stderr)
            return 1
        built.append(kind)
        wrote += svmeta.write_json(out / f"{n}.json", kind)
    wrote += svmeta.write_json(out / "_enums" / "Qt.json", build_enum_table("Qt", namespaces["Qt"][1], enums, [{k: v for k, v in b.items() if k in ("properties", "methods", "signals")} for b in built], src))
    print(f"{len(widget_classes)} classes ({len(meta)} QObject with metaobject); {wrote} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
