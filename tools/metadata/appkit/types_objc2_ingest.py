#!/usr/bin/env python3
"""Ingest objc2's header-translator output (objc2-foundation, objc2-app-kit) into raw type facts (N4-AppKit).

Decision Q7 (plans/research/NON-GUI-TYPES.md section 10): objc2's generated Rust is the primary Apple ingest. It is
readable on Linux (crates.io sources, MIT/Apache), so the facts are made here and checked in; the classifier
(`types_appkit_classify.py`) then runs on the checked-in facts only, without the registry or a Mac.

What objc2 states that the headers alone do not (or not as data):
  #[unsafe(super(A, B, ...))]          the whole superclass chain
  #[thread_kind = MainThreadOnly]      main-thread affinity
  unsafe impl Send / Sync for X        thread-safe classes
  extern_conformance!(impl P for X)    protocol conformances (NSCopying, NSSecureCoding, ...)
  pub fn  vs  pub unsafe fn            objc2's reviewed "safe to call" set, with its `# Safety` reasons
  method_family                        ownership of results (new/copy/init are +1)
  /// Setter for [`key`]              read-write property pairs
  Result<T, Retained<NSError>>         an `NSError **` out parameter (objc2 folds it into a Result)
  block2::DynBlock<dyn Fn(...)>        block parameters with their signature
  Option<...>                          nullability (objc2 applies the header's annotations)

Output: metadata/appkit/types/ingested/{Foundation,AppKit}.json (schema `svelters-objc2-ingest/v1`), deterministic.

  python3 -I tools/metadata/appkit/types_objc2_ingest.py [--registry ~/.cargo/registry/src] [--check]
"""
from __future__ import annotations

import argparse
import ast
import glob
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "metadata/appkit/types/ingested"
CRATES = {"Foundation": "objc2-foundation", "AppKit": "objc2-app-kit"}
VERSION = "0.3.2"


# ---- lexical helpers ------------------------------------------------------------------------------

def blank_comments(src: str) -> str:
    """`src` with the text of `//` comments replaced by spaces (positions kept), for brace matching."""
    out = []
    for line in src.split("\n"):
        i = line.find("//")
        # string literals in this generated code never contain `//` except in `#[deprecated = "..."]` / doc URLs
        if i >= 0 and line[:i].count('"') % 2 == 0:
            line = line[:i] + " " * (len(line) - i)
        out.append(line)
    return "\n".join(out)


def match_close(code: str, i: int, open_: str, close: str) -> int:
    """Index just past the bracket closing the one at `i`."""
    depth = 0
    n = len(code)
    while i < n:
        c = code[i]
        if c == open_:
            depth += 1
        elif c == close:
            depth -= 1
            if depth == 0:
                return i + 1
        elif c == '"':
            i += 1
            while i < n and code[i] != '"':
                i += 2 if code[i] == "\\" else 1
        i += 1
    return n


def split_top(s: str, sep: str = ",") -> list[str]:
    """Splits at `sep` outside <>, (), [] (a `->` arrow is not a closing angle)."""
    parts, depth, cur, i = [], 0, [], 0
    while i < len(s):
        c = s[i]
        if c == "-" and s[i:i + 2] == "->":
            cur.append("->")
            i += 2
            continue
        if c in "<([":
            depth += 1
        elif c in ">)]":
            depth -= 1
        if c == sep and depth == 0:
            parts.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    tail = "".join(cur).strip()
    if tail:
        parts.append(tail)
    return parts


def norm_ty(t: str) -> str:
    return re.sub(r"\s+", " ", t).replace("< ", "<").replace(" >", ">").replace("( ", "(").replace(" )", ")").replace(" ,", ",").strip()


# ---- enum values ----------------------------------------------------------------------------------

CONSTS = {"NSIntegerMax": (1 << 63) - 1, "NSIntegerMin": -(1 << 63), "NSUIntegerMax": (1 << 64) - 1,
          "c_long::MAX": (1 << 63) - 1, "u32::MAX": (1 << 32) - 1, "u64::MAX": (1 << 64) - 1, "i64::MAX": (1 << 63) - 1,
          "usize::MAX": (1 << 64) - 1, "isize::MAX": (1 << 63) - 1, "c_uint::MAX": (1 << 32) - 1, "c_ulong::MAX": (1 << 64) - 1}


