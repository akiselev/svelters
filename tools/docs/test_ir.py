"""Tests for the docs IR: schema validity of every frontend's output, determinism, the Qt text policy.

    python3 -m unittest discover -s tools/docs -p 'test_*.py'
"""
from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/docs/ir"))
import irlib  # noqa: E402


def load(name: str, rel: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / rel)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_cache: dict[str, dict] = {}


def ir_of(tk: str, **kw) -> dict:
    key = tk + json.dumps(kw, sort_keys=True)
    if key not in _cache:
        mod = load(f"{tk}_frontend", f"tools/docs/{tk}/frontend.py")
        _cache[key] = mod.Frontend(**kw).build()
    return _cache[key]


class SchemaTests(unittest.TestCase):
    def test_mini_fixture_is_valid(self):
        ir = json.loads((ROOT / "tools/docs/ir/fixtures/mini.ir.json").read_text())
        self.assertEqual(irlib.validate(ir), [])

    def test_schema_rejects_bad_documents(self):
        ir = json.loads((ROOT / "tools/docs/ir/fixtures/mini.ir.json").read_text())
        bad = copy.deepcopy(ir)
        bad["entities"][0]["kind"] = "widget"
        self.assertTrue(irlib.validate(bad))
        bad = copy.deepcopy(ir)
        del bad["toolkit"]["doc_license"]
        self.assertTrue(irlib.validate(bad))
        bad = copy.deepcopy(ir)
        bad["entities"][1]["properties"][0]["surprise"] = 1
        self.assertTrue(irlib.validate(bad))

    def test_unique_ids(self):
        ir = json.loads((ROOT / "tools/docs/ir/fixtures/mini.ir.json").read_text())
        ir["entities"].append(copy.deepcopy(ir["entities"][0]))
        self.assertTrue(irlib.check_unique(ir))


class FrontendTests(unittest.TestCase):
    # The counter app's elements, in each toolkit's native names (docs/notes/n1-rename-map.md).
    COUNTER_TAGS = {"gtk": ("window", "box", "label", "button", "entry"),
                    "qt": ("main-window", "box-layout", "label", "push-button", "line-edit")}
    BUTTON = {"gtk": "button", "qt": "push-button"}

    def check_frontend(self, tk: str):
        ir = ir_of(tk)
        self.assertEqual(irlib.validate(ir), [], f"{tk} IR does not match the schema")
        self.assertEqual(irlib.check_unique(ir), [])
        self.assertEqual(ir["toolkit"]["id"], tk)
        by_tag = {t: e for e in ir["entities"] for t in e["tags"]}
        for tag in self.COUNTER_TAGS[tk]:
            self.assertIn(tag, by_tag, f"{tk}: no entity for <{tag}>")
            e = by_tag[tag]
            self.assertEqual(e["status"]["tier"], "annotated", f"{tk}:{tag}")
            self.assertTrue(e["status"]["annotation_files"])
        # every cross-reference in the text resolves or is intentionally absent; ids are well formed
        ids = {e["id"] for e in ir["entities"]}
        for e in ir["entities"]:
            for b in e["bases"]:
                self.assertTrue(b.startswith(tk + ":"))
        self.assertIn(by_tag[self.BUTTON[tk]]["id"], ids)
        self.assertEqual(ir["coverage"]["serdes"]["status"], "unknown")

    def test_gtk_frontend(self):
        self.check_frontend("gtk")
        ir = ir_of("gtk")
        button = next(e for e in ir["entities"] if e["tags"] == ["button"])
        self.assertTrue(any(p["name"] == "label" and p["annotated"] for p in button["properties"]))
        ev = next(x for x in button["events"] if x["name"] == "clicked")
        self.assertEqual(ev["handler_prop"], "onclicked")
        self.assertNotIn("handler_aliases", ev)  # no DOM-style aliases (N1)
        # one entity per element kind (kinds marked `unexposed` in the overrides have no tag)
        tagged = sum(1 for p in (ROOT / "metadata/gtk").glob("*.json") if "element" in json.loads(p.read_text()))
        self.assertEqual(len([e for e in ir["entities"] if e["tags"]]), tagged)
        self.assertGreater(tagged, 50)

    def test_qt_frontend(self):
        self.check_frontend("qt")
        ir = ir_of("qt")
        button = next(e for e in ir["entities"] if e["tags"] == ["push-button"])
        text = next(p for p in button["properties"] if p["js_name"] == "text")
        self.assertEqual(text["name"], "text")
        self.assertTrue(text["annotated"])
        self.assertTrue(any(x["name"] == "clicked" and x["annotated"] for x in button["events"]))
        # every annotated element is in the docs: one entity per element class, every tag on it
        import yaml
        tags, classes = set(), set()
        for p in (ROOT / "metadata/overrides/qt").glob("*.yaml"):
            for el in (yaml.safe_load(p.read_text()) or {}).get("elements", []) or []:
                tags.add(el["tag"])
                classes.add(el["construct"]["class"])
        self.assertEqual({t for e in ir["entities"] for t in e["tags"]}, tags)
        self.assertEqual(len([e for e in ir["entities"] if e["tags"]]), len(classes))
        self.assertGreater(len(tags), 50)

    def test_dangerous_members_are_documented(self):
        """Every dangerous member of the metadata (tools/metadata/common/dangerous.py) is in the IR, marked, and the
        examples never use one."""
        dz = load("dangerous_for_docs", "tools/metadata/common/dangerous.py")
        field = {"methods": "methods", "properties": "properties", "signals": "events"}
        for tk in ("gtk", "qt"):
            ir = ir_of(tk)
            by_name = {e["id"].split(":", 1)[1].split(".")[-1]: e for e in ir["entities"] if e["kind"] in ("class", "interface")}
            marked = {(k, f, r["name"]) for k, e in by_name.items() for f in field.values() for r in e[f] if r.get("dangerous")}
            want = {(k, field[s], n) for k, s, n, _ in dz.Dangerous(tk, ROOT).all()}
            self.assertEqual(marked, want, tk)
            self.assertEqual(sum(ir["coverage"]["dangerous"].values()),
                             sum(1 for e in ir["entities"] for f in field.values() for r in e[f] if r.get("dangerous")))
            for e in ir["entities"]:
                names = {r["js_name"] for r in e["properties"] if r.get("dangerous")}
                for ex in e.get("examples", []):
                    for n in names:
                        self.assertNotIn(f" {n}=", ex["code"], f"{tk}:{e['name']} example uses dangerous {n}")
        win = next(e for e in ir_of("gtk")["entities"] if e["tags"] == ["window"])
        titlebar = next(m for m in win["methods"] if m["name"] == "set_titlebar")
        self.assertEqual(titlebar["dangerous"]["severity"], "corrupts-tree")
        self.assertTrue(titlebar["dangerous"]["source"].startswith("metadata/dangerous/gtk.yaml#"))
        app = next(p for p in win["properties"] if p["name"] == "application")
        self.assertEqual(app["dangerous"]["severity"], "owned-by-backend")
        self.assertNotIn("dangerous", next(m for m in win["methods"] if m["name"] == "present"))

    def test_qt_text_policy(self):
        brief = ir_of("qt", text_policy="brief")
        links = ir_of("qt", text_policy="links")
        self.assertEqual(brief["toolkit"]["text_policy"], "brief")
        for ir in (brief, links):
            for e in ir["entities"]:
                self.assertNotIn("description", e)
                for block in [e.get("summary")] + [m.get("doc") for k in ("properties", "events", "methods") for m in e[k]]:
                    if block:
                        self.assertTrue(block.get("truncated"))
                        self.assertLessEqual(len(block["md"]), 260)
                        self.assertEqual(block["license"], "GFDL-1.3-only")

    def test_determinism(self):
        for tk in ("gtk", "qt"):
            mod = load(f"{tk}_det", f"tools/docs/{tk}/frontend.py")
            again = json.dumps(mod.Frontend().build(), sort_keys=True)
            self.assertEqual(json.dumps(ir_of(tk), sort_keys=True), again, tk)


