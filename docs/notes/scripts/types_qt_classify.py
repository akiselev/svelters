#!/usr/bin/env python3
"""Prototype auto-classifier: QtCore/QtGui (+ QtWidgets non-widgets) types -> the svelters taxonomy.

Sources, all machine-readable and installed by distro packages (no hand lists except RECORD_SHAPES):
  qdoc indexes   /usr/share/doc/qt6/<mod>/<mod>.index   classes, bases, functions, properties, enums
  PySide6        /usr/share/PySide6/typesystems/*.xml    value-type / object-type / primitive-type /
                                                         container-type, rejections, removed members,
                                                         ownership modifications
  moc metatypes  /usr/lib/qt6/metatypes/qt6<mod>_metatypes.json   Q_OBJECT / Q_GADGET classes
  PyQt6 sip      site-packages/PyQt6/bindings/<Mod>/*.sip  cross-check: classes PyQt binds

Rules (plans/research/NON-GUI-TYPES.md section 3.1):
  QWidget-derived                          -> widget (elements; out of scope here)
  QAbstractItemModel-derived               -> model
  QObject-derived (qdoc bases / moc)       -> object
  QFuture / QPromise                       -> async
  PySide value-type, record shape known    -> record
  PySide value-type                        -> value
  PySide object-type, not a QObject:
     QEvent-derived / paint / style option -> transient (callback-only, stays native)
     item classes (Q*Item, owned by a view)-> needs-override
     other                                 -> needs-override
  PySide primitive-type / container-type   -> primitive / container
  in qdoc but in no typesystem             -> unclassified (needs a decision)
Members: public, non-deprecated qdoc functions of bindable classes; each parameter/return spelling
is resolved against the classification. Removed-in-PySide members are counted as excluded.

Usage: python3 -I types_qt_classify.py [--show CATEGORY]
"""
import argparse
import collections
import glob
import json
import os
import re
import xml.etree.ElementTree as ET

DOC = "/usr/share/doc/qt6"
TS = "/usr/share/PySide6/typesystems"
MOC = "/usr/lib/qt6/metatypes"
SIP = "/usr/lib/python3.14/site-packages/PyQt6/bindings"
MODS = [("qtcore", "QtCore", "core"), ("qtgui", "QtGui", "gui"), ("qtwidgets", "QtWidgets", "widgets")]
# JS shape decision (portable field names), not a classification fact.
RECORD_SHAPES = {"QPoint", "QPointF", "QSize", "QSizeF", "QRect", "QRectF", "QMargins", "QMarginsF", "QColor",
                 "QLine", "QLineF", "QVector2D", "QVector3D", "QVector4D", "QDate", "QTime", "QDateTime"}
PRIM = {"void", "bool", "int", "uint", "qint8", "quint8", "qint16", "quint16", "qint32", "quint32", "qint64",
        "quint64", "qlonglong", "qulonglong", "long", "ulong", "short", "ushort", "char", "uchar", "float",
        "double", "qreal", "qsizetype", "size_t", "QString", "QStringList", "QByteArray", "QVariant",
        "QVariantMap", "QVariantList", "QVariantHash", "QStringView", "QAnyStringView", "QLatin1StringView",
        "QUtf8StringView", "QChar", "QByteArrayView", "QJsonValue", "QJsonObject", "QJsonArray", "QJsonDocument",
        "unsigned int", "unsigned char", "char16_t", "char32_t", "QModelIndexList", "QRgb", "QRgba64", "unsigned long",
        "QFileInfoList", "QObjectList", "std::chrono::milliseconds", "std::chrono::nanoseconds", "Duration",
        "std::chrono::seconds", "QDeadlineTimer", "qsizetype", "Qt::HANDLE", "WId"}
