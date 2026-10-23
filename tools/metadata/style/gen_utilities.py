#!/usr/bin/env python3
"""Generate the utility-class tables (plans/research/UTILITY-CSS.md, lane U0) from the hand-written
vocabulary metadata/style/utilities.yaml, the Tailwind name list metadata/style/tailwind-v4.yaml and
the egui semantic spec metadata/semantics/egui.json (its style table, pseudo-classes, media features
and provider states, each kind's states and parts, and the theme's token manifest; written by
crates/svelters-egui/src/spec.rs).

Outputs:
  crates/svelters-style/src/utilities.generated.rs        the engine's table (U1 resolves classes from it)
  packages/style/src/utilities.generated.json             the manifest (tools, MCP, the IntelliSense shim)
  packages/style/src/utilities.generated.js               the same as an ES module
  packages/style/src/utilities.completions.generated.json editor completions (default theme)
  docs/egui/reference/utilities.md                        the reference page

  python3 tools/metadata/style/gen_utilities.py            # write
  python3 tools/metadata/style/gen_utilities.py --check    # fail if an output is stale or a check fails
  python3 tools/metadata/style/gen_utilities.py --report   # also print the to-do: table properties no row names

The checks (UTILITY-CSS §3.6; any failure is an error):
  - every row's properties are in the egui style table (a row with `requires:` waits as pending);
  - every value source fits the property's type, every keyword is one of an enum property's values
    or a literal of the property's type, every `var(--sv-*)` names a token;
  - no two rows generate the same utility, and no concrete name falls under another row's numeric
    pattern (`border-2` against a colour named `2`);
  - every Tailwind name in tailwind-v4.yaml is ours or has a reason, every namespace of ours is in
    the Tailwind list (or marked `own`), and no utility we generate falls under an unsupported name;
  - variant names are unique, aliases name states, media queries use known features.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[3]
SPEC = ROOT / "metadata/semantics/egui.json"
VOCAB = ROOT / "metadata/style/utilities.yaml"
TAILWIND = ROOT / "metadata/style/tailwind-v4.yaml"
OUT_RS = ROOT / "crates/svelters-style/src/utilities.generated.rs"
OUT_JSON = ROOT / "packages/style/src/utilities.generated.json"
OUT_JS = ROOT / "packages/style/src/utilities.generated.js"
OUT_COMPLETIONS = ROOT / "packages/style/src/utilities.completions.generated.json"
OUT_DOCS = ROOT / "docs/egui/reference/utilities.md"
GEN = "tools/metadata/style/gen_utilities.py"
SCHEMA = "svelters-utilities/v0"

NAME = re.compile(r"-?[a-z0-9@*]+(-[a-z0-9]+)*")
VAR = re.compile(r"var\(--sv-([a-z0-9-]+)\)")
# value sources and what they generate
OPEN_SOURCES = {"fraction", "int-px", "number", "percent", "integer", "arbitrary"}
# property types a source fits (the table's `type`, or `decl_type` for `via: css`)
FITS = {
    "fraction": {"length", "insets"},
    "int-px": {"length", "insets"},
    "number": {"number"},
    "percent": {"number"},
    "integer": {"raw", "length", "number", "transition"},
    "arbitrary": None,  # any: checked against the property's type when used
}
# token types a scale may feed into a property type
SCALE_FITS = {
    "length": {"length", "insets", "raw"},
    "color": {"color"},
    "shadow": {"shadow"},
    "duration": {"raw", "transition"},
    "easing": {"raw", "transition"},
    "font-family": {"raw"},
}
# sample values of the open sources, for completions and docs only (the resolver takes any)
SAMPLES = {
    "fraction": ["1/2", "1/3", "2/3", "1/4", "3/4"],
    "int-px": ["0", "2", "4"],
    "number": ["0"],
    "percent": ["0", "25", "50", "75", "100"],
    "integer": ["1", "2", "3", "4", "6", "12"],
}
PATTERNS = {
    "fraction": r"\d+/\d+",
    "int-px": r"\d+",
    "number": r"\d+(\.\d+)?",
    "percent": r"\d+",
    "integer": r"\d+",
}
SETS = ["dark", "light", "hc-dark", "hc-light"]


class UtilError(Exception):
    pass


def fail(msg: str):
    raise UtilError(msg)


def ptype(p: dict) -> str:
    """A table property's value type (`type` of a setter, `decl_type` of a `via: css` entry)."""
    return p.get("type") or p.get("decl_type") or "raw"


def literal_ok(name: str, ty: str, prop: dict, v: str, tokens: dict) -> bool:
    """Whether a fixed CSS value is plausible for a property of type `ty` (the engine's parser has
    the last word, U1's tests; this catches the vocabulary's typos)."""
    for t in VAR.findall(v):
        if t not in tokens:
            return False
    if VAR.search(v):
        return True
    if ty == "enum":
        return v in prop.get("values", [])
    if ty in ("length", "insets"):
        # `letter-spacing: normal` (the engine's length keywords)
        return re.fullmatch(r"auto|0|-?\d+(\.\d+)?(px|%)", v) is not None or (v == "normal" and name == "letter-spacing")
    if ty == "number":
        return re.fullmatch(r"-?\d+(\.\d+)?%?", v) is not None
    if ty == "color":
        return v in ("transparent", "currentColor")
    if ty == "shadow":
        return v == "none"
    if ty == "font-weight":
        return v in ("normal", "bold") or re.fullmatch(r"[1-9]00", v) is not None
    if ty == "bool":
        return v in ("true", "false")
    return True  # raw, transition, image, content


def token_value(t: dict, s: str = "dark") -> str:
    return t["value"] if "value" in t else t["values"][s]


# ---- scales ----

