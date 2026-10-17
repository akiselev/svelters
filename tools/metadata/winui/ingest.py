#!/usr/bin/env python3
"""Ingest WinUI 3 classes from the Windows App SDK winmd files into metadata/winui/*.json.

Inputs:
  metadata/overrides/winui/*.yaml   which classes to ingest (each file's `class`), with their bases
  the winmd files from fetch.py      read through `winmd-tool dump` (tools/metadata/winui/winmd-tool)
Outputs (checked in, byte-identical on rerun):
  metadata/winui/<Class>.json        one per class in the override set and its base chains
  metadata/winui/_types.json         enums, structs and event-args classes those classes mention

Only upstream facts are written here (origin: winmd). The annotations stay in the overrides and are
merged by gen_backend.py; see docs/notes/win1-winui.md for why this differs from GTK/Qt.

  python3 tools/metadata/winui/ingest.py --tool target/winmd-tool/release/winmd-tool [--check]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent / "common"))
sys.path.insert(0, str(HERE))
import svmeta  # noqa: E402
import fetch  # noqa: E402

OVERRIDES = ROOT / "metadata/overrides/winui"
OUT = ROOT / "metadata/winui"
PACKAGES = {pid: ver for pid, ver, _ in fetch.PACKAGES}


def split_generic(t: str) -> tuple[str, list[str]]:
    """`A<B, C<D>>` -> ("A", ["B", "C<D>"])."""
    if "<" not in t:
        return t, []
    head, rest = t.split("<", 1)
    rest = rest[:-1]
    args, depth, cur = [], 0, ""
    for ch in rest:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            args.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        args.append(cur.strip())
    return head, args


def type_ref(t: str, types: dict) -> dict:
    """Neutral type vocabulary of widget-metadata.md, plus the native spelling."""
    prim = {"bool": "bool", "i32": "i32", "u32": "u32", "i64": "i64", "u64": "u64", "f32": "f32", "f64": "f64",
            "string": "string", "object": "variant", "u8": "u8", "i16": "i16", "u16": "u16", "char16": "char16"}
    if t in prim:
        return {"ref": prim[t], "native": t}
    head, args = split_generic(t)
    if head == "Windows.Foundation.IReference" and len(args) == 1:
        return {"ref": f"nullable<{type_ref(args[0], types)['ref']}>", "native": t}
    if head in ("Windows.Foundation.Collections.IVector", "Windows.Foundation.Collections.IVectorView") and args:
        return {"ref": f"list<{type_ref(args[0], types)['ref']}>", "native": t}
    cat = types.get(head, {}).get("category")
    if cat == "enum":
        return {"ref": f"{'flags' if types[head].get('flags') else 'enum'}<{head}>", "native": t}
    if cat == "struct":
        return {"ref": f"value<{head}>", "native": t}
    return {"ref": f"object<{head}>", "native": t}


def chain(name: str, types: dict) -> list[str]:
    out = []
    cur = types.get(name, {}).get("extends")
    while cur and cur in types and not cur.startswith("System."):
        out.append(cur)
        cur = types[cur].get("extends")
    return out


def constructible(c: dict) -> tuple[bool, list[dict]]:
    ctors = []
    if c.get("default_constructor"):
        ctors.append({"id": "ctor:default", "native": "IActivationFactory.ActivateInstance", "origin": "winmd"})
    factory_methods = {f["interface"]: f.get("methods", []) for f in c.get("factories", [])}
    for comp in c.get("attributes", {}).get("Composable", []):
        # [Composable(factory, CompositionType, version, contract)]; CompositionType 2 = Public, 1 = Protected.
        # A public factory without CreateInstance (UIElement's) constructs nothing.
        if len(comp) > 1 and str(comp[1]).endswith(".2") and "CreateInstance" in factory_methods.get(comp[0], []):
            ctors.append({"id": "ctor:composable", "native": f"{comp[0]}.CreateInstance", "origin": "winmd"})
    return bool(ctors), ctors


def content_property(name: str, types: dict) -> str | None:
    for n in [name] + chain(name, types):
        cp = types.get(n, {}).get("attributes", {}).get("ContentProperty")
        if cp:
            return cp[0][0]
    return None


def args_info(delegate: str, types: dict) -> dict:
    head, gens = split_generic(delegate)
    if head in ("Windows.Foundation.TypedEventHandler",) and len(gens) == 2:
        sender, args = gens
    elif head == "Windows.Foundation.EventHandler" and len(gens) == 1:
        sender, args = "object", gens[0]
    else:
        d = types.get(head, {})
        ps = d.get("params", [])
        sender = ps[0]["type"] if ps else "object"
        args = ps[1]["type"] if len(ps) > 1 else "object"
    props = []
    for n in [args] + chain(args, types):
        for p in types.get(n, {}).get("props", []):
            props.append({"name": p["name"], "type": type_ref(p["type"], types), "settable": bool(p["set"]), "declared_on": n})
    methods = sorted({m["name"] for m in types.get(args, {}).get("methods", [])})
    return {"sender": sender, "args": args, "args_props": props, "args_methods": methods}


def class_kind(name: str, types: dict) -> dict:
    c = types[name]
    ns, short = name.rsplit(".", 1)
    ok, ctors = constructible(c)
    dps = set(c.get("dependency_properties", []))
    props = []
    for p in c.get("props", []):
        props.append({
            "name": p["name"],
            "type": type_ref(p["type"], types),
            "access": {"read": bool(p["get"]), "write": bool(p["set"])},
            "dependency_property": f"{p['name']}Property" in dps,
            "interface": p["interface"],
            "origin": "winmd",
        })
    signals = []
    for e in c.get("events", []):
        signals.append({"name": e["name"], "delegate": e["delegate"], "interface": e["interface"], **args_info(e["delegate"], types),
                        "origin": "winmd"})
    methods = [{"name": m["name"], "params": [{"name": p["name"], "type": type_ref(p["type"], types)} for p in m["params"]],
                "returns": type_ref(m["ret"], types), "interface": m["interface"], "origin": "winmd"}
               for m in c.get("methods", []) if "name" in m]
    statics = {m["name"]: m for m in c.get("static_methods", [])}
    attached = []
    for n, m in sorted(statics.items()):
        if n.startswith("Get") and f"Set{n[3:]}" in statics and len(m["params"]) == 1:
            attached.append({"name": n[3:], "target": type_ref(m["params"][0]["type"], types),
                             "type": type_ref(m["ret"], types), "dependency_property": f"{n[3:]}Property" in dps,
                             "accessors": {"get": n, "set": f"Set{n[3:]}"}, "origin": "winmd"})
    attrs = c.get("attributes", {})
    contract = attrs.get("ContractVersion", [[None, None]])[0]
    return {
        "schema": "svelters-element-kind/v0",
        "toolkit": "winui",
        "id": f"svelters-api:v1:type:winui:{ns}:{short}",
        "native": {"name": name, "namespace": ns, "short": short},
        "kind": "object",
        "abstract": not ok,
        "constructors": ctors,
        "bases": chain(name, types),
        "content_property": content_property(name, types),
        "content_property_declared": (attrs.get("ContentProperty") or [[None]])[0][0],
        "threading": (attrs.get("Threading") or [[None]])[0][0],
        "marshaling": (attrs.get("MarshalingBehavior") or [[None]])[0][0],
        "availability": {"contract": contract[0], "version": contract[1] if len(contract) > 1 else None},
        "properties": sorted(props, key=lambda p: p["name"]),
        "signals": sorted(signals, key=lambda s: s["name"]),
        "methods": methods,
        "dependency_properties": sorted(dps),
        "attached": attached,
        "source": {"winmd": "Microsoft.UI.Xaml.winmd" if name.startswith("Microsoft.UI.Xaml") else "Microsoft.UI.winmd",
                   "package": f"microsoft.windowsappsdk.winui {PACKAGES['microsoft.windowsappsdk.winui']}"},
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tool", default=str(ROOT / "target/winmd-tool/release/winmd-tool"))
    ap.add_argument("--cache")
    ap.add_argument("--check", action="store_true", help="fail if metadata/winui is stale")
    a = ap.parse_args()
    roots = sorted({svmeta.load_yaml(p)["class"] for p in OVERRIDES.glob("*.yaml")})
    cache = fetch.cache_dir(a.cache)
    winmds = fetch.winmds(cache)
    listing = cache / "winmds.txt"
    listing.write_text("".join(f"{p}\n" for p in winmds))
    out = subprocess.run([a.tool, "dump", *roots, "--", f"@{listing}"], check=True, capture_output=True, text=True).stdout
    types = json.loads(out)["types"]
    unresolved = [k for k, v in types.items() if v["category"] == "unresolved"]
    if unresolved:
        print(f"ingest: unresolved types {unresolved}", file=sys.stderr)
        return 1
    classes = sorted({c for r in roots for c in [r] + chain(r, types)})
    shorts = [c.rsplit(".", 1)[1] for c in classes]
    if len(set(shorts)) != len(shorts):
        print(f"ingest: short class names collide: {shorts}", file=sys.stderr)
        return 1
    outputs = {OUT / f"{c.rsplit('.', 1)[1]}.json": class_kind(c, types) for c in classes}
    # Enums, structs and args classes referenced by the ingested classes.
    wanted = set()
    for k in outputs.values():
        for p in k["properties"] + k["attached"]:
            for frag in json.dumps(p["type"]).replace("<", " ").replace(">", " ").replace('"', " ").split():
                if frag in types and types[frag]["category"] in ("enum", "struct"):
                    wanted.add(frag)
        for s in k["signals"]:
            if s["args"] in types:
                wanted.add(s["args"])
            for p in s["args_props"]:
                head = p["type"]["native"]
                if head in types and types[head]["category"] in ("enum", "struct"):
                    wanted.add(head)
    side = {}
    for n in sorted(wanted):
        t = types[n]
        if t["category"] == "class":
            side[n] = {"category": "class", "extends": t.get("extends"),
                       "props": [{"name": p["name"], "type": p["type"], "settable": p["set"]} for p in t["props"]],
                       "methods": sorted({m["name"] for m in t["methods"] if "name" in m})}
        else:
            side[n] = t
    outputs[OUT / "_types.json"] = {"schema": "svelters-winui-types/v0", "types": side}
    if a.check:
        stale = [p for p, d in outputs.items() if not p.exists() or svmeta.load_json(p) != d]
        stale += [p for p in OUT.glob("*.json") if p not in outputs]
        for p in stale:
            print(f"ingest: {p.relative_to(ROOT)} is stale", file=sys.stderr)
        return 1 if stale else 0
    changed = sum(svmeta.write_json(p, d) for p, d in outputs.items())
    for p in OUT.glob("*.json"):
        if p not in outputs:
            p.unlink()
            changed += 1
    print(f"ingest: {len(classes)} classes, {len(side)} side types, {changed} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