QT_DOCDIR = Path("/usr/share/doc/qt6")


class DocsPassTests(unittest.TestCase):
    # ingest_docs.py reads the Qt docs installed on the machine; without them every brief is null and
    # the comparison with the committed output is meaningless (e.g. on CI runners).
    @unittest.skipUnless((QT_DOCDIR / "qtwidgets/qtwidgets.index").exists(), f"no Qt qdoc indexes under {QT_DOCDIR}")
    def test_qt_docs_pass_is_deterministic(self):
        script = ROOT / "tools/metadata/qt/ingest_docs.py"
        with tempfile.TemporaryDirectory() as d:
            for sub in ("a", "b"):
                subprocess.run([sys.executable, str(script), "--out", f"{d}/{sub}"], check=True, capture_output=True)
            for f in sorted(Path(d, "a").glob("*.json")):
                self.assertEqual(f.read_bytes(), Path(d, "b", f.name).read_bytes())
            # and identical to the committed output
            for f in sorted(Path(d, "a").glob("*.json"))[:20]:
                self.assertEqual(f.read_bytes(), (ROOT / "metadata/docs/qt" / f.name).read_bytes())

    def test_committed_docs_cover_every_kind(self):
        for tk in ("gtk", "qt"):
            kinds = {p.name for p in (ROOT / "metadata" / tk).glob("*.json")}
            docs = {p.name for p in (ROOT / "metadata/docs" / tk).glob("*.json")}
            self.assertEqual(kinds, docs, tk)

    def test_gtk_xref_conversion(self):
        mod = load("gtk_ingest_docs", "tools/metadata/gtk/ingest_docs.py")
        out = mod.convert_markdown("See [class@Gtk.Button], [method@Gtk.Button.set_child], [property@Gtk.Button:label] and [signal@Gtk.Button::clicked]. %TRUE @name")
        self.assertIn("(xref:gtk:Gtk.Button)", out)
        self.assertIn("(xref:gtk:Gtk.Button#method:set_child)", out)
        self.assertIn("(xref:gtk:Gtk.Button#property:label)", out)
        self.assertIn("(xref:gtk:Gtk.Button#event:clicked)", out)
        self.assertIn("`TRUE`", out)


if __name__ == "__main__":
    unittest.main()
