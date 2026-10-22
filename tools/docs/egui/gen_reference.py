#!/usr/bin/env python3
"""The egui widget reference: one Markdown page per kind, generated from the egui semantic spec.

    python3 tools/docs/egui/gen_reference.py            # write docs/egui/reference/widgets/*.md
    python3 tools/docs/egui/gen_reference.py --check    # fail (exit 1) when a generated region is stale

    # a spec fragment (an app's `kinds.json` from `svelters_egui::dump`): pages for its own kinds
    python3 tools/docs/egui/gen_reference.py --fragment .svelters/app/kinds.json --out docs/kinds \\
        --widget-link ../../../svelters/docs/egui/reference/widgets

Input: `metadata/semantics/egui.json` (props, events, methods, bindings, the style table and the docs
fields `category`, `states`, `a11y`, `paint`, `css_note` from `crates/svelters-egui/src/kinds/docs.rs`)
and the theme's base sheet (`crates/svelters-egui/src/theme/base.css`) for the default look.

A page is hand-editable. The generator owns only the text between
`<!-- BEGIN GENERATED: <region> ... -->` and `<!-- END GENERATED -->`, plus the front-matter keys
`title`, `tag`, `kind` and `category`; everything else is kept as written. A missing page is created
from a template whose hand-written sections are TODO markers. A page that lost a region gets it back
under its heading.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = ROOT / "metadata/semantics/egui.json"
BASE_CSS = ROOT / "crates/svelters-egui/src/theme/base.css"
OUT = ROOT / "docs/egui/reference/widgets"
GEN = "tools/docs/egui/gen_reference.py"

CATEGORIES = [
    ("inputs", "Inputs", "Fields and controls that edit a value."),
    ("buttons", "Buttons", "Things the user clicks to act."),
    ("display", "Display", "Text, icons and indicators that show something."),
    ("containers", "Containers", "Elements that hold other elements and add behaviour or chrome."),
    ("layout", "Layout", "Containers that only arrange their children."),
    ("views", "Views", "Lists and tables over app data."),
    ("menus", "Menus", "Menu bars, menu buttons, menus and their items."),
    ("overlays", "Overlays", "Popups, dialogs, inner windows and notifications."),
    ("pickers", "Pickers", "Buttons that open a picker for a date or colour."),
    ("media", "Media", "Images and drawing surfaces."),
    ("custom", "Custom kinds", "Kinds defined in Rust with `#[svelters::widget]`."),
    ("base", "Bases", "Abstract kinds whose props every element of a family has."),
]

# Paint properties of the CSS engine, in table order.
PAINT_PROPS = ["background-color", "border-color", "border-width", "border-radius", "color", "box-shadow", "opacity"]

# Section order of a page: (region, heading). `None` regions are hand-written only.
SECTIONS = [
    ("summary", None),
    ("props", "Props"),
    ("events", "Events"),
    ("methods", "Methods"),
    ("css", "CSS"),
    ("states", "States"),
    ("a11y", "Accessibility"),
]

BEGIN_RE = re.compile(r"<!-- BEGIN GENERATED: ([a-z0-9-]+)\b[^>]*-->")
END = "<!-- END GENERATED -->"
FM_KEYS = ("title", "tag", "kind", "category")


# ---- spec access ----


class Spec:
    """The built-in spec plus, optionally, a fragment whose kinds get the pages."""

    def __init__(self, builtin: dict, fragment: dict | None = None):
        self.builtin = builtin
        self.kinds: dict[str, dict] = dict(builtin["kinds"])
        self.tags: dict[str, str] = dict(builtin["tags"])
        self.fragment = fragment
        if fragment:
            self.kinds.update(fragment.get("kinds", {}))
            self.tags.update(fragment.get("tags", {}))
            self.documented = list(fragment.get("kinds", {}))
        else:
            self.documented = list(builtin["kinds"])
        self.tag_of = {k: t for t, k in self.tags.items()}
        self.style = builtin.get("style", {})

    def page_name(self, kind_id: str) -> str:
        tag = self.tag_of.get(kind_id)
        return tag if tag else kebab(kind_id)

    def is_builtin(self, kind_id: str) -> bool:
        return kind_id in self.builtin["kinds"]


def kebab(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


# ---- text helpers ----


def clean(doc: str) -> str:
    """A doc string for a page: drops parentheticals that name the drawing library's types (the
    element's API stays, how it's drawn today goes), and joins lines."""
    doc = re.sub(r"\s*\((?=[^()]*::)(?![^()]*svelters)[^()]*\)", "", doc or "")
    doc = re.sub(r"\s+", " ", doc).strip()
    return doc


def cell(text: str) -> str:
    return text.replace("|", "\\|").replace("\n", " ")


def first_sentence(doc: str) -> str:
    m = re.match(r"(.+?[.!?])(\s|$)", doc)
    return (m.group(1) if m else doc).strip()


def ts_type(t: str, prop: dict | None = None) -> str:
    if prop is not None and prop.get("ts"):
        return prop["ts"]
    if prop is not None and t == "enum" and prop.get("values"):
        return " | ".join(f"'{v}'" for v in prop["values"])
    if t == "list":
        of = (prop or {}).get("of")
        return {"string": "string[]", "i64": "number[]"}.get(of, "unknown[]")
    return {"bool": "boolean", "f64": "number", "i64": "number", "string": "string", "enum": "string"}.get(t, "unknown")


def default_text(v) -> str:
    if isinstance(v, float) and v.is_integer() and abs(v) < 1e15:
        v = int(v)
    return "`null`" if v is None else f"`{json.dumps(v)}`"


def region(name: str, kind_id: str, body: str) -> str:
    return f"<!-- BEGIN GENERATED: {name} of kinds.{kind_id} ({GEN}). Do not edit. -->\n{body.rstrip()}\n{END}"


# ---- base.css: the theme's default look per tag ----


def theme_rules(css: str) -> dict[str, list[tuple[str, list[tuple[str, str]]]]]:
    """`tag -> [(selector, [(property, value)])]` for the base sheet's rules whose selector is a tag,
    optionally with pseudo-classes (`button`, `button:hover`)."""
    css = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    out: dict[str, list] = {}
    for sel, body in re.findall(r"([^{}@;]+)\{([^{}]*)\}", css):
        decls = [(p.strip(), v.strip()) for p, _, v in (d.partition(":") for d in body.split(";")) if p.strip() and v.strip()]
        for s in (x.strip() for x in sel.split(",")):
            m = re.fullmatch(r"([a-z][a-z0-9-]*)((?::[a-z-]+)*)", s)
            if m:
                out.setdefault(m.group(1), []).append((s, decls))
    return out


# ---- the regions ----


def chain_of(spec: Spec, k: dict) -> list[str]:
    return [c for c in k.get("chain", [])]


def summary(spec: Spec, kid: str, k: dict, link) -> str:
    lines = [clean(k.get("doc", "")) or "_No description._", ""]
    facts = []
    bases = chain_of(spec, k)[1:]
    if bases:
        facts.append("**Base:** " + " → ".join(link(b) for b in bases))
    ch = k.get("children")
    if ch:
        slot = ch["slots"][ch["default_slot"]]
        acc = slot.get("accepts") or []
        if acc == ["Widget"]:
            facts.append("**Children:** any element")
        else:
            facts.append("**Children:** " + ", ".join(link(a) for a in acc))
    elif not k.get("abstract"):
        facts.append("**Children:** none")
    tc = k.get("text_content")
    if tc:
        facts.append(f"**Text content:** sets `{tc}`")
    elif not k.get("abstract"):
        facts.append("**Text content:** no")
    if k.get("attaches_to"):
        facts.append("**Attaches to:** the element it is declared in")
    if k.get("lifetime") == "toplevel":
        facts.append("**Top level:** yes")
    if k.get("abstract"):
        facts.append("**Abstract:** documents the props its family shares; not written as a tag")
    if facts:
        lines.append(" · ".join(facts))
    return "\n".join(lines)


def binding_text(k: dict, name: str, p: dict) -> str:
    b = k.get("bindings", {}).get(name)
    if b:
        return f"**yes**, on `{b['event']}`"
    obs = p.get("observe", {})
    if obs.get("class") == "exact" and obs.get("mechanism") == "widget":
        return "**yes**"
    return "no"


def props_table(k: dict) -> list[str]:
    rows = ["| Prop | Type | Default | `bind:` | Description |", "|---|---|---|---|---|"]
    for name, p in sorted(k.get("props", {}).items()):
        t = ts_type(p.get("type", "other"), p)
        if p.get("nullable") and "null" not in t:
            t += " | null"
        req = " **required**" if p.get("required") else ""
        rows.append(f"| `{name}`{req} | `{cell(t)}` | {cell(default_text(p.get('default')))} | {binding_text(k, name, p)} | {cell(clean(p.get('doc', '')))} |")
    return rows


def props(spec: Spec, kid: str, k: dict, link) -> str:
    out: list[str] = []
    own = k.get("props", {})
    if own:
        out += props_table(k)
    else:
        out.append("No props of its own.")
    for b in chain_of(spec, k)[1:]:
        bk = spec.kinds.get(b, {})
        n = len(bk.get("props", {}))
        if not n:
            continue
        if b == "Widget":
            out += ["", f"Plus the {n} props every element has, from {link(b, '#props')}: " + ", ".join(f"`{x}`" for x in sorted(bk['props'])) + "."]
        elif b == "MenuEntry":
            out += ["", f"Plus the {n} props every menu entry has, from {link(b, '#props')}: " + ", ".join(f"`{x}`" for x in sorted(bk['props'])) + "."]
        else:
            out += ["", f"From {link(b)}:", ""] + props_table(bk)
    return "\n".join(out)


def handler(s: dict, provider: bool = False) -> str:
    # a provider kind's event has one payload, named `value` in its typings (`kinds.d.ts`)
    args = [f"{'value' if provider else a['name'].replace('-', '_')}: {ts_type(a['type'])}" for a in s.get("args", [])]
    if s.get("sync"):
        types = ", ".join(ts_type(a["type"]) for a in s.get("args", []))
        args = [f"e: SvlEvent<[{types}]>"] + args
    return f"({', '.join(args)}) => void"


def events(spec: Spec, kid: str, k: dict, link) -> str:
    sig = k.get("signals", {})
    out: list[str] = []
    if sig:
        out += ["| Attribute | Handler | When |", "|---|---|---|"]
        for name, s in sorted(sig.items()):
            when = clean(s.get("doc", ""))
            if s.get("sync"):
                when += " **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first."
            out.append(f"| `on{name}` | `{cell(handler(s, k.get('category') == 'custom'))}` | {cell(when)} |")
        out += ["", f"`on:{next(iter(sorted(sig)))}` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop."]
    else:
        out.append("No events of its own.")
    for b in chain_of(spec, k)[1:]:
        bs = spec.kinds.get(b, {}).get("signals", {})
        if bs:
            out += ["", f"Plus the events of {link(b, '#events')}: " + ", ".join(f"`on{x}`" for x in sorted(bs)) + "."]
    return "\n".join(out)


def methods(spec: Spec, kid: str, k: dict, link) -> str:
    ms = k.get("methods", {})
    if not ms:
        return "None."
    out = ["| Method | Returns | Description |", "|---|---|---|"]
    for name, m in sorted(ms.items()):
        ret = m.get("returns", {}).get("type", "void")
        out.append(f"| `{name}` | `{ret}` | {cell(clean(m.get('doc', '')))} |")
    first = sorted(ms)[0]
    out += ["", f"Call them through `bind:this`: `el.call('{first}')`."]
    return "\n".join(out)


PAINT_LINE = {
    "stock": "**Painted as:** a themed control. `background-color`, `border-*` and `color` set its look in each "
    "interaction state (normal, `:hover`, `:active`); outer `box-shadow` and `opacity` apply.",
    "box": "**Painted as:** a box. The CSS engine paints its shadows, background and border at its rect, under "
    "its content; `opacity` fades it and its content.",
    "provider": "**Painted as:** a box under the kind's own drawing. The CSS engine paints shadows, background and "
    "border at its rect; the kind's `ui` draws the rest.",
    "none": "**Painted as:** nothing CSS reaches. No paint property applies to the element itself; style its "
    "children instead.",
}


def paint_applies(paint: str, prop: str, container: bool) -> str:
    if paint == "none":
        return "no"
    if prop == "box-shadow":
        return "outer shadows only" if paint == "stock" else "yes (outer and inset)"
    if prop == "color":
        if paint == "stock":
            return "yes (text and marks)"
        return "inherited by its children" if container else "yes (its text)"
    if prop == "background-color" and paint == "stock":
        return "yes, per state"
    return "yes"


def css(spec: Spec, kid: str, k: dict, link, theme: dict) -> str:
    tag = spec.tag_of.get(kid)
    paint = k.get("paint", "box")
    container = bool(k.get("children"))
    out = []
    if tag:
        out.append(f"**Selector:** `{tag}`, plus `.class`, `#id` and attribute selectors on its props.")
    out += ["", PAINT_LINE.get(paint, "")] if out else [PAINT_LINE.get(paint, "")]
    rules = theme.get(tag or "", [])
    base = dict(next((d for s, d in rules if s == tag), []))
    out += ["", "| Property | Applies | Theme default |", "|---|---|---|"]
    hidden = set(k.get("css_hidden", []))
    for p in PAINT_PROPS:
        applies = "no: its own drawing covers it" if p in hidden else paint_applies(paint, p, container)
        out.append(f"| `{p}` | {applies} | {('`' + cell(base[p]) + '`') if p in base else '—'} |")
    variants = [(s, d) for s, d in rules if s != tag]
    if variants:
        out += ["", "Theme rules for its states: " + "; ".join(f"`{s}` sets " + ", ".join(f"`{p}: {v}`" for p, v in d) for s, d in variants) + "."]
    # layout: the style table's setters whose tags list this element, and the engine's own
    props_ = spec.style.get("properties", {})
    setters = sorted(n for n, p in props_.items() if p.get("via") == "setter" and tag and tag in p.get("tags", []))
    if setters:
        holds = "padding" in setters
        engine = ["display", "grid-column", "grid-row", "margin-bottom", "margin-left", "margin-right", "margin-top"]
        if holds:
            engine += ["grid-template-columns", "grid-template-rows", "padding-bottom", "padding-left", "padding-right", "padding-top"]
        out += [
            "",
            "**Layout properties** (as attributes or in CSS; an explicit attribute wins): " + ", ".join(f"`{s}`" for s in setters) + ".",
            "Also in CSS only: " + ", ".join(f"`{s}`" for s in sorted(engine)) + ".",
        ]
    if k.get("css_note"):
        out += ["", clean(k["css_note"])]
    out += ["", "Full matrix: [css/properties](../../css/properties.md). States: below."]
    return "\n".join(out)


def states(spec: Spec, kid: str, k: dict, link) -> str:
    st = k.get("states", [])
    out = []
    if st:
        out += ["| Pseudo-class | Set when | Source |", "|---|---|---|"]
        for s in st:
            pc = " / ".join(f"`{p}`" for p in s["pseudo"])
            if s.get("negated"):
                pc += " (" + ", ".join(f"`{p}`" for p in s["negated"]) + " is the opposite)"
            out.append(f"| {pc} | {cell(s['when'])} | {s['source']} |")
    else:
        out.append("No built-in state is set on it.")
    pcs = spec.style.get("pseudo_classes", [])
    have = {s["name"] for s in st}
    missing = [f"`:{p['name']}`" for p in pcs if not p["negated"] and p.get("state") and p["state"] not in have]
    if k.get("abstract"):
        if st:
            out += ["", "These are the states the walk can set on any element; each element's page lists the ones it gets."]
    elif missing:
        out += ["", "Never set on it (rules with them don't match): " + ", ".join(missing) + "."]
    if k.get("category") == "custom" or k.get("paint") == "provider":
        out += ["", "The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`."]
    if st:
        out += ["", "States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme)."]
    return "\n".join(out)


def a11y(spec: Spec, kid: str, k: dict, link) -> str:
    a = k.get("a11y")
    if not a:
        return "Not documented."
    rows = ["| | |", "|---|---|"]
    role = f"`{a['role']}`" if a.get("role") else "none of its own; its content is announced in place"
    for v in a.get("variants", []):
        role += f"; `{v['role']}` when {v['when']}"
    rows.append(f"| Role | {cell(role)} |")
    if a.get("name_from"):
        names = ", else ".join(n if (" " in n or n.startswith('"')) else f"`{n}`" for n in a["name_from"])
        rows.append(f"| Name | {cell(names)} |")
    if a.get("value"):
        rows.append(f"| Value | {cell(a['value'])} |")
    if a.get("actions"):
        rows.append("| Actions | " + cell(", ".join(x if " " in x else f"`{x}`" for x in a["actions"])) + " |")
    if a.get("invalid"):
        rows.append("| `invalid` | the `invalid` prop sets AccessKit `invalid` |")
    if a.get("author_id"):
        rows.append(f"| Author id | `{a['author_id']}` (unique in its root) |")
    elif not k.get("abstract"):
        rows.append("| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |")
    out = rows
    if a.get("note"):
        out += ["", clean(a["note"])]
    return "\n".join(out)


# ---- pages ----


def link_fn(spec: Spec, widget_link: str | None):
    def link(kid: str, anchor: str = "") -> str:
        name = spec.page_name(kid)
        label = f"`{spec.tag_of[kid]}`" if kid in spec.tag_of else f"`{kid}`"
        if spec.fragment and spec.is_builtin(kid):
            if widget_link is None:
                return label
            return f"[{label}]({widget_link.rstrip('/')}/{name}.md{anchor})"
        return f"[{label}]({name}.md{anchor})"

    return link


def regions(spec: Spec, kid: str, theme: dict, widget_link: str | None) -> dict[str, str]:
    k = spec.kinds[kid]
    link = link_fn(spec, widget_link)
    return {
        "summary": region("summary", kid, summary(spec, kid, k, link)),
        "props": region("props", kid, props(spec, kid, k, link)),
        "events": region("events", kid, events(spec, kid, k, link)),
        "methods": region("methods", kid, methods(spec, kid, k, link)),
        "css": region("css", kid, css(spec, kid, k, link, theme)),
        "states": region("states", kid, states(spec, kid, k, link)),
        "a11y": region("a11y", kid, a11y(spec, kid, k, link)),
    }


def front_matter(spec: Spec, kid: str) -> dict[str, str]:
    k = spec.kinds[kid]
    tag = spec.tag_of.get(kid)
    fm = {"title": tag or kebab(kid), "tag": tag or "", "kind": kid, "category": k.get("category", "custom")}
    return fm


def yaml_str(v: str) -> str:
    return v if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ._/-]*", v) else json.dumps(v)


def new_page(spec: Spec, kid: str, regs: dict[str, str], source: str) -> str:
    fm = front_matter(spec, kid)
    k = spec.kinds[kid]
    title = f"`<{fm['tag']}>`" if fm["tag"] else f"`{kid}` (base)"
    desc = first_sentence(clean(k.get("doc", ""))) or kid
    lines = ["---"]
    lines += [f"{key}: {yaml_str(fm[key])}" for key in FM_KEYS if fm[key] or key != "tag"]
    lines += [
        f"description: {json.dumps(desc)}",
        "status: stub",
        "verified_at: ~",
        "generated: partial   # regions between BEGIN/END GENERATED markers come from the spec",
        "sources:",
        f"  - {source}#kinds.{kid}",
        "---",
        "",
        f"# {title}",
        "",
        regs["summary"],
        "",
        "<!-- TODO(DOC7): when to use it, and what to use instead. -->",
        "",
        "## Example",
        "",
        "<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->",
    ]
    for name, heading in SECTIONS[1:]:
        lines += ["", f"## {heading}", "", regs[name]]
        if name == "methods":
            lines += ["", "## Behaviour you need to know", "", "<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->"]
    lines += [
        "",
        "## Testing",
        "",
        "<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->",
        "",
        "## See also",
        "",
        "<!-- TODO(DOC7) -->",
        "",
    ]
    return "\n".join(lines)


def update_page(text: str, spec: Spec, kid: str, regs: dict[str, str]) -> str:
    # front matter: the generated keys only
    fm = front_matter(spec, kid)
    m = re.match(r"---\n(.*?\n)---\n", text, re.S)
    if m:
        body = m.group(1)
        for key in FM_KEYS:
            if not fm[key] and key == "tag":
                continue
            line = f"{key}: {yaml_str(fm[key])}"
            pat = re.compile(rf"^{key}:.*$", re.M)
            body = pat.sub(lambda _m: line, body, count=1) if pat.search(body) else body + line + "\n"
        text = f"---\n{body}---\n" + text[m.end():]
    # the regions
    present = []
    pos = 0
    out = []
    while True:
        b = BEGIN_RE.search(text, pos)
        if not b:
            out.append(text[pos:])
            break
        e = text.find(END, b.end())
        if e < 0:
            raise SystemExit(f"{kid}: a GENERATED region `{b.group(1)}` has no END marker")
        name = b.group(1)
        out.append(text[pos:b.start()])
        out.append(regs.get(name, text[b.start():e + len(END)]))
        present.append(name)
        pos = e + len(END)
    text = "".join(out)
    # regions the page lost: back under their heading, else after the previous region
    for i, (name, heading) in enumerate(SECTIONS):
        if name in present:
            continue
        h = re.search(rf"^## {re.escape(heading)}[ \t]*$", text, re.M) if heading else None
        if h:
            rest = re.compile(r"\s*").match(text, h.end()).end()
            text = text[:h.end()] + "\n\n" + regs[name] + "\n\n" + text[rest:]
        else:
            prev = [n for n, _ in SECTIONS[:i] if n in present]
            if prev:
                at = text.find(END, text.find(f"BEGIN GENERATED: {prev[-1]} ")) + len(END)
            else:
                t = re.search(r"^# .*$", text, re.M)
                at = t.end() if t else len(text)
            block = ("\n\n" + regs[name]) if not heading else (f"\n\n## {heading}\n\n" + regs[name])
            text = text[:at] + block + text[at:]
        present.append(name)
    return text


def index_page(spec: Spec, existing: str | None) -> str:
    groups: dict[str, list[str]] = {}
    for kid in spec.documented:
        groups.setdefault(spec.kinds[kid].get("category", "custom"), []).append(kid)
    body = []
    for cat, title, blurb in CATEGORIES:
        kids = sorted(groups.get(cat, []), key=spec.page_name)
        if not kids:
            continue
        body += [f"## {title}", "", blurb, "", "| Element | What it is |", "|---|---|"]
        for kid in kids:
            name = spec.page_name(kid)
            label = f"`<{spec.tag_of[kid]}>`" if kid in spec.tag_of else f"`{kid}`"
            body.append(f"| [{label}]({name}.md) | {cell(first_sentence(clean(spec.kinds[kid].get('doc', ''))))} |")
        body.append("")
    reg = f"<!-- BEGIN GENERATED: index ({GEN}). Do not edit. -->\n" + "\n".join(body).rstrip() + f"\n{END}"
    if existing and "BEGIN GENERATED: index" in existing:
        b = existing.index("<!-- BEGIN GENERATED: index")
        e = existing.index(END, b) + len(END)
        return existing[:b] + reg + existing[e:]
    return "\n".join([
        "---",
        "title: Widget reference",
        'description: "Every element: props, events, methods, CSS, states and accessibility, by category."',
        "status: stub",
        "verified_at: ~",
        "generated: partial",
        "---",
        "",
        "# Widget reference",
        "",
        "One page per element. The tables on each page are generated from the kind table; the prose around them is",
        "written by hand. Regenerate with `python3 tools/docs/egui/gen_reference.py`.",
        "",
        reg,
        "",
    ])


def build(spec: Spec, out: Path, theme: dict, source: str, widget_link: str | None) -> dict[Path, str]:
    """Every page's wanted content (existing pages updated in place)."""
    want: dict[Path, str] = {}
    for kid in spec.documented:
        path = out / f"{spec.page_name(kid)}.md"
        regs = regions(spec, kid, theme, widget_link)
        if path.exists():
            want[path] = update_page(path.read_text(), spec, kid, regs)
        else:
            want[path] = new_page(spec, kid, regs, source)
    idx = out / "index.md"
    want[idx] = index_page(spec, idx.read_text() if idx.exists() else None)
    return want


