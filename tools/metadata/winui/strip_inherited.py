#!/usr/bin/env python3
"""Drop inherited instance methods from the element classes in crates/svelters-winui/src/bindings.rs (regen.sh step 5b).

windows-bindgen repeats every selected member of a base class on each derived class (`Button` gets all of
`UIElement`'s, cast to `IUIElement` inside). The generated dispatch code always calls a member on its declaring class
(`o.cast::<UIElement>()?.SetOpacity(..)`), and the hand-written runtime does the same, so these copies are dead code:
with 65 element kinds they were ~80% of the file (WP-W). A method inside `impl <Class> {` whose body casts to `I<Base>`
or `I<Base><n>` (an interface of a class in `<Class>`'s base chain, from metadata/winui/<Class>.json) is removed;
statics, factories and the class's own interfaces are kept. Deterministic; reruns are no-ops.

  python3 tools/metadata/winui/strip_inherited.py [--check] [crates/svelters-winui/src/bindings.rs]
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/winui"
DEFAULT = ROOT / "crates/svelters-winui/src/bindings.rs"

IMPL = re.compile(r"^(\s*)impl (\w+) \{$")
CAST = re.compile(r"cast::<(?:[\w:]*::)?(I\w+?)(\d*)>")


def bases() -> dict[str, set[str]]:
    out = {}
    for p in sorted(META.glob("*.json")):
        if p.name.startswith("_"):
            continue
        k = json.loads(p.read_text())
        out[k["native"]["short"]] = {b.rsplit(".", 1)[1] for b in k["bases"]}
    return out


def strip(text: str, chain: dict[str, set[str]]) -> tuple[str, int]:
    lines = text.split("\n")
    out: list[str] = []
    removed = 0
    i = 0
    while i < len(lines):
        m = IMPL.match(lines[i])
        if not m or m.group(2) not in chain:
            out.append(lines[i])
            i += 1
            continue
        indent, cls = m.group(1), m.group(2)
        inherited = {f"I{b}" for b in chain[cls]}
        out.append(lines[i])
        i += 1
        end = indent + "}"
        method = indent + "    pub fn "
        close = indent + "    }"
        while i < len(lines) and lines[i] != end:
            if lines[i].startswith(method):
                j = i
                while lines[j] != close:
                    j += 1
                body = "\n".join(lines[i:j + 1])
                c = CAST.search(body)
                if c and c.group(1) in inherited:
                    removed += 1
                    i = j + 1
                    continue
                out.extend(lines[i:j + 1])
                i = j + 1
                continue
            out.append(lines[i])
            i += 1
    return "\n".join(out), removed


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    path = Path(args[0]) if args else DEFAULT
    text = path.read_text()
    new, n = strip(text, bases())
    if "--check" in sys.argv:
        if n:
            print(f"strip_inherited: {path} has {n} inherited methods (run tools/metadata/winui/regen.sh)", file=sys.stderr)
            return 1
        return 0
    if new != text:
        path.write_text(new)
    print(f"strip_inherited: removed {n} inherited methods from {path}: {len(new.splitlines())} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
