#!/usr/bin/env python3
"""Generates the `svelters:gtk/<ns>` modules from the type facts (metadata/gtk/types/*.json).

Output (all deterministic, checked in):
  packages/types/gtk/<ns>/<Class>.js    one file per class, one named export, no top-level side effects
  packages/types/gtk/<ns>/index.js      re-exports (`export { File } from './File.js'`)
  packages/types/gtk/<ns>.d.ts          `declare module 'svelters:gtk/<ns>'` typings
  packages/types/gtk/manifest.json      per class: native name, hierarchy, the classes its code imports and
                                        its signature references (the release linker's input, Q9)
  packages/types/package.json           `exports` for every module (the rest of the file is kept)
  docs/site/src/content/docs/types/gtk/ docs pages of the curated types
  crates/svelters-gtk/tests/types_verify.rs + metadata/gtk/types/facts.json   ledger tests of the curated types

Generated code only calls `packages/types/src/core.js` (the hand-written object core). Classes reference
each other by import, so a bundler drops every class nobody imports (`sideEffects: false`). Import cycles
that contain an `extends` edge would hit the temporal dead zone, so the generator drops the wrap import
that would close one and wraps that result as the nearest ancestor class instead.

  python3 tools/metadata/gtk/gen_types.py            # write
  python3 tools/metadata/gtk/gen_types.py --check    # the checked-in output is current (no GIR needed)
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import types_facts as TF  # noqa: E402

FACTS = TF.FACTS_DIR
PKG = ROOT / "packages/types"
OUT = PKG / "gtk"
DOCS = ROOT / "docs/site/src/content/docs/types/gtk"
TESTS = ROOT / "crates/svelters-gtk/tests/types_verify.rs"
FACT_FILE = FACTS / "facts.json"

JS_RESERVED = {"break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else", "enum",
               "export", "extends", "false", "finally", "for", "function", "if", "import", "in", "instanceof", "new", "null",
               "return", "super", "switch", "this", "throw", "true", "try", "typeof", "var", "void", "while", "with", "yield",
               "let", "static", "await", "async", "of", "arguments", "eval", "interface", "package", "private", "protected",
               "public", "implements"}
# Names the base classes (or Function) use: a generated member of that name gets a trailing underscore.
INSTANCE_RESERVED = {"constructor", "prototype", "dispose", "on", "equals", "svlHandle", "disposed", "nativeType", "__svlBoxed", "__proto__"}
STATIC_RESERVED = {"name", "length", "prototype", "caller", "arguments", "call", "apply", "bind", "native", "gtype", "bases", "signalArgs", "from"}


def load_facts() -> dict[str, dict]:
    types: dict[str, dict] = {}
    for p in sorted(FACTS.glob("*.json")):
        if p.name == "facts.json":
            continue
        d = json.loads(p.read_text())
        for rec in d["types"].values():
            types[rec["qualified"]] = rec
    return types


def nsdir(ns: str) -> str:
    return ns.lower()


def inst_name(js: str) -> str:
    return js + "_" if js in INSTANCE_RESERVED else js


def static_name(js: str) -> str:
    return js + "_" if js in STATIC_RESERVED else js


JS_GLOBALS = {"Object", "Error", "Date", "Map", "Set", "Function", "Array", "Promise", "Symbol", "Proxy", "Math", "String",
              "Number", "Boolean", "Reflect", "Blob", "Intl", "JSON", "WeakMap", "WeakSet", "WeakRef", "RegExp", "BigInt"}


def ident(name: str) -> str:
    return name + "_" if name in JS_RESERVED else name


class Gen:
    def __init__(self, types: dict[str, dict]) -> None:
        self.types = types
        self.classes = {q: r for q, r in types.items() if r["category"] in ("object", "model", "value")}
        self.warnings: list[str] = []
        self.enums = self._enum_names()
        self.wrap_edges: dict[str, set[str]] = {q: set() for q in self.classes}
        self.fallback: dict[tuple[str, str], str] = {}
        self._plan_imports()

    def _enum_names(self) -> set[str]:
        out: set[str] = set()

        def walk(t: dict | None) -> None:
            if not t:
                return
            if t["k"] in ("enum", "flags"):
                out.add(t["t"])
            if "elem" in t:
                walk(t["elem"])

        for r in self.types.values():
            for m in r["constructors"] + r["statics"] + r["methods"]:
                walk(m["ret"])
                for p in m["params"]:
                    walk(p["type"])
            for p in r["properties"]:
                walk(p["type"])
            for s in r["signals"]:
                for a in s["args"]:
                    walk(a["type"])
        return out

    # ---- hierarchy ----

    def parent(self, q: str) -> str | None:
        p = self.types[q].get("parent")
        return p if p in self.classes else None

    def ancestors(self, q: str) -> list[str]:
        out: list[str] = []
        p = self.parent(q)
        while p and p not in out:
            out.append(p)
            p = self.parent(p)
        return out

    def iface_closure(self, q: str) -> list[str]:
        """Interfaces of `q` (and of the interfaces' prerequisites), nearest first."""
        out: list[str] = []
        todo = list(self.types[q].get("interfaces", [])) + list(self.types[q].get("prerequisites", []))
        while todo:
            i = todo.pop(0)
            if i in out or i not in self.classes or self.types[i]["kind"] != "interface" or i == q:
                continue
            out.append(i)
            todo += self.types[i].get("prerequisites", []) + self.types[i].get("interfaces", [])
        return out

    def own_ifaces(self, q: str) -> list[str]:
        """Interfaces whose members are copied into `q`: those no ancestor already provides."""
        have: set[str] = set()
        for a in self.ancestors(q):
            have |= set(self.iface_closure(a))
        return [i for i in self.iface_closure(q) if i not in have]

    def bases(self, q: str) -> list[str]:
        out = [q] + self.ancestors(q)
        for c in list(out):
            for i in self.iface_closure(c):
                if i not in out:
                    out.append(i)
        return out

    # ---- members ----

    def effective(self, q: str) -> dict:
        """The members the class body defines: its own plus those of the interfaces it adds."""
        rec = self.types[q]
        sources = [q] + self.own_ifaces(q)
        res = {"methods": [], "statics": [], "constructors": [], "properties": [], "signals": []}
        seen_i: set[str] = set()
        seen_s: set[str] = set()
        seen_sig: set[str] = set()
        # names the ancestors already define
        anc_i: set[str] = set()
        anc_p: set[str] = set()
        for a in self.ancestors(q):
            ar = self.types[a]
            for m in ar["methods"]:
                anc_i.add(inst_name(m["js"]))
            for p in ar["properties"]:
                anc_p.add(inst_name(p["js"]))
        for src in sources:
            r = self.types[src]
            own = src == q
            for m in r["methods"]:
                n = inst_name(m["js"])
                if n in seen_i:
                    continue
                seen_i.add(n)
                res["methods"].append(m)
            for p in r["properties"]:
                n = inst_name(p["js"])
                if n in seen_i or n in seen_s:
                    self.warnings.append(f"{q}: property {p['name']} collides with a method of the same JS name")
                    continue
                seen_s.add(n)
                res["properties"].append(p)
            for s in r["signals"]:
                if s["name"] not in seen_sig:
                    seen_sig.add(s["name"])
                    res["signals"].append(s)
            if own:
                res["statics"] = list(r["statics"])
                res["constructors"] = list(r["constructors"])
        return res

    def all_props(self, q: str) -> list[dict]:
        """Every property the class accepts at construction: own, interface and inherited."""
        out: dict[str, dict] = {}
        for c in reversed([q] + self.ancestors(q)):
            for src in [c] + self.iface_closure(c):
                for p in self.types[src]["properties"]:
                    out[p["name"]] = p
        return list(out.values())

    def sig_refs(self, q: str) -> set[str]:
        """The signature references of everything the class defines (own and copied interface members)."""
        eff = self.effective(q)
        refs: set[str] = set()
        for m in eff["methods"] + eff["statics"] + eff["constructors"]:
            refs |= TF.sig_refs(m)
        for p in eff["properties"]:
            refs |= TF.refs_of(p["type"])
        for s in eff["signals"]:
            for a in s["args"]:
                refs |= TF.refs_of(a["type"])
        refs.discard(q)
        return refs

    # ---- wrap imports ----

    def wraps_of(self, q: str) -> set[str]:
        """Classes the code of `q` wraps results in: returns, outs, property reads, signal args."""
        eff = self.effective(q)
        out: set[str] = set()

        def add(t: dict | None) -> None:
            if not t:
                return
            if t["k"] in ("object", "boxed") and t["t"] in self.classes:
                out.add(t["t"])
            if "elem" in t:
                add(t["elem"])

        for m in eff["methods"] + eff["statics"] + eff["constructors"]:
            add(m["ret"])
            for p in m["params"]:
                if p.get("dir") in ("out", "inout") and not p.get("hidden"):
                    add(p["type"])
        for p in eff["properties"]:
            add(p["type"])
        for s in eff["signals"]:
            for a in s["args"]:
                add(a["type"])
        out.discard(q)
        return out

    def _reaches_with_extends(self, edges: dict[str, set[str]], start: str, goal: str) -> bool:
        """Is there a path start => goal in extends + wrap edges that uses at least one extends edge?"""
        seen: set[tuple[str, bool]] = set()
        stack = [(start, False)]
        while stack:
            n, used = stack.pop()
            if (n, used) in seen:
                continue
            seen.add((n, used))
            if n == goal and used:
                return True
            p = self.parent(n)
            if p:
                stack.append((p, True))
            for w in edges.get(n, ()):
                stack.append((w, used))
        return False

    def _plan_imports(self) -> None:
        edges: dict[str, set[str]] = {q: set() for q in self.classes}
        for q in sorted(self.classes):
            for w in sorted(self.wraps_of(q)):
                target = self._pick_wrap(edges, q, w)
                if target is None:
                    self.fallback[(q, w)] = "NativeObject" if self.types[w]["category"] != "value" else "NativeValue"
                elif target != w:
                    self.fallback[(q, w)] = target
                if target and target != q:
                    edges[q].add(target)
        self.wrap_edges = edges

    def _pick_wrap(self, edges: dict[str, set[str]], a: str, b: str) -> str | None:
        """`b` if importing it from `a` closes no cycle through an extends edge, else its nearest ancestor that is fine."""
        for cand in [b] + self.ancestors(b):
            if cand == a:
                return a
            if cand in self.ancestors(a):
                return cand
            if not self._reaches_with_extends(edges, cand, a):
                return cand
        return None

    def wrap_class(self, q: str, w: str) -> str:
        """The class expression a result declared as `w` is wrapped in inside the code of `q`."""
        return self.fallback.get((q, w), w)

    def imports_of(self, q: str) -> list[str]:
        """Classes `q`'s file imports (parent first, then wrap targets), as qualified names."""
        out: list[str] = []
        p = self.parent(q)
        if p:
            out.append(p)
        for w in sorted(self.wraps_of(q)):
            t = self.wrap_class(q, w)
            if t in self.classes and t != q and t not in out:
                out.append(t)
        return out

    # ---- types ----

    def alias_map(self, q: str, imports: list[str]) -> dict[str, str]:
        names: dict[str, int] = {}
        for i in imports:
            names[self.types[i]["name"]] = names.get(self.types[i]["name"], 0) + 1
        taken = {self.types[q]["name"]}
        alias: dict[str, str] = {}
        for i in imports:
            n = self.types[i]["name"]
            a = n if names[n] == 1 and n not in taken and n not in JS_GLOBALS else f"{self.types[i]['ns']}_{n}"
            alias[i] = a
            taken.add(a)
        return alias

    def w_expr(self, q: str, t: dict, e: str, alias: dict[str, str]) -> str:
        k = t["k"]
        if k in ("object", "boxed") and t["t"] in self.classes:
            w = self.wrap_class(q, t["t"])
            if w in ("NativeObject", "NativeValue"):
                cname = w
            elif w == q:
                cname = self.types[q]["name"]
            else:
                cname = alias[w]
            return f"wrapObject({cname}, {e})" if k == "object" else f"wrapValue({cname}, {e})"
        if k in ("list", "array") and t["elem"]["k"] in ("object", "boxed") and t["elem"]["t"] in self.classes:
            inner = self.w_expr(q, t["elem"], "e", alias)
            return f"({e})?.map((e) => {inner}) ?? null"
        return e

    @staticmethod
    def needs_wrap(t: dict) -> bool:
        return (t["k"] in ("object", "boxed")) or (t["k"] in ("list", "array") and t["elem"]["k"] in ("object", "boxed"))

    # ---- emit JS ----

    def method_js(self, q: str, m: dict, alias: dict[str, str], static: bool) -> str:
        native = self.types[q]["qualified"]
        params = [p for p in m["params"] if not p.get("hidden")]
        ins = [p for p in params if p.get("dir") != "out"]
        outs = [p for p in params if p.get("dir") in ("out", "inout")]
        # trailing nullable parameters default to null
        sig = []
        trailing = True
        flags = []
        for p in reversed(ins):
            if trailing and p.get("nullable"):
                flags.append(True)
            else:
                trailing = False
                flags.append(False)
        flags.reverse()
        for p, d in zip(ins, flags):
            sig.append(f"{ident(p['js'])} = null" if d else ident(p["js"]))
        args = []
        for p in ins:
            n = ident(p["js"])
            args.append(f"typeArg({n})" if p["type"]["k"] == "gtype" else n)
        arglist = "[" + ", ".join(args) + "]"
        name = static_name(m["js"]) if static else inst_name(m["js"])
        if static:
            call = f"callStatic({native!r}, {m['name']!r}, {arglist})"
        else:
            call = f"call(this, {m['name']!r}, {arglist})"
        head = f"  {'static ' if static else ''}{name}({', '.join(sig)}) {{"
        if not outs:
            if m["ret"]["k"] == "void":
                return f"{head}\n    {call};\n  }}"
            if m["ret"]["k"] == "object" and static and m["ret"]["t"] == q and False:
                pass
            return f"{head}\n    return {self.w_expr(q, m['ret'], call, alias)};\n  }}"
        parts = []
        if m["ret"]["k"] != "void":
            parts.append(f"ret: {self.w_expr(q, m['ret'], '__res.ret', alias)}")
        for p in outs:
            key = TF.camel(p["name"])
            parts.append(f"{key}: {self.w_expr(q, p['type'], '__res.' + key, alias)}")
        return f"{head}\n    const __res = {call};\n    return {{ {', '.join(parts)} }};\n  }}"

    def prop_js(self, q: str, p: dict, alias: dict[str, str]) -> str:
        n = inst_name(p["js"])
        lines = []
        if p["readable"]:
            lines.append(f"  get {n}() {{\n    return {self.w_expr(q, p['type'], f'getProp(this, {p['name']!r})', alias)};\n  }}")
        if p["writable"] and not p["construct_only"]:
            v = "typeArg(v)" if p["type"]["k"] == "gtype" else "v"
            lines.append(f"  set {n}(v) {{\n    setProp(this, {p['name']!r}, {v});\n  }}")
        return "\n".join(lines)

    def class_js(self, q: str) -> str:
        rec = self.types[q]
        name = rec["name"]
        parent = self.parent(q)
        imports = self.imports_of(q)
        alias = self.alias_map(q, imports)
        is_value = rec["category"] == "value"
        base = alias[parent] if parent else ("NativeValue" if is_value else "NativeObject")
        eff = self.effective(q)
        core_names = {"NativeObject" if not is_value else "NativeValue", "WRAP", "CONSTRUCT"}
        used: set[str] = set()
        body: list[str] = []
        bases = self.bases(q)
        body.append(f"  static native = {q!r};")
        body.append(f"  static gtype = {rec['gtype']!r};")
        body.append(f"  static bases = {json.dumps(bases)};")
        sigargs = {}
        for s in eff["signals"]:
            spec = []
            has = False
            for a in s["args"]:
                t = a["type"]
                if t["k"] in ("object", "boxed") and t["t"] in self.classes:
                    w = self.wrap_class(q, t["t"])
                    cname = name if w == q else (w if w in ("NativeObject", "NativeValue") else alias[w])
                    spec.append(f"['{'o' if t['k'] == 'object' else 'v'}', () => {cname}]")
                    has = True
                else:
                    spec.append("null")
            if has:
                sigargs[s["name"]] = "[" + ", ".join(spec) + "]"
        if sigargs:
            body.append("  static signalArgs = {\n" + ",\n".join(f"    {k!r}: {v}" for k, v in sorted(sigargs.items())) + ",\n  };")
        # construction
        if is_value:
            ctor = next((c for c in eff["constructors"] if c["name"] == "new"), None)
            if ctor:
                n_args = [p for p in ctor["params"] if not p.get("hidden") and p.get("dir") != "out"]
                conv = ", ".join(f"typeArg({ident(p['js'])})" if p["type"]["k"] == "gtype" else ident(p["js"]) for p in n_args)
                ps = ", ".join(ident(p["js"]) for p in n_args)
                body.append(f"  static [CONSTRUCT]({ps}) {{\n    return callStatic({q!r}, 'new', [{conv}]);\n  }}")
        elif not rec["abstract"] and rec["kind"] == "class":
            pm = {}
            for p in self.all_props(q):
                if p["writable"] or p["construct_only"] or p["construct"]:
                    pm[p["js"]] = [p["name"]] + (["gtype"] if p["type"]["k"] == "gtype" else [])
            body.append("  static [CONSTRUCT](props) {\n    return construct(this.native, constructProps(this, props, "
                        + json.dumps(pm, sort_keys=True).replace('"', "'") + "));\n  }")
        for c in eff["constructors"]:
            body.append(self.method_js(q, c, alias, True))
        for s in eff["statics"]:
            body.append(self.method_js(q, s, alias, True))
        if is_value and any(m["name"] == "equal" for m in eff["methods"]):
            pass
        for m in eff["methods"]:
            body.append(self.method_js(q, m, alias, False))
            for al in m.get("aliases", []):
                if al != m["js"] and al not in {x["js"] for x in eff["methods"]}:
                    body.append(self.method_js(q, {**m, "js": al}, alias, False))
        for p in eff["properties"]:
            t = self.prop_js(q, p, alias)
            if t:
                body.append(t)
        text = "\n\n".join(b for b in body if b)
        code = f"export class {name} extends {base} {{\n{text}\n}}\n"
        # imports
        core = ["WRAP"] if False else []
        needed = ["NativeValue" if is_value else "NativeObject"] if not parent else []
        for fn in ("call", "callStatic", "getProp", "setProp", "construct", "constructProps", "typeArg", "wrapObject", "wrapValue", "CONSTRUCT", "NativeObject", "NativeValue"):
            if re.search(r"(?<![\w.])" + re.escape(fn) + r"\b", code.split("\n", 1)[1]) or (fn == "NativeObject" and not parent and not is_value) or (fn == "NativeValue" and not parent and is_value):
                needed.append(fn)
        needed = sorted(set(needed))
        # a self-referencing `static native` etc. never uses the names above by accident
        head = "// Generated by tools/metadata/gtk/gen_types.py from metadata/gtk/types/*.json. Do not edit.\n"
        head += f"import {{ {', '.join(needed)} }} from '../../src/core.js';\n" if needed else ""
        for i in imports:
            r = self.types[i]
            a = alias[i]
            spec = r["name"] if a == r["name"] else f"{r['name']} as {a}"
            path = f"./{r['name']}.js" if r["ns"] == rec["ns"] else f"../{nsdir(r['ns'])}/{r['name']}.js"
            head += f"import {{ {spec} }} from '{path}';\n"
        return head + "\n" + code

    # ---- d.ts ----

    def ts_type(self, q: str, t: dict, imports: dict[str, str], nullable: bool = False) -> str:
        k = t["k"]
        s = {"bool": "boolean", "int": "number", "float": "number", "str": "string", "unichar": "number", "strv": "string[]",
             "void": "void", "enum": "string", "flags": "string | string[] | number", "gtype": "string | { readonly native: string }",
             "variant": "unknown", "record": "Record<string, number>"}.get(k)
        if s is None:
            if k in ("object", "boxed"):
                s = imports.get(t["t"], "unknown") if t["t"] in self.classes else "unknown"
            elif k == "list" or k == "array":
                if k == "array" and t["elem"]["k"] == "int" and t["elem"].get("t") in ("guint8", "gint8", "guchar"):
                    s = "Uint8Array"
                else:
                    e = self.ts_type(q, t["elem"], imports)
                    s = f"({e})[]" if "|" in e else f"{e}[]"
            else:
                s = "unknown"
        return f"{s} | null" if nullable and s not in ("void", "unknown") and not s.endswith("| null") else s

    def ts_method(self, q: str, m: dict, imports: dict[str, str], static: bool) -> str:
        params = [p for p in m["params"] if not p.get("hidden")]
        ins = [p for p in params if p.get("dir") != "out"]
        outs = [p for p in params if p.get("dir") in ("out", "inout")]
        flags = []
        trailing = True
        for p in reversed(ins):
            if trailing and p.get("nullable"):
                flags.append(True)
            else:
                trailing = False
                flags.append(False)
        flags.reverse()
        sig = ", ".join(f"{ident(p['js'])}{'?' if d else ''}: {self.ts_type(q, p['type'], imports, p.get('nullable', False))}" for p, d in zip(ins, flags))
        ret = self.ts_type(q, m["ret"], imports, m.get("ret_nullable", False))
        if outs:
            parts = []
            if m["ret"]["k"] != "void":
                parts.append(f"ret: {ret}")
            parts += [f"{TF.camel(p['name'])}: {self.ts_type(q, p['type'], imports, p.get('nullable', False))}" for p in outs]
            ret = "{ " + "; ".join(parts) + " }"
        name = static_name(m["js"]) if static else inst_name(m["js"])
        doc = f"  /** `{m['name']}`" + (f", since {m['since']}" if m.get("since") else "") + (", throws" if m.get("throws") else "") + " */\n"
        return f"{doc}  {'static ' if static else ''}{name}({sig}): {ret};"

    @staticmethod
    def sig_key(m: dict) -> str:
        return json.dumps([m["ret"], [(p["type"], p.get("dir"), p.get("hidden")) for p in m["params"]]], sort_keys=True)

    def ancestor_members(self, q: str) -> tuple[dict, dict]:
        """Static and instance members the ancestors define, by JS name, as signature keys."""
        st: dict[str, str] = {}
        inst: dict[str, str] = {}
        for a in reversed(self.ancestors(q)):
            e = self.effective(a)
            for c in e["constructors"] + e["statics"]:
                st[static_name(c["js"])] = self.sig_key(c)
            for m in e["methods"]:
                inst[inst_name(m["js"])] = self.sig_key(m)
        return st, inst

    def loose(self, m: dict, static: bool) -> str:
        """A member whose signature differs from an ancestor's (the same GIR name on unrelated APIs): TypeScript
        needs the override to stay assignable, so it is typed loosely."""
        name = static_name(m["js"]) if static else inst_name(m["js"])
        self.warnings.append(f"typings: {name} overrides an ancestor member with another signature (typed loosely)")
        return f"/** `{m['name']}`; overrides an ancestor member with another signature */\n  {'static ' if static else ''}{name}(...args: any[]): any;"

    def dts(self, ns: str) -> str:
        qs = sorted(q for q, r in self.classes.items() if r["ns"] == ns)
        refs: dict[str, str] = {}
        ext: dict[str, set[str]] = {}
        for q in qs:
            for w in self.sig_refs(q) | set(self.imports_of(q)) | ({self.parent(q)} if self.parent(q) else set()):
                if w in self.classes and self.types[w]["ns"] != ns:
                    ext.setdefault(self.types[w]["ns"], set()).add(w)
        lines = ["// Generated by tools/metadata/gtk/gen_types.py from metadata/gtk/types/*.json. Do not edit.",
                 f"declare module 'svelters:gtk/{nsdir(ns)}' {{",
                 "  import type { NativeObject, NativeValue } from '@svelters/types/core';"]
        imports: dict[str, str] = {}
        taken = {self.types[q]["name"] for q in qs}
        for q in qs:
            imports[q] = self.types[q]["name"]
        for ons in sorted(ext):
            names = []
            for w in sorted(ext[ons]):
                n = self.types[w]["name"]
                a = n if n not in taken else f"{ons}_{n}"
                taken.add(a)
                imports[w] = a
                names.append(n if a == n else f"{n} as {a}")
            lines.append(f"  import type {{ {', '.join(names)} }} from 'svelters:gtk/{nsdir(ons)}';")
        for q in qs:
            rec = self.types[q]
            is_value = rec["category"] == "value"
            parent = self.parent(q)
            base = imports[parent] if parent else ("NativeValue" if is_value else "NativeObject")
            eff = self.effective(q)
            lines.append("")
            lines.append(f"  /** `{q}`, {rec['category']}" + (f", since {rec['since']}" if rec.get("since") else "") + " */")
            kw = "abstract class" if rec["abstract"] or rec["kind"] == "interface" else "class"
            lines.append(f"  export {kw} {rec['name']} extends {base} {{")
            lines.append('    static readonly native: string;')
            if rec["kind"] == "class" and not rec["abstract"] and not is_value:
                ctor_props = ", ".join(f"{p['js']}?: {self.ts_type(q, p['type'], imports, True)}" for p in self.all_props(q)
                                       if p["writable"] or p["construct_only"] or p["construct"])
                lines.append(f"    constructor(props?: {{ {ctor_props} }});")
            elif is_value:
                ctor = next((c for c in eff["constructors"] if c["name"] == "new"), None)
                if ctor:
                    cps = ", ".join(f"{ident(p['js'])}: {self.ts_type(q, p['type'], imports, p.get('nullable', False))}" for p in ctor["params"] if not p.get("hidden") and p.get("dir") != "out")
                    lines.append(f"    constructor({cps});")
            anc_static, anc_inst = self.ancestor_members(q)
            for c in eff["constructors"] + eff["statics"]:
                t = self.ts_method(q, c, imports, True)
                if static_name(c["js"]) in anc_static and anc_static[static_name(c["js"])] != self.sig_key(c):
                    t = self.loose(c, True)
                lines.append("    " + t.replace("\n", "\n    "))
            for m in eff["methods"]:
                t = self.ts_method(q, m, imports, False)
                if inst_name(m["js"]) in anc_inst and anc_inst[inst_name(m["js"])] != self.sig_key(m):
                    t = self.loose(m, False)
                lines.append("    " + t.replace("\n", "\n    "))
            for p in eff["properties"]:
                n = inst_name(p["js"])
                t = self.ts_type(q, p["type"], imports, True)
                ro = "readonly " if not p["writable"] or p["construct_only"] else ""
                if p["readable"]:
                    lines.append(f"    {ro}{n}: {t};")
                else:
                    lines.append(f"    {n}: {t};")
            if eff["signals"]:
                lines.append("    on(signal: string, fn: (...args: any[]) => void): () => void;")
            for s in eff["signals"]:
                args = ", ".join(f"{ident(TF.camel(a['name'] or 'arg'))}: {self.ts_type(q, a['type'], imports, True)}" for a in s["args"])
                lines.append(f"    on(signal: '{s['name']}', fn: ({args}) => void): () => void;")
            lines.append("  }")
        lines.append("}")
        return "\n".join(lines) + "\n"

    # ---- manifest ----

    def manifest(self) -> dict:
        mods: dict[str, dict] = {}
        for q in sorted(self.classes):
            r = self.types[q]
            ns = r["ns"]
            m = mods.setdefault(f"svelters:gtk/{nsdir(ns)}", {"dir": nsdir(ns), "exports": {}})
            m["exports"][r["name"]] = {
                "file": f"{nsdir(ns)}/{r['name']}.js",
                "native": q,
                "category": r["category"],
                "extends": self.parent(q),
                "implements": self.iface_closure(q),
                "imports": [i for i in self.imports_of(q)],
                "signature": sorted(w for w in self.sig_refs(q) if w in self.classes or w in self.enums),
            }
        return {"schema": "svelters-types-manifest/v1", "toolkit": "gtk", "modules": mods}

    # ---- files ----

    def files(self) -> dict[Path, str]:
        out: dict[Path, str] = {}
        by_ns: dict[str, list[str]] = {}
        for q in sorted(self.classes):
            r = self.types[q]
            by_ns.setdefault(r["ns"], []).append(q)
            out[OUT / nsdir(r["ns"]) / f"{r['name']}.js"] = self.class_js(q)
        for ns, qs in sorted(by_ns.items()):
            idx = "// Generated by tools/metadata/gtk/gen_types.py. Do not edit.\n"
            for q in qs:
                n = self.types[q]["name"]
                idx += f"export {{ {n} }} from './{n}.js';\n"
            out[OUT / nsdir(ns) / "index.js"] = idx
            out[OUT / f"{nsdir(ns)}.d.ts"] = self.dts(ns)
        out[OUT / "manifest.json"] = json.dumps(self.manifest(), indent=1, sort_keys=True) + "\n"
        out[PKG / "package.json"] = self.package_json(sorted(by_ns))
        return out

    def package_json(self, namespaces: list[str]) -> str:
        p = PKG / "package.json"
        base = json.loads(p.read_text()) if p.exists() else {}
        base.update({"name": "@svelters/types", "version": "0.0.0", "license": "MIT", "private": True, "type": "module",
                     "description": "Native object model and generated non-UI types (svelters:gtk/gio, ...) for Svelters apps",
                     "sideEffects": False})
        exports = {"./core": {"types": "./src/core.d.ts", "default": "./src/core.js"}, "./vite": "./vite.js", "./manifest": "./gtk/manifest.json"}
        for ns in namespaces:
            exports[f"./gtk/{nsdir(ns)}"] = {"types": f"./gtk/{nsdir(ns)}.d.ts", "default": f"./gtk/{nsdir(ns)}/index.js"}
        base["exports"] = exports
        base["scripts"] = {"test": "node --test test/*.test.mjs && tsc -p test/types/tsconfig.json"}
        base["devDependencies"] = {"typescript": "~6.0.3", "vite": "8.3.4"}
        return json.dumps(base, indent=2) + "\n"


def write_all(files: dict[Path, str], check: bool) -> int:
    bad: list[str] = []
    wanted = set(files)
    for p, text in files.items():
        if check:
            if not p.exists() or p.read_text() != text:
                bad.append(str(p.relative_to(ROOT)))
        else:
            p.parent.mkdir(parents=True, exist_ok=True)
            if not p.exists() or p.read_text() != text:
                p.write_text(text)
    # stale generated class files
    for p in list(OUT.rglob("*.js")):
        if p not in wanted:
            if check:
                bad.append(str(p.relative_to(ROOT)) + " (stale)")
            else:
                p.unlink()
    if bad:
        print("gen_types: generated files are stale: " + ", ".join(bad[:10]) + (" ..." if len(bad) > 10 else "") + "\n  run `python3 tools/metadata/gtk/gen_types.py`", file=sys.stderr)
        return 1
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    g = Gen(load_facts())
    files = g.files()
    import gen_types_extra  # noqa: E402  (docs, ledger facts and tests of the curated set)
    files.update(gen_types_extra.files(g))
    rc = write_all(files, a.check)
    for w in sorted(set(g.warnings)):
        print("gen_types: warning:", w, file=sys.stderr)
    if rc == 0:
        print(f"gen_types: {len(g.classes)} classes, {len(files)} files {'are current' if a.check else 'written'}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