def resolve_scale(name: str, rec: dict, tokens: dict) -> list[tuple[str, str]]:
    """A scale's (utility value name, token) pairs for the given token manifest, in order."""
    w = f"utilities.yaml scales.{name}"
    if rec.get("type") and not rec.get("tokens"):
        out = [(t, t) for t, d in sorted(tokens.items()) if d["type"] == rec["type"]]
        if not out:
            fail(f"{w}: no token of type {rec['type']}")
        return out
    out: list[tuple[str, str]] = []
    exclude = [re.compile("^" + re.escape(e).replace(r"\*", "(.+)") + "$") for e in rec.get("exclude", [])]
    for pat in rec.get("tokens") or fail(f"{w}: needs tokens or a type"):
        if isinstance(pat, dict):
            tok, as_ = pat.get("token"), pat.get("as")
            if tok not in tokens:
                fail(f"{w}: token {tok!r} is not in the theme manifest")
            if as_ is None:
                fail(f"{w}: {{token: {tok}}} needs `as`")
            matches = [(as_, tok)]
        else:
            if pat.count("*") != 1:
                fail(f"{w}: pattern {pat!r} needs one `*` (or use {{token, as}})")
            rx = re.compile("^" + re.escape(pat).replace(r"\*", "(.+)") + "$")
            matches = [(m.group(1), t) for t in sorted(tokens) if (m := rx.match(t)) and not any(e.match(t) for e in exclude)]
            if rec.get("type"):
                matches = [(n, t) for n, t in matches if tokens[t]["type"] == rec["type"]]
            if not matches:
                fail(f"{w}: pattern {pat!r} matches no token")
        for n, t in matches:
            if any(n == x for x, _ in out):
                fail(f"{w}: two tokens are named {n!r} ({t} and {dict(out)[n]})")
            out.append((n, t))
    return out


def scale_type(name: str, pairs: list[tuple[str, str]], tokens: dict) -> str:
    types = sorted({tokens[t]["type"] for _, t in pairs})
    if len(types) != 1:
        fail(f"utilities.yaml scales.{name}: tokens of mixed types {types}")
    return types[0]


# ---- the model ----