def eval_int(expr: str, known: dict[str, int]) -> int | None:
    e = expr.strip()
    e = re.sub(r"\bas\s+\w+", "", e)
    for k, v in CONSTS.items():
        e = e.replace(k, str(v))
    e = re.sub(r"\b(\w+)::(\w+)\.0\b", lambda m: str(known.get(f"{m.group(1)}.{m.group(2)}", "x")), e)
    e = re.sub(r"\bSelf::(\w+)\.0\b", lambda m: str(known.get(f"Self.{m.group(1)}", "x")), e)
    e = re.sub(r"(\d+)(?:u|i)(?:8|16|32|64|size)\b", r"\1", e)
    try:
        node = ast.parse(e, mode="eval")
    except SyntaxError:
        return None

    def ev(n):
        if isinstance(n, ast.Expression):
            return ev(n.body)
        if isinstance(n, ast.Constant) and isinstance(n.value, int):
            return n.value
        if isinstance(n, ast.UnaryOp) and isinstance(n.op, (ast.USub, ast.Invert)):
            v = ev(n.operand)
            return -v if isinstance(n.op, ast.USub) else ~v
        if isinstance(n, ast.BinOp):
            a, b = ev(n.left), ev(n.right)
            ops = {ast.LShift: lambda: a << b, ast.RShift: lambda: a >> b, ast.BitOr: lambda: a | b, ast.BitAnd: lambda: a & b,
                   ast.Add: lambda: a + b, ast.Sub: lambda: a - b, ast.Mult: lambda: a * b, ast.BitXor: lambda: a ^ b}
            f = ops.get(type(n.op))
            if f:
                return f()
        raise ValueError(e)
    try:
        return ev(node)
    except (ValueError, TypeError, RecursionError):
        return None


# ---- the parser -----------------------------------------------------------------------------------

CLASS_RE = re.compile(r"extern_class!\(")
OPENER_RE = re.compile(r"^(?P<indent>[ \t]*)(?:impl(?:<[^{]*?>)?\s+(?P<impl>\w+)(?:<[^{]*?>)?\s*\{"
                       r"|pub unsafe trait (?P<trait>\w+)\s*:[^{]*\{)", re.M)
METHOD_RE = re.compile(r"#\[unsafe\(method\(([^)]*)\)\)\]")
FN_RE = re.compile(r"(?:pub )?(unsafe )?fn (\w+)\s*\(")


def parse_params(inner: str) -> tuple[str | None, list[dict]]:
    recv = None
    params = []
    for p in split_top(inner):
        if not p:
            continue
        if p in ("&self", "&mut self", "self"):
            recv = "self"
            continue
        name, _, ty = p.partition(":")
        ty = norm_ty(ty)
        if name.strip() == "this" and ty.startswith("Allocated<"):
            recv = "alloc"
            continue
        params.append({"name": name.strip(), "rust": ty})
    return recv, params


def preceding_attrs(code_src: str, start: int, floor: int) -> list[str]:
    """The attribute and doc lines directly above `start` (back to the previous item or `floor`)."""
    lines = code_src[floor:start].split("\n")
    out = []
    for line in reversed(lines[:-1] if lines and not lines[-1].strip() else lines):
        s = line.strip()
        if not s:
            if out:
                break
            continue
        if s.startswith("#[") or s.startswith("///") or s.startswith("//"):
            out.append(s)
            continue
        break
    return list(reversed(out))


