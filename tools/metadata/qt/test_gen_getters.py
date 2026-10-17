"""Tests for the object-getter and object-reference parts of gen_cpp.py (plans/milestones/X1-borrowed-gi.md).

Run: python3 -m unittest discover -s tools/metadata/qt -p 'test_*.py'
"""

import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import gen_cpp  # noqa: E402

ROOT = HERE.parents[2]


def model() -> "gen_cpp.Model":
    return gen_cpp.Model(ROOT / "metadata/qt", ROOT / "metadata/overrides/qt", [])


class GetterTable(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.m = model()
        cls.tab = gen_cpp.getter_table(cls.m)

    def test_internal_children_and_parents_are_getters(self):
        self.assertIn(("QComboBox", "QLineEdit"), self.tab["lineEdit"])
        self.assertIn(("QAbstractScrollArea", "QWidget"), self.tab["viewport"])
        self.assertIn(("QMainWindow", "QWidget"), self.tab["centralWidget"])
        self.assertIn(("QWidget", "QWidget"), self.tab["parentWidget"])
        self.assertIn(("QWidget", "QWidget"), self.tab["window"])

    def test_caller_owned_results_are_not_getters(self):
        for name in self.tab:
            self.assertFalse(gen_cpp._GETTER_SKIP.match(name), name)
        self.assertNotIn("takeCentralWidget", self.tab)

    def test_methods_with_required_parameters_are_not_getters(self):
        self.assertNotIn("childAt", self.tab)

    def test_every_class_in_the_table_has_a_header(self):
        for rows in self.tab.values():
            for recv, ret in rows:
                self.assertTrue(self.m.kind(recv)["native"]["header"])
                self.assertTrue(self.m.kind(ret)["native"]["header"])

    def test_generated_dispatch_names_every_getter_once(self):
        text = gen_cpp.emit_getters(self.m)
        names = ["parent"] + sorted(n for n in self.tab if n != "parent")
        self.assertEqual(text.count("// parent\n"), 1)
        for n in names:
            self.assertEqual(text.count(f"case {names.index(n)}: {{  // {n}\n"), 1, n)

    def test_borrowed_kind_skips_composite_elements(self):
        text = gen_cpp.emit_getters(self.m)
        # `box-layout` is a plain QWidget with a layout: a borrowed QWidget must not become a `box-layout`.
        self.assertNotIn('"QWidget")) return K_box_layout', text)
        self.assertIn('"QLineEdit")) return K_line_edit', text)


if __name__ == "__main__":
    unittest.main()
