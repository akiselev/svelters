"""Tests of the Qt non-GUI type pipeline (N2): facts are current, the generated modules are current, and the
release manifest and closure are consistent. They read only checked-in files: no Qt install is needed."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "tools/metadata/gtk"))
import types_closure as TC  # noqa: E402

PKG = ROOT / "packages/types"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *args], cwd=ROOT, capture_output=True, text=True)


def test_classified_facts_are_current():
    r = run(str(HERE / "types_qt_classify.py"), "--check")
    assert r.returncode == 0, r.stderr


def test_generated_modules_are_current():
    r = run(str(HERE / "gen_types_js.py"), "--check")
    assert r.returncode == 0, r.stderr


def test_manifest_matches_the_files_and_the_package_exports():
    m = TC.load(PKG / "qt/manifest.json")
    pkg = json.loads((PKG / "package.json").read_text())
    for mod, d in m["modules"].items():
        ns = mod.split("/")[-1]
        assert f"./qt/{ns}" in pkg["exports"], ns
        for name, e in d["exports"].items():
            assert (PKG / "qt" / e["file"]).exists(), e["file"]
            assert e["native"] == name
            for i in e["imports"]:
                assert i in TC.by_native(m), (name, i)
    # the GTK keys survive the Qt generator (and the reverse: tools/metadata/common/types_pkg.py)
    assert "./gtk/gio" in pkg["exports"] and "./manifest" in pkg["exports"] and "./manifest-qt" in pkg["exports"]


def test_closure_of_a_model_and_a_document():
    m = TC.load(PKG / "qt/manifest.json")
    roots = TC.roots_of(m, ["svelters:qt/core:QStringListModel", "svelters:qt/gui:QTextDocument"])
    cls = TC.classes_in(m, TC.closure(m, roots))
    assert {"QStringListModel", "QAbstractListModel", "QAbstractItemModel", "QObject", "QModelIndex", "QTextDocument"} <= cls
    assert "QTimer" not in cls


def test_overloaded_methods_pass_arguments_through():
    src = (PKG / "qt/core/QTimer.js").read_text()
    assert "start(...args) {\n    call(this, 'start', args);" in src
    assert "callStatic('QTimer', 'new', args)" in src
