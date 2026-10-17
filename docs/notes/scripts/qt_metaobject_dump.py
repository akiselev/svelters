#!/usr/bin/env python3
"""Proof-of-concept: QMetaObject dump + metaobject-vs-ordinary-method coverage (PySide6).

  python3 -I qt_metaobject_dump.py QPushButton      # one class, JSON
  python3 -I qt_metaobject_dump.py --coverage       # all QtWidgets QObject subclasses, table

Coverage method: the public method surface is taken from the PySide6 binding of each class
(= Shiboken ApiExtractor + typesystem, i.e. what a curated binding generator exposes),
counted by *own* method name (inherited names excluded; overloads collapsed; PySide also
exposes protected virtuals such as paintEvent, so "ordinary" is an upper bound - see
qdoc_index_stats.py for the public-only count). Each name is classified as
  signal / slot / invokable   - reachable via QMetaMethod::invoke
  property accessor           - foo(), setFoo(), isFoo(), hasFoo() for a Q_PROPERTY "foo"
                                (reachable via QObject::property/setProperty)
  ordinary                    - needs a generated C++ thunk.
Constructors are counted separately (QWidget subclasses have no Q_INVOKABLE ctors).
"""
import inspect
import json
import os
import sys

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
from PySide6 import QtCore, QtWidgets  # noqa: E402

MT = QtCore.QMetaMethod.MethodType


def b(x):
    return bytes(x).decode() if not isinstance(x, str) else x


def meta_dump(mo):
    own_m = range(mo.methodOffset(), mo.methodCount())
    own_p = range(mo.propertyOffset(), mo.propertyCount())
    own_e = range(mo.enumeratorOffset(), mo.enumeratorCount())
    out = {"class": mo.className(), "super": mo.superClass().className() if mo.superClass() else None,
           "constructors": [b(mo.constructor(i).methodSignature()) for i in range(mo.constructorCount())],
           "properties": [], "signals": [], "slots": [], "invokables": [], "enums": [],
           "classinfo": {mo.classInfo(i).name(): mo.classInfo(i).value()
                         for i in range(mo.classInfoOffset(), mo.classInfoCount())}}
    for i in own_p:
        p = mo.property(i)
        out["properties"].append({
            "name": p.name(), "type": p.typeName(), "readable": p.isReadable(), "writable": p.isWritable(),
            "resettable": p.isResettable(), "designable": p.isDesignable(), "stored": p.isStored(),
            "user": p.isUser(), "constant": p.isConstant(), "final": p.isFinal(), "required": p.isRequired(),
            "enum": p.isEnumType(), "flag": p.isFlagType(),
            "notify": b(p.notifySignal().methodSignature()) if p.hasNotifySignal() else None,
            "revision": p.revision(),
        })
    for i in own_m:
        m = mo.method(i)
        rec = {"sig": b(m.methodSignature()), "return": m.typeName(),
               "params": [b(n) for n in m.parameterNames()], "access": m.access().name, "revision": m.revision()}
        kind = {MT.Signal: "signals", MT.Slot: "slots", MT.Method: "invokables"}.get(m.methodType())
        if kind:
            out[kind].append(rec)
    for i in own_e:
        e = mo.enumerator(i)
        out["enums"].append({"name": e.name(), "flag": e.isFlag(), "scoped": e.isScoped(),
                             "keys": {e.key(k): e.value(k) for k in range(e.keyCount())}})
    return out


def own_public_methods(cls):
    names = set()
    for k, v in vars(cls).items():
        if k.startswith("_") or not callable(v) or isinstance(v, type):
            continue
        if isinstance(v, QtCore.Signal):
            continue
        names.add(k)
    # Remove names that also exist on the base (re-declared overrides of virtuals).
    base = cls.__mro__[1]
    return {n for n in names if not hasattr(base, n)}


def classify(cls):
    mo = cls.staticMetaObject
    d = meta_dump(mo)
    meta_names = {s["sig"].split("(")[0] for k in ("signals", "slots", "invokables") for s in d[k]}
    props = [p["name"] for p in d["properties"]]
    acc = set()
    for p in props:
        cap = p[:1].upper() + p[1:]
        acc |= {p, "set" + cap, "is" + cap, "has" + cap, "reset" + cap}
    own = own_public_methods(cls)
    # Python-side signal objects are not in vars() as functions; slots/invokables are.
    meta = {n for n in own if n in meta_names}
    pacc = {n for n in own - meta if n in acc}
    ordinary = own - meta - pacc
    return d, {"own_methods": len(own), "meta": len(meta), "prop_accessor": len(pacc),
               "ordinary": len(ordinary), "ordinary_names": sorted(ordinary),
               "props": len(props), "signals": len(d["signals"]),
               "ctor_invokable": len(d["constructors"])}


def all_qobject_classes():
    for name in sorted(dir(QtWidgets)):  # PySide6 modules load lazily; vars() is incomplete
        obj = getattr(QtWidgets, name)
        if isinstance(obj, type) and issubclass(obj, QtCore.QObject):
            yield name, obj


def main():
    if "--coverage" in sys.argv:
        tot = {"own_methods": 0, "meta": 0, "prop_accessor": 0, "ordinary": 0,
               "props": 0, "signals": 0, "ctor_invokable": 0}
        n = 0
        rows = []
        for name, cls in all_qobject_classes():
            _, c = classify(cls)
            n += 1
            for k in tot:
                tot[k] += c[k]
            rows.append((name, c))
        for name, c in rows:
            if "--verbose" in sys.argv or name in ("QWidget", "QPushButton", "QAbstractButton", "QMainWindow",
                                                   "QSplitter", "QTabWidget", "QBoxLayout", "QTreeView",
                                                   "QLineEdit", "QDockWidget", "QToolBar", "QMenuBar"):
                print(f"{name:22} own={c['own_methods']:3} meta={c['meta']:3} propacc={c['prop_accessor']:3} "
                      f"ordinary={c['ordinary']:3} props={c['props']:3} signals={c['signals']:2}")
        print(f"\n{n} QObject classes in QtWidgets; totals: {json.dumps(tot)}")
        m = tot["own_methods"]
        print(f"metaobject-invokable {tot['meta']/m:.0%}, property-accessor {tot['prop_accessor']/m:.0%}, "
              f"ordinary {tot['ordinary']/m:.0%} of {m} own public method names; "
              f"invokable constructors: {tot['ctor_invokable']}")
        return
    cls = getattr(QtWidgets, sys.argv[1])
    chain, mo = [], cls.staticMetaObject
    while mo:
        chain.append(mo.className())
        mo = mo.superClass()
    d, c = classify(cls)
    print(json.dumps({"inheritance": chain, "meta": d, "coverage": c}, indent=1, default=str))


if __name__ == "__main__":
    main()