def parse_file(path: str, fw: str, db: dict) -> None:
    src = Path(path).read_text()
    code = blank_comments(src)
    header = Path(path).stem

    # classes
    for m in CLASS_RE.finditer(code):
        end = match_close(code, m.end() - 1, "(", ")")
        block = src[m.end():end - 1]
        sm = re.search(r"pub struct (\w+)(<[^;]*>)?\s*;", block)
        if not sm:
            continue
        name = sm.group(1)
        sup = re.search(r"#\[unsafe\(super\(([^)]*)\)\)\]", block)
        chain = [s.strip().split("<")[0] for s in sup.group(1).split(",")] if sup else []
        cfg = preceding_attrs(src, m.start(), max(0, m.start() - 600))
        db["classes"].setdefault(name, {
            "framework": fw, "header": header, "super": chain, "generic": bool(sm.group(2)),
            "thread": "main" if "MainThreadOnly" in block else None,
            "deprecated": any(a.startswith("#[deprecated") for a in cfg) or "#[deprecated" in block,
            "send": False, "sync": False, "conforms": [], "methods": [],
        })
    for m in re.finditer(r"unsafe impl(?:<[^>]*>)? (Send|Sync) for (\w+)", code):
        db["_marks"].append((m.group(2), m.group(1).lower()))
    for m in re.finditer(r"extern_conformance!\(\s*unsafe impl(?:<[^{]*?>)? (\w+)(?:<[^{]*?>)? for (\w+)", code):
        db["_conf"].append((m.group(2), m.group(1)))
    for m in re.finditer(r"^unsafe impl (\w+) for (\w+)(?:<[^{]*>)? \{\}", code, re.M):
        db["_catimpl"].append((m.group(2), m.group(1)))

    # protocols: `extern_protocol!( ... pub unsafe trait P: A + B {`
    protos = {}
    for m in re.finditer(r"extern_protocol!\(", code):
        end = match_close(code, m.end() - 1, "(", ")")
        tm = re.search(r"pub unsafe trait (\w+)\s*:\s*([^{]*)\{", code[m.end():end])
        if tm:
            supers = [s.strip().split("<")[0] for s in tm.group(2).split("+")]
            protos[tm.group(1)] = (m.start(), end)
            db["protocols"].setdefault(tm.group(1), {"framework": fw, "header": header,
                                                     "inherits": [s for s in supers if s and s not in ("Sized",)],
                                                     "methods": []})

    # method owners: impl blocks and category traits (plus protocol traits)
    owners = []  # (start, end, kind, owner, category, inherited)
    for m in OPENER_RE.finditer(code):
        brace = code.index("{", m.start())
        end = match_close(code, brace, "{", "}")
        before = preceding_attrs(src, m.start(), max(0, m.start() - 800))
        if m.group("impl"):
            line = code[m.start():brace]
            if " for " in line:
                continue
            inherited = any("Methods declared on superclass" in a for a in before)
            owners.append((brace, end, "class", m.group("impl"), None, inherited))
        else:
            trait = m.group("trait")
            if trait in protos:
                owners.append((brace, end, "protocol", trait, None, False))
                continue
            cat_on = None
            for a in before:
                cm = re.search(r"Category(?: \"[^\"]*\")? on \[`(\w+)`\]", a)
                if cm:
                    cat_on = cm.group(1)
            owners.append((brace, end, "category", cat_on, trait, False))

    for mm in METHOD_RE.finditer(code):
        pos = mm.start()
        ctx = None
        for o in owners:
            if o[0] < pos < o[1] and (ctx is None or o[0] > ctx[0]):
                ctx = o
        if ctx is None:
            continue
        fm = FN_RE.search(code, mm.end())
        if not fm:
            continue
        pstart = fm.end() - 1
        pend = match_close(code, pstart, "(", ")")
        semi = pend
        depth = 0
        while semi < len(code):
            c = code[semi]
            if c in "<([":
                depth += 1
            elif c in ">)]" and not (c == ">" and code[semi - 1] == "-"):
                depth -= 1
            elif c in ";{" and depth <= 0:
                break
            semi += 1
        ret = code[pend:semi].strip()
        ret = norm_ty(ret[2:]) if ret.startswith("->") else None
        recv, params = parse_params(code[pstart + 1:pend - 1])
        # attributes: from the end of the previous item to the fn
        prev_end = max(code.rfind(";", 0, pos), code.rfind("extern_methods!(", 0, pos) + len("extern_methods!("))
        attrs = [a.strip() for a in src[prev_end + 1:fm.start()].split("\n") if a.strip()]
        fam = next((re.search(r"method_family = (\w+)", a).group(1) for a in attrs if "method_family" in a), "none")
        setter_for = next((re.search(r"Setter for \[`(\w+)`\]", a).group(1) for a in attrs if "Setter for [`" in a), None)
        safety, in_safety = [], False
        for a in attrs:
            if a.startswith("/// # Safety"):
                in_safety = True
                continue
            if in_safety and a.startswith("///"):
                t = a[3:].strip()
                if t:
                    safety.append(t)
            elif in_safety:
                in_safety = False
        sel = mm.group(1)
        rec = {
            "sel": sel[:-1] if sel.endswith("_") else sel, "rust": fm.group(2), "family": fam, "static": recv is None, "init": recv == "alloc",
            "unsafe": bool(fm.group(1)), "params": params, "ret": ret,
        }
        if sel.endswith("_"):
            rec["error_out"] = True
        if safety:
            rec["safety"] = safety
        if setter_for:
            rec["setter_for"] = setter_for
        if any(a.startswith("#[deprecated") for a in attrs):
            rec["deprecated"] = True
        if any(a == "#[optional]" for a in attrs):
            rec["optional"] = True
        if ctx[2] == "protocol":
            db["protocols"][ctx[3]]["methods"].append(rec)
            continue
        if ctx[2] == "category":
            rec["category"] = ctx[4]
            if ctx[3] is None:
                db["_pending_cat"].append((ctx[4], rec))
                continue
        if ctx[5]:
            rec["inherited"] = True
        db["_methods"].append((ctx[3], rec))

    # structs (records)
    for m in re.finditer(r"#\[repr\(C\)\]\s*(?:#\[[^\n]*\]\s*)*pub struct (\w+) \{([^}]*)\}", code):
        fields = []
        for f in split_top(m.group(2)):
            fm = re.match(r"pub (\w+)\s*:\s*(.+)", f.strip())
            if fm:
                fields.append([fm.group(1), norm_ty(fm.group(2))])
        if fields:
            db["structs"][m.group(1)] = {"framework": fw, "header": header, "fields": fields}

    # enums / options: `// NS_ENUM` ... `pub struct X(pub T);` ... `impl X { pub const A: Self = Self(v); }`
    for m in re.finditer(r"// (NS_ENUM|NS_OPTIONS|NS_CLOSED_ENUM|NS_TYPED_ENUM|NS_TYPED_EXTENSIBLE_ENUM)\s*\n(?:\s*#\[[^\n]*\]\s*\n)*\s*pub struct (\w+)\(pub ([\w:]+)\);", src):
        kind, name, repr = m.group(1), m.group(2), m.group(3)
        rec = db["enums"].setdefault(name, {"framework": fw, "header": header, "kind": "options" if kind == "NS_OPTIONS" else "enum",
                                            "repr": repr, "members": {}})
        im = re.search(r"impl " + re.escape(name) + r"(?::\s*[\w:]+)? \{", code[m.end():])
        if im:
            b = m.end() + im.end() - 1
            body = code[b:match_close(code, b, "{", "}")]
            for cm in re.finditer(r"(?:pub const (\w+): Self = Self\(([^;]*)\)|const (\w+) = ([^;]*));", body):
                mname, expr = (cm.group(1), cm.group(2)) if cm.group(1) else (cm.group(3), cm.group(4))
                known = {f"{pre}.{k}": x for k, x in rec["members"].items() if x is not None for pre in ("Self", name)}
                known.update({f"Self::{k}.bits()": x for k, x in rec["members"].items() if x is not None})
                expr = re.sub(r"Self::(\w+)\.bits\(\)", lambda q: str(rec["members"].get(q.group(1), "x")), expr)
                rec["members"][mname] = eval_int(expr, known)

    # typedefs and string constants
    for m in re.finditer(r"^pub type (\w+)(?:<[^=]*>)? = ([^;]+);", code, re.M):
        db["typedefs"].setdefault(m.group(1), norm_ty(m.group(2)))
    for m in re.finditer(r"pub static (\w+): (?:&'static )?([^;]+);", code):
        db["statics"].setdefault(m.group(1), {"framework": fw, "header": header, "rust": norm_ty(m.group(2))})