def build(vocab: dict, tailwind: dict, spec: dict) -> dict:
    """Check the vocabulary against the spec and Tailwind's list; return the model every output is
    rendered from. Raises UtilError on the first failure."""
    style = spec["style"]
    table = style["properties"]
    tokens = spec["theme"]["tokens"]
    for k in set(vocab) - {"config", "scales", "utilities", "variants"}:
        fail(f"utilities.yaml: unknown section {k!r}")

    # scales
    scales = {}
    for name, rec in (vocab.get("scales") or {}).items():
        if set(rec) - {"tokens", "exclude", "type"}:
            fail(f"utilities.yaml scales.{name}: unknown field(s) {sorted(set(rec) - {'tokens', 'exclude', 'type'})}")
        pairs = resolve_scale(name, rec, tokens)
        scales[name] = {
            "patterns": [p if isinstance(p, dict) else {"glob": p} for p in rec.get("tokens", [])],
            "exclude": rec.get("exclude", []),
            "token_type": rec.get("type") or scale_type(name, pairs, tokens),
            "values": pairs,
        }

    rows = []
    names: dict[str, int] = {}  # concrete utility name -> row index
    used_props: set[str] = set()
    for i, r in enumerate(vocab.get("utilities") or []):
        fixed = "utility" in r
        nm = r.get("utility") if fixed else r.get("ns")
        w = f"utilities.yaml utilities[{i}] ({nm})"
        allowed = {"utility", "decls", "family", "requires", "own"} if fixed else {"ns", "props", "values", "keywords", "negative", "family", "requires", "own"}
        if set(r) - allowed:
            fail(f"{w}: unknown field(s) {sorted(set(r) - allowed)}")
        if not nm or not NAME.fullmatch(nm):
            fail(f"{w}: a name is kebab-case")
        if not r.get("family"):
            fail(f"{w}: needs a family")
        requires = r.get("requires")
        props = list(r["decls"]) if fixed else r.get("props") or fail(f"{w}: needs props")
        missing = [p for p in props if p not in table]
        if missing and not requires:
            fail(f"{w}: property {missing[0]!r} is not in the egui style table")
        pending = bool(missing)
        used_props.update(p for p in props if p in table)
        row = {"name": nm, "fixed": fixed, "family": r["family"], "props": props, "rank": i, "requires": requires, "pending": pending, "own": bool(r.get("own"))}
        concrete: list[tuple[str, dict]] = []  # (utility name, decls)
        if fixed:
            decls = {k: str(v) for k, v in r["decls"].items()}
            for p, v in decls.items():
                if p in table and not literal_ok(p, ptype(table[p]), table[p], v, tokens):
                    fail(f"{w}: {p}: {v!r} is not a {ptype(table[p])} value of {p}")
                for t in VAR.findall(v):
                    if t not in tokens:
                        fail(f"{w}: {p}: var(--sv-{t}) names no token")
            row["decls"] = decls
            concrete.append((nm, decls))
        else:
            types = [ptype(table[p]) for p in props if p in table]
            values = []
            for v in r.get("values") or []:
                if isinstance(v, dict):
                    if set(v) != {"integer"} or "{n}" not in v["integer"]:
                        fail(f"{w}: a templated source is {{integer: '<css with {{n}}>'}}")
                    src = {"kind": "integer", "template": v["integer"]}
                elif v in scales:
                    src = {"kind": "scale", "scale": v}
                elif v in FITS:
                    src = {"kind": v}
                else:
                    fail(f"{w}: unknown value source {v!r}")
                if not pending:
                    for p, ty in zip(props, types):
                        if src["kind"] == "scale":
                            st = scales[src["scale"]]["token_type"]
                            if ty not in SCALE_FITS.get(st, set()):
                                fail(f"{w}: scale {src['scale']} ({st} tokens) doesn't fit {p}, a {ty} property")
                        elif FITS[src["kind"]] is not None and ty not in FITS[src["kind"]]:
                            fail(f"{w}: value source {src['kind']} doesn't fit {p}, a {ty} property")
                        if src["kind"] == "percent" and (table[p].get("range") or {}).get("max") != 1:
                            fail(f"{w}: percent needs a property ranged 0 to 1 ({p})")
                values.append(src)
            row["values"] = values
            kw = r.get("keywords")
            keywords: dict[str, list[str]] = {}
            if kw == "table":
                enum = [ptype(table[p]) == "enum" for p in props if p in table]
                if not enum or not all(enum):
                    fail(f"{w}: `keywords: table` needs enum properties")
                vals = table[props[0]]["values"]
                if any(table[p]["values"] != vals for p in props):
                    fail(f"{w}: `keywords: table` over properties with different keywords")
                keywords = {k: [k] * len(props) for k in vals}
            elif isinstance(kw, dict):
                for k, v in kw.items():
                    k = str(k)
                    vs = [str(x) for x in v] if isinstance(v, list) else [str(v)] * len(props)
                    if len(vs) != len(props):
                        fail(f"{w}: keyword {k!r} needs one value per property ({len(props)})")
                    if not pending:
                        for p, x in zip(props, vs):
                            if not literal_ok(p, ptype(table[p]), table[p], x, tokens):
                                if ptype(table[p]) == "enum":
                                    fail(f"{w}: keyword {k!r}: {x!r} is not one of {p}'s values {table[p]['values']}")
                                fail(f"{w}: keyword {k!r}: {x!r} is not a {ptype(table[p])} value of {p}")
                    keywords[k] = vs
            elif kw is not None:
                fail(f"{w}: keywords is a map or `table`")
            if not values and not keywords:
                fail(f"{w}: needs values or keywords")
            row["keywords"] = keywords
            if r.get("negative"):
                if pending or any(t not in ("length", "insets") for t in types):
                    fail(f"{w}: only length properties take negative values")
            row["negative"] = bool(r.get("negative"))
            # the concrete names (default theme): keywords, then scale values; sample open values
            for k, vs in keywords.items():
                concrete.append((f"{nm}-{k}", dict(zip(props, vs))))
            for src in values:
                if src["kind"] == "scale":
                    for vn, tok in scales[src["scale"]]["values"]:
                        concrete.append((f"{nm}-{vn}" if vn else nm, {p: f"var(--sv-{tok})" for p in props}))
            row["patterns"] = sorted({PATTERNS[s["kind"]] for s in values if s["kind"] in PATTERNS})
        row["concrete"] = concrete
        if not pending:
            for n, _ in concrete:
                if n in names:
                    fail(f"utilities.yaml: utility {n!r} comes from two rows ({rows[names[n]]['name']} #{names[n]} and {nm} #{i})")
                names[n] = i
        rows.append(row)

    # numeric patterns against concrete names: `border-2` must not also be a colour named `2`
    for r in rows:
        if r["fixed"] or r["pending"] or not r["patterns"]:
            continue
        rx = re.compile("^" + re.escape(r["name"]) + "-(" + "|".join(r["patterns"]) + ")$")
        for n, i in names.items():
            if not rx.match(n):
                continue
            if i == r["rank"]:
                fail(f"utilities.yaml: {n!r} is both a named value and a number of row {r['name']} #{i}")
            fail(f"utilities.yaml: {n!r} (row {rows[i]['name']} #{i}) also matches the number pattern of row {r['name']} #{r['rank']}")
    ns_rows: dict[str, list[int]] = {}
    for r in rows:
        if not r["fixed"]:
            ns_rows.setdefault(r["name"], []).append(r["rank"])
    for ns, idx in ns_rows.items():
        pats = [p for i in idx for p in rows[i]["patterns"]]
        overlap = {p for p in pats if pats.count(p) > 1} | ({PATTERNS["int-px"], PATTERNS["number"]} if PATTERNS["int-px"] in pats and PATTERNS["number"] in pats else set())
        if overlap:
            fail(f"utilities.yaml: two rows of namespace {ns!r} take numbers ({', '.join(sorted(overlap))})")

    ours_utils = {r["name"] for r in rows}

    # ---- Tailwind ----
    tw_utils, unsupported_utils = check_tailwind_list("utilities", tailwind.get("utilities") or [])
    for n in tw_utils:
        if n not in ours_utils:
            fail(f"tailwind-v4.yaml utilities: Tailwind namespace {n!r} is neither in utilities.yaml nor given a reason")
    for u in unsupported_utils:
        if u["name"] in ours_utils:
            fail(f"tailwind-v4.yaml utilities: {u['name']!r} has a reason but utilities.yaml renders it")
        for n in names:
            if n.startswith(u["name"] + "-") and n not in u["allow"]:
                fail(f"tailwind-v4.yaml utilities: our {n!r} falls under unsupported {u['name']!r} (rename, or list it in that entry's `allow`)")
    for r in rows:
        if r["name"] not in tw_utils and not r["own"]:
            fail(f"utilities.yaml: {r['name']!r} is not a Tailwind v4 name in tailwind-v4.yaml (list it, or mark the row `own: true`)")

    # ---- variants ----
    variants = build_variants(vocab.get("variants") or {}, style, spec, scales, tokens)
    tw_vars, unsupported_vars = check_tailwind_list("variants", tailwind.get("variants") or [])
    ours_vars = variants["names"]
    for n in tw_vars:
        if n not in ours_vars:
            fail(f"tailwind-v4.yaml variants: Tailwind variant {n!r} is neither ours nor given a reason")
    for u in unsupported_vars:
        if u["name"] in ours_vars:
            fail(f"tailwind-v4.yaml variants: {u['name']!r} has a reason but is one of our variants")

    # ---- kinds ----
    kinds = {}
    for kid, k in sorted(spec["kinds"].items()):
        tag = next((t for t, i in spec["tags"].items() if i == kid), None)
        if tag is None:
            continue
        states = sorted({p.lstrip(":") for s in k.get("states", []) for p in s.get("pseudo", []) + s.get("negated", [])})
        kinds[tag] = {
            "kind": kid,
            "states": states,
            "parts": [{"name": p["name"], "node": p["node"]} for p in k.get("parts", [])],
            "provider_states": k.get("provider_states", []),
        }
    state_names = {v["name"] for v in variants["states"]}
    part_state_clashes = sorted({p["name"] for k in kinds.values() for p in k["parts"] if p["name"] in state_names})

    todo = sorted(p for p in table if p not in used_props)
    return {
        "config": vocab.get("config") or {},
        "scales": scales,
        "rows": rows,
        "variants": variants,
        "kinds": kinds,
        "provider_states": [s["name"] for s in style.get("provider_states", [])],
        "unsupported": {"utilities": unsupported_utils, "variants": unsupported_vars},
        "part_state_clashes": part_state_clashes,
        "todo": todo,
        "tokens": tokens,
        "table": table,
    }


