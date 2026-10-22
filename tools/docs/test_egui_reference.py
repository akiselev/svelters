"""Tests for the egui widget reference generator (tools/docs/egui/gen_reference.py).

    python3 -m unittest discover -s tools/docs -p 'test_*.py'

The checked-in pages under docs/egui/reference/widgets must be what the generator writes from
metadata/semantics/egui.json (the freshness check CI runs), hand-written text outside the generated
regions must survive a run, and a stale region must fail `--check`.
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = ROOT / "metadata/semantics/egui.json"
FRAGMENT = ROOT / "examples/egui-kinds/app/.svelters/app/kinds.json"


def load():
    spec = importlib.util.spec_from_file_location("gen_reference", ROOT / "tools/docs/egui/gen_reference.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gen = load()


def run(*args: str) -> int:
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
        return gen.main(list(args))


class CheckedInPages(unittest.TestCase):
    def test_pages_are_current(self):
        self.assertEqual(run("--check"), 0, "docs/egui/reference/widgets is stale: run python3 tools/docs/egui/gen_reference.py")

    def test_every_kind_has_a_page_with_its_fields(self):
        spec = json.loads(SPEC.read_text())
        out = ROOT / "docs/egui/reference/widgets"
        s = gen.Spec(spec)
        for kid, k in spec["kinds"].items():
            for field in ("category", "states", "a11y", "paint"):
                self.assertIn(field, k, f"egui.json kinds.{kid} has no `{field}`")
            page = (out / f"{s.page_name(kid)}.md").read_text()
            for region in ("summary", "props", "events", "methods", "css", "states", "a11y"):
                self.assertIn(f"BEGIN GENERATED: {region} of kinds.{kid} ", page)
        self.assertTrue((out / "index.md").is_file())


class Regions(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.out = self.tmp / "widgets"
        self.assertEqual(run("--out", str(self.out)), 0)
        self.page = self.out / "text-edit.md"

    def test_a_fresh_tree_checks_clean(self):
        self.assertEqual(run("--out", str(self.out), "--check"), 0)

    def test_stale_region_fails_check_and_is_rewritten(self):
        text = self.page.read_text()
        stale = text.replace("| `hint-text` |", "| `hint` |")
        self.assertNotEqual(text, stale)
        self.page.write_text(stale)
        self.assertEqual(run("--out", str(self.out), "--check"), 1)
        self.assertEqual(run("--out", str(self.out)), 0)
        self.assertEqual(self.page.read_text(), text)

    def test_missing_page_fails_check(self):
        self.page.unlink()
        self.assertEqual(run("--out", str(self.out), "--check"), 1)

    def test_hand_text_survives(self):
        text = self.page.read_text()
        hand = "Use it for free text. For numbers, use `drag-value`."
        text = text.replace("<!-- TODO(DOC7): when to use it, and what to use instead. -->", hand)
        text = text.replace("status: stub", "status: available")
        self.page.write_text(text)
        self.assertEqual(run("--out", str(self.out), "--check"), 0)
        self.assertEqual(run("--out", str(self.out)), 0)
        after = self.page.read_text()
        self.assertIn(hand, after)
        self.assertIn("status: available", after)

    def test_lost_region_comes_back_under_its_heading(self):
        text = self.page.read_text()
        b = text.index("<!-- BEGIN GENERATED: states")
        e = text.index(gen.END, b) + len(gen.END)
        self.page.write_text(text[:b] + text[e:])
        self.assertEqual(run("--out", str(self.out), "--check"), 1)
        self.assertEqual(run("--out", str(self.out)), 0)
        self.assertEqual(self.page.read_text(), text)

    def test_generated_front_matter_keys_follow_the_spec(self):
        text = self.page.read_text()
        self.page.write_text(text.replace("category: inputs", "category: display"))
        self.assertEqual(run("--out", str(self.out), "--check"), 1)
        self.assertEqual(run("--out", str(self.out)), 0)
        self.assertIn("category: inputs", self.page.read_text())

    def test_page_of_a_removed_kind_fails_check(self):
        shutil.copy(self.page, self.out / "gone-widget.md")
        (self.out / "gone-widget.md").write_text(self.page.read_text().replace("kind: TextEdit", "kind: GoneWidget"))
        self.assertEqual(run("--out", str(self.out), "--check"), 1)


class Content(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.spec = gen.Spec(json.loads(SPEC.read_text()))
        cls.theme = gen.theme_rules((ROOT / "crates/svelters-egui/src/theme/base.css").read_text())

    def regions(self, kid):
        return gen.regions(self.spec, kid, self.theme, None)

    def test_bindable_props_name_their_event(self):
        r = self.regions("TextEdit")["props"]
        self.assertIn("| `text` | `string` | `\"\"` | **yes**, on `changed` |", r)

    def test_sync_signal_handler_gets_the_event_first(self):
        r = self.regions("Window")["events"]
        self.assertIn("`(e: SvlEvent<[]>) => void`", r)
        self.assertIn("**Sync:**", r)

    def test_theme_defaults_come_from_base_css(self):
        r = self.regions("Button")["css"]
        self.assertIn("`var(--sv-button-bg)`", r)
        self.assertIn("`button:hover`", r)

    def test_states_list_what_is_set_and_what_is_not(self):
        r = self.regions("Checkbox")["states"]
        self.assertIn("`:checked`", r.split("Never set")[0])
        self.assertIn("`:invalid`", r.split("Never set")[1])
        none = self.regions("Popup")["states"]
        self.assertIn("No built-in state is set on it.", none)

    def test_drawing_library_types_are_left_out(self):
        r = self.regions("InnerWindow")["summary"]
        self.assertNotIn("egui::", r)

    def test_a11y_roles_and_variants(self):
        r = self.regions("TextEdit")["a11y"]
        self.assertIn("`TextInput`; `MultilineTextInput` when `multiline`", r)


@unittest.skipUnless(FRAGMENT.is_file(), "no example fragment")
class Fragment(unittest.TestCase):
    def test_fragment_pages(self):
        frag = json.loads(FRAGMENT.read_text())
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp)
        self.assertEqual(run("--fragment", str(FRAGMENT), "--out", str(tmp), "--widget-link", "../svelters/widgets"), 0)
        self.assertEqual(run("--fragment", str(FRAGMENT), "--out", str(tmp), "--widget-link", "../svelters/widgets", "--check"), 0)
        names = sorted(p.name for p in tmp.glob("*.md"))
        self.assertEqual(names, sorted([f"{t}.md" for t in frag["tags"]] + ["index.md"]))
        for tag in frag["tags"]:
            page = (tmp / f"{tag}.md").read_text()
            # built-in bases link to the built-in pages; no built-in kind gets a page here
            self.assertIn("[`Widget`](../svelters/widgets/widget.md)", page)
            self.assertIn("category: custom", page)

    def test_fragment_needs_out(self):
        with self.assertRaises(SystemExit):
            run("--fragment", str(FRAGMENT))


if __name__ == "__main__":
    unittest.main()
