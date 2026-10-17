#!/usr/bin/env python3
"""N1 one-shot: rewrite metadata/overrides/qt/*.yaml to the native kebab-case names (docs/notes/n1-rename.md).

Text-level, so comments and layout survive. Per element: `tag` becomes the class tag (a plain QWidget
carrying a layout takes the layout's tag), prop/signal `js` names become kebab(native) (kebab(getter)
for accessor props), and the references to them (bindings, `exclude`, `attr.<name>` args) follow.
Writes the old -> new map to the path given as the first argument (JSON).
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import naming  # noqa: E402
import yaml  # noqa: E402

OV = ROOT / "metadata/overrides/qt"
# Synthetic event-filter signals named after their QWidget event handler (mouseDoubleClickEvent, closeEvent).
SYNTHETIC_SIGNAL = {"double-click": "mouse-double-click", "close-request": "close"}
ATTR = {"breakBefore": "tool-bar-break", "tabLabel": "tab-text", "tabToolTip": "tab-tool-tip", "tabEnabled": "tab-enabled",
        "align": "alignment", "columnSpan": "column-span", "rowSpan": "row-span"}
DIRECTION_VALUES = ("left-to-right: QBoxLayout::LeftToRight", "right-to-left: QBoxLayout::RightToLeft",
                    "top-to-bottom: QBoxLayout::TopToBottom", "bottom-to-top: QBoxLayout::BottomToTop")


def el_tag(e: dict) -> str:
    c = e["construct"]
    lay = (c.get("composite") or {}).get("layout", {}).get("class")
    return naming.tag_for("qt", lay if c["class"] == "QWidget" and lay else c["class"])


def prop_js(p: dict) -> str:
    if p.get("native"):
        return naming.kebab(p["native"])
    if p.get("getter"):
        return naming.kebab(p["getter"])
    return p["js"]  # dynamic / synthetic


def sig_js(s: dict) -> str:
    if s.get("native"):
        return naming.kebab(s["native"])
    return SYNTHETIC_SIGNAL.get(s["js"], s["js"])


def main() -> int:
    out_map = {"tags": {}, "props": {}, "signals": {}, "attrs": {}, "bindings": {}}
    docs = {p: yaml.safe_load(p.read_text()) or {} for p in sorted(OV.glob("*.yaml"))}
    # Inherited records: one global map (exclude lists name them from other files).
    inh = {"props": {}, "signals": {}}
    for p, d in docs.items():
        b = d.get("inherit") or {}
        for r in b.get("props", []):
            inh["props"][r["js"]] = prop_js(r)
        for r in b.get("signals", []):
            inh["signals"][r["js"]] = sig_js(r)
    for path, d in docs.items():
        text = path.read_text()
        lines = text.split("\n")
        # spans: inherit block, then one per element (`  - tag:` lines)
        starts = [(i, "inherit") for i, ln in enumerate(lines) if ln.startswith("inherit:")]
        tag_lines = [i for i, ln in enumerate(lines) if re.match(r"^  - tag: ", ln)]
        starts += [(i, "el") for i in tag_lines]
        starts.sort()
        bounds = []
        for k, (i, kind) in enumerate(starts):
            j = starts[k + 1][0] if k + 1 < len(starts) else len(lines)
            if kind == "inherit":
                # the inherit block ends at the next top-level key
                for x in range(i + 1, len(lines)):
                    if re.match(r"^[a-z_]+:", lines[x]):
                        j = min(j, x)
                        break
            bounds.append((i, j, kind))
        els = list(d.get("elements") or [])
        ei = 0
        recs = []
        for i, j, kind in bounds:
            recs.append(None if kind == "inherit" else els[ei])
            ei += 0 if kind == "inherit" else 1
        # last block first, so line insertions do not shift the spans still to do
        for (i, j, kind), el in reversed(list(zip(bounds, recs))):
            if kind == "inherit":
                rec = d["inherit"]
                pmap = {r["js"]: prop_js(r) for r in rec.get("props", [])}
                smap = {r["js"]: sig_js(r) for r in rec.get("signals", [])}
                label = f"inherit:{path.stem}"
                newtag = None
            else:
                rec = el
                pmap = {r["js"]: prop_js(r) for r in rec.get("props", [])}
                smap = {r["js"]: sig_js(r) for r in rec.get("signals", [])}
                newtag = el_tag(rec)
                label = rec["tag"]
                out_map["tags"][rec["tag"]] = {"new": newtag, "class": rec["construct"]["class"]}
            mjs = {r["js"] for r in rec.get("methods", [])}
            for old, new in list(pmap.items()) + list(smap.items()):
                if old != new and old in mjs:
                    print(f"WARN {path.name} {label}: {old} is also a method name", file=sys.stderr)
            for old, new in pmap.items():
                out_map["props"].setdefault(label, {})[old] = new
            for old, new in smap.items():
                out_map["signals"].setdefault(label, {})[old] = new
            allp = {**inh["props"], **pmap}
            alls = {**inh["signals"], **smap}
            seg = "\n".join(lines[i:j])
            if newtag:
                seg = re.sub(r"^(  - tag: )\S+$", lambda m: m.group(1) + newtag, seg, count=1, flags=re.M)
            # js names of props and signals (methods keep theirs)
            def js_sub(m):
                old = m.group(2)
                new = pmap.get(old, smap.get(old, old)) if old not in mjs else old
                return m.group(1) + new
            seg = re.sub(r"(\bjs: )([A-Za-z][A-Za-z0-9_-]*)", js_sub, seg)
            # exclude lists
            def ex_sub(m):
                names = [n.strip() for n in m.group(2).split(",")]
                return m.group(1) + ", ".join(allp.get(n, alls.get(n, n)) for n in names) + "]"
            seg = re.sub(r"(exclude: \[)([^\]]*)\]", ex_sub, seg)
            # bindings: name / property / change.signal
            for b in rec.get("bindings", []) or []:
                newp = allp.get(b["property"], b["property"])
                out_map["bindings"].setdefault(label, {})[b["name"]] = newp
                seg = re.sub(rf"^(\s+- name: ){re.escape(b['name'])}$", lambda m: m.group(1) + newp, seg, flags=re.M)
                seg = re.sub(rf"(\bproperty: ){re.escape(b['property'])}\b(?!-)", lambda m: m.group(1) + newp, seg)
                sig = b["change"]["signal"]
                news = alls.get(sig, sig)
                seg = re.sub(rf"(\bsignal: ){re.escape(sig)}\b(?!-)", lambda m: m.group(1) + news, seg)
            # child attributes
            for old, new in ATTR.items():
                seg = re.sub(rf"^(\s+- name: ){old}$", lambda m: m.group(1) + new, seg, flags=re.M)
                seg = re.sub(rf"\battr\.{old}\b", "attr." + new, seg)
                seg = re.sub(rf"parent:{old}\b", "parent:" + new, seg)
            # QBoxLayout direction: native values instead of the GTK orientation vocabulary
            if "direction" in pmap.values():
                seg = re.sub(r"^(\s+)values:\n\s+horizontal: QBoxLayout::LeftToRight\n\s+vertical: QBoxLayout::TopToBottom$",
                             lambda m: m.group(1) + "values:\n" + "\n".join(m.group(1) + "  " + v for v in DIRECTION_VALUES), seg, flags=re.M)
                seg = seg.replace("QBoxLayout::direction (qdoc); GTK orientation vocabulary", "QBoxLayout::direction (qdoc)")
            lines = lines[:i] + seg.split("\n") + lines[j:]
        for sl_e in d.get("elements") or []:
            for sl in (sl_e.get("children") or {}).get("slots", []) or []:
                for a in sl.get("attrs", []) or []:
                    out_map["attrs"].setdefault(sl_e["tag"], {})[a["name"]] = ATTR.get(a["name"], naming.kebab(a["name"]))
        new_text = "\n".join(lines)
        if new_text != text:
            path.write_text(new_text)
    Path(sys.argv[1]).write_text(json.dumps(out_map, indent=1, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
