"""Tests of the WinUI non-GUI type pipeline (N4): facts and generated files are current, the generated Rust is data only
(section 2.4), parameterized IIDs follow the WinRT algorithm, and the manifest matches the files and package exports.
They read only checked-in files: no Windows App SDK download is needed."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import types_winui_classify as C  # noqa: E402

PKG = ROOT / "packages/types"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *args], cwd=ROOT, capture_output=True, text=True)


def test_classified_facts_are_current():
    r = run(str(HERE / "types_winui_classify.py"), "--check")
    assert r.returncode == 0, r.stderr


def test_generated_files_are_current():
    r = run(str(HERE / "gen_types_winui.py"), "--check")
    assert r.returncode == 0, r.stderr


def test_parameterized_iids_follow_the_winrt_algorithm():
    ing = C.Ingest()
    # Values Windows SDK headers spell out (windows.foundation.collections.h).
    assert ing.iid("Windows.Foundation.Collections.IIterable<string>") == "e2fcc7c1-3bfc-5a0b-b2b0-72e769d1cb7e"
    assert ing.iid("Windows.Foundation.Collections.IVector<string>") == "98b9acc1-4b56-532e-ac73-03d5291cca90"
    assert ing.iid("Windows.Foundation.Collections.IVectorView<string>") == "2f13c006-a03a-5f69-b090-75a43e33423e"
    assert ing.iid("Windows.Foundation.Collections.IMap<string, object>") == "1b0d3570-0877-5ec2-8a2c-3b9539506aca"
    # A runtime class answers its default interface.
    assert ing.iid("Windows.Foundation.Uri") == ing.iid("Windows.Foundation.IUriRuntimeClass")


def test_generated_rust_is_data_only():
    """Section 2.4: the generated tables are const data with no unsafe and no native calls."""
    text = (ROOT / "crates/svelters-winui/src/generated/types_tables.rs").read_text()
    for banned in ("unsafe", "fn ", "impl ", "Release(", "AddRef(", "transmute"):
        assert banned not in text, banned
    assert re.findall(r"^pub (static|const) ", text, re.M), "tables"


def test_classification_rules():
    facts = C.load_facts()
    cats = {n: c for ns in facts["namespaces"].values() for n, c in ns.items()}
    assert cats["Windows.Foundation.Uri"] == "object"
    assert cats["Windows.Foundation.Collections.PropertySet"] == "model"
    assert cats["Microsoft.UI.Colors"] == "statics"
    assert cats["Windows.UI.Color"] == "record"
    assert cats["Windows.Storage.CreationCollisionOption"] == "enum"
    assert cats["Windows.Storage.FileAttributes"] == "flags"
    assert cats["Windows.Foundation.IAsyncAction"] == "async"
    assert cats["Microsoft.UI.Xaml.FrameworkElement"] == "widget"
    assert cats["Windows.Foundation.TypedEventHandler"] == "callback"
    k = facts["kinds"]
    # Thread classes (section 4.1): DependencyObjects are dispatcher-bound, Windows.Foundation is free.
    assert k["Microsoft.UI.Xaml.Media.SolidColorBrush"]["thread"] == "dispatcher"
    assert k["Windows.Foundation.Uri"]["thread"] == "any"
    assert k["Microsoft.UI.Xaml.Media.FontFamily"]["thread"] == "ui"
    # Every runtime class is SHARED (decision Q1/Q8).
    assert {r["lifetime"] for r in k.values()} == {"shared"}
    # Overloads by arity; the composable factory hides its two ABI tail parameters.
    assert sorted(len(c["params"]) for c in k["Windows.Foundation.Uri"]["constructors"]) == [1, 2]
    ff = k["Microsoft.UI.Xaml.Media.FontFamily"]["constructors"]
    assert [c["composable"] for c in ff] == [True] and len(ff[0]["params"]) == 1
    # Async results fold into Promises.
    sf = k["Windows.Storage.StorageFolder"]
    assert all(o["ret"]["k"] == "async" for o in sf["methods"]["createFileAsync"])
    # DependencyProperty-backed props are observable.
    assert any(p["dp"] for p in k["Microsoft.UI.Xaml.Media.SolidColorBrush"]["props"] if p["name"] == "color")


def test_manifest_matches_the_files_and_the_package_exports():
    m = json.loads((PKG / "winui/manifest.json").read_text())
    pkg = json.loads((PKG / "package.json").read_text())
    for mod, d in m["modules"].items():
        ns = mod.split("/")[-1]
        assert f"./winui/{ns}" in pkg["exports"], ns
        for name, e in d["exports"].items():
            assert (PKG / "winui" / e["file"]).exists(), e["file"]
    assert pkg["exports"]["./manifest-winui"] == "./winui/manifest.json"
