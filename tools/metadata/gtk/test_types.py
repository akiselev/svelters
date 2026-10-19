"""Tests of the non-GUI types pipeline (NT0): facts, generated modules, the release manifest and its closure.

They read only checked-in files, so they run on any GTK (the 4.14 CI job included). Set
SVELTERS_CHECK_GIR=1 to also compare the facts with the GIR installed on the machine.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import gen_types as GT  # noqa: E402
import types_closure as TC  # noqa: E402
import types_facts as TF  # noqa: E402

PKG = ROOT / "packages/types"


@pytest.fixture(scope="module")
def facts():
    return GT.load_facts()


@pytest.fixture(scope="module")
def gen(facts):
    return GT.Gen(facts)


@pytest.fixture(scope="module")
def manifest():
    return TC.load()


def test_generated_output_is_current():
    r = subprocess.run([sys.executable, "-I", str(HERE / "gen_types.py"), "--check"], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr


@pytest.mark.skipif(not os.environ.get("SVELTERS_CHECK_GIR"), reason="compares with the installed GIR")
def test_facts_match_the_installed_gir():
    assert TF.check(TF.build()) == 0


def test_the_curated_set_is_bound_with_the_expected_categories(facts):
    want = {"Gio.File": "object", "Gio.ListStore": "model", "Gtk.TextBuffer": "object", "Gtk.StringList": "model",
            "Gtk.DirectoryList": "model", "Gtk.FileDialog": "object", "Gdk.Texture": "object", "Gdk.Clipboard": "object",
            "Pango.AttrList": "value", "Pango.FontDescription": "value"}
    for q, cat in want.items():
        assert facts[q]["category"] == cat, q
        assert facts[q]["curated"], q
    assert sum(1 for r in facts.values() if r["curated"]) == len(want)


def test_value_mutators_are_hidden_and_readers_kept(facts):
    fd = facts["Pango.FontDescription"]
    names = {m["name"] for m in fd["methods"]}
    assert {"get_family", "get_size", "to_string", "equal", "hash"} <= names
    assert not {"set_family", "set_size", "merge", "set_style"} & names
    assert {"set_family", "set_size", "merge"} <= set(fd["hidden"])
    ti = facts["Gtk.TextIter"]
    assert "forward_char" in ti["hidden"] and "get_offset" in {m["name"] for m in ti["methods"]}
    assert not ti["undecided"], "mutator rules decide every TextIter method"
    for q, r in facts.items():
        if r["category"] == "value" and not r["builder"]:
            assert not r["undecided"] or q in ("Gtk.TextIter",), f"{q} has undecided mutators {r['undecided']}"


def test_every_bound_member_is_marshallable_by_the_n0_invoker(facts):
    """No bound member mentions a callback, an async pair, a variant (outside properties), a hash table or a
    by-value struct: the rules block them with a reason instead."""
    bad = {"callback", "async", "variant", "hash table", "struct by value", "fundamental", "gpointer"}
    for q, r in facts.items():
        for m in r["constructors"] + r["statics"] + r["methods"]:
            blob = json.dumps([m["ret"]] + [p["type"] for p in m["params"]])
            assert '"k": "variant"' not in blob and '"k": "callback"' not in blob, f"{q}.{m['name']}"
        assert not (set(r["blocked"].values()) - set(r["blocked"].values())) & bad
        for name, why in r["blocked"].items():
            assert why, (q, name)


def test_the_async_and_callback_members_are_reported_blocked_for_n1(facts):
    f = facts["Gio.File"]
    assert f["blocked"]["load_contents_async"] == "async"
    assert f["blocked"].get("read_async") == "async"
    assert "load_contents" in {m["name"] for m in f["methods"]}


def test_classes_have_one_export_per_file_and_nothing_at_import_time(gen):
    n = 0
    for q in gen.classes:
        r = gen.types[q]
        p = PKG / "gtk" / r["ns"].lower() / f"{r['name']}.js"
        text = p.read_text()
        top = [l for l in text.splitlines() if l and not l.startswith((" ", "}", "//"))]
        assert top and all(re.match(r"(import \{|export class )", l) for l in top), (p, top[:3])
        assert len([l for l in top if l.startswith("export class ")]) == 1
        n += 1
    assert n == len(gen.classes) > 250
    pkg = json.loads((PKG / "package.json").read_text())
    assert pkg["sideEffects"] is False
    for mod in {f"svelters:gtk/{r['ns'].lower()}" for r in gen.types.values()}:
        assert f"./gtk/{mod.rsplit('/', 1)[1]}" in pkg["exports"]


def test_the_manifest_lists_every_class_with_its_native_name_and_imports(gen, manifest):
    seen = 0
    for mod, m in manifest["modules"].items():
        for name, e in m["exports"].items():
            seen += 1
            src = (PKG / "gtk" / e["file"]).read_text()
            # the file imports exactly the classes the manifest says
            imported = set(re.findall(r"^import \{[^}]*\} from '(\.[^']+)';$", src, re.M)) - {"../../src/core.js"}
            want = {f"./{gen.types[i]['name']}.js" if gen.types[i]["ns"] == gen.types[e["native"]]["ns"] else f"../{gen.types[i]['ns'].lower()}/{gen.types[i]['name']}.js" for i in e["imports"]}
            assert imported == want, (e["native"], imported ^ want)
            assert f"static native = '{e['native']}'" in src
    assert seen == len(gen.classes)


def test_import_cycles_never_contain_an_extends_edge(gen, manifest):
    """An ES import cycle through `extends` hits the temporal dead zone for one entry order."""
    idx = TC.by_native(manifest)
    edges = {n: set(e["imports"]) | ({e["extends"]} if e["extends"] else set()) for n, e in idx.items()}
    ext = {n: e["extends"] for n, e in idx.items() if e["extends"]}
    # Tarjan SCCs
    index, low, onstack, stack, sccs = {}, {}, set(), [], []
    counter = [0]

    def strong(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        onstack.add(v)
        for w in edges[v]:
            if w not in index:
                strong(w)
                low[v] = min(low[v], low[w])
            elif w in onstack:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = set()
            while True:
                w = stack.pop()
                onstack.discard(w)
                comp.add(w)
                if w == v:
                    break
            sccs.append(comp)

    sys.setrecursionlimit(10000)
    for v in edges:
        if v not in index:
            strong(v)
    for comp in sccs:
        for n in comp:
            assert ext.get(n) not in comp, f"{n} extends {ext.get(n)} inside the import cycle {sorted(comp)[:6]}"


def test_signature_closure_contains_what_the_signature_mentions(manifest):
    idx = TC.by_native(manifest)
    roots = TC.roots_of(manifest, ["svelters:gtk/gio:ListStore"])
    c = TC.closure(manifest, roots)
    assert {"Gio.ListStore", "GObject.Object", "Gio.ListModel"} <= c
    assert "Gtk.TextBuffer" not in c
    # a file's closure reaches the types of its parameters, returns and, through them, theirs
    fc = TC.closure(manifest, ["Gio.File"])
    for must in ("Gio.FileInfo", "Gio.Cancellable", "GLib.Bytes", "Gio.FileInputStream"):
        assert must in fc, must
    for ref in idx["Gio.File"]["signature"]:
        assert ref in fc
    # the closure is monotone and idempotent
    assert TC.closure(manifest, sorted(fc)) == fc
    both = TC.closure(manifest, ["Gio.File", "Gtk.TextBuffer"])
    assert fc <= both and TC.closure(manifest, ["Gtk.TextBuffer"]) <= both
    # enums and flags appear as leaves
    assert any(n not in idx for n in fc)


def test_closure_is_computable_per_module_import(manifest):
    roots = TC.roots_of(manifest, ["svelters:gtk/pango:FontDescription", "svelters:gtk/pango:AttrList"])
    c = TC.closure(manifest, roots)
    assert len(TC.classes_in(manifest, c)) < 30
    with pytest.raises(KeyError):
        TC.roots_of(manifest, ["svelters:gtk/pango:NoSuch"])


def test_typings_cover_every_namespace_module(gen):
    for ns in {r["ns"] for r in gen.types.values()}:
        d = (PKG / "gtk" / f"{ns.lower()}.d.ts").read_text()
        assert f"declare module 'svelters:gtk/{ns.lower()}'" in d
    d = (PKG / "gtk/gio.d.ts").read_text()
    assert "export class ListStore" in d and "constructor(props?: { itemType?:" in d
    assert "on(signal: 'items-changed'" in d or "on(signal: string" in d


def test_the_ledger_facts_cover_the_curated_types_and_their_tests_exist():
    data = json.loads((TF.FACTS_DIR / "facts.json").read_text())
    tests = (ROOT / "crates/svelters-gtk/tests/types_verify.rs").read_text()
    ids = {f["fact"] for f in data["facts"]}
    assert len(ids) == 50
    assert "svelters-api:v1:type:gtk:Gio:File:construct" in ids
    assert "svelters-api:v1:type:gtk:Pango:FontDescription:lifetime" in ids
    for f in data["facts"]:
        for t in f["tests"]:
            fn = t.split("::")[1]
            assert re.search(rf"^fn {fn}\(\)", tests, re.M), t
