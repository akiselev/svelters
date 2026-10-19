#!/usr/bin/env python3
"""Ingest the Qt non-GUI type surface (plans/research/NON-GUI-TYPES.md section 3.2, N2) into raw,
origin-tagged facts: `metadata/qt/types/ingested/<Module>.json`, one record per public class of QtCore,
QtGui and QtWidgets (widgets keep only their name and bases: they are elements, not types).

Sources (none of them is installed by default; every path is an option):
  qdoc indexes    <docdir>/{qtcore,qtgui,qtwidgets}/*.index      classes, bases, functions (const, static,
                                                                   params, defaults, since), properties, enums
  PySide6         <typesystems>/typesystem_{core,gui,widgets}_common.xml
                                                                   value-type / object-type / primitive-type,
                                                                   removed members, ownership modifications
  moc metatypes   <metatypes>/qt6{core,gui,widgets}_metatypes.json Q_OBJECT / Q_GADGET, NOTIFY signals
  headers         <include>/Qt{Core,Gui,Widgets}/*.h               `enum class` (scoped enums)

PyQt6's sip files are not ingested: they are GPL, and the one rule taken from them (a non-const `T *` of
a basic or value type is an out parameter) is applied by the classifier as a rule. Nested public-field
structs (QFont::Tag, QTextOption::Tab) are not in qdoc; they stay blocked (no libclang pass yet).

Deterministic: the output depends only on the inputs (sorted keys, sorted records). The classification
(`types_qt_classify.py`) reads only these files and the overrides, so it runs without any Qt install.

  python3 tools/metadata/qt/ingest_types.py --docdir <Docs/Qt-6.11.0> --typesystems <PySide6/typesystems> \
      --metatypes <qt/6.11.3/gcc_64/metatypes> --include <qt/6.11.3/gcc_64/include>

How to get the sources (docs/notes/nt2-qt-types.md): `pip install aqtinstall`, then
`aqt install-doc linux 6.11.0 -O <dir>` and `aqt install-qt linux desktop 6.11.3 linux_gcc_64 -O <dir> --archives qtbase icu`;
PySide6 6.11.0 from pip (the same venv the AI1 probe uses).
"""
from __future__ import annotations

import argparse
import glob
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "metadata/qt/types/ingested"
MODS = [("qtcore", "QtCore"), ("qtgui", "QtGui"), ("qtwidgets", "QtWidgets")]
CACHE = Path.home() / ".cache/claude-svelters/nt2"
DEFAULTS = {
    "docdir": CACHE / "qtdoc/Docs/Qt-6.11.0",
    "typesystems": Path.home() / ".cache/claude-svelters/venv-qt/lib/python3.12/site-packages/PySide6/typesystems",
    "metatypes": CACHE / "qt/6.11.3/gcc_64/metatypes",
    "include": CACHE / "qt/6.11.3/gcc_64/include",
}
SKIP_META = {"destructor", "copy-assign", "move-assign", "copy-constructor", "move-constructor", "macrowithparams",
             "macrowithoutparams"}


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def unesc(s: str | None) -> str | None:
    return None if s is None else s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")


def norm_sig(sig: str) -> str:
    """PySide signature spelling without names/spaces: `appendRow(QStandardItem*)`."""
    return re.sub(r"\s+", "", sig)


def load_typesystem(paths: list[Path]) -> dict:
    out: dict = {}
    for p in paths:
        root = ET.parse(p).getroot()
        for el in root.iter():
            if el.tag not in ("value-type", "object-type", "primitive-type", "container-type", "namespace-type",
                              "interface-type"):
                continue
            n = el.get("name")
            if not n or n in out:
                continue
            rec = {"kind": el.tag, "removed": [], "ownership": [], "file": p.name}
            for mf in el.findall("modify-function"):
                sig = norm_sig(unesc(mf.get("signature")) or "")
                if mf.get("remove"):
                    rec["removed"].append(sig)
                for arg in mf.findall("modify-argument"):
                    idx = arg.get("index")
                    for par in arg.findall("parent"):
                        rec["ownership"].append({"sig": sig, "arg": idx, "fact": "parent", "index": par.get("index"),
                                                 "action": par.get("action")})
                    for own in arg.findall("define-ownership"):
                        rec["ownership"].append({"sig": sig, "arg": idx, "fact": "ownership", "owner": own.get("owner"),
                                                 "class": own.get("class") or "target"})
            rec["removed"].sort()
            rec["ownership"].sort(key=lambda o: json.dumps(o, sort_keys=True))
            out[n] = rec
        for rj in root.iter("rejection"):
            c = rj.get("class")
            if c and not rj.get("function-name") and not rj.get("field-name") and c not in out:
                out[c] = {"kind": "rejected", "removed": [], "ownership": [], "file": p.name}
    return out


