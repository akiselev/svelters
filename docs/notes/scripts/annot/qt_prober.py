#!/usr/bin/env python3
"""Automated Qt 6 prober (PySide6, offscreen): candidate facts with evidence for QtWidgets classes.

Same shape as gtk_prober.py:
  props    QMetaProperty write/read round trip with a typed sample; QSignalSpy on every signal of
           the instance records which signals fired and with which args (trigger, expect_args,
           NOTIFY confirmation, binding change signal). Recipes on failure: range, populate,
           checkable, text.
  methods  accessors (metadata `associated_property`) checked against the property; zero-arg const
           getters called; zero-arg non-const public methods called and diffed (props + signals).
  slots    setX(QWidget*) with a matching getter: parented? old child deleted or unparented on
           replace (shiboken isValid after DeferredDelete)? add/insert + remove/take families:
           parented, order (indexOf), removal unparents or deletes?
Crash isolation: per-op markers, the driver skips the op that killed the worker and reruns.

Usage: python3 docs/notes/scripts/annot/qt_prober.py --out probe-qt.json [--kinds QSpinBox,QLineEdit] [-j 4]
"""
from __future__ import annotations

import argparse
import concurrent.futures as cf
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DANGEROUS = re.compile(r"^(exec|show\w*|open|close|raise_|activateWindow|grab\w*|release\w*|deleteLater|destroy|"
                       r"create|winId|effectiveWinId|createWinId|internalWinId|repaint|update|adjustSize|"
                       r"metaObject|staticMetaObject|dumpObject\w*|accept|reject|done|restart|next|back|"
                       r"getOpen\w+|getSave\w+|getExisting\w+|get(Int|Double|Item|Text|MultiLineText|Color|Font)|"
                       r"paintEvent|\w+Event|event|eventFilter|nativeEvent|devicePixelRatio\w*|"
                       r"inputMethodQuery|sizeHint|minimumSizeHint|render|grab|backingStore|"
                       r"windowHandle|screen|nativeParentWidget|window|topLevelWidget|focusProxy|"
                       r"popup|exec_|hidePopup|showPopup|showNormal|showMaximized|showMinimized|showFullScreen|"
                       r"clipboard|copy|cut|paste|print_?|undo|redo)$")


def kinds_from_metadata() -> list[str]:
    out = []
    for p in sorted((ROOT / "metadata/qt").glob("Q*.json")):
        d = json.loads(p.read_text())
        chain = [p.stem] + d.get("bases", [])
        if not d.get("qobject") or "QWidget" not in chain or d.get("abstract"):
            continue
        if d["native"].get("module") != "QtWidgets":
            continue
        out.append(p.stem)
    return out


