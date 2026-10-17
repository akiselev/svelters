#!/usr/bin/env python3
"""Type-category inventory of the Qt metadata (metadata/qt/*.json) for the serdes note.

The checked-in metadata maps most non-scalar C++ types to ref "opaque", so this script
re-classifies every property, method/constructor parameter, return and signal argument
from its native C++ spelling. Enum/flags/QObject facts come from the qdoc .index files
(qtcore, qtgui, qtwidgets). Stdlib only.

Usage: python3 -I serdes_qt_inventory.py [--meta metadata/qt] [--docdir /usr/share/doc/qt6]
"""
import argparse
import collections
import glob
import json
import os
import re
import xml.etree.ElementTree as ET

VALUE_STRUCTS = {"QRect", "QRectF", "QSize", "QSizeF", "QPoint", "QPointF", "QMargins", "QMarginsF",
                 "QLine", "QLineF", "QColor", "QVector2D", "QVector3D", "QVector4D"}
RICH_VALUES = {"QFont", "QPalette", "QSizePolicy", "QKeySequence", "QBrush", "QPen", "QCursor",
               "QRegion", "QTransform", "QLocale", "QUrl", "QTextCharFormat", "QTextFormat",
               "QTextBlockFormat", "QTextOption", "QFontMetrics", "QFontInfo", "QPainterPath",
               "QStyleOption", "QPolygon", "QPolygonF", "QGradient", "QTextCursor", "QTextBlock",
               "QTextDocumentFragment", "QItemSelection", "QItemSelectionRange", "QFileInfo",
               "QDir", "QRegularExpression", "QUuid", "QTextLength", "QMatrix4x4", "QTextFrameFormat"}
DATES = {"QDate", "QTime", "QDateTime", "QTimeZone", "QDeadlineTimer"}
IMAGES = {"QIcon", "QPixmap", "QImage", "QBitmap", "QPicture", "QMovie"}
BYTES = {"QByteArray", "QByteArrayView"}
STRINGS = {"QString", "QStringView", "QLatin1String", "QLatin1StringView", "QAnyStringView", "QChar",
           "QUtf8StringView"}
VARIANTS = {"QVariant", "QVariantList", "QVariantMap", "QVariantHash", "QJsonValue", "QJsonObject",
            "QJsonDocument"}
MODEL_INDEX = {"QModelIndex", "QPersistentModelIndex"}
INTS = {"int", "uint", "unsigned int", "short", "ushort", "unsigned short", "long", "ulong", "char",
        "uchar", "unsigned char", "qsizetype", "size_t", "quint8", "qint8", "quint16", "qint16",
        "quint32", "qint32", "char16_t", "char32_t", "QRgb", "WId", "qintptr", "quintptr"}
I64 = {"qint64", "quint64", "qlonglong", "qulonglong", "long long", "unsigned long long"}
FLOATS = {"qreal", "double", "float"}


def load_qdoc(docdir):
    enums, flags, bases = set(), set(), {}
    for mod in ("qtcore", "qtgui", "qtwidgets"):
        path = f"{docdir}/{mod}/{mod}.index"
        if not os.path.exists(path):
            continue
        for _, el in ET.iterparse(path):
            if el.tag == "enum":
                full = el.get("fullname") or el.get("name")
                enums.add(full)
                if el.get("typedef"):
                    flags.add(el.get("typedef"))
            elif el.tag == "typedef" and el.get("enum"):
                flags.add(el.get("fullname"))
            elif el.tag == "class" and el.get("name"):
                b = el.get("bases") or ""
                bases[el.get("name")] = [x.strip() for x in b.split(",") if x.strip()]
    qobj = set()

    def is_qobj(c, depth=0):
        if c == "QObject":
            return True
        if depth > 20:
            return False
        return any(is_qobj(b, depth + 1) for b in bases.get(c, []))
    for c in bases:
        if is_qobj(c):
            qobj.add(c)
    qobj.add("QObject")
    return enums, flags, qobj


def norm(native):
    s = native.replace("const ", "").replace("&&", "").strip()
    return re.sub(r"\s+", " ", s)


def classify(native, owner, enums, flags, qobj):
    """Return (category, mods) for one C++ type spelling."""
    raw = native or ""
    mods = []
    s = norm(raw)
    ref = s.endswith("&")
    ptr = s.endswith("*")
    base = s.rstrip("&* ").strip()
    if "(*" in raw or "std::function" in raw or "Functor" in raw or "PointerToMemberFunction" in raw:
        return "callback", mods
    if base == "void" and not ptr:
        return "void", mods
    if "const" not in raw and (ref or (ptr and base in INTS | I64 | FLOATS | {"bool"} | STRINGS)):
        if ref or base not in qobj:
            mods.append("out")
    if base.startswith("QFlags<") or base in flags or f"{owner}::{base}" in flags:
        return "flags", mods
    if base in enums or f"{owner}::{base}" in enums or base.startswith("Qt::") and base in enums:
        return "enum", mods
    if base == "bool":
        return "bool", mods
    if base in INTS:
        return "i32", mods
    if base in I64:
        return "i64", mods
    if base in FLOATS:
        return "f64", mods
    if base in STRINGS or (base == "char" and ptr):
        return "string", mods
    if base == "QStringList" or base == "QList<QString>":
        return "strv", mods
    if base in VARIANTS:
        return "variant", mods
    if base in BYTES:
        return "bytes", mods
    if base in DATES:
        return "date", mods
    if base in IMAGES:
        return "image_value", mods
    if base in VALUE_STRUCTS:
        return "value_struct", mods
    if base in RICH_VALUES:
        return "rich_value", mods
    if base in MODEL_INDEX:
        return "model_index", mods
    if base.startswith(("QList<", "QVector<", "QSet<", "std::initializer_list<")):
        return "list", mods
    if base.startswith(("QMap<", "QHash<", "QMultiMap<")):
        return "map", mods
    if base in ("QModelIndexList",):
        return "list", mods
    if ptr and base in qobj:
        mods.append("nullable")
        return "object", [m for m in mods if m != "out"]
    if ptr and base in ("void",):
        return "raw_pointer", mods
    if ptr:
        return "nonqobject_pointer", mods  # QLayoutItem*, QGraphicsItem*, QPainter*, QEvent*, ...
    if base in qobj:
        return "object", mods
    return "other_value", mods