def load_moc(d: Path) -> dict:
    out: dict = {}
    for mod, _ in MODS:
        p = d / f"qt6{mod[2:]}_metatypes.json"
        for e in json.loads(p.read_text()):
            for c in e.get("classes", []):
                n = c["qualifiedClassName"]
                props = {}
                for pr in c.get("properties", []):
                    props[pr["name"]] = {k: pr.get(k) for k in ("type", "read", "write", "notify", "constant", "member")
                                         if pr.get(k) not in (None, False)}
                out[n] = {"object": bool(c.get("object")), "gadget": bool(c.get("gadget")), "properties": props,
                          "enums": sorted(x["name"] for x in c.get("enums", [])),
                          "scoped_enums": sorted(x["name"] for x in c.get("enums", []) if x.get("isClass")),
                          "invokables": sorted({m["name"] for k in ("slots", "methods") for m in c.get(k, [])})}
    return out


def scoped_enums(include: Path) -> set[str]:
    """`Owner::Name` of every `enum class` in the public headers (best effort: the owner is the innermost class
    or namespace the regex sees; good enough for qdoc's owner-qualified enum names)."""
    out: set[str] = set()
    for mod in ("QtCore", "QtGui", "QtWidgets"):
        for h in sorted(glob.glob(str(include / mod / "q*.h"))):
            text = Path(h).read_text(errors="replace")
            owners = [(m.start(), m.group(2)) for m in re.finditer(r"^\s*(class|struct|namespace)\s+(?:Q_\w+\s+)?(\w+)\b[^;]*$", text, re.M)]
            for m in re.finditer(r"\benum\s+(?:class|struct)\s+(\w+)", text):
                owner = None
                for pos, name in owners:
                    if pos < m.start():
                        owner = name
                out.add(f"{owner}::{m.group(1)}" if owner else m.group(1))
                out.add(m.group(1))
    return out


def param(p: ET.Element) -> dict:
    d = {"name": p.get("name") or "", "type": unesc(p.get("type")) or ""}
    if p.get("default"):
        d["default"] = unesc(p.get("default"))
    return d


def ingest_class(c: ET.Element, module: str) -> dict:
    name = c.get("fullname") or c.get("name")
    rec: dict = {"name": name, "module": module, "header": c.get("location"),
                 "bases": [b.strip().split("<")[0] for b in (unesc(c.get("bases")) or "").split(",") if b.strip()],
                 "status": c.get("status"), "since": c.get("since"), "functions": [], "properties": [], "enums": []}
    # qdoc `brief` texts are GFDL documentation: never copied (docs/notes/docs-generator.md, Qt brief policy).
    for f in c.findall("function"):
        meta = f.get("meta")
        if meta in SKIP_META or f.get("access") != "public" or f.get("related"):
            continue
        fr = {"name": f.get("name"), "meta": meta, "signature": unesc(f.get("signature")),
              "type": unesc(f.get("type")) or "", "params": [param(p) for p in f.findall("parameter")]}
        for k in ("const", "static"):
            if f.get(k) == "true":
                fr[k] = True
        for k in ("virtual", "since", "associated-property", "overload-number"):
            if f.get(k):
                fr[k.replace("-", "_")] = f.get(k)
        if f.get("status") != "active":
            fr["status"] = f.get("status")
        rec["functions"].append(fr)
    for p in c.findall("property"):
        pr = {"name": p.get("name"), "type": unesc(p.get("dataType")) or "", "writable": p.get("writable") != "false"}
        if p.get("since"):
            pr["since"] = p.get("since")
        if p.get("status") != "active":
            pr["status"] = p.get("status")
        rec["properties"].append(pr)
    for e in c.findall("enum"):
        er = {"name": e.get("name"), "fullname": e.get("fullname") or f"{name}::{e.get('name')}",
              "members": [{"name": v.get("name"), "value": v.get("value")} for v in e.findall("value")]}
        if e.get("typedef"):
            er["flags"] = e.get("typedef")
        if e.get("status") != "active":
            er["status"] = e.get("status")
        rec["enums"].append(er)
    for t in c.findall("typedef"):
        if t.get("enum"):
            rec.setdefault("flag_typedefs", []).append({"name": t.get("fullname") or t.get("name"), "enum": t.get("enum")})
    rec["functions"].sort(key=lambda f: (f["name"], f["signature"] or ""))
    rec["properties"].sort(key=lambda p: p["name"])
    rec["enums"].sort(key=lambda e: e["fullname"])
    if "flag_typedefs" in rec:
        rec["flag_typedefs"].sort(key=lambda t: t["name"])
    return rec


