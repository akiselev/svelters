#!/usr/bin/env python3
"""Qt documentation-text ingestion (svelters-docs/v0), a separate pass from ingest.py.

Sources: qdoc `.index` files (class and property briefs, member anchors) and, when present, the
qdoc HTML under <docdir>/{qtcore,qtgui,qtwidgets}/ (per-class and per-member descriptions, plus the
Qt Style Sheets reference for sub-controls and pseudo-states).

Licence: Qt documentation text is GFDL-1.3. Redistributing it needs the licence text, attribution,
and modification notices (and respect for invariant sections). The safe default (`--mode brief`)
stores only short briefs (the qdoc index briefs and the first sentence of each member) plus deep
links to doc.qt.io. `--mode links` stores only index briefs and links. `--mode full` also stores
the whole description as markdown; use it only when the generated site carries the GFDL notice
(the site's text policy switch, tools/docs/qt/config.json, still defaults to `brief`).

    python3 tools/metadata/qt/ingest_docs.py [--docdir /usr/share/doc/qt6] [--mode brief|links|full]
"""
from __future__ import annotations

import argparse
import html
import re
import sys
import xml.etree.ElementTree as ET
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "docs"))
import docs_common as D  # noqa: E402
import svmeta  # noqa: E402

TOOLKIT = "qt"
MODULES = ("qtcore", "qtgui", "qtwidgets")
BASE_URL = "https://doc.qt.io/qt-6/"


class MdConverter(HTMLParser):
    """Small qdoc-HTML -> CommonMark converter (enough for class/member descriptions)."""

    def __init__(self, classes: dict[str, str]):
        super().__init__(convert_charrefs=True)
        self.classes = classes  # lowercase page stem -> class name
        self.out: list[str] = []
        self.stack: list[str] = []
        self.href: list[str | None] = []
        self.in_pre = 0
        self.lists: list[str] = []
        self.skip = 0
        self.quote = 0

    def _emit(self, s: str) -> None:
        self.out.append(s)

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        self.stack.append(tag)
        if self.skip or tag in ("script", "style", "table", "img", "svg"):
            if tag in ("table", "script", "style", "svg"):
                self.skip += 1
            return
        if tag == "p":
            self._emit("\n\n" + ("> " if self.quote else ""))
        elif tag == "br":
            self._emit("\n")
        elif tag in ("b", "strong"):
            self._emit("**")
        elif tag in ("i", "em"):
            self._emit("*")
        elif tag == "code":
            if not self.in_pre:
                self._emit("`")
        elif tag == "pre":
            self.in_pre += 1
            self._emit("\n\n```cpp\n")
        elif tag in ("ul", "ol"):
            self.lists.append(tag)
            self._emit("\n")
        elif tag == "li":
            self._emit("\n" + "  " * (len(self.lists) - 1) + ("1. " if self.lists and self.lists[-1] == "ol" else "- "))
        elif tag in ("h2", "h3", "h4"):
            self._emit("\n\n#### ")
        elif tag == "div" and "admonition" in (a.get("class") or ""):
            self.quote += 1
            self._emit("\n\n")
        elif tag == "a":
            href = a.get("href")
            self.href.append(self._map_href(href) if href and not self.in_pre else None)
            if self.href[-1]:
                self._emit("[")
        else:
            pass

    def handle_endtag(self, tag):
        while self.stack and self.stack[-1] != tag:
            self.stack.pop()
        if self.stack:
            self.stack.pop()
        if tag in ("table", "script", "style", "svg") and self.skip:
            self.skip -= 1
            return
        if self.skip:
            return
        if tag in ("b", "strong"):
            self._emit("**")
        elif tag in ("i", "em"):
            self._emit("*")
        elif tag == "code" and not self.in_pre:
            self._emit("`")
        elif tag == "pre":
            self.in_pre -= 1
            self._emit("\n```\n")
        elif tag in ("ul", "ol") and self.lists:
            self.lists.pop()
            self._emit("\n")
        elif tag == "div" and self.quote:
            self.quote -= 1
        elif tag == "a" and self.href:
            h = self.href.pop()
            if h:
                self._emit(f"]({h})")

    def handle_data(self, data):
        if self.skip:
            return
        if self.in_pre:
            self._emit(data)
        else:
            self._emit(re.sub(r"\s+", " ", data))

    def _map_href(self, href: str) -> str | None:
        if href.startswith("#"):
            return None
        if href.startswith(("http://", "https://")):
            return href
        href = re.sub(r"^(\.\./\w+/)", "", href)
        page, _, frag = href.partition("#")
        stem = page[:-5] if page.endswith(".html") else page
        cls = self.classes.get(stem)
        if cls:
            return f"xref:qt:{cls}"
        return BASE_URL + href

    def result(self) -> str:
        text = "".join(self.out)
        text = re.sub(r"```cpp\n\s*\n", "```cpp\n", text)
        text = re.sub(r"\n\s*\n```\n", "\n```\n", text)
        text = re.sub(r"[ \t]+\n", "\n", text)
        text = re.sub(r"\n{3,}", "\n\n", text)
        return text.strip()


def html_to_md(frag: str, classes: dict[str, str]) -> str:
    c = MdConverter(classes)
    c.feed(frag)
    c.close()
    return c.result()


def parse_indexes(docdir: Path):
    classes: dict[str, dict] = {}
    props: dict[tuple[str, str], dict] = {}
    funcs: dict[tuple[str, str], str] = {}
    for mod in MODULES:
        p = docdir / mod / f"{mod}.index"
        if not p.exists():
            continue
        root = ET.parse(p).getroot()
        for c in root.iter("class"):
            classes[c.get("name")] = {"brief": c.get("brief") or "", "href": c.get("href")}
            for ch in c:
                if ch.tag == "property":
                    props[(c.get("name"), ch.get("name"))] = {"brief": ch.get("brief") or "", "href": ch.get("href")}
                elif ch.tag == "function":
                    sig = ch.get("signature")
                    if sig:
                        funcs[(c.get("name"), sig)] = ch.get("href")
    return classes, props, funcs


