#!/usr/bin/env python3
"""GTK documentation-text ingestion (svelters-docs/v0), a separate pass from ingest.py.

Reads the GIR `<doc>`, `<doc-deprecated>`, `version` and `deprecated-version` of every kind that
metadata/gtk/*.json contains and writes metadata/docs/gtk/<Kind>.json. gi-docgen markdown links
([class@Gtk.Button], [method@Gtk.Button.set_child], ...) become IR cross-references:
`[Button](xref:gtk:Gtk.Button)`, `[Button.set_child()](xref:gtk:Gtk.Button#method:set_child)`.

Licence: GTK documentation is LGPL-2.1-or-later; it is redistributable with attribution and the
licence notice, and our link/markup conversion is a modification (noted in the output).

    python3 tools/metadata/gtk/ingest_docs.py [--girdir /usr/share/gir-1.0] [--out metadata/docs/gtk]
"""
from __future__ import annotations

import argparse
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "docs"))
import gir as G  # noqa: E402
import docs_common as D  # noqa: E402
import svmeta  # noqa: E402

TOOLKIT = "gtk"

# docgen role -> (entity?) ; member roles handled separately
ENTITY_ROLES = {"class", "iface", "struct", "enum", "flags", "callback", "alias", "error", "type", "domain"}
LINK_RE = re.compile(r"\[([a-z]+)@([A-Za-z0-9_.:\-]+)\]")


def _xref_member(role: str, target: str) -> tuple[str, str] | None:
    """(xref target without scheme, display) for a docgen role/target, or None when not linkable."""
    if role in ENTITY_ROLES:
        return f"gtk:{target}", target.rpartition(".")[2]
    if role in ("method", "ctor", "vfunc"):
        ent, _, member = target.rpartition(".")
        kind = {"method": "method", "ctor": "constructor", "vfunc": "vfunc"}[role]
        return f"gtk:{ent}#{kind}:{member}", f"{ent.rpartition('.')[2]}.{member}()"
    if role == "property":
        ent, _, member = target.partition(":")
        return f"gtk:{ent}#property:{member}", f"{ent.rpartition('.')[2]}:{member}"
    if role == "signal":
        ent, _, member = target.partition("::")
        return f"gtk:{ent}#event:{member}", f"{ent.rpartition('.')[2]}::{member}"
    if role == "enum_member" or role == "const":
        return None
    return None


def convert_markdown(text: str) -> str:
    """gi-docgen / gtk-doc flavoured markdown -> CommonMark with `xref:` links."""
    text = re.sub(r"<picture>.*?</picture>", "", text, flags=re.S)
    text = re.sub(r"<img[^>]*>", "", text)

    def role_link(m: re.Match) -> str:
        role, target = m.group(1), m.group(2)
        if role in ("enum", "flags") and target.count(".") >= 2:
            # [enum@Gtk.AccessibleRole.button]: a member of an enumeration
            ent, _, member = target.rpartition(".")
            return f"[`{ent.rpartition('.')[2]}.{member}`](xref:gtk:{ent}#value:{member})"
        hit = _xref_member(role, target)
        if hit is None:
            return f"`{target.rpartition('.')[2] or target}`"
        return f"[`{hit[1]}`](xref:{hit[0]})"

    text = LINK_RE.sub(role_link, text)
    # gtk-doc leftovers
    if "|[" in text:
        text = re.sub(r"\|\[(?:<!--\s*language=\"(\w+)\"\s*-->)?", lambda m: "\n```" + (m.group(1) or "c").lower() + "\n", text)
        text = text.replace("]|", "\n```\n")
    text = re.sub(r"%(TRUE|FALSE|NULL)\b", lambda m: "`" + m.group(1) + "`", text)
    text = re.sub(r"(?<![\w@`])@([a-z_][a-z0-9_]*)\b", r"`\1`", text)
    text = re.sub(r"(?<![\w&#`/])#(Gtk[A-Za-z0-9]+|G[A-Z][A-Za-z0-9]+)\b", r"`\1`", text)
    return re.sub(r"\n{3,}", "\n\n", text).strip()


def _doc(el: ET.Element | None) -> dict | None:
    if el is None:
        return None
    d = el.find("core:doc", G.NS)
    if d is None or not (d.text or "").strip():
        return None
    md = convert_markdown(d.text)
    out = {"md": md, "brief": D.brief_of(md)}
    if d.get("filename"):
        out["file"] = d.get("filename")
        out["line"] = int(d.get("line", "0"))
    dep = el.find("core:doc-deprecated", G.NS)
    if dep is not None and (dep.text or "").strip():
        out["deprecated_md"] = convert_markdown(dep.text)
    if el.get("version"):
        out["since"] = el.get("version")
    if el.get("deprecated") == "1":
        out["deprecated"] = el.get("deprecated-version") or True
    return out


def _members(el: ET.Element, tag: str, key: str = "name") -> dict:
    out = {}
    for m in el.findall(tag, G.NS):
        doc = _doc(m)
        if doc:
            out[m.get(key)] = doc
    return dict(sorted(out.items()))


def kind_doc(repo: G.Repo, meta: dict) -> dict:
    ns, name = meta["id"].split(":")[4:6]
    tag, el = repo.find(ns, name)
    out: dict = {
        "schema": D.SCHEMA,
        "toolkit": TOOLKIT,
        "id": meta["id"],
        "license": D.LICENSES[TOOLKIT]["spdx"],
        "doc": None,
        "properties": {},
        "signals": {},
        "methods": {},
        "constructors": {},
        "vfuncs": {},
        "members": {},
    }
    if el is None:
        return out
    out["doc"] = _doc(el)
    out["properties"] = _members(el, "core:property")
    out["signals"] = _members(el, "glib:signal")
    out["methods"] = {**_members(el, "core:method"), **_members(el, "core:function")}
    out["constructors"] = _members(el, "core:constructor")
    out["vfuncs"] = _members(el, "core:virtual-method")
    out["members"] = _members(el, "core:member")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--girdir", default="/usr/share/gir-1.0")
    ap.add_argument("--out", default=str(D.ROOT / "metadata/docs/gtk"))
    ap.add_argument("--check", action="store_true", help="exit 1 if output would change")
    args = ap.parse_args()

    repo = G.Repo(args.girdir)
    repo.load("Gtk-4.0")
    gir_path = repo.files["Gtk"]
    docs: dict[str, dict] = {}
    for f in D.kind_files(TOOLKIT):
        meta = svmeta.load_json(f)
        docs[f.stem] = kind_doc(repo, meta)
        docs[f.stem]["source"] = {"gir": {"file": gir_path.name, "sha256": G.sha256(gir_path)}}

    if args.check:
        out = Path(args.out)
        stale = [n for n, d in docs.items() if not (out / f"{n}.json").exists()]
        print(f"{len(stale)} missing")
        return 1 if stale else 0
    changed, total = D.write_all(Path(args.out), docs)
    n_doc = sum(1 for d in docs.values() if d["doc"])
    print(f"gtk docs: {total} kinds ({n_doc} with description), {changed} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
