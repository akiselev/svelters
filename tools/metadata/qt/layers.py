"""Qt annotation layers: manual overrides (metadata/overrides/qt) over the inferred layer (metadata/inferred/qt).

Precedence is ingested < inferred < manual, as on GTK (plans/research/ANNOTATION-AUTOMATION.md section 5.2). Qt
facts live on elements (an element is a manual exposure decision: tag, construct), so the inferred layer only adds
records to elements a manual override declares:

- an inferred record is dropped when the element already has a record of that JS name (props, signals, methods)
  or binding name, manual or inherited through a manual `inherit:` block of the class or a base, or the element
  `exclude`s the name;
- a manual element's `deny: ["<section>:<glob>"]` drops inferred records by pattern (sections props, signals,
  methods, bindings);
- an inferred record parked by `ledger.py refresh` (metadata/inferred/qt/_parked.json, matched by kind, section,
  name and record fingerprint) is left out.

Every inferred record that stays is marked `_origin: inferred` and `_fp: <fingerprint of the inferred YAML
record>`; underscore keys are bookkeeping, never part of a fact.
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
SECTIONS = {"props": "js", "signals": "js", "methods": "js", "bindings": "name"}
PARKED_SECTION = {"props": "props", "signals": "signals", "methods": "methods", "bindings": "bindings"}


def default_inferred(ov_dir: Path) -> Path:
    """metadata/overrides/qt -> metadata/inferred/qt."""
    return Path(ov_dir).resolve().parent.parent / "inferred" / Path(ov_dir).name


def load_parked(inferred_dir: Path | None) -> dict[tuple[str, str, str], str | None]:
    """(class, section, name) -> record fingerprint of the parked inferred facts."""
    if inferred_dir is None:
        return {}
    p = Path(inferred_dir) / "_parked.json"
    if not p.exists():
        return {}
    return {(r["kind"], r["section"], r["name"]): r.get("record_fp") for r in json.loads(p.read_text())["parked"]}


def load_inferred(inferred_dir: Path | None) -> dict[str, dict]:
    """class -> inferred file content ({elements: [{tag, props, signals, methods, bindings}]})."""
    if inferred_dir is None or not Path(inferred_dir).is_dir():
        return {}
    return {p.stem: svmeta.load_yaml(p) for p in sorted(Path(inferred_dir).glob("*.yaml"))}


def bases(meta_dir: Path, cls: str) -> list[str]:
    p = Path(meta_dir) / f"{cls}.json"
    return svmeta.load_json(p).get("bases", []) if p.exists() else []


def inherited_names(files: dict[str, dict], meta_dir: Path, cls: str, section: str) -> set[str]:
    """JS names an element of `cls` gets from manual `inherit:` blocks (the class and its bases)."""
    out = set()
    for c in [cls] + bases(meta_dir, cls):
        for r in ((files.get(c) or {}).get("inherit") or {}).get(section, []) or []:
            out.add(r["js"])
    return out


def denied(patterns: list[str], section: str, name: str) -> bool:
    return any(fnmatch.fnmatchcase(f"{section}:{name}", p) for p in patterns)


def merge(files: dict[str, dict], inferred: dict[str, dict], parked: dict, meta_dir: Path) -> dict[str, dict]:
    """The manual override files with the inferred records added to their elements (a new dict; inputs untouched)."""
    import copy
    out = copy.deepcopy(files)
    for cls, inf in sorted(inferred.items()):
        data = out.get(cls)
        if not data:
            continue
        by_tag = {el["tag"]: el for el in data.get("elements", []) or []}
        for iel in inf.get("elements", []) or []:
            el = by_tag.get(iel["tag"])
            if el is None or el["construct"]["class"] != cls:
                continue
            excl = set(el.get("exclude", []))
            deny = list(el.get("deny", []))
            for section, key in SECTIONS.items():
                have = {r[key] for r in el.get(section, []) or []} | excl
                if section != "bindings":
                    have |= inherited_names(files, meta_dir, cls, section)
                for rec in iel.get(section, []) or []:
                    name = rec[key]
                    if name in have or denied(deny, section, name):
                        continue
                    fp = svmeta.fingerprint(rec)
                    if parked.get((cls, PARKED_SECTION[section], name), "<none>") == fp:
                        continue
                    el.setdefault(section, []).append({**copy.deepcopy(rec), "_origin": "inferred", "_fp": fp})
                    have.add(name)
    return out


def merged_overrides(ov_dir: Path | None = None, inferred_dir: Path | None = None, meta_dir: Path | None = None) -> dict[str, dict]:
    """class -> override data with the inferred layer merged in (for readers of the override files)."""
    ov_dir = Path(ov_dir or ROOT / "metadata/overrides/qt")
    meta_dir = Path(meta_dir or ROOT / "metadata/qt")
    inferred_dir = Path(inferred_dir) if inferred_dir else default_inferred(ov_dir)
    files = {p.stem: svmeta.load_yaml(p) for p in sorted(ov_dir.glob("*.yaml"))}
    return merge(files, load_inferred(inferred_dir), load_parked(inferred_dir), meta_dir)


def clean(rec):
    """A record without the layer bookkeeping (`_origin`, `_fp`)."""
    if isinstance(rec, dict):
        return {k: clean(v) for k, v in rec.items() if k not in ("_origin", "_fp")}
    if isinstance(rec, list):
        return [clean(x) for x in rec]
    return rec
