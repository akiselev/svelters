"""Docs pages, ledger facts and generated tests of the curated types (part of gen_types.py).

For every curated type (metadata/overrides/gtk/types/gtk-types.yaml `curated`) this writes:

  crates/svelters-gtk/tests/types_verify.rs   one test per fact group, run on the real backend
  metadata/gtk/types/facts.json               the ledger manifest: fact id -> tests (+ a digest of the facts)
  docs/site/src/content/docs/types/gtk/       one page per curated type, plus an index of all bound types

Fact ids follow the widget ledger: `svelters-api:v1:type:gtk:<Ns>:<Type>:<group>` with the groups
`construct`, `props`, `methods`, `signals` and `lifetime`, plus `async` (N1) for types with folded
`_async`/`_finish` members: every async member answers an `SVL_M_ASYNC` descriptor ending in the
completion callback, and those a generic test can call (reader-like names whose arguments are an
`io_priority`, flags, `attributes` or nullable) complete exactly once, again after a cancel. A subject (the instance the tests use) comes from
the override `verify_subject`; members the tests must not call come from `verify_skip`.
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

import types_facts as TF

ROOT = TF.ROOT
DOCS = ROOT / "docs/site/src/content/docs/types/gtk"
MEMBERS_TEST = ROOT / "crates/svelters-gtk/tests/types_members.rs"
TESTS = ROOT / "crates/svelters-gtk/tests/types_verify.rs"
FACT_FILE = TF.FACTS_DIR / "facts.json"
READER = re.compile(r"^(get|is|has|peek|supports|dup|hash|to|can|contains|n)(_|$)")
GROUPS = ("construct", "props", "methods", "signals", "lifetime")
ASYNC_READER = re.compile(r"^(load|query|enumerate|find|read)_")


def async_call_args(m: dict) -> list[str] | None:
    """Harness arguments a generic test can give an async member, or None."""
    if not ASYNC_READER.match(m["name"]):
        return None
    out = []
    for p in m["params"]:
        if p.get("hidden") or p.get("dir") == "out":
            continue
        t = p["type"]
        if p["name"] == "io_priority" and t["k"] == "int":
            out.append("V::I64(0)")
        elif t["k"] == "flags":
            out.append("V::List(vec![])")
        elif t["k"] == "str" and p["name"] == "attributes":
            out.append('V::Str("standard::*".into())')
        elif p.get("nullable"):
            out.append("V::Null")
        else:
            return None
    return out


def rust_str(s: str) -> str:
    return json.dumps(s)


def v_of(x) -> str:
    """A YAML scalar/list as a harness `V` expression."""
    if x is None:
        return "V::Null"
    if isinstance(x, bool):
        return f"V::Bool({str(x).lower()})"
    if isinstance(x, int):
        return f"V::I64({x})"
    if isinstance(x, float):
        return f"V::F64({x})"
    if x == "$png":
        return "V::Str(png_path())"
    if isinstance(x, str):
        return f"V::Str({rust_str(x)}.into())"
    if isinstance(x, list):
        return "V::List(vec![" + ", ".join(v_of(i) for i in x) + "])"
    raise TypeError(x)


def ident(q: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", q.lower()).strip("_")


def subject_fn(q: str, spec: dict, rec: dict) -> str:
    """A Rust function `subject_<q>(h) -> u64` (a handle) following the override recipe."""
    name = f"subject_{ident(q)}"
    lines = [f"fn {name}(h: &Harness) -> u64 {{"]
    cur = None

    def step(s: dict, recv: str | None) -> str:
        if "static" in s:
            kind, method, args = s["static"]
            return f"{'handle' if rec['category'] != 'value' else 'boxed'}(h.call_static({rust_str(kind)}, {rust_str(method)}, &[{', '.join(v_of(a) for a in args)}]).unwrap())"
        if "construct" in s:
            props = ", ".join(f"({rust_str(k)}, {v_of(v)})" for k, v in s["construct"].items())
            return f"h.construct({rust_str(q)}, &[{props}]).unwrap()"
        if "call" in s:
            method, *args = s["call"]
            return f"handle(h.call_args({recv}, {rust_str(method)}, &[{', '.join(v_of(a) for a in args)}]).unwrap())"
        raise KeyError(s)

    steps = spec.get("chain") or [spec]
    for i, s in enumerate(steps):
        expr = step(s, cur)
        if i == len(steps) - 1:
            lines.append(f"    {expr}")
        else:
            lines.append(f"    let s{i} = {expr};")
            cur = f"s{i}"
    lines.append("}")
    return "\n".join(lines)


def members(g, q: str) -> dict:
    eff = g.effective(q)
    return eff


def zero_arg(m: dict) -> bool:
    return not [p for p in m["params"] if not p.get("hidden") and p.get("dir") != "out"] and not m.get("throws")


def since_guard(member: dict, present: str, body: list[str]) -> list[str]:
    """Statements for one member. A member introduced in a release (`since`) may be absent from an older
    installed library (the Ubuntu GTK 4.14 job), so it is skipped at run time when it does not resolve;
    members without `since` stay strict."""
    if not member.get("since"):
        return ["        " + b for b in body]
    name = member["name"]
    return (
        [f"        if {present} {{"]
        + ["            " + b for b in body]
        + ["        } else {", f"            eprintln!(\"SKIP {name}: not in the running library (since {member['since']})\");", "        }"]
    )


def gen_tests(g, ov: dict) -> tuple[str, list[dict]]:
    curated = [q for q in sorted(g.classes) if g.types[q]["curated"]]
    subjects = ov.get("verify_subject") or {}
    skips = ov.get("verify_skip") or {}
    out = [
        "// Generated by tools/metadata/gtk/gen_types.py from metadata/gtk/types/*.json. Do not edit.",
        "//! Ledger tests of the curated non-GUI types (NT0): per type, construction, property reads and",
        "//! write-backs, zero-argument readers, signal connection and the SHARED/VALUE lifetime; async members",
        "//! (N1): descriptors, one completion per call, completion after a cancel. Needs a display.",
        "#![allow(clippy::all, unused)]",
        "",
        "use gtk::prelude::*;",
        "use svelters_abi::*;",
        "use svelters_gtk::testing::{on_gtk, pump, Harness, V};",
        "",
        "fn handle(v: V) -> u64 {",
        "    match v {",
        "        V::Handle(h) | V::Boxed(h) => h,",
        "        other => panic!(\"expected a handle, got {other:?}\"),",
        "    }",
        "}",
        "",
        "fn boxed(v: V) -> u64 {",
        "    handle(v)",
        "}",
        "",
        "/// A 1x1 PNG, written once per process, for the types that load an image file.",
        "fn png_path() -> String {",
        "    let path = std::env::temp_dir().join(format!(\"svl-types-verify-{}.png\", std::process::id()));",
        "    if !path.exists() {",
        "        let pixel = gtk::glib::Bytes::from(&[0u8, 0, 0, 255][..]);",
        "        let _ = gtk::gdk::MemoryTexture::new(1, 1, gtk::gdk::MemoryFormat::R8g8b8a8, &pixel, 4).save_to_png(&path);",
        "    }",
        "    path.to_string_lossy().into_owned()",
        "}",
        "",
    ]
    facts: list[dict] = []
    for q in curated:
        rec = g.types[q]
        spec = subjects.get(q)
        if spec is None:
            raise SystemExit(f"gen_types: curated type {q} has no verify_subject in {TF.OVERRIDES}")
        eff = members(g, q)
        skip = set(skips.get(q) or [])
        x = ident(q)
        out.append(subject_fn(q, spec, rec))
        out.append("")
        value = rec["category"] == "value"
        # construct: the subject exists, has the right lifetime and kind chain
        life = "SVL_LIFE_VALUE" if value else "SVL_LIFE_SHARED"
        out += [
            "#[test]",
            f"fn {x}_construct() {{",
            "    on_gtk(|| {",
            "        let h = Harness::new();",
            f"        let s = subject_{x}(&h);",
            f"        assert_eq!(h.handle_info(s).unwrap().lifetime, {life});",
            f"        assert!(h.type_chain(s).iter().any(|n| n == {rust_str(q)}) || {str(rec['kind'] == 'interface').lower()} || {str(bool(rec.get('interfaces'))).lower()}, \"{{:?}}\", h.type_chain(s));",
            "    });",
            "}",
            "",
        ]
        # props
        lines = []
        for p in eff["properties"]:
            if p["name"] in skip or not p["readable"]:
                continue
            body_ = [f"let _ = h.get(s, {rust_str(p['name'])}); // {p['type']['k']}"]
            if p["writable"] and not p["construct_only"] and p["type"]["k"] in ("bool", "int", "float", "str", "enum"):
                body_.append(f"let cur = h.get_value(s, {rust_str(p['name'])});")
                body_.append(f"h.set_value(s, {rust_str(p['name'])}, &cur).unwrap();")
                body_.append(f"assert_eq!(h.get_value(s, {rust_str(p['name'])}), cur, {rust_str('write-back of ' + p['name'])});")
            lines += since_guard(p, f"h.has_prop(s, {rust_str(p['name'])})", body_)
        out += ["#[test]", f"fn {x}_props() {{", "    on_gtk(|| {", "        let h = Harness::new();", f"        let s = subject_{x}(&h);"] + (lines or ["        let _ = &s;"]) + ["    });", "}", ""]
        # methods
        ml = []
        for m in eff["methods"]:
            if m["name"] in skip or not READER.match(m["name"]) or not zero_arg(m):
                continue
            ml += since_guard(m, f"h.resolve_method(s, {rust_str(m['name'])}) != 0", [f"h.call_args(s, {rust_str(m['name'])}, &[]).unwrap_or_else(|e| panic!(\"{m['name']}: {{e}}\"));"])
        out += ["#[test]", f"fn {x}_methods() {{", "    on_gtk(|| {", "        let h = Harness::new();", f"        let s = subject_{x}(&h);"] + (ml or ["        let _ = &s;"]) + ["    });", "}", ""]
        # signals
        sl = []
        if not value:
            for s_ in eff["signals"]:
                if s_["name"] in skip:
                    continue
                sl += since_guard(s_, f"h.has_signal(s, {rust_str(s_['name'])})", [
                    f"h.listen(s, {rust_str(s_['name'])}, true).unwrap_or_else(|e| panic!(\"{s_['name']}: {{e}}\"));",
                    f"h.listen(s, {rust_str(s_['name'])}, false).unwrap();",
                ])
        out += ["#[test]", f"fn {x}_signals() {{", "    on_gtk(|| {", "        let h = Harness::new();", f"        let s = subject_{x}(&h);"] + (sl or ["        let _ = &s;"]) + ["    });", "}", ""]
        # lifetime
        if value:
            body = [
                f"        let a = subject_{x}(&h);",
                f"        let b = subject_{x}(&h);",
                "        assert_ne!(a, b, \"a value has no identity\");",
                "        h.destroy(a);",
                "        assert!(h.handle_info(a).is_err(), \"released: stale\");",
                "        h.destroy(a);",
                "        h.destroy(b);",
            ]
        else:
            body = [
                f"        let s = subject_{x}(&h);",
                "        let weak = h.obj(s).downgrade();",
                "        let live = h.backend.live_handles();",
                "        pump(30);",
                "        assert!(weak.upgrade().is_some(), \"SHARED keeps the object alive\");",
                "        h.destroy(s);",
                "        pump(60);",
                "        assert!(h.handle_info(s).is_err(), \"released: stale\");",
                "        h.destroy(s);",
                "        assert!(h.backend.live_handles() < live, \"nothing else leaked a handle\");",
            ]
        out += ["#[test]", f"fn {x}_lifetime() {{", "    on_gtk(|| {", "        let h = Harness::new();"] + body + ["    });", "}", ""]
        # async (N1)
        groups = list(GROUPS)
        amembers = [m for m in eff["methods"] if m.get("async") and m["name"] not in skip]
        if amembers:
            groups.append("async")
            al = []
            cancelled = False
            for m in amembers:
                n = rust_str(m["name"])
                b = [
                    f"let d = h.method_desc(s, {n}).expect({rust_str(m['name'] + ': no descriptor')});",
                    f"assert!(d.flags & SVL_M_ASYNC != 0, {rust_str(m['name'] + ' is async')});",
                    f"assert_eq!(d.params.last().map(|p| p.tag), Some(SVL_CALLBACK), {rust_str(m['name'] + ': completion slot')});",
                ]
                args = async_call_args(m)
                if args is not None:
                    a = ", ".join(args)
                    b += [
                        f"let cb = h.call_async(s, {n}, &[{a}]).unwrap_or_else(|e| panic!(\"{m['name']}: {{e}}\"));",
                        f"let v = h.wait_completion(cb, 10_000).expect({rust_str(m['name'] + ': no completion')});",
                        f"eprintln!(\"{m['name']}: {{v:?}}\");",
                        f"assert!(h.wait_completion(cb, 30).is_none(), {rust_str(m['name'] + ': completed twice')});",
                    ]
                    if not cancelled:
                        cancelled = True
                        b += [
                            f"let cb = h.call_async(s, {n}, &[{a}]).unwrap();",
                            "h.cancel(cb);",
                            f"h.wait_completion(cb, 10_000).expect({rust_str(m['name'] + ': no completion after cancel')});",
                        ]
                al += since_guard(m, f"h.resolve_method(s, {n}) != 0", b)
            out += ["#[test]", f"fn {x}_async() {{", "    on_gtk(|| {", "        let h = Harness::new();", f"        let s = subject_{x}(&h);"] + al + ["    });", "}", ""]
        digest = hashlib.sha256(json.dumps(rec, sort_keys=True).encode()).hexdigest()[:16]
        for grp in groups:
            facts.append({
                "fact": f"svelters-api:v1:type:gtk:{rec['ns']}:{rec['name']}:{grp}",
                "tests": [f"types_verify::{x}_{grp}"],
                "kind": rec["name"], "name": grp, "section": "types", "facts_digest": digest,
            })
    return "\n".join(out) + "\n", facts


MEMBERS_RS_HEAD = r'''// Generated by tools/metadata/gtk/gen_types.py from metadata/gtk/types/*.json. Do not edit.
//! The facts and the backend agree (N1, every bound member since FU3): every bound method, static and
//! constructor of every bound type resolves through the GI invoker with a descriptor whose async and
//! static flags and JS argument count match the facts
//! (callback user data, destroy notifies, GCancellable/GAsyncReadyCallback of async members and array
//! lengths are hidden on both sides). A member with a `since` newer than the running library (the
//! GTK 4.14 job) is skipped; on a library that has it, it must resolve like any other. Needs a display.
#![allow(clippy::all)]

use svelters_abi::*;
use svelters_gtk::testing::{on_gtk, Harness};

/// (type, member, flags (1 async, 2 static), JS arguments incl. the completion, `since` or "")
type Row = (&'static str, &'static str, u32, usize, &'static str);

/// Whether the running library of `q`'s namespace is at least `since` ("2.74", "4.16").
fn available(q: &str, since: &str) -> bool {
    let mut v = since.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let (maj, min) = (v.next().unwrap_or(0), v.next().unwrap_or(0));
    match q.split('.').next().unwrap_or("") {
        "GLib" | "GObject" | "Gio" => gtk::glib::check_version(maj, min, 0).is_none(),
        "Gtk" | "Gdk" | "Gsk" => (gtk::major_version(), gtk::minor_version()) >= (maj, min),
        "Pango" => gtk::pango::version_check(maj as i32, min as i32, 0).is_none(),
        // Graphene has no run-time version: the build minimum has every bound member.
        _ => true,
    }
}

fn check(rows: &'static [Row]) {
    on_gtk(move || {
        let h = Harness::new();
        let mut bad = Vec::new();
        for &(q, m, flags, n, since) in rows {
            let Some((_, desc)) = h.kind_method(q, m) else {
                if since.is_empty() || available(q, since) {
                    bad.push(format!("{q}.{m}: does not resolve"));
                } else {
                    eprintln!("SKIP {q}.{m}: not in the running library (since {since})");
                }
                continue;
            };
            let Some(d) = desc else {
                bad.push(format!("{q}.{m}: no descriptor (the invoker refuses it)"));
                continue;
            };
            if (d.flags & SVL_M_ASYNC != 0) != (flags & 1 != 0) {
                bad.push(format!("{q}.{m}: async flag differs"));
            }
            if (d.flags & SVL_M_STATIC != 0) != (flags & 2 != 0) {
                bad.push(format!("{q}.{m}: static flag differs"));
            }
            let js = d.params.iter().filter(|p| p.flags & SVL_D_OUT == 0).count();
            if js != n {
                bad.push(format!("{q}.{m}: {js} JS arguments, the facts say {n}"));
            }
        }
        assert!(bad.is_empty(), "{} of {} members disagree:\n{}", bad.len(), rows.len(), bad.join("\n"));
    });
}
'''


def gen_members_test(g) -> str:
    """Every bound method, static and constructor of every bound type, checked against the running
    typelib (see the header of the generated file)."""
    rows: dict[str, list[str]] = {}
    for q in sorted(g.classes):
        r = g.types[q]
        for kind in ("constructors", "statics", "methods"):
            for m in r[kind]:
                n_js = len([p for p in m["params"] if not p.get("hidden") and p.get("dir") != "out"]) + (1 if m.get("async") else 0)
                flags = (1 if m.get("async") else 0) | (2 if kind != "methods" else 0)
                rows.setdefault(r["ns"], []).append(f"    ({rust_str(q)}, {rust_str(m['name'])}, {flags}, {n_js}, {rust_str(m.get('since') or '')}),")
    out = [MEMBERS_RS_HEAD]
    for ns, rs in sorted(rows.items()):
        c = ns.upper()
        out.append(f"const {c}: &[Row] = &[")
        out += rs
        out += ["];", "", "#[test]", f"fn members_of_{ns.lower()}_resolve() {{", f"    check({c});", "}", ""]
    return "\n".join(out)


def md_type(g, q: str) -> str:
    rec = g.types[q]
    eff = g.effective(q)
    value = rec["category"] == "value"
    ns = rec["ns"]
    imp = f"import {{ {rec['name']} }} from 'svelters:gtk/{ns.lower()}';"
    out = ["---", f"title: {ns}.{rec['name']}", f"description: \"{ns}.{rec['name']} from svelters:gtk/{ns.lower()}, a {'value' if value else 'shared object'} class generated from the GIR.\"", "---", ""]
    out += [f"```js\n{imp}\n```", ""]
    life = ("a **VALUE** handle: an immutable snapshot with no identity. Its mutating methods are not exposed. "
            "`dispose()` releases it now, otherwise the garbage collector does.") if value else (
        "a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is "
        "collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.")
    out += ["## Class", "", f"`{q}`, GType `{rec['gtype']}`" + (f", since {rec['since']}" if rec.get("since") else "") + ".", ""]
    out += [f"Lifetime: {life}", ""]
    if rec.get("parent") or rec.get("interfaces"):
        bits = []
        if rec.get("parent"):
            bits.append(f"extends `{rec['parent']}`")
        if rec.get("interfaces"):
            bits.append("implements " + ", ".join(f"`{i}`" for i in rec["interfaces"]))
        out += ["It " + " and ".join(bits) + ".", ""]
    out += ["Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:" + f"{ns}:{rec['name']}:*`).", ""]
    if not value and rec["kind"] == "class" and not rec["abstract"]:
        props = [p for p in g.all_props(q) if p["writable"] or p["construct_only"] or p["construct"]]
        out += ["## Construct", "", "```js", f"const x = new {rec['name']}({{ " + ", ".join(f"{p['js']}: ..." for p in props[:3]) + " });", "```", "",
                "Construct-time properties (construct-only ones can only be given here): " + ", ".join(f"`{p['js']}`" + (" (construct-only)" if p["construct_only"] else "") for p in props) + ".", ""]
    if eff["constructors"] or eff["statics"]:
        out += ["## Statics", ""]
        for m in eff["constructors"] + eff["statics"]:
            out.append(f"- `{rec['name']}.{TF.camel(m['name'])}({', '.join(p['js'] for p in m['params'] if not p.get('hidden') and p.get('dir') != 'out')})`" + (f", since {m['since']}" if m.get("since") else ""))
        out.append("")
    if eff["properties"]:
        out += ["## Properties", "",
                "Getters marked *reactive* re-run the effects (and template expressions) that read them when the property "
                "changes natively; GTK announces the change (`notify`) and the host delivers it once per main-loop turn. "
                "*Reactive on writes* re-runs only after your own writes. The other getters are snapshots. To read a reactive "
                "getter without subscribing, use `obj.peek('prop')` or Svelte's `untrack(() => obj.prop)`. "
                "See [Reactivity](../../../concepts/reactivity/).", ""]
        for p in eff["properties"]:
            fact = g.observe_fact(p) or {}
            cls = fact.get("class")
            react = {"exact": "reactive", "partial": "reactive, misses changes by " + ", ".join(fact.get("missed") or ["some causes"]),
                     "set_only": "reactive on writes", "none": "not reactive: changes are not announced",
                     "constant": None}.get(cls) if p["readable"] else None
            if react and fact.get("value_cost") == "proportional":
                react += "; each read copies the whole value"
            flags = ", ".join(x for x, on in (("read-only", not p["writable"]), ("construct-only", p["construct_only"]), (react, bool(react))) if on)
            out.append(f"- `{p['js']}` ({p['type']['k']}{', ' + flags if flags else ''})")
        out.append("")
    sync = [m for m in eff["methods"] if not m.get("async")]
    asyncs = [m for m in eff["methods"] if m.get("async")]
    if sync:
        out += ["## Methods", ""]
        for m in sync:
            cbs = [p for p in m["params"] if p["type"]["k"] == "callback"]
            note = "".join(f"; `{p['js']}` is a callback ({p['type']['scope']} scope)" for p in cbs)
            out.append(f"- `{TF.camel(m['name'])}({', '.join(p['js'] for p in m['params'] if not p.get('hidden') and p.get('dir') != 'out')})`" + (f", since {m['since']}" if m.get("since") else "") + note)
        out.append("")
    if asyncs:
        out += ["## Async methods", "",
                "Each folds a native `_async`/`_finish` pair into one call that returns a Promise. It resolves with the finish result "
                "(an object of the out parameters where there are any) and rejects with a `NativeError`. The native `GCancellable` is not exposed: "
                "pass `{ signal }` (an `AbortSignal`) as the last argument to cancel, which rejects with code `cancelled`. "
                "See [Async calls and callbacks](../async/).", ""]
        for m in asyncs:
            vis = [p["js"] for p in m["params"] if not p.get("hidden") and p.get("dir") != "out"]
            outs = [TF.camel(p["name"]) for p in m["async"]["outs"] if not p.get("hidden")]
            res = ("{ " + ", ".join((["ret"] if m["ret"]["k"] != "void" else []) + outs) + " }") if outs else ("void" if m["ret"]["k"] == "void" else m["ret"]["k"])
            out.append(f"- `{TF.camel(m['name'])}({', '.join(vis + ['{ signal }?'])})` → `Promise<{res}>` (`{m['name']}` + `{m['async']['finish']}`)" + (f", since {m['since']}" if m.get("since") else ""))
        out.append("")
    if rec.get("hidden"):
        out += ["## Hidden mutators", "", "Value handles are immutable in JS; these native methods are not exposed: " + ", ".join(f"`{h}`" for h in rec["hidden"]) + ".", ""]
    if eff["signals"]:
        out += ["## Events", "", "Listen with `obj.on('name', fn)`; it returns its disposer.", ""]
        for s in eff["signals"]:
            out.append(f"- `{s['name']}`")
        out.append("")
    if rec["blocked"]:
        out += ["## Not bound yet", "", f"{len(rec['blocked'])} members are not bound; reasons: " + ", ".join(f"{k} {v}" for k, v in sorted(__import__('collections').Counter(r.split(':')[0] for r in rec['blocked'].values()).items())) + ".", ""]
    return "\n".join(out) + "\n"


def md_index(g) -> str:
    out = ["---", "title: Non-UI types (GTK)", "description: \"Gio, GLib, Gdk, Gsk, Pango and Gtk classes as ES module named exports (svelters:gtk/gio, ...).\"", "---", "",
           "Every non-UI GTK type is a named export of a `svelters:gtk/<namespace>` module. Elements stay tags; objects, models and values are imported:", "",
           "```svelte", "<script>", "  import { TextBuffer } from 'svelters:gtk/gtk';", "  const buffer = new TextBuffer({ text: 'Hello' });", "</script>", "<text-view {buffer} />", "```", "",
           "- **Objects** (`Gio.File`, `Gtk.StringList`) are SHARED handles: one JS object per native object, `===` works, the object lives while the wrapper or anything else holds it.",
           "- **Values** (`Pango.FontDescription`, `Gdk.ContentFormats`) are VALUE handles: immutable, no identity, `equals`, `toString`, `dispose`.",
           "- **Release builds** keep only what an app imports, plus the types its signatures mention. The manifest is `packages/types/gtk/manifest.json`.", "",
           "- **Async methods** (`file.loadContentsAsync()`, `dialog.open(window)`) return Promises; `{ signal }` cancels. **Callbacks** are plain JS functions. See [Async calls and callbacks](async/).", "",
           "The curated set below has a page and ledger-verified tests; the other bound types are generated from the same facts.", "", "## Curated types", ""]
    for q in sorted(g.classes):
        r = g.types[q]
        if r["curated"]:
            out.append(f"- [{r['ns']}.{r['name']}]({r['ns'].lower()}-{r['name'].lower()}/)")
    out += ["", "## Bound types by namespace", ""]
    by: dict[str, list[str]] = {}
    for q in sorted(g.classes):
        by.setdefault(g.types[q]["ns"], []).append(g.types[q]["name"])
    for ns, names in sorted(by.items()):
        out.append(f"- `svelters:gtk/{ns.lower()}` ({len(names)}): " + ", ".join(names))
    return "\n".join(out) + "\n"


def files(g) -> dict[Path, str]:
    ov = TF.load_overrides()
    tests, facts = gen_tests(g, ov)
    out: dict[Path, str] = {TESTS: tests, MEMBERS_TEST: gen_members_test(g)}
    out[FACT_FILE] = json.dumps({"schema": "svelters-types-facts/v1", "toolkit": "gtk", "facts": facts}, indent=1, sort_keys=True) + "\n"
    out[DOCS / "index.md"] = md_index(g)
    for q in sorted(g.classes):
        r = g.types[q]
        if r["curated"]:
            out[DOCS / f"{r['ns'].lower()}-{r['name'].lower()}.md"] = md_type(g, q)
    return out
