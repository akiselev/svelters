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
            "Pango.AttrList": "value", "Pango.FontDescription": "value",
            "Gtk.FilterListModel": "model", "Gtk.SortListModel": "model", "Gtk.SingleSelection": "model",
            "Gtk.MultiSelection": "model", "Gtk.CustomFilter": "object", "Gtk.CustomSorter": "object"}
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


def test_every_bound_member_is_marshallable_by_the_invoker(facts):
    """No bound member mentions a variant (outside properties), a hash table or a by-value struct: the rules
    block them with a reason instead. Callbacks (N1) carry their scope and a signature the trampoline
    converts; no GCancellable, GAsyncReadyCallback or GAsyncResult reaches JS in an async member."""
    bad = {"variant", "hash table", "struct by value", "fundamental", "gpointer"}
    for q, r in facts.items():
        for m in r["constructors"] + r["statics"] + r["methods"]:
            blob = json.dumps([m["ret"]] + [p["type"] for p in m["params"]])
            assert '"k": "variant"' not in blob, f"{q}.{m['name']}"
            for p in m["params"]:
                if p["type"]["k"] == "callback":
                    assert p["type"]["scope"] in ("call", "async", "notified", "forever"), (q, m["name"])
                    assert "sig" in p["type"] and p.get("dir") is None, (q, m["name"])
            if m.get("async"):
                assert '"Gio.AsyncReadyCallback"' not in blob and '"Gio.AsyncResult"' not in blob, f"{q}.{m['name']}"
                assert '"Gio.Cancellable"' not in json.dumps([p["type"] for p in m["params"]]), f"{q}.{m['name']}"
                assert m["async"]["finish"].endswith("_finish")
        assert not (set(r["blocked"].values()) - set(r["blocked"].values())) & bad
        for name, why in r["blocked"].items():
            assert why, (q, name)


def test_async_pairs_are_folded_and_callbacks_bound_n1(facts):
    f = facts["Gio.File"]
    by = {m["name"]: m for m in f["methods"] + f["statics"]}
    lc = by["load_contents_async"]
    assert lc["async"]["finish"] == "load_contents_finish" and lc["params"] == [] and lc["throws"]
    assert [p["name"] for p in lc["async"]["outs"] if not p.get("hidden")] == ["contents", "etag_out"]
    assert by["query_info_async"]["ret"] == {"k": "object", "t": "Gio.FileInfo"}
    assert "new_tmp_async" in {m["name"] for m in f["statics"]}
    assert "load_contents_finish" not in by and "load_contents" in by
    assert "load_partial_contents_async" not in by, "not introspectable (its call-scope callback outlives the call)"
    md = by["measure_disk_usage"]
    cb = next(p for p in md["params"] if p["type"]["k"] == "callback")
    assert cb["type"]["scope"] == "call" and [a["name"] for a in cb["type"]["sig"]["args"]] == ["reporting", "current_size", "num_dirs", "num_files"]
    assert "progress_data" not in {p["name"] for p in md["params"]}
    cf = {m["name"]: m for m in facts["Gtk.CustomFilter"]["methods"]}["set_filter_func"]
    assert [p["type"]["scope"] for p in cf["params"]] == ["notified"]
    fd = {m["name"]: m for m in facts["Gtk.FileDialog"]["methods"]}["open"]
    assert fd["params"][0]["type"] == {"k": "widget", "t": "Gtk.Window"} and fd["ret"] == {"k": "object", "t": "Gio.File"}


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


def test_async_members_return_promises_in_modules_and_typings():
    src = (PKG / "gtk/gio/File.js").read_text()
    assert "loadContentsAsync(options) {\n    return callAsync(this, 'load_contents_async', [], options, (r) => ({ ret: r.ret, contents: r.contents, etagOut: r.etag_out }));" in src
    assert "queryInfoAsync(attributes, flags, ioPriority = 0, options)" in src
    d = (PKG / "gtk/gio.d.ts").read_text()
    assert "loadContentsAsync(options?: AsyncOptions): Promise<{ ret: boolean; contents: Uint8Array; etagOut: string | null }>;" in d
    g = (PKG / "gtk/gtk.d.ts").read_text()
    assert "open(parent?: NativeElement | null, options?: AsyncOptions): Promise<File>;" in g
    assert "setFilterFunc(matchFunc?: ((item: GObject_Object | null) => boolean) | null): void;" in g or "setFilterFunc(matchFunc?: ((item: Object" in g


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
    # five groups per curated type, plus `async` for the four with async members (N1); N3 added six model kinds
    assert len(ids) == 84
    assert "svelters-api:v1:type:gtk:Gio:File:async" in ids
    assert "svelters-api:v1:type:gtk:Gio:File:construct" in ids
    assert "svelters-api:v1:type:gtk:Pango:FontDescription:lifetime" in ids
    for f in data["facts"]:
        for t in f["tests"]:
            fn = t.split("::")[1]
            assert re.search(rf"^fn {fn}\(\)", tests, re.M), t