def worker(kinds: list[str]) -> None:
    import faulthandler
    faulthandler.enable()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    import shiboken6
    from PySide6 import QtCore, QtGui, QtWidgets
    from PySide6.QtCore import QCoreApplication, QEvent, QMetaMethod, QtMsgType
    from PySide6.QtTest import QSignalSpy

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    skip = set(filter(None, os.environ.get("SVL_PROBE_SKIP", "").split(";")))
    logs: list[str] = []

    def handler(mode, ctx, msg):
        if mode in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg):
            logs.append(msg[:160])
    QtCore.qInstallMessageHandler(handler)

    def op(kind, section, name):
        key = f"{kind}:{section}:{name}"
        if key in skip:
            return False
        print("#op " + key, flush=True)
        return True

    def flush():
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        app.processEvents()

    def jsonable(v):
        if v is None or isinstance(v, (bool, int, str)):
            return v
        if isinstance(v, float):
            return round(v, 6)
        if hasattr(v, "name") and hasattr(v, "value") and not isinstance(v, QtCore.QObject):
            return str(getattr(v, "name", v))
        if isinstance(v, (list, tuple)):
            return [jsonable(x) for x in v]
        return f"<{type(v).__name__}>"

    class Spy:
        def __init__(self, w):
            self.w, self.spies = w, {}
            mo = w.metaObject()
            for i in range(mo.methodCount()):
                m = mo.method(i)
                if m.methodType() == QMetaMethod.Signal:
                    sig = bytes(m.methodSignature()).decode()
                    if sig.startswith(("destroyed", "objectNameChanged", "windowIconTextChanged")):
                        continue
                    try:
                        self.spies[sig] = QSignalSpy(w, m)
                    except Exception:
                        pass

            self.seen = {k: 0 for k in self.spies}

        def take(self):
            out = {}
            for k, s in self.spies.items():
                n0, n = self.seen[k], s.count()
                if n > n0:
                    out[k] = [jsonable(list(s.at(i))) for i in range(n0, min(n, n0 + 2))]
                self.seen[k] = n
            return out

    def new(kind):
        return getattr(QtWidgets, kind)()

    def samples(w, p, cur):
        t = p.typeName()
        if p.isEnumType():
            me = p.enumerator()
            vals = [me.value(i) for i in range(me.keyCount())]
            curv = int(cur.value) if hasattr(cur, "value") else cur
            alt = [v for v in vals if v != curv]
            if not alt:
                return []
            try:
                if hasattr(cur, "value"):
                    return [type(cur)(alt[0])]
                return [alt[0]]
            except Exception:
                return [alt[0]]
        if t == "bool":
            return [not cur]
        if t in ("int", "uint", "qlonglong", "qulonglong"):
            return [x for x in (cur + 1, 7, 1, cur - 1) if x != cur and -2**31 <= x < 2**31][:2]
        if t in ("double", "float", "qreal"):
            return [x for x in (cur + 1.5, 7.25, 0.5) if x != cur][:2]
        if t == "QString":
            n = p.name()
            if re.search(r"[Mm]ask", n):
                return ["AAA"]
            if re.search(r"[Ff]ormat", n):
                return ["yyyy-MM-dd"]
            return ["svl probe"]
        if t in ("QDate",):
            return [QtCore.QDate(2001, 2, 3)]
        if t in ("QTime",):
            return [QtCore.QTime(4, 5, 6)]
        if t in ("QDateTime",):
            return [QtCore.QDateTime(QtCore.QDate(2001, 2, 3), QtCore.QTime(4, 5, 6))]
        if t == "QColor":
            return [QtGui.QColor(10, 20, 30)]
        if t == "QSize":
            return [QtCore.QSize(31, 17)]
        return []

    def eq(a, b):
        try:
            if isinstance(a, float) or isinstance(b, float):
                return abs(float(a) - float(b)) < 1e-6
            if hasattr(a, "value") and not hasattr(b, "value"):
                return int(a.value) == b
            if hasattr(a, "value") and hasattr(b, "value"):
                return int(a.value) == int(b.value)
        except Exception:
            return False
        return a == b

    def r_range(w):
        w.setRange(0, 100) if hasattr(w, "setRange") else (_ for _ in ()).throw(RuntimeError())

    def r_populate(w):
        for m, mk in (("addItems", lambda: (["a", "b", "c"],)), ("addTab", lambda: (QtWidgets.QWidget(), "t")),
                      ("addWidget", lambda: (QtWidgets.QWidget(),)), ("addItem", lambda: ("x",))):
            if hasattr(w, m):
                for _ in range(3):
                    getattr(w, m)(*mk())
                return
        raise RuntimeError("no populate op")

    def r_checkable(w):
        w.setCheckable(True) if hasattr(w, "setCheckable") else (_ for _ in ()).throw(RuntimeError())

    def r_text(w):
        for m in ("setText", "setPlainText"):
            if hasattr(w, m):
                getattr(w, m)("hello world")
                return
        raise RuntimeError()

    RECIPES = [("range", r_range), ("populate", r_populate), ("checkable", r_checkable), ("text", r_text)]

    def read(w, name):
        # QObject::property() has no PySide converter for some namespaced enums/flags: use the getter
        try:
            return w.property(name)
        except Exception:
            mo = w.metaObject()
            g = getattr(w, name, None) or getattr(w, "is" + name[0].upper() + name[1:])
            return g()

    def write(w, name, v):
        try:
            return w.setProperty(name, v)
        except Exception:
            getattr(w, "set" + name[0].upper() + name[1:])(v)
            return True

    def probe_prop(kind, name):
        res = {}
        for rn, rf in [(None, None)] + RECIPES:
            w = new(kind)
            if rf:
                try:
                    rf(w)
                except Exception:
                    continue
            mo = w.metaObject()
            p = mo.property(mo.indexOfProperty(name))
            cur = read(w, name)
            ss = samples(w, p, cur)
            if not ss:
                return {"outcome": "no_sample", "type": p.typeName()}
            spy = Spy(w)
            logs.clear()
            ok = write(w, name, ss[0])
            got = read(w, name)
            fired = spy.take()
            notify = bytes(p.notifySignal().methodSignature()).decode() if p.hasNotifySignal() else None
            outcome = "error" if (logs or not ok) else "exact" if eq(got, ss[0]) else "ignored" if eq(got, cur) else "normalized"
            res = {"outcome": outcome, "sample": jsonable(ss[0]), "read": jsonable(got), "default": jsonable(cur),
                   "notify": notify, "notify_fired": bool(notify and notify in fired), "signals": fired,
                   "user": p.isUser(), "recipe": rn, "log": logs[:1]}
            if outcome == "exact":
                return res
        return res

    def meta_methods(kind):
        p = ROOT / f"metadata/qt/{kind}.json"
        return json.loads(p.read_text())["methods"] if p.exists() else []

    def snapshot(w):
        mo, s = w.metaObject(), {}
        for i in range(mo.propertyCount()):
            p = mo.property(i)
            if p.typeName() in ("bool", "int", "double", "QString") or p.isEnumType():
                try:
                    s[p.name()] = jsonable(w.property(p.name()))
                except Exception:
                    pass
        return s

    def probe_methods(kind, recipes):
        out = {}
        seen = set()
        for m in meta_methods(kind):
            n = m["name"]
            if n in seen or m.get("access") != "public" or m.get("static") or m.get("deprecated"):
                continue
            seen.add(n)
            if not op(kind, "method", n):
                out[n] = {"class": "crash"}
                continue
            if DANGEROUS.match(n):
                out[n] = {"class": "dangerous"}
                continue
            w = new(kind)
            logs.clear()
            ap = m.get("associated_property")
            try:
                if ap and not m["params"] and m["returns"].get("ref") not in (None, "void"):
                    r = getattr(w, n)()
                    out[n] = {"class": "accessor_get", "prop": ap,
                              "outcome": "exact" if eq(r, w.property(ap)) else "differs"}
                elif ap and len(m["params"]) == 1 and n.startswith("set"):
                    rn = recipes.get(ap)
                    if rn:
                        dict(RECIPES)[rn](w)
                    mo = w.metaObject()
                    pi = mo.indexOfProperty(ap)
                    if pi < 0:
                        out[n] = {"class": "accessor_set", "prop": ap, "outcome": "no_prop"}
                        continue
                    ss = samples(w, mo.property(pi), w.property(ap))
                    if not ss:
                        out[n] = {"class": "accessor_set", "prop": ap, "outcome": "no_sample"}
                        continue
                    v = ss[0]
                    if mo.property(pi).isEnumType():
                        v = type(w.property(ap))(v)
                    getattr(w, n)(v)
                    out[n] = {"class": "accessor_set", "prop": ap, "recipe": rn,
                              "outcome": "error" if logs else "exact" if eq(w.property(ap), ss[0]) else "differs"}
                elif not m["params"] and m.get("const") and m["returns"].get("ref") not in (None, "void"):
                    r = getattr(w, n)()
                    out[n] = {"class": "pure_getter", "outcome": "error" if logs else "ok", "ret": jsonable(r)}
                elif not m["params"] and m["returns"].get("ref") in (None, "void") and not m.get("const"):
                    spy = Spy(w)
                    before = snapshot(w)
                    getattr(w, n)()
                    flush()
                    after = snapshot(w)
                    diff = {k: [before[k], after[k]] for k in before if before[k] != after.get(k)}
                    out[n] = {"class": "action", "outcome": "error" if logs else "ok", "changed": diff,
                              "signals": sorted(spy.take())}
            except Exception as e:
                out[n] = {"class": "exception", "outcome": "error", "log": [str(e)[:120]]}
            if logs and n in out:
                out[n]["log"] = logs[:1]
        return out

    def probe_slots(kind):
        out = {}
        ms = meta_methods(kind)
        names = {m["name"] for m in ms}
        for m in ms:
            n = m["name"]
            pt = [p["type"].get("native", "") for p in m["params"]]
            if m.get("access") != "public":
                continue
            # one-slots: setX(QWidget*-ish) with a getter
            if re.match(r"set[A-Z]\w*", n) and len(pt) == 1 and re.match(r"Q\w*(Widget|Bar|Menu)\s*\*", pt[0]):
                key = n
                if (key in out and out[key].get("role") != "error") or not op(kind, "slot", key):
                    continue
                getter = next((g for g in (n[3].lower() + n[4:], "widget", "menuWidget") if g in names), None)
                cls = getattr(QtWidgets, pt[0].replace("*", "").strip(), None)
                if cls is None or cls is QtWidgets.QWidget:
                    cls = QtWidgets.QLabel if pt[0].startswith("QWidget") else cls
                w = new(kind)
                logs.clear()
                try:
                    a = cls()
                    getattr(w, n)(a)
                    par = "direct" if a.parent() is w else "ancestor" if w.isAncestorOf(a) else "none"
                    rb = getter and getattr(w, getter)() is a
                    b = cls()
                    getattr(w, n)(b)
                    flush()
                    old = "deleted" if not shiboken6.isValid(a) else ("unparented" if a.parent() is None else "still_parented")
                    out[key] = {"role": "slot" if par != "none" else "reference", "parent": par, "getter": getter,
                                "readback": bool(rb), "old_on_replace": old, "accepts": cls.__name__, "log": logs[:1]}
                except Exception as e:
                    out[key] = {"role": "error", "log": [str(e)[:120]]}
            # many-slots: add/insert taking a widget + remove/take
            if re.match(r"(add|insert)(Widget|Tab|Page|Item)?$", n) and any(re.match(r"QWidget\s*\*", x) for x in pt):
                key = f"<{n}>"
                if (key in out and out[key].get("role") != "error") or not op(kind, "slot", key):
                    continue
                rem = next((r for r in ("removeWidget", "removeTab", "removePage", "takeWidget", "removeItem") if r in names), None)
                w = new(kind)
                logs.clear()
                try:
                    kids = [QtWidgets.QLabel(f"c{i}") for i in range(2)]
                    for k in kids:
                        args = []
                        for p in m["params"]:
                            nt = p["type"].get("native", "")
                            if re.match(r"QWidget\s*\*", nt):
                                args.append(k)
                            elif p.get("default") is not None:
                                break
                            elif nt.startswith("int"):
                                args.append(-1 if n.startswith("insert") else 0)
                            elif "QString" in nt:
                                args.append("t")
                            else:
                                raise RuntimeError(f"arg {nt}")
                        getattr(w, n)(*args)
                    par = ["direct" if k.parent() is w else "ancestor" if w.isAncestorOf(k) else "none" for k in kids]
                    order = [w.indexOf(k) for k in kids] if hasattr(w, "indexOf") else None
                    removed = None
                    if rem:
                        if rem in ("removeTab", "removePage", "removeItem"):
                            getattr(w, rem)(0)
                        else:
                            getattr(w, rem)(kids[0])
                        flush()
                        removed = "deleted" if not shiboken6.isValid(kids[0]) else (
                            "unparented" if kids[0].parent() is None else "still_parented")
                    out[key] = {"role": "many_slot", "insert": n, "remove": rem, "parent": par, "order": order,
                                "on_remove": removed, "log": logs[:1]}
                except Exception as e:
                    out[key] = {"role": "error", "log": [str(e)[:120]]}
        return out

    for kind in kinds:
        res: dict = {"kind": kind}
        try:
            w = new(kind)
            mo = w.metaObject()
            sup = mo.superClass()
            first = sup.propertyCount() if sup else 0
            res["props"] = {}
            for i in range(first, mo.propertyCount()):
                p = mo.property(i)
                n = p.name()
                if not (p.isReadable() and p.isWritable()):
                    continue
                if not op(kind, "prop", n):
                    res["props"][n] = {"outcome": "crash"}
                    continue
                try:
                    res["props"][n] = probe_prop(kind, n)
                except Exception as e:
                    res["props"][n] = {"outcome": "error", "log": [f"probe: {e}"[:120]]}
            res["methods"] = probe_methods(kind, {k: v.get("recipe") for k, v in res["props"].items()})
            res["slots"] = probe_slots(kind)
            first_m = sup.methodCount() if sup else 0
            res["signals"] = {bytes(mo.method(i).methodSignature()).decode(): {}
                              for i in range(first_m, mo.methodCount()) if mo.method(i).methodType() == QMetaMethod.Signal}
        except Exception as e:
            res["error"] = str(e)[:200]
        print(json.dumps(res, sort_keys=True, default=str), flush=True)