def ingest(registry: str) -> dict[str, dict]:
    out = {}
    for fw, crate in CRATES.items():
        roots = sorted(glob.glob(f"{registry}/*/{crate}-{VERSION}/src"))
        if not roots:
            raise SystemExit(f"{crate}-{VERSION} not found under {registry} (cargo fetch on any workspace that depends on it)")
        db = {"classes": {}, "protocols": {}, "structs": {}, "enums": {}, "typedefs": {}, "statics": {},
              "_marks": [], "_conf": [], "_catimpl": [], "_methods": [], "_pending_cat": []}
        files = sorted(glob.glob(f"{roots[0]}/generated/*.rs"))
        for f in files:
            if f.endswith("mod.rs"):
                continue
            parse_file(f, fw, db)
        # hand-written parts of objc2-foundation that matter: NSRange
        if fw == "Foundation":
            db["structs"].setdefault("NSRange", {"framework": fw, "header": "range", "fields": [["location", "NSUInteger"], ["length", "NSUInteger"]]})
        out[fw] = db
    # cross-framework resolution: marks, conformances, categories and methods attach to classes in either crate
    classes = {n: c for db in out.values() for n, c in db["classes"].items()}
    for fw, db in out.items():
        for n, mark in db["_marks"]:
            if n in classes:
                classes[n][mark] = True
        for n, p in db["_conf"]:
            if n in classes and p not in classes[n]["conforms"]:
                classes[n]["conforms"].append(p)
        cat_target = {cat: n for n, cat in db["_catimpl"]}
        for cat, rec in db["_pending_cat"]:
            n = cat_target.get(cat)
            if n:
                db["_methods"].append((n, rec))
        for n, rec in db["_methods"]:
            if n in classes:
                rec["framework"] = fw
                classes[n]["methods"].append(rec)
    result = {}
    for fw, db in out.items():
        for c in db["classes"].values():
            c["conforms"].sort()
            # one record per (selector, static): "Methods declared on superclass" re-declarations repeat init/new
            seen, uniq = set(), []
            for r in c["methods"]:
                key = (r["sel"], r["static"])
                if key in seen:
                    continue
                seen.add(key)
                uniq.append(r)
            c["methods"] = sorted(uniq, key=lambda r: (r["static"], r["sel"]))
        result[fw] = {
            "schema": "svelters-objc2-ingest/v1", "framework": fw, "crate": CRATES[fw], "version": VERSION,
            "classes": dict(sorted(db["classes"].items())), "protocols": dict(sorted(db["protocols"].items())),
            "structs": dict(sorted(db["structs"].items())), "enums": dict(sorted(db["enums"].items())),
            "typedefs": dict(sorted(db["typedefs"].items())), "statics": dict(sorted(db["statics"].items())),
        }
    return result


