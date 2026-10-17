import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def load(name):
    spec = importlib.util.spec_from_file_location("shot_" + name, HERE / (name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def missing_deps():
    missing = []
    for mod in ("gi", "PySide6"):
        if importlib.util.find_spec(mod) is None:
            missing.append(mod)
    if shutil.which("xvfb-run") is None:
        missing.append("xvfb-run")
    return missing


class RecipeTables(unittest.TestCase):
    def test_gtk_recipes_name_existing_tags(self):
        gtk = load("gtk")
        self.assertFalse(set(gtk.RECIPES) - set(gtk.metadata()))
        self.assertFalse(set(gtk.UNSUPPORTED) - set(gtk.metadata()))

    def test_qt_recipes_name_existing_tags(self):
        qt = load("qt")
        self.assertFalse(set(qt.RECIPES) - set(qt.metadata()))


class Generate(unittest.TestCase):
    def test_button_both_toolkits(self):
        missing = missing_deps()
        if missing:
            self.skipTest("missing: " + ", ".join(missing))
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run([sys.executable, str(HERE / "run.py"), "--toolkit", "all",
                            "--only", "button,push-button", "--jobs", "2", "--out", tmp],
                           check=True, timeout=300)
            manifest = json.loads((Path(tmp) / "manifest.json").read_text())
            for key in ("gtk/button", "qt/push-button"):
                self.assertIn(key, manifest["keys"])
                data = (Path(tmp) / (key + ".png")).read_bytes()
                self.assertTrue(data.startswith(PNG_MAGIC))
                self.assertGreater(len(data), 300)


if __name__ == "__main__":
    unittest.main()