def run_batch(kinds: list[str], timeout: int, skip: tuple[str, ...] = ()) -> list[dict]:
    env = dict(os.environ, SVL_PROBE_SKIP=";".join(skip), QT_QPA_PLATFORM="offscreen")
    try:
        p = subprocess.run([sys.executable, __file__, "--worker", ",".join(kinds)], capture_output=True, text=True,
                           timeout=timeout, env=env)
        out, err, crashed = p.stdout, p.stderr[-2000:] + f"\nexit {p.returncode}", p.returncode != 0
    except subprocess.TimeoutExpired as e:
        out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        err, crashed = "timeout", True
    done = [json.loads(l) for l in out.splitlines() if l.startswith("{")]
    ops = [l[4:] for l in out.splitlines() if l.startswith("#op ")]
    if not crashed:
        return done
    seen = {d["kind"] for d in done}
    rest = [k for k in kinds if k not in seen]
    if not rest:
        return done
    bad = rest[0]
    last = ops[-1] if ops and ops[-1].startswith(bad + ":") else None
    if last and len(skip) < 12:
        crash = {"op": last, "why": "timeout" if err == "timeout" else err.strip().splitlines()[-1]}
        again = run_batch(rest, timeout, skip + (last,))
        for d in again:
            if d["kind"] == bad:
                d.setdefault("crashes", []).append(crash)
        return done + again
    done.append({"kind": bad, "crash": err.strip().splitlines()[-1] if err.strip() else "crash"})
    return done + (run_batch(rest[1:], timeout, skip) if rest[1:] else [])