def check_tailwind_list(section: str, entries: list) -> tuple[list[str], list[dict]]:
    plain, unsupported, seen = [], [], set()
    for e in entries:
        if isinstance(e, str):
            n = e
            plain.append(n)
        else:
            if set(e) - {"name", "reason", "use", "allow"} or not e.get("name") or not e.get("reason"):
                fail(f"tailwind-v4.yaml {section}: {e!r} needs name and reason (and optional use, allow)")
            n = e["name"]
            unsupported.append({"name": n, "reason": e["reason"], "use": e.get("use"), "allow": e.get("allow", [])})
        if n in seen:
            fail(f"tailwind-v4.yaml {section}: {n!r} is listed twice")
        seen.add(n)
    return plain, unsupported


def build_variants(v: dict, style: dict, spec: dict, scales: dict, tokens: dict) -> dict:
    allowed = {"extra_states", "pseudo_elements", "part_prefix", "state_prefix", "aliases", "structural", "compound", "prop_prefix", "media", "breakpoints", "containers"}
    if set(v) - allowed:
        fail(f"utilities.yaml variants: unknown field(s) {sorted(set(v) - allowed)}")
    names: dict[str, str] = {}

    def claim(n: str, what: str):
        if n in names:
            fail(f"utilities.yaml variants: {n!r} is both a {names[n]} and a {what} variant")
        names[n] = what

    states = []
    for pc in style["pseudo_classes"]:
        claim(pc["name"], "state")
        states.append({"name": pc["name"], "state": pc["state"], "negated": pc["negated"]})
    for n, bit in (v.get("extra_states") or {}).items():
        claim(n, "state")
        states.append({"name": n, "state": bit, "negated": False})
    state_names = {s["name"] for s in states}
    aliases = {}
    for a, t in (v.get("aliases") or {}).items():
        if t not in state_names:
            fail(f"utilities.yaml variants.aliases: {a!r} -> {t!r}, which is not a state variant")
        claim(a, "alias")
        aliases[a] = t
    pseudo_elements = dict(v.get("pseudo_elements") or {})
    for n in pseudo_elements:
        claim(n, "pseudo-element")
    structural = dict(v.get("structural") or {})
    for n, sel in structural.items():
        if "&" not in sel:
            fail(f"utilities.yaml variants.structural.{n}: the selector needs `&`")
        claim(n, "structural")
    compound = {}
    for n, rec in (v.get("compound") or {}).items():
        if "{state}" not in rec.get("selector", "") or "&" not in rec["selector"]:
            fail(f"utilities.yaml variants.compound.{n}: the selector needs `&` and `{{state}}`")
        claim(n, "compound")
        compound[n] = {"selector": rec["selector"], "requires": rec.get("requires")}
    features = {f["name"]: f for f in style.get("media_features", [])}
    media = {}
    for n, rec in (v.get("media") or {}).items():
        q, req = (rec, None) if isinstance(rec, str) else (rec.get("query"), rec.get("requires"))
        for feat, val in re.findall(r"\(([a-z-]+)\s*:\s*([a-z-]+)\)", q or ""):
            ok = feat in features and (features[feat]["type"] == "range" or val in features[feat]["values"])
            if not ok and not req:
                fail(f"utilities.yaml variants.media.{n}: ({feat}: {val}) is not a media feature the engine evaluates")
        claim(n, "media")
        media[n] = {"query": q, "requires": req, "pending": bool(req) and not all(f in features for f, _ in re.findall(r"\(([a-z-]+)\s*:\s*([a-z-]+)\)", q))}

    def sized(key: str) -> dict | None:
        rec = v.get(key)
        if not rec:
            return None
        sc = scales.get(rec.get("scale")) or fail(f"utilities.yaml variants.{key}: unknown scale {rec.get('scale')!r}")
        sizes = []
        for n, tok in sc["values"]:
            px = token_value(tokens[tok])
            if not re.fullmatch(r"\d+(\.\d+)?px", px):
                fail(f"utilities.yaml variants.{key}: {tok} is {px!r}, not a px length (media queries can't use var())")
            sizes.append({"name": n, "token": tok, "px": float(px[:-2])})
        sizes.sort(key=lambda s: (s["px"], s["name"]))  # ascending: the canonical media order
        return {"scale": rec["scale"], "min": rec.get("min"), "max": rec.get("max", rec.get("min", "").replace(">=", "<")), "max_prefix": rec.get("max_prefix"), "requires": rec.get("requires"), "sizes": sizes}

    breakpoints = sized("breakpoints")
    containers = sized("containers")
    if breakpoints:
        for s in breakpoints["sizes"]:
            claim(s["name"], "breakpoint")
        claim(breakpoints["max_prefix"].rstrip("-"), "breakpoint max")
    if containers:
        for s in containers["sizes"]:
            claim("@" + s["name"], "container")
        claim(containers["max_prefix"].rstrip("-"), "container max")
    prefixes = {k: v[k] for k in ("part_prefix", "state_prefix", "prop_prefix") if v.get(k)}
    for k, p in prefixes.items():
        claim(p.rstrip("-"), k.replace("_prefix", ""))
    return {
        "names": names,
        "states": states,
        "aliases": aliases,
        "pseudo_elements": pseudo_elements,
        "structural": structural,
        "compound": compound,
        "media": media,
        "breakpoints": breakpoints,
        "containers": containers,
        "prefixes": prefixes,
    }


# ---- outputs ----

