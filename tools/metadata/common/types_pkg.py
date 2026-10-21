"""`packages/types/package.json`, shared by the per-toolkit type generators (gtk/gen_types.py, qt/gen_types_js.py,
winui/gen_types_winui.py).

Each generator owns the export keys of its toolkit (`./gtk/*` + `./manifest`, `./qt/*` + `./manifest-qt`, `./winui/*` + `./manifest-winui`) and
keeps the others' keys, so the file is the same whichever generator ran last. Key order is canonical: the fixed
head keys, then the `./gtk/*` group, then `./qt/*`, then anything else.
"""
from __future__ import annotations

import json
from pathlib import Path

HEAD = ["./core", "./vite", "./manifest", "./manifest-qt", "./manifest-winui"]
GROUPS = ["./gtk/", "./qt/", "./winui/"]
DESCRIPTION = "Native object model and generated non-UI types (svelters:gtk/gio, svelters:qt/core, ...) for Svelters apps"


def owned(key: str, prefix: str, head: list[str]) -> bool:
    return key.startswith(prefix) or key in head


def package_json(path: Path, own: dict, prefix: str, own_head: list[str]) -> str:
    """The package.json text with `own` (this generator's export entries) merged in."""
    base = json.loads(path.read_text()) if path.exists() else {}
    old = {k: v for k, v in (base.get("exports") or {}).items() if not owned(k, prefix, own_head)}
    merged = {**old, **own}
    merged.setdefault("./core", {"types": "./src/core.d.ts", "default": "./src/core.js"})
    merged.setdefault("./vite", "./vite.js")
    exports: dict = {}
    for k in HEAD:
        if k in merged:
            exports[k] = merged[k]
    for g in GROUPS:
        src = own if g == prefix else old
        for k, v in src.items():
            if k.startswith(g):
                exports[k] = v
    for k, v in merged.items():
        if k not in exports:
            exports[k] = v
    base.update({"name": "@svelters/types", "version": "0.0.0", "license": "MIT", "private": True, "type": "module",
                 "description": DESCRIPTION, "sideEffects": False})
    base["exports"] = exports
    base["scripts"] = {"test": "node --test test/*.test.mjs && tsc -p test/types/tsconfig.json"}
    base["devDependencies"] = {"typescript": "~6.0.3", "vite": "8.3.4"}
    return json.dumps(base, indent=2) + "\n"
