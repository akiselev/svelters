#!/usr/bin/env python3
"""Qt 6 runtime prober (PySide6, offscreen): observations the annotation inference builds on.

    python3 tools/metadata/qt/probe.py [--kinds QSpinBox,QLineEdit] [-j 4] [--out metadata/probe/qt/<qt>.json]

A maintainer step, like ingestion (plans/research/ANNOTATION-AUTOMATION.md section 11, decision 5): CI never
probes, `ledger.py check` only checks that the checked-in probe is for the toolkit version of the ledger.
Needs PySide6 of the same Qt version as the ingested metadata (`pip install PySide6==<qt>`); no display
(QT_QPA_PLATFORM=offscreen).

Per class (QMetaObject of a default-constructed instance, own members only, the class's metadata/qt record):
  props    every declared property. Writable ones: write a typed sample, read it back, outcome exact |
           normalized | ignored | error | no_sample, every signal the write emitted (QSignalSpy) with its first
           arguments; when exact, the sequence sample -> default -> sample and the emissions per step (what a
           generated trigger test asserts). Recipes on failure: range, populate, checkable, text.
           Read-only ones: the value on a new instance and whether a second instance agrees (`stable`).
  methods  accessors (`associated_property`) checked against the property, zero-arg const getters called,
           zero-arg non-const methods called and diffed (props + signals). A denylist skips blocking or
           display-dependent calls.
  slots    setX(QWidget*) with a getter, add/insert + remove/take families (parenting, order, removal).
  signals  own signals with their metaobject signature and parameter types.

Isolation: every class runs in its own worker process. A probe op that kills the worker (some Qt classes
abort on odd inputs) is recorded under `crashes` and skipped on the rerun; a class that keeps crashing is
recorded as `crash`. The run never dies with a class.

Deterministic: same Qt, same output (sorted keys, rounded floats, no addresses, paths or timings).
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

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/qt"
OVERRIDES = ROOT / "metadata/overrides/qt"
OUT_DIR = ROOT / "metadata/probe/qt"
SCHEMA = "svelters-probe/v1"
MAX_CRASHES = 12

DANGEROUS = re.compile(r"^(exec|show\w*|open|close|raise_|activateWindow|grab\w*|release\w*|deleteLater|destroy|"
                       r"create|winId|effectiveWinId|createWinId|internalWinId|repaint|update|adjustSize|"
                       r"metaObject|staticMetaObject|dumpObject\w*|accept|reject|done|restart|next|back|"
                       r"getOpen\w+|getSave\w+|getExisting\w+|get(Int|Double|Item|Text|MultiLineText|Color|Font)|"
                       r"paintEvent|\w+Event|event|eventFilter|nativeEvent|devicePixelRatio\w*|"
                       r"inputMethodQuery|sizeHint|minimumSizeHint|render|grab|backingStore|"
                       r"windowHandle|screen|nativeParentWidget|window|topLevelWidget|focusProxy|"
                       r"popup|exec_|hidePopup|showPopup|showNormal|showMaximized|showMinimized|showFullScreen|"
                       r"clipboard|copy|cut|paste|print_?|undo|redo|trigger|hover|toggle|click|animateClick)$")
ADDR = re.compile(r"0x[0-9a-fA-F]+")
SKIP_SIGNALS = ("destroyed", "objectNameChanged", "windowIconTextChanged")


def kinds_from_metadata() -> list[str]:
    """Concrete QObject classes: every element class of metadata/overrides/qt plus every concrete QtWidgets
    widget class of the metadata (the inference only uses element classes; the rest is evidence for later)."""
    import yaml
    out = set()
    for p in sorted(OVERRIDES.glob("*.yaml")):
        for el in (yaml.safe_load(p.read_text()) or {}).get("elements", []) or []:
            out.add(el["construct"]["class"])
    for p in sorted(META.glob("Q*.json")):
        d = json.loads(p.read_text())
        chain = [p.stem] + d.get("bases", [])
        if d.get("qobject") and "QWidget" in chain and not d.get("abstract") and d["native"].get("module") == "QtWidgets":
            out.add(p.stem)
    return sorted(out)


def scrub(s: str) -> str:
    s = ADDR.sub("0x?", str(s))
    return s.replace(str(ROOT), "<root>").replace(str(Path.home()), "~")[:160]


# ---------------------------------------------------------------------------------------------------------
# worker (one class per process)

def worker(kind: str) -> None:
    import enum
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
            logs.append(scrub(msg))
    QtCore.qInstallMessageHandler(handler)

    def op(section, name):
        key = f"{kind}:{section}:{name}"
        if key in skip:
            return False
        print("#op " + key, flush=True)
        return True

    def flush():
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        app.processEvents()

    def jsonable(v):
        if isinstance(v, enum.Enum):
            return str(v.name)                                      # an enum: its member name (C++ key)
        if v is None or isinstance(v, bool):
            return v
        if isinstance(v, str):
            # file dialogs and the like report the working directory or the Qt settings history
            return "<path>" if v.startswith(("/", "file:")) else v
        if isinstance(v, int):
            return int(v)
        if isinstance(v, float):
            return round(v, 6)
        if isinstance(v, (list, tuple)):
            return [jsonable(x) for x in v]
        return f"<{type(v).__name__}>"

    def enum_int(v):
        try:
            return int(v.value) if isinstance(v, enum.Enum) else int(v)
        except Exception:
            return None

    class Spy:
        def __init__(self, w):
            self.spies = {}
            mo = w.metaObject()
            for i in range(mo.methodCount()):
                m = mo.method(i)
                if m.methodType() == QMetaMethod.Signal:
                    sig = bytes(m.methodSignature()).decode()
                    if sig.startswith(SKIP_SIGNALS):
                        continue
                    try:
                        self.spies[sig] = QSignalSpy(w, m)
                    except Exception:
                        pass
            self.seen = {k: 0 for k in self.spies}

        def take(self) -> dict:
            out = {}
            for k, s in sorted(self.spies.items()):
                n0, n = self.seen[k], s.count()
                if n > n0:
                    out[k] = {"count": n - n0, "args": [jsonable(x) for x in list(s.at(n0))]}
                self.seen[k] = n
            return out

    cls = getattr(QtWidgets, kind, None) or getattr(QtGui, kind, None) or getattr(QtCore, kind, None)

    def new():
        return cls()

    def samples(p, cur):
        t = p.typeName()
        n = p.name()
        if p.isEnumType():
            if p.isFlagType():
                return []
            me = p.enumerator()
            vals = [me.value(i) for i in range(me.keyCount())]
            curv = enum_int(cur)
            alt = [v for v in vals if v != curv]
            if not alt:
                return []
            try:
                return [type(cur)(alt[0])] if isinstance(cur, enum.Enum) else [alt[0]]
            except Exception:
                return [alt[0]]
        if t == "bool":
            return [not cur]
        if t in ("int", "uint", "qlonglong", "qulonglong"):
            return [x for x in (cur + 1, 7, 1, cur - 1) if x != cur and 0 <= x < 2**31][:2]
        if t in ("double", "float", "qreal"):
            return [x for x in (cur + 1.5, 7.25, 0.5) if x != cur][:2]
        if t == "QString":
            if re.search(r"[Mm]ask", n):
                return ["AAA"]
            if re.search(r"[Ff]ormat", n):
                return ["yyyy-MM-dd"]
            return ["svl probe"]
        return []                                                    # records, dates, objects: no sampler yet

    def eq(a, b):
        try:
            if isinstance(a, float) or isinstance(b, float):
                return abs(float(a) - float(b)) < 1e-6
            if isinstance(a, enum.Enum) or isinstance(b, enum.Enum):
                return enum_int(a) == enum_int(b)
        except Exception:
            return False
        return a == b

    # Recipes: preconditions a property needs before it can hold a sample. `how` names the call made, so the
    # inference can write the same precondition as C++.
    def r_range(w):
        if not hasattr(w, "setRange"):
            raise RuntimeError("no setRange")
        w.setRange(0, 100)
        return "setRange"

    def r_populate(w):
        for m, mk in (("addItems", lambda: (["a", "b", "c"],)), ("addTab", lambda: (QtWidgets.QWidget(), "t")),
                      ("addWidget", lambda: (QtWidgets.QWidget(),)), ("addItem", lambda: ("x",))):
            if hasattr(w, m):
                if m == "addItems":
                    getattr(w, m)(*mk())
                else:
                    for _ in range(3):
                        getattr(w, m)(*mk())
                return m
        raise RuntimeError("no populate op")

    def r_checkable(w):
        if not hasattr(w, "setCheckable"):
            raise RuntimeError("no setCheckable")
        w.setCheckable(True)
        return "setCheckable"

    def r_text(w):
        for m in ("setText", "setPlainText"):
            if hasattr(w, m):
                getattr(w, m)("hello world")
                return m
        raise RuntimeError("no text setter")

    RECIPES = [("range", r_range), ("populate", r_populate), ("checkable", r_checkable), ("text", r_text)]

    def read(w, name):
        try:
            return w.property(name)
        except Exception:
            g = getattr(w, name, None) or getattr(w, "is" + name[0].upper() + name[1:])
            return g()

    def write(w, name, v):
        try:
            return bool(w.setProperty(name, v))
        except Exception:
            getattr(w, "set" + name[0].upper() + name[1:])(v)
            return True

    def mprop(w, name):
        mo = w.metaObject()
        return mo.property(mo.indexOfProperty(name))

    def probe_writable(name):
        res: dict = {}
        for rn, rf in [(None, None)] + RECIPES:
            w = new()
            how = None
            if rf:
                try:
                    how = rf(w)
                except Exception:
                    continue
            p = mprop(w, name)
            cur = read(w, name)
            ss = samples(p, cur)
            if not ss:
                return {"outcome": "no_sample"}
            spy = Spy(w)
            logs.clear()
            ok = write(w, name, ss[0])
            got = read(w, name)
            fired = spy.take()
            outcome = "error" if (logs or not ok) else "exact" if eq(got, ss[0]) else "ignored" if eq(got, cur) else "normalized"
            res = {"outcome": outcome, "sample": jsonable(ss[0]), "read": jsonable(got), "default": jsonable(cur),
                   "signals": fired, "recipe": rn, "recipe_call": how, "log": logs[:1]}
            if p.isEnumType():
                res["sample_int"], res["default_int"] = enum_int(ss[0]), enum_int(cur)
            if outcome == "exact":
                # The trigger sequence a generated test runs: sample -> default -> sample, emissions per step.
                w2 = new()
                if rf:
                    rf(w2)
                spy2 = Spy(w2)
                steps = []
                for v in (ss[0], cur, ss[0]):
                    write(w2, name, v)
                    steps.append(spy2.take())
                seq = {}
                for sig in sorted({s for st in steps for s in st}):
                    seq[sig] = [st.get(sig, {}).get("count", 0) for st in steps]
                res["sequence"] = seq
                res["sequence_args"] = {sig: [st[sig]["args"] if sig in st else None for st in steps] for sig in seq}
                return res
        return res

    def probe_readonly(name):
        a, b = new(), new()
        logs.clear()
        va, vb = read(a, name), read(b, name)
        return {"outcome": "readonly", "value": jsonable(va), "stable": eq(va, vb) and jsonable(va) == jsonable(vb),
                "log": logs[:1]}

    def meta_record():
        p = META / f"{kind}.json"
        return json.loads(p.read_text()) if p.exists() else {"methods": [], "properties": []}

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

    def probe_methods(recipes):
        out = {}
        seen = set()
        for m in meta_record()["methods"]:
            n = m["name"]
            if n in seen or m.get("access") != "public" or m.get("static") or m.get("deprecated"):
                continue
            seen.add(n)
            if not op("method", n):
                out[n] = {"class": "crash"}
                continue
            if DANGEROUS.match(n):
                out[n] = {"class": "dangerous"}
                continue
            w = new()
            logs.clear()
            ap = m.get("associated_property")
            ret = m["returns"].get("ref")
            try:
                if ap and not m["params"] and ret not in (None, "void"):
                    r = getattr(w, n)()
                    out[n] = {"class": "accessor_get", "prop": ap, "ret": jsonable(r),
                              "outcome": "exact" if eq(r, read(w, ap)) else "differs"}
                elif ap and len(m["params"]) == 1 and n.startswith("set"):
                    rn = recipes.get(ap)
                    if rn:
                        dict(RECIPES)[rn](w)
                    mo = w.metaObject()
                    pi = mo.indexOfProperty(ap)
                    if pi < 0:
                        out[n] = {"class": "accessor_set", "prop": ap, "outcome": "no_prop"}
                        continue
                    ss = samples(mo.property(pi), read(w, ap))
                    if not ss:
                        out[n] = {"class": "accessor_set", "prop": ap, "outcome": "no_sample"}
                        continue
                    getattr(w, n)(ss[0])
                    out[n] = {"class": "accessor_set", "prop": ap, "recipe": rn, "sample": jsonable(ss[0]),
                              "outcome": "error" if logs else "exact" if eq(read(w, ap), ss[0]) else "differs"}
                elif not m["params"] and m.get("const") and ret not in (None, "void"):
                    r = getattr(w, n)()
                    r2 = getattr(new(), n)()
                    out[n] = {"class": "pure_getter", "outcome": "error" if logs else "ok", "ret": jsonable(r),
                              "stable": jsonable(r) == jsonable(r2)}
                elif not m["params"] and ret in (None, "void") and not m.get("const"):
                    spy = Spy(w)
                    before = snapshot(w)
                    getattr(w, n)()
                    flush()
                    after = snapshot(w)
                    diff = {k: [before[k], after[k]] for k in sorted(before) if before[k] != after.get(k)}
                    out[n] = {"class": "action", "outcome": "error" if logs else "ok", "changed": diff,
                              "signals": sorted(spy.take())}
                else:
                    out[n] = {"class": "call", "params": [q["type"].get("ref") for q in m["params"]], "returns": ret}
            except Exception as e:
                out[n] = {"class": "exception", "outcome": "error", "log": [scrub(e)]}
            if logs and n in out:
                out[n]["log"] = logs[:1]
        return out

    def probe_slots():
        out = {}
        ms = meta_record()["methods"]
        names = {m["name"] for m in ms}
        for m in ms:
            n = m["name"]
            pt = [p["type"].get("native", "") for p in m["params"]]
            if m.get("access") != "public":
                continue
            if re.match(r"set[A-Z]\w*", n) and len(pt) == 1 and re.match(r"Q\w*(Widget|Bar|Menu)\s*\*", pt[0]):
                key = n
                if (key in out and out[key].get("role") != "error") or not op("slot", key):
                    continue
                getter = next((g for g in (n[3].lower() + n[4:], "widget", "menuWidget") if g in names), None)
                ccls = getattr(QtWidgets, pt[0].replace("*", "").strip(), None)
                if ccls is None or ccls is QtWidgets.QWidget:
                    ccls = QtWidgets.QLabel if pt[0].startswith("QWidget") else ccls
                w = new()
                logs.clear()
                try:
                    a = ccls()
                    getattr(w, n)(a)
                    par = "direct" if a.parent() is w else "ancestor" if w.isAncestorOf(a) else "none"
                    rb = getter and getattr(w, getter)() is a
                    b = ccls()
                    getattr(w, n)(b)
                    flush()
                    old = "deleted" if not shiboken6.isValid(a) else ("unparented" if a.parent() is None else "still_parented")
                    out[key] = {"role": "slot" if par != "none" else "reference", "parent": par, "getter": getter,
                                "readback": bool(rb), "old_on_replace": old, "accepts": ccls.__name__, "log": logs[:1]}
                except Exception as e:
                    out[key] = {"role": "error", "log": [scrub(e)]}
            if re.match(r"(add|insert)(Widget|Tab|Page|Item)?$", n) and any(re.match(r"QWidget\s*\*", x) for x in pt):
                key = f"<{n}>"
                if (key in out and out[key].get("role") != "error") or not op("slot", key):
                    continue
                rem = next((r for r in ("removeWidget", "removeTab", "removePage", "takeWidget", "removeItem") if r in names), None)
                w = new()
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
                    out[key] = {"role": "error", "log": [scrub(e)]}
        return out

    res: dict = {"kind": kind}
    try:
        if cls is None:
            raise RuntimeError(f"PySide6 has no class {kind}")
        w = new()
        mo = w.metaObject()
        sup = mo.superClass()
        first = sup.propertyCount() if sup else 0
        res["props"] = {}
        for i in range(first, mo.propertyCount()):
            p = mo.property(i)
            n = p.name()
            info = {"type": p.typeName(), "readable": p.isReadable(), "writable": p.isWritable(), "user": p.isUser(),
                    "enum": p.isEnumType(), "flag": p.isFlagType(),
                    "scoped": bool(p.isEnumType() and p.enumerator().isScoped()),
                    "notify": bytes(p.notifySignal().methodSignature()).decode() if p.hasNotifySignal() else None}
            if not p.isReadable():
                res["props"][n] = {**info, "outcome": "unreadable"}
                continue
            if not op("prop", n):
                res["props"][n] = {**info, "outcome": "crash"}
                continue
            try:
                r = probe_writable(n) if p.isWritable() else probe_readonly(n)
            except Exception as e:
                r = {"outcome": "error", "log": [scrub(f"probe: {e}")]}
            if info["notify"] and r.get("signals") is not None:
                r["notify_fired"] = info["notify"] in r["signals"]
            res["props"][n] = {**info, **r}
        res["methods"] = probe_methods({k: v.get("recipe") for k, v in res["props"].items() if v.get("recipe")})
        res["slots"] = probe_slots()
        first_m = sup.methodCount() if sup else 0
        res["signals"] = {}
        for i in range(first_m, mo.methodCount()):
            m = mo.method(i)
            if m.methodType() == QMetaMethod.Signal:
                res["signals"][bytes(m.methodSignature()).decode()] = {
                    "name": bytes(m.name()).decode(), "params": [bytes(t).decode() for t in m.parameterTypes()]}
    except Exception as e:
        res["error"] = scrub(e)
    print(json.dumps(res, sort_keys=True, default=str), flush=True)


# ---------------------------------------------------------------------------------------------------------
# driver

def run_kind(kind: str, timeout: int) -> dict:
    """Probe one class in its own process; an op that kills the worker is recorded and skipped on a rerun."""
    import tempfile
    skip: list[str] = []
    crashes: list[dict] = []
    while True:
        cfg = tempfile.mkdtemp(prefix="svl-qt-probe-")      # an empty Qt settings home: no file-dialog history
        env = dict(os.environ, SVL_PROBE_SKIP=";".join(skip), QT_QPA_PLATFORM="offscreen", PYTHONHASHSEED="0",
                   XDG_CONFIG_HOME=cfg, XDG_DATA_HOME=cfg, XDG_CACHE_HOME=cfg, LC_ALL="C.UTF-8", LANG="C.UTF-8")
        try:
            p = subprocess.run([sys.executable, "-I", str(Path(__file__).resolve()), "--worker", kind], capture_output=True,
                               text=True, timeout=timeout, env=env, cwd=cfg)
            out, why, crashed = p.stdout, (p.stderr.strip().splitlines() or [f"exit {p.returncode}"])[-1], p.returncode != 0
            if crashed:
                why = f"exit {p.returncode}" if p.returncode < 0 else scrub(why)
        except subprocess.TimeoutExpired as e:
            out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
            why, crashed = "timeout", True
        import shutil
        shutil.rmtree(cfg, ignore_errors=True)
        done = [json.loads(line) for line in out.splitlines() if line.startswith("{")]
        if done and not crashed:
            d = done[-1]
            if crashes:
                d["crashes"] = crashes
            return d
        ops = [line[4:] for line in out.splitlines() if line.startswith("#op ")]
        last = ops[-1] if ops else None
        if last is None or last in skip or len(skip) >= MAX_CRASHES:
            return {"kind": kind, "crash": scrub(why), "crashes": crashes}
        crashes.append({"op": last, "why": scrub(why)})
        skip.append(last)


def summarize(doc: dict) -> list[str]:
    import collections
    c = collections.Counter()
    for k in doc["kinds"].values():
        c["kind:crash" if k.get("crash") else "kind:error" if k.get("error") else "kind:ok"] += 1
        c["op:crash"] += len(k.get("crashes", []))
        for p in k.get("props", {}).values():
            c[f"prop:{p.get('outcome')}"] += 1
        for m in k.get("methods", {}).values():
            c[f"method:{m['class']}:{m.get('outcome', '-')}"] += 1
        for s in k.get("slots", {}).values():
            c[f"slot:{s.get('role')}"] += 1
        c["signal:own"] += len(k.get("signals", {}))
    return [f"  {key:44} {v}" for key, v in sorted(c.items())]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--worker")
    ap.add_argument("--kinds")
    ap.add_argument("--out", help="default: metadata/probe/qt/<qt version>.json")
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--section", default="props", choices=["props", "observe"],
                    help="props: the PySide6 probe (default); observe: the observability probe (C++ against a Qt SDK)")
    ap.add_argument("--qtdir", help="observe: the Qt SDK to build and run against (<qt>/gcc_64)")
    a = ap.parse_args()
    if a.section == "observe":
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import observe_probe
        return observe_probe.main(a)
    if a.worker:
        worker(a.worker)
        return 0
    from PySide6 import QtCore
    kinds = a.kinds.split(",") if a.kinds else kinds_from_metadata()
    with cf.ThreadPoolExecutor(a.j) as ex:
        results = list(ex.map(lambda k: run_kind(k, a.timeout), kinds))
    results.sort(key=lambda d: d["kind"])
    version = QtCore.qVersion()
    doc = {"schema": SCHEMA, "toolkit": "qt", "version": version, "kinds": {d["kind"]: d for d in results}}
    out = Path(a.out) if a.out else OUT_DIR / f"{version}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, indent=1, sort_keys=True, ensure_ascii=False) + "\n")
    print(f"probed {len(results)} Qt {version} classes -> {out}")
    print("\n".join(summarize(doc)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