def stray(out: Path, want: dict[Path, str]) -> list[Path]:
    """Generated pages (`generated: partial` with a `kind:`) whose kind is gone."""
    if not out.is_dir():
        return []
    return [p for p in sorted(out.glob("*.md")) if p not in want and re.search(r"^generated: partial", p.read_text(), re.M) and re.search(r"^kind: ", p.read_text(), re.M)]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--spec", type=Path, default=SPEC, help="the built-in egui spec (default: %(default)s)")
    ap.add_argument("--fragment", type=Path, help="a spec fragment (`kinds.json`): document its kinds only")
    ap.add_argument("--out", type=Path, help="output directory (default: docs/egui/reference/widgets; required with --fragment)")
    ap.add_argument("--base-css", type=Path, default=BASE_CSS, help="the theme's base sheet (default: %(default)s)")
    ap.add_argument("--widget-link", help="with --fragment: the path or URL of the built-in widget pages, for links to `Widget` etc.")
    ap.add_argument("--check", action="store_true", help="write nothing; exit 1 when a page is missing or a generated region is stale")
    a = ap.parse_args(argv)
    if a.fragment and not a.out:
        ap.error("--fragment needs --out")
    builtin = json.loads(a.spec.read_text())
    fragment = json.loads(a.fragment.read_text()) if a.fragment else None
    spec = Spec(builtin, fragment)
    out = a.out or OUT
    theme = theme_rules(a.base_css.read_text()) if a.base_css.exists() else {}
    if fragment:
        source = a.fragment.name
    else:
        try:
            source = str(a.spec.resolve().relative_to(ROOT))
        except ValueError:
            source = a.spec.name
    want = build(spec, out, theme, source, a.widget_link)
    gone = stray(out, want)
    if a.check:
        stale = [p for p, t in want.items() if not p.exists() or p.read_text() != t]
        for p in stale:
            print(f"stale: {p}", file=sys.stderr)
        for p in gone:
            print(f"no such kind any more: {p}", file=sys.stderr)
        if stale or gone:
            print(f"{len(stale) + len(gone)} reference page(s) out of date: run python3 {GEN}" + (" with the same arguments" if a.fragment else ""), file=sys.stderr)
            return 1
        return 0
    out.mkdir(parents=True, exist_ok=True)
    n = 0
    for p, t in want.items():
        if not p.exists() or p.read_text() != t:
            p.write_text(t)
            n += 1
    for p in gone:
        print(f"note: {p} documents a kind that no longer exists; delete it", file=sys.stderr)
    print(f"{n} page(s) written, {len(want) - n} unchanged ({out})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
