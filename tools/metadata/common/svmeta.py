"""Shared helpers for svelters-element-kind/v0 metadata (see plans/architecture/widget-metadata.md).

Deterministic by contract: the same inputs always produce byte-identical output, so the
M3 Rust port can be checked by diffing. Nothing here touches the network, clocks or env.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

import yaml

SCHEMA = "svelters-element-kind/v0"
ORIGINS = {"gir", "runtime", "qdoc", "metaobject", "typesystem", "apiextractor", "uic", "manual"}
REQUIRED = ("schema", "toolkit", "id", "native", "kind")
# Lists of records merged by this key instead of replaced wholesale.
LIST_KEYS = {"properties": "name", "signals": "name", "methods": "name", "constructors": "id", "slots": "name"}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(Path(path).read_text())


def load_yaml(path: Path) -> dict[str, Any]:
    return yaml.safe_load(Path(path).read_text()) or {}


def write_json(path: Path, data: Any) -> bool:
    """Write sorted, 2-space JSON with a trailing newline. Returns True if the file changed."""
    text = json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    path = Path(path)
    if path.exists() and path.read_text() == text:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return True


def _mark_manual(value: Any) -> Any:
    if isinstance(value, dict):
        out = {k: _mark_manual(v) for k, v in value.items()}
        out.setdefault("origin", "manual")
        return out
    if isinstance(value, list):
        return [_mark_manual(v) for v in value]
    return value


def merge(base: Any, override: Any, key: str | None = None) -> Any:
    """Merge an override (hand/agent annotation) over ingested facts.

    Dicts merge recursively; keyed record lists (LIST_KEYS) merge by key; anything else is
    replaced. Records introduced by the override are marked origin: manual.
    """
    if isinstance(base, dict) and isinstance(override, dict):
        out = dict(base)
        for k, v in override.items():
            out[k] = merge(base[k], v, k) if k in base else _mark_manual(copy.deepcopy(v))
        return out
    if isinstance(base, list) and isinstance(override, list) and key in LIST_KEYS:
        ident = LIST_KEYS[key]
        out = [dict(r) for r in base]
        index = {r.get(ident): i for i, r in enumerate(out)}
        for rec in override:
            if rec.get(ident) in index:
                i = index[rec[ident]]
                out[i] = merge(out[i], rec)
                out[i]["origin"] = "manual" if out[i].get("origin") is None else out[i]["origin"]
            else:
                out.append(_mark_manual(copy.deepcopy(rec)))
        return out
    return _mark_manual(copy.deepcopy(override))


def validate(kind: dict[str, Any]) -> list[str]:
    """Return a list of problems (empty if valid). Deliberately shallow during P."""
    errs = [f"missing {k}" for k in REQUIRED if k not in kind]
    if kind.get("schema") != SCHEMA:
        errs.append(f"schema must be {SCHEMA}")
    if kind.get("toolkit") not in ("gtk", "qt"):
        errs.append("toolkit must be gtk|qt")

    def walk(v: Any, path: str) -> None:
        if isinstance(v, dict):
            o = v.get("origin")
            if o is not None and o not in ORIGINS:
                errs.append(f"{path}: unknown origin {o!r}")
            for k, x in v.items():
                walk(x, f"{path}.{k}")
        elif isinstance(v, list):
            for i, x in enumerate(v):
                walk(x, f"{path}[{i}]")

    walk(kind, kind.get("id", "?"))
    for key, ident in LIST_KEYS.items():
        recs = kind.get(key) if key != "slots" else (kind.get("children") or {}).get("slots")
        names = [r.get(ident) for r in recs or []]
        dupes = sorted({n for n in names if names.count(n) > 1})
        if dupes:
            errs.append(f"duplicate {key}: {dupes}")
    return errs
