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
ORIGINS = {"gir", "runtime", "qdoc", "metaobject", "typesystem", "apiextractor", "uic", "manual", "inferred", "probe", "rule", "docs", "source", "derived"}
REQUIRED = ("schema", "toolkit", "id", "native", "kind")
# Lists of records merged by this key instead of replaced wholesale.
LIST_KEYS = {"properties": "name", "signals": "name", "methods": "name", "constructors": "id", "slots": "name", "bindings": "name"}
# Annotation layers, lowest precedence first: ingested < inferred < manual (plans/research/ANNOTATION-AUTOMATION.md section 5.2).
LAYERS = ("inferred", "manual")
# Override keys that are read where they are used and never merged into a kind's facts (so no fact record or
# fingerprint changes when they change). `dangerous`: members the backend owns (tools/metadata/common/dangerous.py,
# metadata/dangerous/<tk>.yaml < the override's `dangerous:` block; docs/notes/dangerous-members.md).
READ_TIME_KEYS = ("dangerous",)
DANGEROUS_SEVERITIES = ("corrupts-tree", "owned-by-backend", "crashes", "other")


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


def _mark(value: Any, origin: str) -> Any:
    if isinstance(value, dict):
        out = {k: _mark(v, origin) for k, v in value.items()}
        out.setdefault("origin", origin)
        return out
    if isinstance(value, list):
        return [_mark(v, origin) for v in value]
    return value


def merge(base: Any, override: Any, key: str | None = None, origin: str = "manual") -> Any:
    """Merge an override layer (hand/agent annotation, or the inferred layer) over lower-precedence facts.

    Dicts merge recursively; keyed record lists (LIST_KEYS) merge by key; anything else is
    replaced. Records introduced by the override are marked with the layer's `origin`
    (`manual` or `inferred`); a record that already has an origin keeps it, so a manual field
    merged into an inferred record is still told apart by the record's `annotated.origin`.
    """
    if isinstance(base, dict) and isinstance(override, dict):
        out = dict(base)
        for k, v in override.items():
            if k in READ_TIME_KEYS:
                continue  # never part of a fact (see READ_TIME_KEYS)
            out[k] = merge(base[k], v, k, origin) if k in base else _mark(copy.deepcopy(v), origin)
        return out
    if isinstance(base, list) and isinstance(override, list) and key in LIST_KEYS:
        ident = LIST_KEYS[key]
        out = [dict(r) for r in base]
        index = {r.get(ident): i for i, r in enumerate(out)}
        for rec in override:
            if rec.get(ident) in index:
                i = index[rec[ident]]
                out[i] = merge(out[i], rec, None, origin)
                out[i]["origin"] = origin if out[i].get("origin") is None else out[i]["origin"]
            else:
                out.append(_mark(copy.deepcopy(rec), origin))
        return out
    return _mark(copy.deepcopy(override), origin)


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
            for k in READ_TIME_KEYS:
                if k in v:
                    errs.append(f"{path}: {k!r} is a read-time layer and must not be merged into the facts")
            for k, x in v.items():
                walk(x, f"{path}.{k}")
        elif isinstance(v, list):
            for i, x in enumerate(v):
                walk(x, f"{path}[{i}]")

    walk(kind, kind.get("id", "?"))
    for key, ident in LIST_KEYS.items():
        recs = kind.get(key) if key != "slots" else (kind.get("children") or {}).get("slots")
        # Additive: overloaded records (same name) are distinguished by their unique `id` when every record has one.
        recs = recs or []
        use = "id" if recs and all(r.get("id") for r in recs) else ident
        names = [r.get(use) for r in recs]
        dupes = sorted({n for n in names if names.count(n) > 1})
        if dupes:
            errs.append(f"duplicate {key}: {dupes}")
    return errs


# ---- A1: override schemas, provenance, fact fingerprints ----------------------------------------

def check_override(path: Path, schema_path: Path) -> list[str]:
    """Validate one override file against its toolkit's JSON Schema. Returns error strings."""
    import jsonschema

    schema = json.loads(Path(schema_path).read_text())
    data = load_yaml(path)
    v = jsonschema.Draft202012Validator(schema)
    errs = []
    for e in sorted(v.iter_errors(data), key=lambda e: [str(p) for p in e.absolute_path]):
        where = "/".join(str(p) for p in e.absolute_path) or "<root>"
        errs.append(f"{Path(path).name}: {where}: {e.message}")
    return errs


def fingerprint(fact: Any) -> str:
    """Stable short hash of a fact record (sorted JSON). A changed fact needs a new verification."""
    import hashlib

    text = json.dumps(fact, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    return hashlib.sha256(text.encode()).hexdigest()[:16]
