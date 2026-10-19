"""The AppKit annotation layers, merged: ingested < inferred < manual.

AppKit ingest runs only on a Mac (it reads the SDK), so unlike GTK the layers are not merged into
metadata/appkit/*.json; every consumer (gen_backend.py and through it gen_semantics.py,
gen_bindings.py, probe.py) reads them through `load()`:

  metadata/inferred/appkit/<Class>.yaml   written by infer.py (same schema as the manual overrides)
  metadata/overrides/appkit/<Class>.yaml  manual annotations; a record replaces the inferred record of
                                          the same section and name whole; `deny: ["<section>:<glob>"]`
                                          removes inferred records
  metadata/inferred/appkit/_parked.json   inferred facts whose generated test failed on macOS
                                          (ledger.py import): left out while their record is unchanged

`element` is manual only. Each merged record's layer is in `origin[(class, section, name)]`:
("manual", None) or ("inferred", <record fingerprint>).
"""
from __future__ import annotations

import fnmatch
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "common"))
import svmeta  # noqa: E402

ROOT = HERE.parents[2]
OVERRIDES = ROOT / "metadata/overrides/appkit"
INFERRED = ROOT / "metadata/inferred/appkit"
PARKED = INFERRED / "_parked.json"
SCHEMA = HERE / "override.schema.json"

# record lists of an override file and the field that names a record
SECTIONS = {"props": "name", "setters": "name", "enums": "prop", "signals": "name", "slots": "name",
            "methods": "name", "bindings": "name"}


def parked_keys(path: Path = PARKED) -> set[tuple[str, str, str, str]]:
    """(class, section, name, record fingerprint) of the parked inferred facts."""
    if not path.exists():
        return set()
    return {(r["kind"], r["section"], r["name"], r.get("record_fp") or "") for r in json.loads(path.read_text())["parked"]}


def denied(patterns: list[str], section: str, name: str) -> bool:
    return any(fnmatch.fnmatchcase(f"{section}:{name}", p) for p in patterns)


def merge(manual: dict | None, inferred: dict | None, cls: str, parked: set, origin: dict) -> dict:
    """One class's merged annotation (see the module docs); fills `origin`."""
    manual = manual or {}
    inferred = inferred or {}
    deny = manual.get("deny", [])
    out: dict = {"class": cls}
    if manual.get("element"):
        out["element"] = manual["element"]
    for sec, key in SECTIONS.items():
        recs: dict[str, dict] = {}
        for r in inferred.get(sec, []):
            fp = svmeta.fingerprint(r)
            if denied(deny, sec, r[key]) or (cls, sec, r[key], fp) in parked:
                continue
            recs[r[key]] = r
            origin[(cls, sec, r[key])] = ("inferred", fp)
        for r in manual.get(sec, []):
            recs[r[key]] = r
            origin[(cls, sec, r[key])] = ("manual", None)
        if recs:
            # manual records keep their file order, inferred ones follow sorted by name
            names = [r[key] for r in manual.get(sec, [])]
            names += sorted(n for n in recs if n not in names)
            out[sec] = [recs[n] for n in names]
    return out


def load(validate: bool = True) -> tuple[dict[str, dict], dict, list[str]]:
    """(class -> merged annotation, origin table, schema errors of the manual and inferred files)."""
    errs: list[str] = []
    manual, inferred = {}, {}
    for p in sorted(OVERRIDES.glob("*.yaml")):
        if validate:
            errs += svmeta.check_override(p, SCHEMA)
        data = svmeta.load_yaml(p)
        if data.get("class") != p.stem:
            errs.append(f"{p.name}: class must be {p.stem}")
        manual[p.stem] = data
    for p in sorted(INFERRED.glob("NS*.yaml")) if INFERRED.exists() else []:
        if validate:
            errs += [f"inferred/{e}" for e in svmeta.check_override(p, SCHEMA)]
        inferred[p.stem] = svmeta.load_yaml(p)
    parked = parked_keys()
    origin: dict = {}
    merged = {cls: merge(manual.get(cls), inferred.get(cls), cls, parked, origin) for cls in sorted(set(manual) | set(inferred))}
    return merged, origin, errs