def summarize(doc: dict) -> None:
    import collections
    c = collections.Counter()
    for kind, k in doc["kinds"].items():
        c["kind:crash" if k.get("crash") else "kind:error" if k.get("error") else "kind:ok"] += 1
        c["op:crash"] += len(k.get("crashes", []))
        for n, p in k.get("props", {}).items():
            c[f"prop:{p.get('outcome')}"] += 1
            if p.get("outcome") == "exact":
                c["prop:exact:notify_fired" if p.get("notify_fired") else "prop:exact:no_notify" if not p.get("notify") else "prop:exact:notify_silent"] += 1
                if p.get("recipe"):
                    c[f"prop:recipe:{p['recipe']}"] += 1
                if p.get("signals"):
                    c["prop:exact:expect_args_observed"] += 1
        for n, m in k.get("methods", {}).items():
            c[f"method:{m['class']}:{m.get('outcome', '-')}"] += 1
            if m["class"] == "action" and m.get("outcome") == "ok":
                c["method:action:observable" if (m.get("changed") or m.get("signals")) else "method:action:no_effect"] += 1
        for n, s in k.get("slots", {}).items():
            c[f"slot:{s.get('role')}"] += 1
            if s.get("role") == "slot":
                c[f"slot:old:{s.get('old_on_replace')}"] += 1
            if s.get("role") == "many_slot":
                c[f"slot:many:on_remove:{s.get('on_remove')}"] += 1
        c["signal:own"] += len(k.get("signals", {}))
        fired = {sig for p in k.get("props", {}).values() for sig in (p.get("signals") or {})}
        c["signal:own_trigger_found"] += sum(1 for s in k.get("signals", {}) if s in fired)
    for key, v in sorted(c.items()):
        print(f"  {key:44} {v}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--worker")
    ap.add_argument("--kinds")
    ap.add_argument("--out", default="probe-qt.json")
    ap.add_argument("--summary")
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--batch", type=int, default=6)
    ap.add_argument("--timeout", type=int, default=120)
    a = ap.parse_args()
    if a.worker:
        worker(a.worker.split(","))
        return 0
    if a.summary:
        summarize(json.loads(Path(a.summary).read_text()))
        return 0
    kinds = a.kinds.split(",") if a.kinds else kinds_from_metadata()
    batches = [kinds[i:i + a.batch] for i in range(0, len(kinds), a.batch)]
    results = []
    with cf.ThreadPoolExecutor(a.j) as ex:
        for r in ex.map(lambda b: run_batch(b, a.timeout), batches):
            results.extend(r)
    results.sort(key=lambda d: d["kind"])
    from PySide6 import QtCore
    doc = {"schema": "svelters-probe/v0", "toolkit": "qt", "version": QtCore.qVersion(),
           "kinds": {d["kind"]: d for d in results}}
    Path(a.out).write_text(json.dumps(doc, indent=1, sort_keys=True, default=str))
    print(f"probed {len(results)} kinds -> {a.out}")
    summarize(doc)
    return 0


if __name__ == "__main__":
    sys.exit(main())
