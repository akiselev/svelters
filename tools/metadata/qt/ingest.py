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


def neutral_type(native, qobjects, enums) -> dict:
    """Map a C++ type spelling onto the neutral vocabulary; `native` keeps the original."""
    n = (native or "void").strip()
    core = re.sub(r"\bconst\b", "", n).replace("&", "").strip()
    ptr = core.endswith("*")
    core = core.rstrip("*").strip()
    out: dict = {"native": n, "origin": "qdoc"}
    if ptr and core in qobjects:
        out.update(ref=f"object<{core}>", nullable=True)
    elif core in PRIM and not ptr:
        out.update(ref=PRIM[core], nullable=False)
    elif core in enums and not ptr:
        out.update(ref=f"enum<{core}>", nullable=False)
    elif core.startswith("QList<") or core.startswith("QStringList"):
        out.update(ref="list", nullable=False)
    else:
        out.update(ref="opaque", nullable=ptr)
    return out


def parse_qdoc(docdir: Path):
    classes, files = {}, {}
    for mod in ("qtcore", "qtgui", "qtwidgets"):
        p = docdir / mod / f"{mod}.index"
        files[mod] = p
        root = ET.parse(p).getroot()
        for c in root.iter("class"):
            classes.setdefault(c.get("name"), (mod, c))
    return classes, files


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


# Classes that are not instantiated for defaults (application singletons, or constructors that
# need arguments PySide cannot default).
NO_INSTANCE = {"QApplication", "QWidgetItem", "QSpacerItem", "QWidgetItemV2", "QAccessibleWidget", "QGraphicsProxyWidget"}


def instance_defaults(cls, mo, QtCore) -> dict:
    """Typed defaults of the class's own properties, read from a default-constructed instance
    (offscreen). Only scalars and enum keys; anything else is left out."""
    name = cls.__name__
    if name in NO_INSTANCE:
        return {}
    try:
        obj = cls()
    except Exception:  # noqa: BLE001 - abstract or needs constructor arguments
        return {}
    out = {}
    for i in range(mo.propertyOffset(), mo.propertyCount()):
        p = mo.property(i)
        try:
            v = p.read(obj)
        except Exception:  # noqa: BLE001
            continue
        if p.isEnumType() or p.isFlagType():
            e = p.enumerator()
            iv = v.value if hasattr(v, "value") else v
            try:
                iv = int(iv)
            except Exception:  # noqa: BLE001
                continue
            key = e.valueToKeys(iv) if p.isFlagType() else e.valueToKey(iv)
            key = bytes(key).decode() if isinstance(key, (bytes, bytearray, QtCore.QByteArray)) else key
            out[p.name()] = key if key else iv
        elif isinstance(v, bool) or (isinstance(v, (int, float)) and abs(v) < 2**53) or isinstance(v, str):
            out[p.name()] = v
    try:
        obj.deleteLater()
    except Exception:  # noqa: BLE001
        pass
    return out


def metaobject_dump(names):
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6 import QtCore, QtWidgets, __version__ as pyside

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])  # noqa: F841 - widgets need one

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
        defaults = instance_defaults(cls, mo, QtCore)
        for pname, dv in defaults.items():
            d["properties"][pname]["default"] = dv
        for i in range(mo.enumeratorOffset(), mo.enumeratorCount()):
            e = mo.enumerator(i)
            d["enums"][e.name()] = {"flag": e.isFlag(), "scoped": e.isScoped(),
                                    "keys": {e.key(k): e.value(k) for k in range(e.keyCount())}}
        dumps[n] = d
    return dumps, {"pyside6": pyside, "qt": QtCore.qVersion()}


def qt_lifetime(name, classes, qobject, abstract, prefix) -> dict:
    """Per-kind lifetime block (docs/notes/ownership-and-lifetimes.md 2.3), derived from the class
    hierarchy the same way the generator decides `gen_is_toplevel`."""
    chain = [name] + base_chain(name, classes)
    out = {"id": f"{prefix}:{name}:lifetime", "origin": "derived"}
    if not qobject or abstract:
        out.update({"class": "borrowed_only", "native_refcount": "qobject_tree" if qobject else "none",
                    "provenance": "derived: abstract or not a QObject; JS cannot create it"})
    elif "QMainWindow" in chain or "QDialog" in chain:
        out.update({"class": "toplevel", "native_refcount": "qobject_tree", "destroy": {"native": "QObject::deleteLater()"},
                    "close": {"request_signal": "close-request", "veto": "event_ignore", "default": "destroy"},
                    "provenance": "derived: QMainWindow/QDialog subclass; ABI v2 toplevel class (plans/architecture/backend-abi.md)"})
    else:
        out.update({"class": "owned", "native_refcount": "qobject_tree", "destroy": {"native": "QObject::deleteLater()"},
                    "provenance": "derived: QObject; deleted with its parent or by destroy (deleteLater)"})
    return out


