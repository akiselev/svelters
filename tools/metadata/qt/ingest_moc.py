#!/usr/bin/env python3
"""Ingest the Q_PROPERTY facts of moc's metatypes JSON (milestone R0: observability).

The PySide6 metaobject dump behind metadata/qt/*.json has NOTIFY and CONSTANT but not BINDABLE, and the non-GUI
type facts (metadata/qt/types) keep only NOTIFY. moc writes every Q_PROPERTY attribute into
<qt>/metatypes/qt6{core,gui,widgets}_metatypes.json; this extract keeps what observability needs, per class and
property: notify, bindable, constant, final, user, read, write, reset, stored, type, plus the superclass chain.

    python3 tools/metadata/qt/ingest_moc.py [--metatypes <qt>/gcc_64/metatypes] [--check]

Output: metadata/qt/moc/properties.json (sorted, sha256 of the inputs recorded). Deterministic: a pure function of
the three metatypes files. Default input: the Qt 6.11.3 install of docs/notes/nt2-qt-types.md (aqt), the Qt the CI
job and the type facts use.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "metadata/qt/moc/properties.json"
MODS = [("qtcore", "QtCore"), ("qtgui", "QtGui"), ("qtwidgets", "QtWidgets")]
DEFAULT = Path.home() / ".cache/claude-svelters/nt2/qt/6.11.3/gcc_64/metatypes"
KEEP = ("type", "read", "write", "reset", "notify", "bindable", "constant", "final", "user", "stored", "member")


def build(d: Path) -> dict:
    classes: dict[str, dict] = {}
    files = {}
    for mod, name in MODS:
        p = d / f"qt6{mod[2:]}_metatypes.json"
        files[p.name] = hashlib.sha256(p.read_bytes()).hexdigest()
        for e in json.loads(p.read_text()):
            for c in e.get("classes", []):
                n = c["qualifiedClassName"]
                if "::" in n or not (c.get("object") or c.get("gadget")):
                    continue
                props = {}
                for pr in c.get("properties", []):
                    props[pr["name"]] = {k: pr[k] for k in KEEP if pr.get(k) not in (None, False, "")}
                supers = [s["name"] for s in c.get("superClasses", []) if s.get("access") == "public"]
                classes[n] = {"module": name, "object": bool(c.get("object")), "super": supers, "properties": props}
    return {"schema": "svelters-qt-moc-properties/v1", "toolkit": "qt", "sources": {"metatypes": files},
            "classes": dict(sorted(classes.items()))}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--metatypes", default=str(DEFAULT))
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    text = json.dumps(build(Path(a.metatypes)), indent=1, sort_keys=True) + "\n"
    if a.check:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"ingest_moc: {OUT.relative_to(ROOT)} is stale", file=sys.stderr)
            return 1
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text)
    doc = json.loads(text)
    n = sum(len(c["properties"]) for c in doc["classes"].values())
    b = sum(1 for c in doc["classes"].values() for p in c["properties"].values() if p.get("bindable"))
    print(f"ingest_moc: {len(doc['classes'])} classes, {n} properties ({b} BINDABLE) -> {OUT.relative_to(ROOT)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