SIMPLE = {"void", "bool", "i32", "i64", "f64", "string", "enum", "flags", "object"}


def rows_for(d, enums, flags, qobj):
    owner = d["native"]["name"]
    for p in d.get("properties", []):
        c, m = classify(p["type"].get("native"), owner, enums, flags, qobj)
        yield ("property", owner, p["name"], c, p["type"].get("native"), [x for x in m if x != "out"])
    for sec, site in (("methods", "method"), ("constructors", "method"), ("signals", "signal")):
        for me in d.get(sec, []):
            if me.get("access", "public") != "public" or me.get("deprecated"):
                continue
            name = me.get("name") or me.get("native")
            key = (me.get("signature") or me.get("native") or name)
            any_row = False
            for p in me.get("params", []):
                c, m = classify(p["type"].get("native"), owner, enums, flags, qobj)
                any_row = True
                yield (site, owner, key, c, p["type"].get("native"), m)
            r = me.get("returns") or {}
            if sec == "methods" and r.get("native"):
                c, m = classify(r.get("native"), owner, enums, flags, qobj)
                yield (site, owner, key, c, r.get("native"), [x for x in m if x != "out"])
            elif not any_row:
                yield (site, owner, key, "void", None, [])


def summarize(rows, label):
    by_site = collections.defaultdict(collections.Counter)
    members = collections.defaultdict(set)
    for site, owner, mem, c, _, mods in rows:
        tag = c + ("(out)" if "out" in mods else "")
        members[(site, owner, mem)].add(tag)
        if c != "void":
            by_site[site][tag] += 1
    tot, simple, blockers = collections.Counter(), collections.Counter(), collections.Counter()
    for (site, _, _), cats in members.items():
        tot[site] += 1
        if cats <= SIMPLE:
            simple[site] += 1
        else:
            for x in cats - SIMPLE:
                blockers[x] += 1
    return {"label": label, "by_site": {k: dict(v.most_common()) for k, v in by_site.items()},
            "members_total": dict(tot), "members_simple": dict(simple),
            "blockers": dict(blockers.most_common(25))}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--meta", default="metadata/qt")
    ap.add_argument("--docdir", default="/usr/share/doc/qt6")
    ap.add_argument("--json")
    a = ap.parse_args()
    enums, flags, qobj = load_qdoc(a.docdir)
    docs = [json.load(open(f)) for f in sorted(glob.glob(f"{a.meta}/*.json"))]
    p_set = {"QWidget", "QLabel", "QLineEdit", "QMainWindow", "QMenuBar", "QPushButton", "QScrollArea",
             "QStatusBar", "QAbstractButton", "QFrame", "QAbstractScrollArea"}
    common = p_set | {"QCheckBox", "QRadioButton", "QComboBox", "QSpinBox", "QDoubleSpinBox",
                      "QAbstractSpinBox", "QSlider", "QAbstractSlider", "QProgressBar", "QTextEdit",
                      "QPlainTextEdit", "QListWidget", "QListView", "QTreeView", "QTreeWidget",
                      "QTableView", "QAbstractItemView", "QTabWidget", "QStackedWidget", "QSplitter",
                      "QGroupBox", "QToolButton", "QMenu", "QToolBar", "QDialog", "QDialogButtonBox",
                      "QBoxLayout", "QHBoxLayout", "QVBoxLayout", "QGridLayout", "QFormLayout",
                      "QLayout", "QDockWidget", "QFileDialog", "QMessageBox", "QDateEdit",
                      "QDateTimeEdit", "QCalendarWidget", "QScrollBar", "QApplication"}
    out = []
    for label, sel in (("P kinds (+bases)", p_set), ("common (~50 kinds)", common), ("all metadata/qt", None)):
        rows = [r for d in docs if sel is None or d["native"]["name"] in sel for r in rows_for(d, enums, flags, qobj)]
        out.append(summarize(rows, label))
    det = collections.Counter()
    for d in docs:
        for site, _, _, c, nat, _ in rows_for(d, enums, flags, qobj):
            if c not in SIMPLE:
                det[f"{c}: {norm(nat or '').rstrip('&* ')}"] += 1
    out.append({"label": "detail (all)", "top": dict(det.most_common(70))})
    print(json.dumps(out, indent=1))
    if a.json:
        json.dump(out, open(a.json, "w"), indent=1)


if __name__ == "__main__":
    main()
