"""Native name -> JS name rules for AppKit (kebab and tags: the shared tools/metadata/common/naming.py). Mirrors crates/svelters-apple/src/names.rs; both are
tested against the same pairs (test_names.py, `names::tests::PAIRS`)."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
import naming  # noqa: E402  (the shared rule: tools/metadata/common/naming.py)

kebab = naming.kebab


def class_tag(cls: str, prefix: str = "NS") -> str:
    return naming.class_tag(cls, prefix)


def selector_name(sel: str) -> str:
    parts = [p for p in sel.split(":") if p]
    s = parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:]) if parts else ""
    return kebab(s)


def selector_arity(sel: str) -> int:
    return sel.count(":")


def notification_signal(name: str, prefix: str = "NS") -> str:
    n = name[len(prefix):] if name.startswith(prefix) else name
    return kebab(n.removesuffix("Notification"))