def manifest(m: dict) -> dict:
    rows = []
    for r in m["rows"]:
        e = {"name": r["name"], "family": r["family"], "rank": r["rank"]}
        if r["fixed"]:
            e["decls"] = r["decls"]
        else:
            e["props"] = r["props"]
            if r["values"]:
                e["values"] = r["values"]
            if r["keywords"]:
                e["keywords"] = r["keywords"]
            if r["negative"]:
                e["negative"] = True
        if r["requires"]:
            e["requires"] = r["requires"]
        if r["pending"]:
            e["pending"] = True
        if r["own"]:
            e["own"] = True
        rows.append(e)
    scales = {
        n: {"patterns": s["patterns"], **({"exclude": s["exclude"]} if s["exclude"] else {}), "token_type": s["token_type"], "values": {vn: tok for vn, tok in s["values"]}}
        for n, s in m["scales"].items()
    }
    return {
        "schema": SCHEMA,
        "generator": GEN,
        "sources": ["metadata/style/utilities.yaml", "metadata/style/tailwind-v4.yaml", "metadata/semantics/egui.json"],
        "config": m["config"],
        "scales": scales,
        "utilities": rows,
        "variants": {k: v for k, v in m["variants"].items() if k != "names"},
        "kinds": m["kinds"],
        "provider_states": m["provider_states"],
        "unsupported": m["unsupported"],
    }


def render_json(m: dict) -> str:
    return json.dumps(manifest(m), indent=1, sort_keys=False) + "\n"


def render_js(m: dict) -> str:
    return (
        f"// @generated by {GEN} from metadata/style/utilities.yaml, metadata/style/tailwind-v4.yaml and\n"
        "// metadata/semantics/egui.json. Do not edit. The same data as utilities.generated.json.\n"
        "//\n"
        "// The utility-class vocabulary on egui (plans/research/UTILITY-CSS.md): rows (namespace, properties,\n"
        "// value sources, keywords) in canonical cascade order, token scales, variants, per-tag states and parts,\n"
        "// and the Tailwind v4 names we don't render, with a reason.\n"
        f"export const UTILITIES = {json.dumps(manifest(m), indent=1)};\n"
        "export default UTILITIES;\n"
    )


def expand(m: dict) -> list[dict]:
    """Every concrete utility of the default theme (open sources by sample), for completions."""
    out = []
    for r in m["rows"]:
        if r["pending"]:
            continue
        for n, decls in r["concrete"]:
            out.append({"label": n, "family": r["family"], "decls": decls})
            if r.get("negative") and not n.endswith("-0"):
                out.append({"label": "-" + n, "family": r["family"], "decls": {p: f"calc({v} * -1)" for p, v in decls.items()}})
        if r["fixed"]:
            continue
        for s in r["values"]:
            for x in SAMPLES.get(s["kind"], []):
                if s["kind"] == "fraction":
                    a, b = x.split("/")
                    v = f"{round(100 * int(a) / int(b), 6):g}%"
                elif s["kind"] == "int-px":
                    v = f"{x}px"
                elif s["kind"] == "percent":
                    v = f"{int(x) / 100:g}"
                elif s["kind"] == "integer":
                    v = s["template"].replace("{n}", x)
                else:
                    v = x
                out.append({"label": f"{r['name']}-{x}", "family": r["family"], "decls": {p: v for p in r["props"]}, "sample": True})
    return out


def render_completions(m: dict) -> str:
    tokens = m["tokens"]

    def detail(decls: dict) -> str:
        parts = []
        for p, v in decls.items():
            t = VAR.fullmatch(v)
            hint = f" /* {token_value(tokens[t.group(1)])} */" if t and "value" in tokens[t.group(1)] else ""
            parts.append(f"{p}: {v}{hint}")
        return "; ".join(parts)

    utils = [{"label": u["label"], "detail": detail(u["decls"]), "family": u["family"], **({"sample": True} if u.get("sample") else {})} for u in expand(m)]
    v = m["variants"]
    variants = [{"label": s["name"] + ":", "detail": f"&:{s['name']}"} for s in v["states"]]
    variants += [{"label": a + ":", "detail": f"&:{t} (alias)"} for a, t in v["aliases"].items()]
    variants += [{"label": n + ":", "detail": "&" + sel} for n, sel in v["pseudo_elements"].items()]
    variants += [{"label": n + ":", "detail": sel} for n, sel in v["structural"].items()]
    variants += [{"label": n + "-<state>:", "detail": c["selector"]} for n, c in v["compound"].items() if not c["requires"]]
    variants += [{"label": n + ":", "detail": f"@media {mv['query']}"} for n, mv in v["media"].items() if not mv["pending"]]
    if v["breakpoints"]:
        variants += [{"label": s["name"] + ":", "detail": "@media " + v["breakpoints"]["min"].replace("{px}", f"{s['px']:g}px")} for s in v["breakpoints"]["sizes"]]
    parts = sorted({p["name"] for k in m["kinds"].values() for p in k["parts"]})
    variants += [{"label": f"{v['prefixes']['part_prefix']}{p}:", "detail": f"&::part({p})"} for p in parts]
    variants += [{"label": f"{v['prefixes']['state_prefix']}[{s}]:", "detail": f"&:state({s})"} for s in m["provider_states"]]
    # one entry per line: diffs stay readable, the file stays small
    lines = ["{", f' "schema": {json.dumps(SCHEMA + "/completions")},', f' "generator": {json.dumps(GEN)},']
    for key, items in (("utilities", utils), ("variants", variants)):
        lines.append(f' "{key}": [')
        lines += ["  " + json.dumps(x) + ("," if i < len(items) - 1 else "") for i, x in enumerate(items)]
        lines.append(" ]," if key == "utilities" else " ]")
    lines.append("}")
    return "\n".join(lines) + "\n"


# ---- Rust ----

def rs_lit(s: str) -> str:
    """A Rust string literal: JSON escapes are Rust escapes, but non-ASCII stays UTF-8 (Rust has no \\uXXXX)."""
    return json.dumps(s, ensure_ascii=False)


def rs(s) -> str:
    return "None" if s is None else rs_lit(s)


def rs_opt(s) -> str:
    return "None" if s is None else f"Some({rs_lit(s)})"


def rs_strs(xs) -> str:
    return "&[" + ", ".join(rs_lit(x) for x in xs) + "]"


