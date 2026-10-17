#!/usr/bin/env python3
"""Generate metadata/semantics/dom.json: a `svelters-semantics/v0` spec for the HTML DOM.

This backend exists only for conformance testing: experiments/native-compiler/conformance runs
Svelte's own runtime tests (tests/runtime-runes) with the NC1 compiler against a DOM host
(jsdom), so the semantics are the browser's, written down here by hand (no upstream metadata).

What the spec says:
  - one kind per HTML tag below, chain [tag, "HTMLElement"]; the abstract HTMLElement carries the
    global attributes and the DOM events; every non-void kind has a default `children` slot
    (accepts elements and text); void elements take no children;
  - attributes are props named as in markup (`class` is a token list, as GTK `cssClasses`; bool
    attributes are `bool`; everything else is a string); the host decides attribute vs property;
  - events: `on<event>` -> signal `<event>` with one arg (the DOM Event); `<event>capture` is the
    capture-phase listener (Svelte's `onclickcapture`);
  - bindings: input/textarea `value` (input event), select `value` and input `checked` (change
    event), details `open` (toggle); value read back from the node (`readback`);
  - extensions used only by this spec (semantic-spec.md): `text_node` (text runs become `#text`
    nodes whose `data` holds the text) and `html_syntax` (HTML parsing rules: void elements,
    implicit closing).

  python3 tools/metadata/semantics/dom/gen_dom.py           # write
  python3 tools/metadata/semantics/dom/gen_dom.py --check   # fail if dom.json is stale
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / "metadata/semantics/dom.json"
SCHEMA = "svelters-semantics/v0"
PROVENANCE = "HTML Living Standard (elements, global attributes, events); conformance backend, tools/metadata/semantics/dom/gen_dom.py"

# Global attributes (HTML "global attributes" + ARIA basics + data-* names the runtime-runes
# samples use; a data-* wildcard would need a spec extension, which this backend avoids).
GLOBAL = {
    "id": "string", "class": "list", "style": "string", "title": "string", "lang": "string",
    "dir": "string", "hidden": "bool", "tabindex": "string", "contenteditable": "string",
    "draggable": "string", "translate": "string", "role": "string", "inert": "bool",
    "autofocus": "bool", "is": "string", "accesskey": "string", "spellcheck": "string",
    "aria-label": "string", "aria-hidden": "string", "aria-current": "string",
    "data-id": "string", "data-foo": "string", "data-item": "string", "data-kind": "string",
    "data-resolved": "string", "data-testid": "string", "data-value": "string",
}

FORM = {"name": "string", "disabled": "bool", "form": "string"}

ELEMENTS: dict[str, dict[str, str]] = {
    # sections / grouping / text-level
    **{t: {} for t in [
        "div", "span", "p", "section", "article", "aside", "header", "footer", "main", "nav",
        "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "dl", "dt", "dd", "pre",
        "blockquote", "figure", "figcaption", "strong", "em", "b", "i", "u", "s", "small",
        "code", "kbd", "samp", "mark", "sub", "sup", "abbr", "cite", "q", "time", "summary",
        "fieldset", "legend", "table", "thead", "tbody", "tfoot", "tr", "caption", "address",
    ]},
    "td": {"colspan": "string", "rowspan": "string"},
    "th": {"colspan": "string", "rowspan": "string", "scope": "string"},
    "a": {"href": "string", "target": "string", "rel": "string", "download": "string"},
    "label": {"for": "string"},
    "details": {"open": "bool"},
    "dialog": {"open": "bool"},
    "form": {"action": "string", "method": "string", "novalidate": "bool", "noValidate": "bool"},
    "button": {**FORM, "type": "string", "value": "string", "formnovalidate": "bool"},
    "input": {**FORM, "type": "string", "value": "string", "checked": "bool", "placeholder": "string",
              "readonly": "bool", "required": "bool", "min": "string", "max": "string", "step": "string",
              "multiple": "bool", "defaultValue": "string", "defaultChecked": "bool", "autocomplete": "string",
              "list": "string", "pattern": "string", "size": "string", "accept": "string",
              "maxlength": "string", "indeterminate": "bool"},
    "textarea": {**FORM, "value": "string", "defaultValue": "string", "placeholder": "string",
                 "readonly": "bool", "required": "bool", "rows": "string", "cols": "string"},
    "select": {**FORM, "value": "string", "multiple": "bool", "required": "bool", "size": "string"},
    "option": {"value": "string", "selected": "bool", "disabled": "bool", "label": "string", "defaultSelected": "bool"},
    "optgroup": {"label": "string", "disabled": "bool"},
    "progress": {"value": "string", "max": "string"},
    "img": {"src": "string", "alt": "string", "width": "string", "height": "string", "loading": "string"},
    "iframe": {"src": "string", "title": "string", "width": "string", "height": "string"},
    "video": {"src": "string", "controls": "bool", "autoplay": "bool", "loop": "bool", "muted": "bool", "playsinline": "bool"},
    "audio": {"src": "string", "controls": "bool", "autoplay": "bool", "loop": "bool", "muted": "bool"},
    "br": {}, "hr": {}, "wbr": {},
    "meta": {"name": "string", "content": "string"},
    "track": {"src": "string", "kind": "string", "default": "bool"},
    "source": {"src": "string", "type": "string"},
}

VOID = {"input", "br", "hr", "wbr", "img", "meta", "track", "source"}

EVENTS = [
    "click", "dblclick", "contextmenu", "auxclick", "mousedown", "mouseup", "mousemove", "mouseover",
    "mouseout", "mouseenter", "mouseleave", "pointerdown", "pointerup", "pointermove", "pointerenter",
    "pointerleave", "pointerover", "pointerout", "pointercancel", "touchstart", "touchend", "touchmove",
    "keydown", "keyup", "keypress", "input", "beforeinput", "change", "focus", "blur", "focusin",
    "focusout", "submit", "reset", "invalid", "select", "scroll", "wheel", "load", "error", "toggle",
    "animationstart", "animationend", "transitionend", "dragstart", "drag", "dragend", "drop",
]
CAPTURE = ["click", "mousedown", "mouseup", "keydown", "keyup", "input", "change", "focus", "blur", "pointerdown"]

BINDINGS = {
    "input": {"value": "input", "checked": "change"},
    "textarea": {"value": "input"},
    "select": {"value": "change"},
    "details": {"open": "toggle"},
}


def prop(name: str, ty: str) -> dict:
    p = {"construct_only": False, "default": None, "js": name, "native": name, "nullable": True,
         "readable": True, "type": ty, "writable": True}
    if ty == "list":
        p.update({"of": "string", "ref": "list<string>"})
    return p


def signal(name: str) -> dict:
    return {"args": [{"name": "event", "type": "object"}], "disconnect_on_drop": True,
            "handler_result": "none", "js_event": name, "name": name, "sync_required": False}


def binding(prop_name: str, event: str) -> dict:
    return {"compare_before_set": True, "direction": "two_way", "echo": "runtime_compare", "event": event,
            "lifetime": {"on_destroy": "drop", "on_native_death": "drop", "on_stale_update": "ignore"},
            "natively_normalizes": False, "property": prop_name, "provenance": PROVENANCE,
            "read_back": False, "sync_signals_on_set": [], "value": "readback"}


def kind(native: str, chain: list[str], *, abstract=False, props=None, signals=None, bindings=None, children=None) -> dict:
    return {"abstract": abstract, "bindings": bindings or {}, "chain": chain, "children": children,
            "interface": False, "lifetime": "owned", "methods": [], "native": native, "present": None,
            "props": props or {}, "signals": signals or {}, "text_content": None}


def build() -> dict:
    signals = {e: signal(e) for e in EVENTS}
    signals.update({f"{e}capture": signal(f"{e}capture") for e in CAPTURE})
    children = {"default_slot": "children", "policy": "dom",
                "slots": {"children": {"accepts": ["HTMLElement", "#text"], "many": True, "order": "before_sibling", "route_by_kind": False}}}
    kinds = {
        "HTMLElement": kind("HTMLElement", ["HTMLElement"], abstract=True, props={n: prop(n, t) for n, t in GLOBAL.items()},
                            signals=signals),
        "#text": kind("Text", ["#text"], props={"data": prop("data", "string")}),
    }
    tags = {"#text": "#text"}
    for tag, attrs in sorted(ELEMENTS.items()):
        tags[tag] = tag
        b = {name: binding(name, ev) for name, ev in BINDINGS.get(tag, {}).items()}
        # slots are looked up along the chain, so each non-void kind declares its own
        kinds[tag] = kind(f"HTML{tag}", [tag, "HTMLElement"], props={n: prop(n, t) for n, t in attrs.items()}, bindings=b,
                          children=None if tag in VOID else children)
    return {
        "schema": SCHEMA, "backend": "dom", "framework": "HTML DOM (jsdom), conformance testing only",
        "source": {"generator": "tools/metadata/semantics/dom/gen_dom.py", "provenance": PROVENANCE},
        "event_aliases": {}, "tags": tags, "kinds": kinds, "style": {"dialect": "none", "setter_selectors": ["type", "class", "id"], "units": {}, "properties": {}},
        "element_hints": {}, "text_node": {"kind": "#text", "prop": "data"}, "html_syntax": True,
    }


def compact(v) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def render(doc: dict) -> str:
    """Same layout as gen_semantics.py: top-level keys one per line, kinds/tags one entry per line."""
    lines = ["{"]
    keys = sorted(doc)
    for i, k in enumerate(keys):
        comma = "," if i < len(keys) - 1 else ""
        v = doc[k]
        if k in ("kinds", "tags") and isinstance(v, dict) and v:
            lines.append(f"{compact(k)}:{{")
            sub = sorted(v)
            for j, sk in enumerate(sub):
                lines.append(f"  {compact(sk)}:{compact(v[sk])}{',' if j < len(sub) - 1 else ''}")
            lines.append("}" + comma)
        else:
            lines.append(f"{compact(k)}:{compact(v)}{comma}")
    lines.append("}")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    text = render(build())
    if args.check:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT.relative_to(ROOT)} is stale: run {Path(__file__).relative_to(ROOT)}", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(text)
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
