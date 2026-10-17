#!/usr/bin/env python3
"""Run every docs frontend (tools/docs/<toolkit>/frontend.py) and write its IR.

Frontends are independent processes; a future toolkit (winui, appkit, egui) is picked up by adding
tools/docs/<toolkit>/frontend.py. Output: docs/site/.generated/ir/<toolkit>.json (not checked in).
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docs/site/.generated/ir"


def main() -> int:
    fronts = sorted(p.parent.name for p in (ROOT / "tools/docs").glob("*/frontend.py"))
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.json"):
        if old.stem not in fronts:
            old.unlink()
    for tk in fronts:
        r = subprocess.run([sys.executable, "-I", str(ROOT / f"tools/docs/{tk}/frontend.py"), "--out", str(OUT / f"{tk}.json")])
        if r.returncode:
            return r.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
