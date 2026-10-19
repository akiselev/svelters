#!/usr/bin/env python3
"""Regenerate tests/fixtures/ir/{gtk,qt}.json: a small, checked-in subset of the real docs IR.

The golden tests run on this subset rather than on docs/site/.generated/ir, so they change only
when this script is re-run on purpose (metadata edits by other lanes do not churn them). The pack
consistency test (tests/site_consistency.rs) covers the real IR.

    python3 crates/svelters-cli/tests/fixtures/make_fixture.py      # from the repo root, after pnpm docs:ir

Entities in FULL keep every member. The others keep the members named in KEEP plus the first
two dangerous ones of each kind, so inheritance, overloads and danger marks stay testable.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
IR = ROOT / "docs/site/.generated/ir"
OUT = Path(__file__).resolve().parent / "ir"

FULL = {
    "gtk": {"gtk:Gtk.Button", "gtk:Gtk.ApplicationWindow", "gtk:Gtk.Orientation", "gtk:Gtk.Actionable"},
    "qt": {"qt:QPushButton", "qt:QMainWindow.DockOption"},
}
KEEP = {
    "gtk": {
        "gtk:Gtk.Widget": {"visible", "tooltip-text", "css-classes", "hexpand", "can-focus", "destroy", "show", "activate",
                           "set_parent", "unparent", "grab_focus", "get_visible", "set_visible"},
        "gtk:Gtk.Window": {"title", "child", "titlebar", "modal", "close-request", "present", "close", "set_titlebar", "set_child", "new"},
        "gtk:Gtk.Grid": {"row-spacing", "column-homogeneous", "attach", "new"},
    },
    "qt": {
        "qt:QMainWindow": {"window-title", "document-mode", "animated", "dockOptions", "close", "setCentralWidget", "centralWidget",
                           "addDockWidget", "addToolBar", "statusBar", "QMainWindow"},
        "qt:QWidget": {"enabled", "tool-tip", "window-title", "close", "resize", "setParent", "setLayout", "windowTitle", "show"},
        "qt:QAbstractButton": {"text", "checkable", "checked", "clicked", "toggled", "click", "text", "setText", "isChecked"},
    },
}
MEMBER_FIELDS = ("properties", "events", "methods", "constructors", "values")


def member_names(field: str, m: dict) -> set[str]:
    if field == "properties":
        return {m["name"], m["js_name"]}
    if field == "events":
        return {m["name"], m.get("js_event", "")}
    return {m["name"]}


def trim(e: dict, keep: set[str]) -> dict:
    e = dict(e)
    for field in MEMBER_FIELDS:
        kept, dangerous = [], 0
        for m in e.get(field, []):
            if member_names(field, m) & keep:
                kept.append(m)
            elif m.get("dangerous") and dangerous < 2:
                kept.append(m)
                dangerous += 1
        e[field] = kept
    return e


def main() -> int:
    OUT.mkdir(exist_ok=True)
    for tk in ("gtk", "qt"):
        ir = json.loads((IR / f"{tk}.json").read_text())
        want = FULL[tk] | set(KEEP[tk])
        ents = []
        for e in ir["entities"]:
            if e["id"] in FULL[tk]:
                ents.append(e)
            elif e["id"] in KEEP[tk]:
                ents.append(trim(e, KEEP[tk][e["id"]]))
        missing = want - {e["id"] for e in ents}
        if missing:
            print(f"{tk}: not in the IR: {sorted(missing)}", file=sys.stderr)
            return 1
        ir["entities"] = ents
        ir["toolkit"].pop("sources", None)
        (OUT / f"{tk}.json").write_text(json.dumps(ir, indent=1, sort_keys=True, ensure_ascii=False) + "\n")
        print(f"{tk}: {len(ents)} entities, {(OUT / f'{tk}.json').stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