def class_html(docdir: Path, href: str) -> str | None:
    for mod in MODULES:
        p = docdir / mod / href.split("#")[0]
        if p.exists():
            return p.read_text(errors="replace")
    return None


MEMBER_RE = re.compile(r'<h3 class="fn"[^>]*\bid="([^"]+)"[^>]*>.*?</h3>(.*?)(?=<h3 class="fn"|<!-- @@@|<h2)', re.S)


def parse_class_html(h: str, cname: str, classes_lc: dict[str, str]):
    brief_m = re.search(r"<!-- \$\$\$%s-brief -->\s*<p>(.*?)</p>" % re.escape(cname), h, re.S)
    brief_html = re.sub(r'<a href="#details">More\.\.\.</a>', "", brief_m.group(1)) if brief_m else ""
    desc_m = re.search(r'<h2 id="details">Detailed Description</h2>(.*?)<!-- @@@%s' % re.escape(cname), h, re.S)
    members = {}
    for m in MEMBER_RE.finditer(h):
        members.setdefault(m.group(1), m.group(2))
    return brief_html, (desc_m.group(1) if desc_m else ""), members


def qss_table(docdir: Path) -> dict[str, dict]:
    p = docdir / "qtwidgets" / "stylesheet-reference.html"
    if not p.exists():
        return {}
    h = p.read_text(errors="replace")
    out = {}
    for m in re.finditer(r'<tr[^>]*><td >\s*<a href="(q\w+)\.html"[^>]*>(\w+)</a>\s*<span id="(\w+-widget)"></span></td><td >(.*?)</td></tr>', h, re.S):
        row = m.group(4)
        subs = sorted(set(re.findall(r"stylesheet-reference\.html#([\w-]+)-sub\"", row)))
        pss = sorted(set(re.findall(r"stylesheet-reference\.html#([\w-]+)-ps\"", row)))
        out[m.group(2)] = {
            "url": BASE_URL + "stylesheet-reference.html#" + m.group(3),
            "sub_controls": ["::" + s for s in subs],
            "pseudo_states": [":" + s for s in pss],
            "box_model": "box model" in row,
        }
    return out


def sentence(md: str) -> str:
    return D.brief_of(md, 200)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--docdir", default="/usr/share/doc/qt6")
    ap.add_argument("--out", default=str(D.ROOT / "metadata/docs/qt"))
    ap.add_argument("--mode", choices=("links", "brief", "full"), default="brief")
    args = ap.parse_args()
    docdir = Path(args.docdir)

    idx_classes, idx_props, idx_funcs = parse_indexes(docdir)
    classes_lc = {v["href"][:-5]: k for k, v in idx_classes.items() if v["href"] and v["href"].endswith(".html")}
    qss = qss_table(docdir)
    have_html = any((docdir / m).glob("qpushbutton.html") for m in MODULES)

    docs: dict[str, dict] = {}
    for f in D.kind_files(TOOLKIT):
        meta = svmeta.load_json(f)
        cname = meta["native"]["name"]
        ic = idx_classes.get(cname)
        entry: dict = {
            "schema": D.SCHEMA,
            "toolkit": TOOLKIT,
            "id": meta["id"],
            "license": D.LICENSES[TOOLKIT]["spdx"],
            "mode": args.mode,
            "doc": None,
            "properties": {},
            "members": {},
            "qss": qss.get(cname),
            "source": {"qdoc": {"version": "index", "files": [m + ".index" for m in MODULES]}, "html": have_html},
        }
        h = class_html(docdir, ic["href"]) if (ic and have_html and args.mode != "links") else None
        brief_html, desc_html, members_html = parse_class_html(h, cname, classes_lc) if h else ("", "", {})
        if ic:
            doc = {"url": BASE_URL + ic["href"], "index_brief": ic["brief"]}
            if brief_html:
                doc["brief"] = sentence(html_to_md(brief_html, classes_lc))
            else:
                doc["brief"] = ic["brief"]
            if args.mode == "full" and desc_html:
                doc["md"] = html_to_md(desc_html, classes_lc)
            entry["doc"] = doc
        for pr in meta["properties"]:
            ip = idx_props.get((cname, pr["name"]))
            if not ip:
                continue
            e = {"url": BASE_URL + ip["href"], "brief": ip["brief"]}
            anchor = ip["href"].split("#")[-1]
            if args.mode == "full" and anchor in members_html:
                e["md"] = html_to_md(members_html[anchor], classes_lc)
            entry["properties"][pr["name"]] = e
        for m in meta["methods"] + meta["signals"] + meta["constructors"]:
            sig = m.get("signature") or m.get("native")
            href = idx_funcs.get((cname, sig))
            if not href:
                continue
            e = {"url": BASE_URL + href}
            anchor = href.split("#")[-1]
            frag = members_html.get(anchor)
            if frag and args.mode != "links":
                md = html_to_md(frag, classes_lc)
                first = [p for p in re.split(r"\n\s*\n", md) if p.strip() and not p.startswith(("**Access", "```", "#", "Note"))]
                if first:
                    e["brief"] = sentence(first[0])
                if args.mode == "full":
                    e["md"] = md
            entry["members"][sig] = e
        docs[f.stem] = entry

    changed, total = D.write_all(Path(args.out), docs)
    n = sum(1 for d in docs.values() if d["doc"])
    print(f"qt docs ({args.mode}): {total} classes ({n} with description), {changed} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