def _c(x) -> str:
    return json.dumps(x, ensure_ascii=False, separators=(",", ":"))


def serialise(d: dict) -> str:
    """Compact but diffable: one line per enum/struct/typedef/static, one line per method of a class or protocol."""
    out = ["{"]
    keys = list(d)
    for i, k in enumerate(keys):
        v = d[k]
        tail = "," if i < len(keys) - 1 else ""
        if k in ("classes", "protocols"):
            recs = []
            for n, x in v.items():
                head = {a: b for a, b in x.items() if a != "methods"}
                ms = ",\n".join(f"   {_c(m)}" for m in x["methods"])
                recs.append(f"  {_c(n)}: {_c(head)[:-1]},\"methods\":[\n{ms}\n  ]}}" if x["methods"] else f"  {_c(n)}: {_c(x)}")
            out.append(f" {_c(k)}: {{\n" + ",\n".join(recs) + f"\n }}{tail}")
        elif isinstance(v, dict):
            body = ",\n".join(f"  {_c(n)}: {_c(x)}" for n, x in v.items())
            out.append(f" {_c(k)}: {{\n{body}\n }}{tail}")
        else:
            out.append(f" {_c(k)}: {_c(v)}{tail}")
    out.append("}")
    return "\n".join(out) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--registry", default=os.path.expanduser("~/.cargo/registry/src"))
    ap.add_argument("--check", action="store_true", help="fail if the checked-in files differ")
    a = ap.parse_args()
    data = ingest(a.registry)
    bad = 0
    for fw, d in data.items():
        p = OUT / f"{fw}.json"
        text = serialise(d)
        if a.check:
            if not p.exists() or p.read_text() != text:
                print(f"stale: {p.relative_to(ROOT)}", file=sys.stderr)
                bad += 1
            continue
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
        n_m = sum(len(c["methods"]) for c in d["classes"].values())
        print(f"{p.relative_to(ROOT)}: {len(d['classes'])} classes, {n_m} methods, {len(d['protocols'])} protocols, "
              f"{len(d['structs'])} structs, {len(d['enums'])} enums, {len(d['typedefs'])} typedefs, {len(d['statics'])} statics")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