def chain_of(name: str, classes: dict, all_classes: dict) -> list[str]:
    out: list[str] = []
    todo = list((classes.get(name) or all_classes.get(name) or {}).get("bases", []))
    while todo:
        b = todo.pop(0)
        if b in out:
            continue
        out.append(b)
        todo += (classes.get(b) or all_classes.get(b) or {}).get("bases", [])
    return out


def build(a) -> dict[str, dict]:
    docdir, tsdir, mocdir, inc = Path(a.docdir), Path(a.typesystems), Path(a.metatypes), Path(a.include)
    ts_files = [tsdir / f"typesystem_{m}_common.xml" for m in ("core", "gui", "widgets")]
    ts = load_typesystem(ts_files)
    moc = load_moc(mocdir)
    scoped = scoped_enums(inc)
    files: dict[str, dict] = {}
    seen: set[str] = set()
    sources = {"qdoc": {}, "pyside6": {p.name: sha(p) for p in ts_files},
               "moc": {f"qt6{m[2:]}_metatypes.json": sha(mocdir / f"qt6{m[2:]}_metatypes.json") for m, _ in MODS},
               "qt_headers": str(inc.parent.parent.name)}
    namespaces: dict[str, dict] = {}
    all_classes: dict[str, dict] = {}
    for mod, Mod in MODS:
        p = docdir / mod / f"{mod}.index"
        root = ET.parse(p).getroot()
        sources["qdoc"][f"{mod}.index"] = sha(p)
        sources["qdoc_version"] = root.get("version")
        classes = {}
        for c in root.iter("class"):
            n = c.get("fullname") or c.get("name")
            if c.get("access") != "public" or n in seen or "<" in n:
                continue
            seen.add(n)
            rec = ingest_class(c, Mod)
            classes[n] = rec
            t = ts.get(n) or ts.get(n.split("::")[-1] if "::" in n else "")
            if t:
                rec["pyside"] = t
            m = moc.get(n)
            if m:
                rec["moc"] = m
        all_classes.update(classes)
        # Widgets are elements (metadata/qt/*.json): only their place in the hierarchy is kept here.
        for n, rec in classes.items():
            if n == "QWidget" or "QWidget" in chain_of(n, classes, all_classes):
                for k in ("functions", "properties", "enums", "flag_typedefs", "moc", "pyside"):
                    rec.pop(k, None)
                rec["widget"] = True
        for ns in root.iter("namespace"):
            if ns.get("name") == "Qt" and "Qt" not in namespaces:
                namespaces["Qt"] = {"name": "Qt", "module": "QtCore", "enums": ingest_class(ns, "QtCore")["enums"],
                                    "flag_typedefs": ingest_class(ns, "QtCore").get("flag_typedefs", [])}
        files[Mod] = {"schema": "svelters-qt-types-ingest/v1", "module": Mod, "classes": classes}
    files["QtCore"]["namespaces"] = namespaces
    for f in files.values():
        f["sources"] = sources
    files["QtCore"]["scoped_enums"] = sorted(scoped)
    return files


def serialise(d: dict) -> str:
    return json.dumps(d, indent=0, sort_keys=True, ensure_ascii=False, separators=(",", ":")) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    for k, v in DEFAULTS.items():
        ap.add_argument(f"--{k}", default=str(v))
    ap.add_argument("--out", default=str(OUT))
    a = ap.parse_args()
    files = build(a)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for Mod, d in files.items():
        (out / f"{Mod}.json").write_text(serialise(d))
    n = sum(len(d["classes"]) for d in files.values())
    print(f"ingest_types: {n} classes in {len(files)} modules -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
