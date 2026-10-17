"""Shared helpers for the documentation-text ingestion pass (svelters-docs/v0).

The pass is separate from the element-kind ingesters so that it never conflicts with them. It writes
metadata/docs/<toolkit>/<Kind>.json, one file per element-kind file of the same name, keyed by the
same member names/ids. Deterministic: sorted keys, no clocks, no absolute paths.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import svmeta  # noqa: E402

SCHEMA = "svelters-docs/v0"

LICENSES = {
    "gtk": {
        "spdx": "LGPL-2.1-or-later",
        "name": "GNU Lesser General Public License v2.1 or later",
        "url": "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
        "attribution": "Documentation text from the GTK project (GIR documentation comments), (c) the GTK authors.",
    },
    "qt": {
        "spdx": "GFDL-1.3-only",
        "name": "GNU Free Documentation License v1.3",
        "url": "https://www.gnu.org/licenses/fdl-1.3.html",
        "attribution": "Documentation from Qt, (c) The Qt Company Ltd. and other contributors, licensed under the GNU Free Documentation License v1.3.",
    },
}


def brief_of(md: str, limit: int = 220) -> str:
    """First sentence of the first paragraph, whitespace-collapsed, at most `limit` chars."""
    para = re.split(r"\n\s*\n", md.strip(), maxsplit=1)[0]
    para = re.sub(r"\s+", " ", para).strip()
    m = re.search(r"(?<=[.!?])\s+(?=[A-Z`\[])", para)
    if m and m.start() >= 20:
        para = para[: m.start()]
    if len(para) > limit:
        cut = para[:limit].rsplit(" ", 1)[0]
        para = cut.rstrip(",;:") + "..."
    return para


def kind_files(toolkit: str) -> list[Path]:
    return sorted((ROOT / "metadata" / toolkit).glob("*.json"))


def write_all(out_dir: Path, docs: dict[str, dict]) -> tuple[int, int]:
    """Write every kind file; remove stale ones. Returns (changed, total)."""
    out_dir.mkdir(parents=True, exist_ok=True)
    changed = 0
    for name, data in sorted(docs.items()):
        if svmeta.write_json(out_dir / f"{name}.json", data):
            changed += 1
    for old in out_dir.glob("*.json"):
        if old.stem not in docs:
            old.unlink()
            changed += 1
    return changed, len(docs)
