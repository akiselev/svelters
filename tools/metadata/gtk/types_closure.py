#!/usr/bin/env python3
"""Signature closure of imported types (decision Q9, plans/research/NON-GUI-TYPES.md section 3.5).

The release binding linker keeps, for every class an app imports from `svelters:gtk/<ns>`, the native glue of
that class and of everything its signature mentions: the types reachable through the params, returns,
properties and signals of the class (including the members copied from its interfaces), plus its parent
and interfaces. `packages/types/gtk/manifest.json` records those edges per class; this module computes the
closure from it. Enums and flags are leaves (they have no class).

  python3 tools/metadata/gtk/types_closure.py svelters:gtk/gio:File svelters:gtk/gtk:TextBuffer
  python3 tools/metadata/gtk/types_closure.py --module svelters:gtk/gio     # every export of a module
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "packages/types/gtk/manifest.json"


def load(path: Path = MANIFEST) -> dict:
    return json.loads(Path(path).read_text())


def by_native(manifest: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for mod, m in manifest["modules"].items():
        for name, e in m["exports"].items():
            out[e["native"]] = {**e, "module": mod, "export": name}
    return out


def roots_of(manifest: dict, imports: list[str]) -> list[str]:
    """`svelters:gtk/gio:File` (module:name) -> native names. A bare module name selects all its exports."""
    natives = []
    for spec in imports:
        mod, _, name = spec.rpartition(":") if spec.count(":") >= 2 else (spec, "", "")
        m = manifest["modules"].get(mod)
        if m is None:
            raise KeyError(f"unknown module {mod!r}")
        names = [name] if name else sorted(m["exports"])
        for n in names:
            if n not in m["exports"]:
                raise KeyError(f"{mod} has no export {n!r}")
            natives.append(m["exports"][n]["native"])
    return natives


def closure(manifest: dict, roots: list[str]) -> set[str]:
    """Native names of every class the imported `roots` need: themselves, parents, interfaces, and the
    classes their signatures reference, transitively. Enums and flags that a signature mentions are
    included as names that have no class."""
    idx = by_native(manifest)
    seen: set[str] = set()
    todo = list(roots)
    while todo:
        n = todo.pop()
        if n in seen:
            continue
        seen.add(n)
        e = idx.get(n)
        if e is None:
            continue  # an enum or flags type: a leaf
        todo += [e["extends"]] if e["extends"] else []
        todo += e["implements"]
        todo += e["signature"]
    return seen


def classes_in(manifest: dict, names: set[str]) -> set[str]:
    idx = by_native(manifest)
    return {n for n in names if n in idx}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("imports", nargs="*")
    ap.add_argument("--module", action="append", default=[])
    ap.add_argument("--manifest", default=str(MANIFEST))
    a = ap.parse_args()
    m = load(Path(a.manifest))
    roots = roots_of(m, a.imports + a.module)
    c = closure(m, roots)
    cls = sorted(classes_in(m, c))
    print(f"{len(roots)} imported, {len(cls)} classes retained, {len(c) - len(cls)} enum/flags types referenced")
    for n in cls:
        print("  " + n)
    return 0


if __name__ == "__main__":
    sys.exit(main())
