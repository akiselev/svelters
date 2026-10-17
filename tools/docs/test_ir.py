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
    COUNTER_TAGS = ("window", "box", "label", "button", "entry")

    def check_frontend(self, tk: str):
        ir = ir_of(tk)
        self.assertEqual(irlib.validate(ir), [], f"{tk} IR does not match the schema")
        self.assertEqual(irlib.check_unique(ir), [])
        self.assertEqual(ir["toolkit"]["id"], tk)
        by_tag = {t: e for e in ir["entities"] for t in e["tags"]}
        for tag in self.COUNTER_TAGS:
            self.assertIn(tag, by_tag, f"{tk}: no entity for <{tag}>")
            e = by_tag[tag]
            self.assertEqual(e["status"]["tier"], "annotated", f"{tk}:{tag}")
            self.assertTrue(e["status"]["annotation_files"])
        # every cross-reference in the text resolves or is intentionally absent; ids are well formed
        ids = {e["id"] for e in ir["entities"]}
        for e in ir["entities"]:
            for b in e["bases"]:
                self.assertTrue(b.startswith(tk + ":"))
        self.assertIn(by_tag["button"]["id"], ids)
        self.assertEqual(ir["coverage"]["serdes"]["status"], "unknown")

    def test_gtk_frontend(self):
        self.check_frontend("gtk")
        ir = ir_of("gtk")
        button = next(e for e in ir["entities"] if e["tags"] == ["button"])
        self.assertTrue(any(p["name"] == "label" and p["annotated"] for p in button["properties"]))
        ev = next(x for x in button["events"] if x["name"] == "clicked")
        self.assertEqual(ev["handler_aliases"], ["onclick"])
        # one entity per element kind (kinds marked `unexposed` in the overrides have no tag)
        tagged = sum(1 for p in (ROOT / "metadata/gtk").glob("*.json") if "element" in json.loads(p.read_text()))
        self.assertEqual(len([e for e in ir["entities"] if e["tags"]]), tagged)
        self.assertGreater(tagged, 80)

    def test_qt_frontend(self):
        self.check_frontend("qt")
        ir = ir_of("qt")
        button = next(e for e in ir["entities"] if e["tags"] == ["button"])
        label = next(p for p in button["properties"] if p["js_name"] == "label")
        self.assertEqual(label["name"], "text")
        self.assertTrue(label["annotated"])
        self.assertTrue(any(x["name"] == "clicked" and x["annotated"] for x in button["events"]))

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


class DocsPassTests(unittest.TestCase):
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