def render_rs(m: dict) -> str:
    c = m["config"]
    lint = c.get("lint", {}).get("arbitrary_color", {})
    L = [
        f"// @generated by {GEN} from metadata/style/utilities.yaml, metadata/style/tailwind-v4.yaml and",
        "// metadata/semantics/egui.json. Do not edit.",
        "",
        "use crate::utility_table::*;",
        "",
        "/// The engine settings of the vocabulary (UQ3, UQ6, UQ8).",
        "pub static CONFIG: Config = Config {",
        f"    layer: {rs(c.get('layer', 'utilities'))},",
        f"    opt_in: {'true' if c.get('opt_in', True) else 'false'},",
        f"    prefix_optional: {'true' if c.get('prefix') == 'optional' else 'false'},",
        f"    token_prefix: {rs(c.get('token_prefix', '--sv-'))},",
        f"    arbitrary_color: ({rs(lint.get('default', 'warning'))}, {rs(lint.get('strict', 'error'))}),",
        "};",
        "",
        "/// The token scales: the patterns resolve against the active theme at run time; `defaults` are",
        "/// the built-in theme's (value name, token).",
        "pub static SCALES: &[Scale] = &[",
    ]
    for n, s in m["scales"].items():
        pats = ", ".join(
            f"TokenPattern {{ glob: {rs(p.get('glob', p.get('token')))}, name: {rs_opt(p.get('as'))} }}" for p in s["patterns"]
        )
        defaults = ", ".join(f"({rs(vn)}, {rs(tok)})" for vn, tok in s["values"])
        L.append(f"    Scale {{ name: {rs(n)}, token_type: {rs(s['token_type'])}, patterns: &[{pats}], exclude: {rs_strs(s['exclude'])}, defaults: &[{defaults}] }},")
    L += ["];", "", "/// The vocabulary rows in canonical cascade order (`rank`): one namespace and property group each.", "pub static ROWS: &[Row] = &["]
    for r in m["rows"]:
        if r["fixed"]:
            body = f"kind: RowKind::Fixed {{ decls: &[{', '.join(f'({rs(p)}, {rs(v)})' for p, v in r['decls'].items())}] }}"
        else:
            vals = []
            for s in r["values"]:
                k = s["kind"]
                vals.append({
                    "scale": lambda: f"ValueSource::Scale({rs(s['scale'])})",
                    "integer": lambda: f"ValueSource::Integer({rs(s['template'])})",
                    "fraction": lambda: "ValueSource::Fraction",
                    "int-px": lambda: "ValueSource::IntPx",
                    "number": lambda: "ValueSource::Number",
                    "percent": lambda: "ValueSource::Percent",
                    "arbitrary": lambda: "ValueSource::Arbitrary",
                }[k]())
            kws = ", ".join(f"Keyword {{ name: {rs(k)}, values: {rs_strs(v)} }}" for k, v in r["keywords"].items())
            body = (
                f"kind: RowKind::Namespace {{ props: {rs_strs(r['props'])}, values: &[{', '.join(vals)}], keywords: &[{kws}], "
                f"negative: {'true' if r['negative'] else 'false'} }}"
            )
        L.append(f"    Row {{ name: {rs(r['name'])}, rank: {r['rank']}, family: {rs(r['family'])}, requires: {rs_opt(r['requires'])}, pending: {'true' if r['pending'] else 'false'}, {body} }},")
    L += ["];", ""]
    v = m["variants"]
    L.append("/// The state variants: the style table's pseudo-classes (`hover:` is `&:hover`).")
    L.append("pub static STATE_VARIANTS: &[StateVariant] = &[")
    for s in v["states"]:
        L.append(f"    StateVariant {{ name: {rs(s['name'])}, state: {rs_opt(s['state'])}, negated: {'true' if s['negated'] else 'false'} }},")
    L += ["];", "", "/// Accepted spellings of state variants (`aria-checked:` is `checked:`), with an info fix-it.", "pub static ALIASES: &[(&str, &str)] = &["]
    L += [f"    ({rs(a)}, {rs(t)})," for a, t in v["aliases"].items()]
    L += ["];", "", "/// Pseudo-element variants (`placeholder:` is `&::placeholder`).", "pub static PSEUDO_ELEMENTS: &[(&str, &str)] = &["]
    L += [f"    ({rs(n)}, {rs(s)})," for n, s in v["pseudo_elements"].items()]
    L += ["];", "", "/// Structural variants (`&` is the utility's selector).", "pub static STRUCTURAL: &[(&str, &str)] = &["]
    L += [f"    ({rs(n)}, {rs(s)})," for n, s in v["structural"].items()]
    L += ["];", "", "/// Compound prefixes over the state variants (`not-hover:`); `{state}` is the state.", "pub static COMPOUND: &[CompoundVariant] = &["]
    L += [f"    CompoundVariant {{ name: {rs(n)}, selector: {rs(c['selector'])}, requires: {rs_opt(c['requires'])} }}," for n, c in v["compound"].items()]
    L += ["];", "", "/// Media variants (hoisted outside the rule).", "pub static MEDIA: &[MediaVariant] = &["]
    L += [f"    MediaVariant {{ name: {rs(n)}, query: {rs(mv['query'])}, requires: {rs_opt(mv['requires'])}, pending: {'true' if mv['pending'] else 'false'} }}," for n, mv in v["media"].items()]
    L.append("];")
    for key, const in (("breakpoints", "BREAKPOINTS"), ("containers", "CONTAINERS")):
        b = v[key]
        L += ["", f"/// `{key}`: sizes from the `{b['scale']}` scale in px (the built-in theme; media queries can't use `var()`)." if b else f"/// No {key}."]
        if b:
            sizes = ", ".join(f"({rs(s['name'])}, {s['px']!r}_f32)" for s in b["sizes"])
            L.append(f"pub static {const}: SizedVariants = SizedVariants {{ scale: {rs(b['scale'])}, min: {rs(b['min'])}, max: {rs(b['max'])}, max_prefix: {rs(b['max_prefix'])}, requires: {rs_opt(b['requires'])}, sizes: &[{sizes}] }};")
    p = v["prefixes"]
    L += [
        "",
        "/// `part-<name>:` (`&::part(<name>)`), `state-[ns.name]:` (`&:state(ns.name)`), `prop-[name=value]:`.",
        f"pub const PART_PREFIX: &str = {rs(p.get('part_prefix', 'part-'))};",
        f"pub const STATE_PREFIX: &str = {rs(p.get('state_prefix', 'state-'))};",
        f"pub const PROP_PREFIX: &str = {rs(p.get('prop_prefix', 'prop-'))};",
        "",
        "/// svelters' own provider states (`state-[sv.overflow-top]:`); an app's come with its fragment.",
        f"pub static PROVIDER_STATES: &[&str] = {rs_strs(m['provider_states'])};",
        "",
        "/// Per element tag: the state variants its kind can match, its parts (name, node) and provider states.",
        "pub static KINDS: &[KindStyle] = &[",
    ]
    for tag, k in m["kinds"].items():
        parts = ", ".join(f"({rs(x['name'])}, {'true' if x['node'] else 'false'})" for x in k["parts"])
        L.append(f"    KindStyle {{ tag: {rs(tag)}, states: {rs_strs(k['states'])}, parts: &[{parts}], provider_states: {rs_strs(k['provider_states'])} }},")
    L += ["];", "", "/// The Tailwind v4 utilities we don't render: name prefix, reason, replacement.", "pub static UNSUPPORTED_UTILITIES: &[Unsupported] = &["]
    L += [f"    Unsupported {{ name: {rs(u['name'])}, reason: {rs(u['reason'])}, replacement: {rs_opt(u['use'])}, allow: {rs_strs(u['allow'])} }}," for u in m["unsupported"]["utilities"]]
    L += ["];", "", "/// The Tailwind v4 variants we don't render.", "pub static UNSUPPORTED_VARIANTS: &[Unsupported] = &["]
    L += [f"    Unsupported {{ name: {rs(u['name'])}, reason: {rs(u['reason'])}, replacement: {rs_opt(u['use'])}, allow: {rs_strs(u['allow'])} }}," for u in m["unsupported"]["variants"]]
    L += ["];", ""]
    return "\n".join(L)


