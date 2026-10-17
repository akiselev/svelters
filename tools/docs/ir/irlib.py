"""Toolkit-neutral helpers for docs frontends: IR text blocks, validation and deterministic output.

Nothing here knows about a particular toolkit. Frontends (tools/docs/<toolkit>/frontend.py) build a
dict conforming to tools/docs/ir/schema.json and hand it to `finish`.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import jsonschema

IR_SCHEMA = "svelters-docs-ir/v0"
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCHEMA_PATH = HERE / "schema.json"

PRIMITIVE_TS = {
    "bool": "boolean", "i32": "number", "u32": "number", "i64": "number", "u64": "number",
    "f32": "number", "f64": "number", "string": "string", "void": "void", "variant": "unknown",
    "pointer": "unknown", "opaque": "unknown", "unknown": "unknown", "varargs": "unknown",
}


def text(md: str, license: str, origin: str, truncated: bool = False, upstream_url: str | None = None) -> dict:
    t: dict[str, Any] = {"md": md, "license": license, "origin": origin}
    if truncated:
        t["truncated"] = True
    if upstream_url:
        t["upstream_url"] = upstream_url
    return t


def neutral_to_ts(ref: str, name_of) -> tuple[str, str | None]:
    """Map a neutral type ref (`bool`, `enum<Id>`, `list<T>`...) to (TypeScript display, entity ref).

    `name_of(type_id)` returns (display_name, entity_id|None) for a metadata type id.
    """
    head, _, rest = ref.partition("<")
    inner = rest[:-1] if rest.endswith(">") else rest
    if not rest:
        return PRIMITIVE_TS.get(ref, "unknown"), None
    if head == "list":
        ts, ent = neutral_to_ts(inner, name_of)
        return f"{ts}[]", ent
    if head in ("enum", "flags", "object", "boxed", "value"):
        disp, ent = name_of(inner)
        return disp, ent
    if head == "callback":
        return "(...args: unknown[]) => unknown", None
    return "unknown", None


def display_value(v: Any) -> str | None:
    if v is None:
        return None
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        if isinstance(v, float) and abs(v) > 1e300:
            return "unbounded"
        return repr(v)
    if isinstance(v, str):
        return json.dumps(v, ensure_ascii=False)
    return json.dumps(v, sort_keys=True, ensure_ascii=False)


def validate(ir: dict) -> list[str]:
    schema = json.loads(SCHEMA_PATH.read_text())
    v = jsonschema.Draft202012Validator(schema)
    errs = sorted(v.iter_errors(ir), key=lambda e: list(map(str, e.absolute_path)))
    out = []
    for e in errs[:40]:
        path = "/".join(map(str, e.absolute_path))
        out.append(f"{path}: {e.message[:200]}")
    return out


def check_unique(ir: dict) -> list[str]:
    seen, errs = set(), []
    for e in ir["entities"]:
        if e["id"] in seen:
            errs.append(f"duplicate entity id {e['id']}")
        seen.add(e["id"])
    return errs


def finish(ir: dict, out_path: Path) -> bool:
    """Validate, then write deterministic JSON. Returns True when the file changed."""
    errs = validate(ir) + check_unique(ir)
    if errs:
        raise SystemExit("IR validation failed:\n  " + "\n  ".join(errs))
    ir["entities"].sort(key=lambda e: e["id"])
    body = json.dumps(ir, indent=1, sort_keys=True, ensure_ascii=False) + "\n"
    out_path = Path(out_path)
    if out_path.exists() and out_path.read_text() == body:
        return False
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(body)
    return True