ALIASES = {"std::filesystem::path": "QString", "QLocale::Territory": "int", "std::byte": "uchar", "quint128": "qint64"}
TEMPLATE = {"T", "auto", "Callable", "Predicate", "Args ...", "Args...", "InputIterator", "Container", "Key", "U"}
PLATFORM = re.compile(r"^(emscripten|QWinEventNotifier::HANDLE|CF|CG|NS|UI|HWND|HICON|HBITMAP|HRGN|HMONITOR|Vk|IUnknown|jobject|JNIEnv|wl_|xcb_|Display|EGL|GL)")
CONTAINERS = ("QList<", "QVector<", "QSet<", "QMap<", "QHash<", "std::optional<", "QSpan<")
PAINT = {"QPainter", "QPaintDevice", "QPaintEngine", "QPaintEngineState", "QBackingStore", "QOpenGLContext",
         "QStyleOption", "QStylePainter", "QRhi"}


def load_qdoc():
    classes, enums, funcs, props = {}, set(), {}, {}
    for mod, Mod, _ in MODS:
        root = ET.parse(f"{DOC}/{mod}/{mod}.index").getroot()
        for c in root.iter("class"):
            if c.get("access") != "public" or c.get("status") == "deprecated":
                continue
            name = c.get("fullname") or c.get("name")
            if name in classes:
                continue
            classes[name] = (Mod, [b.strip().split("<")[0] for b in (c.get("bases") or "").split(",") if b.strip()])
            funcs[name] = [f for f in c.findall("function") if f.get("access") == "public"]
            props[name] = c.findall("property")
        for e in root.iter("enum"):
            enums.add(e.get("fullname"))
            enums.add(e.get("name"))
            if e.get("typedef"):
                enums.add(e.get("typedef"))
                enums.add(e.get("typedef").split("::")[-1])
        for t in root.iter("typedef"):
            if t.get("enum"):
                enums.add(t.get("fullname"))
                enums.add(t.get("name"))
    return classes, enums, funcs, props


def chain(name, classes, seen=()):
    out = []
    for b in classes.get(name, (None, []))[1]:
        if b in seen:
            continue
        out += [b] + chain(b, classes, seen + (b,))
    return out


def load_typesystem():
    kinds, removed, rejected, ownership = {}, collections.defaultdict(set), set(), collections.Counter()
    for f in glob.glob(f"{TS}/typesystem_*.xml"):
        try:
            root = ET.parse(f).getroot()
        except ET.ParseError:
            continue  # fragment files (glGet* includes) are not standalone documents
        for el in root.iter():
            tag = el.tag
            n = el.get("name")
            if tag in ("value-type", "object-type", "primitive-type", "container-type", "namespace-type", "interface-type"):
                if n:
                    kinds.setdefault(n, tag)
                    for mf in el.findall("modify-function"):
                        if mf.get("remove"):
                            removed[n].add(mf.get("signature", "").split("(")[0])
                        if mf.find(".//parent") is not None or mf.find(".//define-ownership") is not None:
                            ownership[tag] += 1
            elif tag == "rejection" and el.get("class") and not el.get("function-name") and not el.get("field-name"):
                rejected.add(el.get("class"))
    return kinds, removed, rejected, ownership


def load_moc():
    objs, gadgets = set(), set()
    for f in glob.glob(f"{MOC}/qt6*_metatypes.json"):
        for e in json.load(open(f)):
            for c in e.get("classes", []):
                (objs if c.get("object") else gadgets if c.get("gadget") else set()).add(c["qualifiedClassName"])
    return objs, gadgets


def load_sip():
    names = set()
    for f in glob.glob(f"{SIP}/Qt*/*.sip"):
        for m in re.finditer(r"^\s*class\s+(\w+)", open(f, errors="replace").read(), re.M):
            names.add(m.group(1))
    return names


# C++ machinery and platform plumbing: excluded by rule, never bound (templates, smart pointers,
# iterators, atomics, locks, property-system internals, JNI/Android/Mac/Windows/Vulkan/RHI/OpenGL,
# accessibility interfaces, logging/debug streams).
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
                       r"^QRangeModelAdapter|Converter$|^QStringDecoder|^QStringEncoder")


def statics_only(name, funcs):
    fs = [f for f in funcs.get(name, []) if f.get("meta") in ("plain", "constructor", "slot", "signal")]
    return bool(fs) and all(f.get("static") == "true" for f in fs if f.get("meta") != "constructor") and not any(
        f.get("meta") == "constructor" for f in fs)