# ---- docs ----

def md_cell(s: str) -> str:
    return s.replace("|", "\\|").replace("\n", " ")


def values_text(m: dict, r: dict) -> str:
    if r["fixed"]:
        return "; ".join(f"`{p}: {v}`" for p, v in r["decls"].items())
    parts = []
    for s in r["values"]:
        k = s["kind"]
        if k == "scale":
            sc = m["scales"][s["scale"]]
            if s["scale"] == "colors":
                parts.append("a colour token (see Colours)")
            else:
                names = ", ".join(f"`{vn}`" if vn else "(bare)" for vn, _ in sc["values"])
                parts.append(f"{names} (`{s['scale']}` scale)")
        elif k == "integer":
            parts.append(f"an integer *n*: `{s['template']}`")
        else:
            parts.append({
                "fraction": "a fraction `a/b` (percent)",
                "int-px": "an integer *n* px",
                "number": "a non-negative number",
                "percent": "an integer 0 to 100 (percent)",
                "arbitrary": "`[value]`",
            }[k])
    if r["keywords"]:
        parts.append(", ".join(f"`{k}`" for k in r["keywords"]))
    text = "; ".join(parts)
    if r["negative"]:
        text += "; negative with a leading `-`"
    return text


def render_docs(m: dict) -> str:
    tokens = m["tokens"]
    L = [
        "---",
        "title: Utility classes",
        'description: "Tailwind-style utility classes on egui: names, values from the theme tokens, variants, and what is not supported."',
        "status: generated",
        "verified_at: ~",
        "generated: full   # every line comes from the generator; edit the metadata instead",
        "sources:",
        "  - metadata/style/utilities.yaml",
        "  - metadata/style/tailwind-v4.yaml",
        "  - metadata/semantics/egui.json",
        "  - plans/research/UTILITY-CSS.md",
        "---",
        "",
        f"<!-- @generated by {GEN}. Do not edit. -->",
        "",
        "# Utility classes",
        "",
        "Tailwind v4's *names* with svelters' *values*: `p-2`, `flex items-center gap-1`, `bg-surface`,",
        "`hover:bg-list-hover`, `part-indicator:rounded-xs`. Every value is a theme token (`var(--sv-*)`), so",
        "utilities follow the colour scheme, high contrast, density and an app's own tokens. There is no",
        "Tailwind palette and no 4px spacing multiplier: `p-7` is `--sv-space-7` (32px).",
        "",
        "- **Opt-in per embed**" + (" (with an optional prefix such as `tw:`)" if m["config"].get("prefix") == "optional" else "") + ": existing semantic classes keep their meaning.",
        f"- **Cascade**: `@layer {m['config'].get('layer', 'utilities')}` at author origin, below a component's own `<style>` rules; a trailing `!` makes one important.",
        "- **Rules are generated at run time** by the host's style engine, so dynamic strings (`p-{n}`) work; the compiler checks the classes it can see.",
        "- **Arbitrary values** (`w-[120px]`) are checked against the property's type. A colour literal (`bg-[#f00]`) is a warning, an error under the strict profile.",
        "- **Variants stack left to right**: `hover:part-thumb:bg-accent` is the host hovered, `part-thumb:hover:` the part's own state.",
        "",
    ]
    families: dict[str, list] = {}
    for r in m["rows"]:
        families.setdefault(r["family"], []).append(r)
    for fam, rows in families.items():
        L += [f"## {fam.capitalize()}", "", "| Utility | Sets | Values |", "|---|---|---|"]
        for r in rows:
            name = f"`{r['name']}`" if r["fixed"] else f"`{r['name']}-*`"
            sets = ", ".join(f"`{p}`" for p in r["props"])
            note = f" *(waits for {r['requires']})*" if r["pending"] else ""
            L.append(f"| {name}{note} | {sets} | {md_cell(values_text(m, r))} |")
        L.append("")
    L += ["## Scales", "", "Each scale is a token family of the theme. An app's own tokens matching the pattern are values too.", "", "| Scale | Value | Token | Default |", "|---|---|---|---|"]
    for n, s in m["scales"].items():
        if n == "colors":
            continue
        for vn, tok in s["values"]:
            t = tokens[tok]
            default = t["value"] if "value" in t else t["values"]["dark"] + " (dark)"
            L.append(f"| `{n}` | `{vn or '(bare)'}` | `--sv-{tok}` | `{md_cell(default)}` |")
    L += ["", "## Colours", "", "`bg-*`, `text-*`, `border-*`, `border-{x,y,t,r,b,l}-*` and `outline-*` take every colour token: the", "roles (L2) and component colours (L3). Values in the built-in sets:", "", "| Name | Layer | Dark | Light | HC dark | HC light |", "|---|---|---|---|---|---|"]
    for vn, tok in m["scales"]["colors"]["values"]:
        t = tokens[tok]
        vals = [t["value"]] * 4 if "value" in t else [t["values"][s] for s in SETS]
        src = f" (`{t['vscode']}`)" if t.get("vscode") else (f" (from `{t['from']}`)" if t.get("from") else "")
        L.append(f"| `{vn}` | {t['layer']}{src} | " + " | ".join(f"`{x}`" for x in vals) + " |")
    v = m["variants"]
    L += ["", "## Variants", "", "| Variant | Selects |", "|---|---|"]
    L += [f"| `{s['name']}:` | `&:{s['name']}` |" for s in v["states"]]
    L += [f"| `{a}:` | `{t}:` (alias) |" for a, t in v["aliases"].items()]
    L += [f"| `{n}:` | `&{sel}` |" for n, sel in v["pseudo_elements"].items()]
    L += [f"| `{n}:` | `{sel}` |" for n, sel in v["structural"].items()]
    L += [f"| `{n}-<state>:` | `{md_cell(c['selector'])}`" + (f" *(waits for {c['requires']})*" if c["requires"] else "") + " |" for n, c in v["compound"].items()]
    L += [f"| `{n}:` | `@media {mv['query']}`" + (f" *(waits for {mv['requires']})*" if mv["pending"] else "") + " |" for n, mv in v["media"].items()]
    for key in ("breakpoints", "containers"):
        b = v[key]
        if not b:
            continue
        at = "@" if key == "containers" else ""
        kw = "@container" if key == "containers" else "@media"
        wait = f" *(waits for {b['requires']})*" if b["requires"] else ""
        for s in b["sizes"]:
            q = b["min"].replace("{px}", "%gpx" % s["px"])
            L.append(f"| `{at}{s['name']}:` | `{kw} {q}` (`--sv-{s['token']}`){wait} |")
        L.append(f"| `{b['max_prefix']}<size>:` | `{kw} {b['max']}`{wait} |")
    pp = v["prefixes"]
    L += [
        f"| `{pp.get('part_prefix', 'part-')}<name>:` | `&::part(<name>)`: the kinds' parts below |",
        f"| `{pp.get('state_prefix', 'state-')}[<ns>.<name>]:` | `&:state(<ns>.<name>)`: provider states |",
        f"| `{pp.get('prop_prefix', 'prop-')}[<name>=<value>]:` | `&[<name>=\"<value>\"]`: a prop as an attribute |",
        "",
        "## Parts and states by element",
        "",
        "The `part-<name>:` variants and provider states each element has (its state variants are in the widget reference).",
        "",
        "| Element | Parts (node parts marked *) | Provider states |",
        "|---|---|---|",
    ]
    for tag, k in m["kinds"].items():
        if not k["parts"] and not k["provider_states"]:
            continue
        parts = ", ".join(f"`{p['name']}`" + ("*" if p["node"] else "") for p in k["parts"]) or "–"
        st = ", ".join(f"`{s}`" for s in k["provider_states"]) or "–"
        L.append(f"| `{tag}` | {parts} | {st} |")
    if m["part_state_clashes"]:
        L += ["", "Part names that are also state names (why parts take the `part-` prefix): " + ", ".join(f"`{n}`" for n in m["part_state_clashes"]) + "."]
    L += ["", "## Not supported", "", "Tailwind v4 names svelters doesn't render. The compiler reports them (`utility_unsupported`) with the replacement.", "", "| Utility | Why | Use instead |", "|---|---|---|"]
    L += [f"| `{u['name']}` | {md_cell(u['reason'])} | {('`' + md_cell(u['use']) + '`') if u['use'] else '–'} |" for u in m["unsupported"]["utilities"]]
    L += ["", "| Variant | Why | Use instead |", "|---|---|---|"]
    L += [f"| `{u['name']}:` | {md_cell(u['reason'])} | {('`' + md_cell(u['use']) + '`') if u['use'] else '–'} |" for u in m["unsupported"]["variants"]]
    L.append("")
    return "\n".join(L)


