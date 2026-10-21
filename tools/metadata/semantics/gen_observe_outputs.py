#!/usr/bin/env python3
"""The observability outputs of milestone R2 (plans/research/REACTIVITY.md sections 8.3-8.4, 9.3).

Inputs (nothing else; output is byte-identical on rerun):
  metadata/semantics/{gtk,qt,appkit,winui}.json  (props with their `observe` facts, bindings; gen_semantics.py)
  metadata/observe/{gtk,qt}.json                  (facts of the non-GUI types; tools/metadata/common/observe.py)
  packages/types/{gtk,qt}/manifest.json + the generated class files (module exports, getter -> native prop)
Outputs:
  metadata/semantics/types/{gtk,qt}.json   the compiler's table of non-GUI classes for `native_read_not_observable`
                                           (svelters_compiler::semantics::TypeTable)
  packages/native-runtime/types/observe.d.ts  `bind:<prop>` and `bind:<prop>|nonreactive` per element tag and
                                           platform, with `@observable` / `Snapshot: not reactive` remarks

The `bind:` policy here mirrors svelters_compiler::semantics::plan_bind (decision 13 #8: `exact` lowers onto
observation, read-only observable props bind without an annotation, `partial` binds with a warning, a prop
without a fact binds only through a declared signal plan, `set_only`/`none`/`constant` do not bind;
`|nonreactive` binds every readable prop). crates/svelters-compiler/tests/r2_observe.rs checks that the two agree.

  python3 tools/metadata/semantics/gen_observe_outputs.py           # write
  python3 tools/metadata/semantics/gen_observe_outputs.py --check   # fail if an output is stale
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SEM = ROOT / "metadata/semantics"
OBSERVE = ROOT / "metadata/observe"
TYPES = ROOT / "packages/types"
DTS = ROOT / "packages/native-runtime/types/observe.d.ts"
TOOLKITS = ["gtk", "qt", "appkit", "winui"]
TYPE_TOOLKITS = ["gtk", "qt"]
TYPES_SCHEMA = "svelters-semantics-types/v0"
# svelters:qt/<module> -> the Qt module of the observe facts' owner keys
QT_MODULES = {"core": "QtCore", "gui": "QtGui", "widgets": "QtWidgets"}
# A property-backed getter: `getProp` (plain), or R1's reactive `observed` / `observedLocal` (set_only).
# `observedSignal` accessors have no native property behind them and are left out.
GETTER = re.compile(r"^  get (\w+)\(\) \{\n    return .*?\b(?:getProp|observed|observedLocal)\(this, '([^']+)'\)", re.M)

CAUSES = {
    "user_edit": "user edits",
    "programmatic": "programmatic changes",
    "cross_prop": "side effects of setting other properties",
    "parent": "changes of the parent",
    "layout": "layout",
    "wm": "the window manager",
    "async": "asynchronous changes",
}


def load(p: Path) -> dict:
    return json.loads(p.read_text())


# ---------------------------------------------------------------------------------------------
# metadata/semantics/types/<tk>.json


def owner_key(tk: str, module: str, native: str) -> str:
    if tk == "gtk":
        ns, _, name = native.partition(".")
        return f"{ns}:{name}"
    return f"{QT_MODULES[module.rsplit('/', 1)[1]]}:{native}"


def type_table(tk: str) -> dict:
    manifest = load(TYPES / tk / "manifest.json")
    facts = load(OBSERVE / f"{tk}.json")["facts"]
    by_native: dict[str, str] = {}
    entries: list[tuple[str, str, str, dict]] = []
    for module, m in sorted(manifest["modules"].items()):
        for export, e in sorted(m["exports"].items()):
            key = owner_key(tk, module, e["native"])
            by_native[e["native"]] = key
            entries.append((module, export, key, e))
    modules: dict[str, dict[str, str]] = {}
    types: dict[str, dict] = {}
    for module, export, key, e in entries:
        modules.setdefault(module, {})[export] = key
        getters = dict((g, n) for g, n in GETTER.findall((TYPES / tk / e["file"]).read_text()))
        own = facts.get(key, {})
        props = {}
        for g, native in sorted(getters.items()):
            f = own.get(native)
            if not f:
                continue  # inherited (the parent's entry has it) or no fact (unknown)
            rec = {"native": native, "class": f["class"]}
            if f.get("missed"):
                rec["missed"] = f["missed"]
            if f.get("value_cost", "cheap") != "cheap":
                rec["value_cost"] = f["value_cost"]
            props[g] = rec
        parents = [by_native[p] for p in [e.get("extends"), *e.get("implements", [])] if p and p in by_native]
        types[key] = {"parents": parents, "props": props}
    return {"schema": TYPES_SCHEMA, "toolkit": tk, "modules": modules, "types": dict(sorted(types.items()))}


def render_json(d: dict) -> str:
    # one module and one type per line, like the semantic specs
    out = ["{", f' "schema": {json.dumps(d["schema"])},', f' "toolkit": {json.dumps(d["toolkit"])},', ' "modules": {']
    mods = list(d["modules"].items())
    for i, (m, ex) in enumerate(mods):
        out.append(f"  {json.dumps(m)}: {json.dumps(ex, sort_keys=True)}{',' if i < len(mods) - 1 else ''}")
    out.append(" },")
    out.append(' "types": {')
    ts = list(d["types"].items())
    for i, (k, v) in enumerate(ts):
        out.append(f"  {json.dumps(k)}: {json.dumps(v, sort_keys=True)}{',' if i < len(ts) - 1 else ''}")
    out.append(" }")
    out.append("}")
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------------------------
# packages/native-runtime/types/observe.d.ts

RECORDS = {"Point", "Rect", "Size", "Color", "Insets"}


def ts_type(p: dict) -> str:
    t = p["type"]
    if t == "bool":
        s = "boolean"
    elif t in ("i32", "u32", "i64", "f64"):
        s = "number"
    elif t == "string":
        s = "string"
    elif t == "enum":
        s = " | ".join(json.dumps(v) for v in p["values"]) if p.get("values") else "string"
    elif t == "flags":
        s = f"Array<{' | '.join(json.dumps(v) for v in p['values'])}>" if p.get("values") else "string[]"
    elif t == "list":
        s = "string[]" if p.get("of") == "string" else "unknown[]"
    elif t == "record":
        s = p["of"] if p.get("of") in RECORDS else "unknown"
    elif t == "object":
        s = "number"
    else:
        s = "unknown"
    return f"{s} | null" if p.get("nullable") and s != "unknown" else s


def missed_text(missed: list[str]) -> str:
    w = [CAUSES.get(c, c) for c in missed]
    if not w:
        return "some changes"
    return w[0] if len(w) == 1 else f"{', '.join(w[:-1])} and {w[-1]}"


def chain_props(spec: dict, kind: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for k in kind["chain"]:
        for js, p in spec["kinds"].get(k, {}).get("props", {}).items():
            out.setdefault(js, p)
    return out


def chain_binding_props(spec: dict, kind: dict) -> set[str]:
    return {b["property"] for k in kind["chain"] for b in spec["kinds"].get(k, {}).get("bindings", {}).values()}


def bind_doc(tk: str, p: dict, declared: bool) -> str | None:
    """The remark of a plain `bind:<prop>`, or None when the compiler rejects it (plan_bind)."""
    o = p.get("observe")
    cls = o["class"] if o else None
    mech = f" ({o['mechanism']})" if o and o.get("mechanism") else ""
    ro = "" if p["writable"] else "Read-only. "
    if cls == "exact":
        return f"{ro}@observable exact on {tk}{mech}"
    if cls == "partial":
        return f"{ro}@observable partial on {tk}: misses {missed_text(o.get('missed', []))}{mech}"
    if cls is None and declared:
        return f"{ro}Bound through its change signal on {tk} (no observe fact)"
    return None


def once_doc(tk: str, p: dict) -> str:
    o = p.get("observe")
    when = "after the first layout pass" if not p["writable"] else "after creation, then write-only"
    extra = ""
    if o and o["class"] == "none":
        extra = f"; native changes are not announced on {tk}"
    elif o is None:
        extra = f"; observability unknown on {tk}"
    return f"@remarks Snapshot: not reactive. Read once {when}{extra}"


def keys_of(tk: str, spec: dict, props: dict[str, dict], declared: set[str]) -> dict[str, str]:
    """`bind:` key -> its member line (doc, type) for the readable props in `props`."""
    out = {}
    for js, p in sorted(props.items()):
        if not p["readable"]:
            continue
        t = ts_type(p)
        doc = bind_doc(tk, p, js in declared)
        if doc:
            out["bind:" + js] = f"/** {doc} */ {json.dumps('bind:' + js)}?: {t};"
        out[f"bind:{js}|nonreactive"] = f"/** {once_doc(tk, p)} */ {json.dumps('bind:' + js + '|nonreactive')}?: {t};"
    return out


def ident(tk: str, kid: str) -> str:
    return f"{tk[0].upper() + tk[1:]}Obs_{re.sub(r'[^A-Za-z0-9]', '_', kid)}"


def dts() -> str:
    o = [
        "// @generated by tools/metadata/semantics/gen_observe_outputs.py from metadata/semantics/<tk>.json. Do not edit.\n",
        "// `bind:<prop>` (lowered onto observation where the prop is `exact`) and `bind:<prop>|nonreactive` (a snapshot)\n",
        "// per element tag and platform (plans/research/REACTIVITY.md sections 8.2-8.4). One interface per kind with its own\n",
        "// props; a tag's type composes its chain (nearest first: a prop redeclared lower in the chain is omitted there).\n",
        "import type { Color, Insets, Point, Rect, Size } from './values';\n",
    ]
    for tk in TOOLKITS:
        spec = load(SEM / f"{tk}.json")
        kinds = spec["kinds"]
        used: set[str] = set()
        for kid in spec["tags"].values():
            used.update(k for k in kinds[kid]["chain"] if k in kinds)
        own_keys: dict[str, dict[str, str]] = {}
        o.append(f"\n// ---- {tk}\n")
        for kid in sorted(used):
            k = kinds[kid]
            own_keys[kid] = keys_of(tk, spec, k["props"], chain_binding_props(spec, k))
            if not own_keys[kid]:
                continue
            o.append(f"interface {ident(tk, kid)} {{\n")
            o.extend(f"  {line}\n" for line in own_keys[kid].values())
            o.append("}\n")
        o.append(f"export interface {tk[0].upper() + tk[1:]}ObservedBindings {{\n")
        for tag, kid in sorted(spec["tags"].items()):
            kind = kinds[kid]
            chain = [k for k in kind["chain"] if k in kinds]
            parts, seen = [], set()
            for k in chain:
                keys = set(own_keys[k])
                if not keys:
                    continue
                dup = sorted(keys & seen)
                seen |= keys
                parts.append(ident(tk, k) if not dup else f"Omit<{ident(tk, k)}, {' | '.join(json.dumps(d) for d in dup)}>")
            expected = keys_of(tk, spec, chain_props(spec, kind), chain_binding_props(spec, kind))
            extra = [line for key, line in expected.items() if key not in seen]
            if extra:
                parts.append("{ " + " ".join(extra) + " }")
            o.append(f"  {json.dumps(tag)}: {' & '.join(parts) if parts else '{}'};\n")
        o.append("}\n")
    return "".join(o)


def outputs() -> dict[Path, str]:
    out = {SEM / "types" / f"{tk}.json": render_json(type_table(tk)) for tk in TYPE_TOOLKITS}
    out[DTS] = dts()
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="exit 1 if a checked-in output is stale")
    a = ap.parse_args()
    stale = []
    for p, text in outputs().items():
        if p.exists() and p.read_text() == text:
            continue
        stale.append(str(p.relative_to(ROOT)))
        if not a.check:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
    if a.check and stale:
        print(f"gen_observe_outputs: stale {stale}; run python3 tools/metadata/semantics/gen_observe_outputs.py", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