def classify(name, classes, ts, moc_objs, funcs=None):
    if MACHINERY.search(name):
        return "excluded"
    ch = chain(name, classes)
    if name == "QWidget" or "QWidget" in ch:
        return "widget"
    if name == "QAbstractItemModel" or "QAbstractItemModel" in ch:
        return "model"
    if name == "QObject" or "QObject" in ch or name in moc_objs:
        return "object"
    if name.startswith("QFuture") or name.startswith("QPromise"):
        return "async"
    k = ts.get(name)
    if k == "value-type":
        return "record" if name in RECORD_SHAPES else "value"
    if funcs is not None and k in ("object-type", None) and statics_only(name, funcs):
        return "statics"
    if k == "object-type":
        if name == "QEvent" or "QEvent" in ch or name in PAINT or any(b in PAINT for b in ch) or name.startswith("QStyleOption"):
            return "transient"
        # Items whose ownership moves into a container (QTreeWidget, QGraphicsScene, QLayout, QUndoStack):
        # the PySide typesystem's parent/define-ownership modifications on the containers say when.
        if name.endswith("Item") or name.endswith("Command") or any(b.endswith("Item") for b in ch):
            return "item"
        # A non-copyable, non-QObject C++ object with a public constructor: JS owns it (OWNED,
        # deleted on dispose or scope end): QXmlStreamReader, QCommandLineParser, QTemporaryDir.
        if funcs is not None and any(f.get("meta") == "constructor" for f in funcs.get(name, [])):
            return "unique"
        return "needs-override"
    if k in ("primitive-type", "container-type"):
        return "primitive"
    if k == "namespace-type":
        return "namespace"
    if name in RECORD_SHAPES:
        return "record"
    return "unclassified"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--show")
    a = ap.parse_args()
    classes, enums, funcs, props = load_qdoc()
    ts, removed, rejected, ownership = load_typesystem()
    moc_objs, gadgets = load_moc()
    sip = load_sip()
    cats = {n: classify(n, classes, ts, moc_objs, funcs) for n in classes if "::" not in n}
    order = ["record", "value", "object", "model", "statics", "unique", "item", "async", "transient", "needs-override",
             "unclassified", "excluded", "primitive", "widget"]
    per = collections.defaultdict(collections.Counter)
    for n, c in cats.items():
        per[classes[n][0]][c] += 1
    print("== classes by category (qdoc + PySide typesystem + moc metatypes)")
    print(f"{'module':10}" + "".join(f"{o[:8]:>9}" for o in order))
    tot = collections.Counter()
    for _, Mod, _ in MODS:
        print(f"{Mod:10}" + "".join(f"{per[Mod][o]:9}" for o in order))
        tot.update(per[Mod])
    print(f"{'total':10}" + "".join(f"{tot[o]:9}" for o in order))
    print(f"   typesystem entries: {collections.Counter(ts.values())}; ownership modifications: {dict(ownership)}; class rejections {len(rejected)}")
    print(f"   moc: {len(moc_objs)} Q_OBJECT, {len(gadgets)} Q_GADGET; PyQt6 sip binds {len(sip)} classes; "
          f"qdoc classes in neither typesystem nor sip: {sum(1 for n in cats if n not in ts and n not in sip)}")

    bindable = {"record", "value", "object", "model", "statics", "unique"}
    ok_cat = bindable | {"widget", "primitive", "async"}

    def type_ok(spelling, owner):
        s = re.sub(r"\b(const|volatile|typename|struct|class|enum)\b", "", spelling or "void").replace("&&", "").replace("&", "").strip()
        s = re.sub(r"\s+", " ", s)
        if s.endswith("*"):
            base = s.rstrip("*").strip()
            if base in ("char",):
                return None
            c = cats.get(base)
            if c in ("object", "model", "widget", "unique", "item"):
                return None
            if c in ("value", "record"):
                return "out-pointer"
            if base in PRIM or base in enums or f"{owner}::{base}" in enums:
                return "out-pointer"  # S2 out params: needs `out:` facts (PySide/sip carry them)
            return f"type:{c or 'unknown'}:{base}*"
        s = ALIASES.get(s, s)
        if s in TEMPLATE or re.search(r"::(const_)?(reverse_)?(key_value_)?iterator$", s):
            return "template"
        if PLATFORM.match(s):
            return "platform"
        if s in PRIM or s in enums or f"{owner}::{s}" in enums or s.split("::")[-1] in enums:
            return None
        if s.startswith("QFlags<") or s.startswith("Qt::"):
            return None
        for cprefix in CONTAINERS:
            if s.startswith(cprefix):
                inner = s[len(cprefix):-1]
                parts = [p.strip() for p in re.split(r",(?![^<]*>)", inner)]
                for p in parts:
                    r = type_ok(p, owner)
                    if r:
                        return r
                return None
        if "Functor" in s or s.startswith("std::function") or s.startswith("Func"):
            return None  # CALLBACK (S2 functor path)
        if s.startswith("QBindable<") or s.startswith("QProperty<"):
            return "bindable"
        c = cats.get(s) or cats.get(f"{owner}::{s}")
        if c in ok_cat:
            return None
        if c is None and s in ts:
            return None if ts[s] in ("primitive-type", "container-type") else f"type:{ts[s]}"
        return f"type:{c or 'unknown'}:{s}" if c is None else f"type:{c}"

    mem = collections.defaultdict(collections.Counter)
    reasons = collections.Counter()
    excluded = collections.Counter()
    blockers = collections.Counter()
    for n, c in cats.items():
        if c not in bindable:
            continue
        Mod = classes[n][0]
        for f in funcs.get(n, []):
            meta = f.get("meta")
            if meta in ("destructor", "copy-assign", "move-assign", "copy-constructor", "move-constructor",
                        "macrowithparams", "macrowithoutparams"):
                excluded["special members (core)"] += 1
                continue
            if f.get("status") == "deprecated":
                excluded["deprecated"] += 1
                continue
            if f.get("name") in removed.get(n, ()):
                excluded["removed by PySide"] += 1
                continue
            sig_types = [f.get("type")] + [p.get("type") for p in f.findall("parameter")]
            if any(t and (t.strip() in TEMPLATE or PLATFORM.match(re.sub(r"\bconst\b", "", t).strip())) for t in sig_types):
                excluded["templates / platform-native (CF*, HWND, Vk*)"] += 1
                continue
            if f.get("name", "").startswith("operator"):
                excluded["operators (equals via QMetaType)"] += 1
                continue
            r = None if meta == "constructor" else type_ok(f.get("type"), n)
            for p in f.findall("parameter"):
                r = r or type_ok(p.get("type"), n)
            mem[Mod]["auto" if r is None else "blocked"] += 1
            if r:
                reasons[r if not r.startswith("type:") else ":".join(r.split(":")[:2])] += 1
                if r.startswith("type:unknown:"):
                    blockers[r.split(":", 2)[2]] += 1
        for p in props.get(n, []):
            r = type_ok(p.get("dataType"), n)
            mem[Mod]["auto" if r is None else "blocked"] += 1
            mem[Mod]["props"] += 1
    print("\n== members of bindable classes (record/value/object/model)")
    allc = collections.Counter()
    for _, Mod, _ in MODS:
        m = mem[Mod]
        k = m["auto"] + m["blocked"]
        allc.update(m)
        if k:
            print(f"  {Mod:10} members {k:5}  auto {m['auto']:5} ({100 * m['auto'] // k:3}%)  blocked {m['blocked']:4}  (props {m['props']})")
    k = allc["auto"] + allc["blocked"]
    print(f"  {'all':10} members {k:5}  auto {allc['auto']:5} ({100 * allc['auto'] // k:3}%)  blocked {allc['blocked']:4}")
    print("  excluded automatically:", dict(excluded))
    print("  blocking reasons:", ", ".join(f"{r} {v}" for r, v in reasons.most_common(12)))
    print(f"  distinct unknown spellings: {len(blockers)}")
    print("  unknown spellings blocking most:", ", ".join(f"{r} {v}" for r, v in blockers.most_common(20)))

    if a.show:
        print(f"\n== {a.show}:")
        print("  " + " ".join(sorted(n for n, c in cats.items() if c == a.show)))


if __name__ == "__main__":
    main()