# ---- main ----

def load_inputs() -> tuple[dict, dict, dict]:
    return (
        yaml.safe_load(VOCAB.read_text()),
        yaml.safe_load(TAILWIND.read_text()),
        json.loads(SPEC.read_text()),
    )


def outputs(m: dict) -> list[tuple[Path, str]]:
    return [
        (OUT_RS, render_rs(m)),
        (OUT_JSON, render_json(m)),
        (OUT_JS, render_js(m)),
        (OUT_COMPLETIONS, render_completions(m)),
        (OUT_DOCS, render_docs(m)),
    ]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--report", action="store_true")
    a = ap.parse_args(argv)
    try:
        m = build(*load_inputs())
    except UtilError as e:
        print(f"gen_utilities: {e}", file=sys.stderr)
        return 1
    if a.report:
        print("egui style properties no utility row names (reachable as [prop:value]):")
        for p in m["todo"]:
            print(f"  {p}")
        pending = [f"{r['name']} ({r['requires']})" for r in m["rows"] if r["pending"]]
        print("rows waiting for engine items: " + (", ".join(pending) or "none"))
    stale = []
    for out, text in outputs(m):
        if out.exists() and out.read_text() == text:
            continue
        if a.check:
            stale.append(out.relative_to(ROOT).as_posix())
        else:
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(text)
    if stale:
        print(f"gen_utilities: {', '.join(stale)} stale; run python3 {GEN}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