def build_kind(name, c, classes, qobjects, enums, meta, tsys, src):
    prefix = src["id_prefix"]
    ctors, methods, signals, props = [], [], [], []
    func_prop: dict = {}
    for f in c.findall("function"):
        m = f.get("meta")
        acc = f.get("access")
        if m in ("destructor", "copy-assign", "copy-constructor", "macrowithoutparams", "macrowithparams"):
            continue
        if acc not in ("public", "protected") or f.get("status") == "obsolete":
            continue
        params = [{"name": p.get("name") or "", "type": neutral_type(p.get("type"), qobjects, enums),
                   "default": p.get("default") or None} for p in f.findall("parameter")]
        sig = f.get("signature") or f.get("name")
        rec = {"name": f.get("name"), "signature": sig, "params": params, "access": acc, "origin": "qdoc",
               "since": f.get("since"), "deprecated": f.get("status") == "deprecated"}
        if m == "constructor":
            ctors.append({"id": f"{prefix}:{name}:ctor:{sig}", "native": sig, "params": params,
                          "returns": {"type": "self", "ownership": "parent_owned"},
                          "invocation": {"dev": "generated_thunk"}, "origin": "qdoc"})
            continue
        rec["returns"] = neutral_type(f.get("type"), qobjects, enums)
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
        rec = {"name": pn, "js_name": pn, "type": neutral_type(p.get("dataType"), qobjects, enums),
               "access": {"read": True, "write": p.get("writable") != "false"},
               "accessors": {"get": getter, "set": setter, "reset": reset, "origin": "qdoc"},
               "brief": p.get("brief"), "role": "value", "origin": "qdoc"}
        mp = (md or {}).get("properties", {}).get(pn)
        rec["default"], rec["range"] = None, None
        if mp:
            mp = dict(mp)
            if "default" in mp:
                rec["default"] = mp.pop("default")
                rec["default_origin"] = "runtime"  # read from a default-constructed instance (PySide6, offscreen)
            rec["metaobject"] = {**mp, "origin": "metaobject"}
            rec["notify"] = {"signal": mp["notify"], "origin": "metaobject"} if mp["notify"] else None
            rec["access"]["write"] = mp["writable"]
        props.append(rec)
    # Numeric value properties bounded by sibling minimum/maximum properties: their default range.
    byname = {r["name"]: r for r in props}
    for vn in ("value", "sliderPosition"):
        lo, hi = byname.get("minimum"), byname.get("maximum")
        if vn in byname and lo and hi and isinstance(lo["default"], (int, float)) and isinstance(hi["default"], (int, float)):
            byname[vn]["range"] = {"min": lo["default"], "max": hi["default"], "follows": ["minimum", "maximum"], "origin": "runtime"}
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
        enum_recs.append({"name": e.get("name"),
                          "members": [{"name": v.get("name"), "value": v.get("value")} for v in e.findall("value")],
                          "origin": "qdoc", "metaobject": (md or {}).get("enums", {}).get(e.get("name"))})

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
        "lifetime": qt_lifetime(name, classes, md is not None, any(f.get("virtual") == "pure" for f in c.findall("function")), prefix),
        "children": {"policy": "none", "default_slot": None, "slots": []},
        "source": src["source"],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--docdir", default="/usr/share/doc/qt6")
    ap.add_argument("--typesystem", default="/usr/share/PySide6/typesystems/typesystem_widgets_common.xml")
    ap.add_argument("--out", default=str(HERE.parents[2] / "metadata" / "qt"))
    a = ap.parse_args()
    docdir = Path(a.docdir)
    classes, files = parse_qdoc(docdir)
    widget_classes = sorted(
        n for n, (m, c) in classes.items()
        if m == "qtwidgets" and c.get("access") == "public" and c.get("status") in ("active", "deprecated")
        and c.get("location") and "::" not in n and "<" not in n)
    qobjects = {n for n in classes if n == "QObject" or "QObject" in base_chain(n, classes)}
    enums = {e.get("name") for _, c in classes.values() for e in c.findall("enum")}
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
    for n in widget_classes:
        kind = build_kind(n, classes[n][1], classes, qobjects, enums, meta, tsys, src)
        errs = svmeta.validate(kind)
        if errs:
            print(f"{n}: {errs}", file=sys.stderr)
            return 1
        wrote += svmeta.write_json(out / f"{n}.json", kind)
    print(f"{len(widget_classes)} classes ({len(meta)} QObject with metaobject); {wrote} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
