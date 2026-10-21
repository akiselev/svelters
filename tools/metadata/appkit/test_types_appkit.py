"""Tests of the AppKit non-GUI type pipeline (N4): the classifier's rules, the checked-in output being current, and
the hand-written/generated boundary (NON-GUI-TYPES section 2.4). Pure: needs neither a Mac nor the cargo registry."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))

import types_appkit_classify as TA  # noqa: E402


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *args], cwd=ROOT, capture_output=True, text=True)


def test_classifier_output_is_current():
    r = run("tools/metadata/appkit/types_appkit_classify.py", "--check")
    assert r.returncode == 0, r.stderr


def test_generated_modules_are_current():
    r = run("tools/metadata/appkit/gen_types.py", "--check")
    assert r.returncode == 0, r.stderr


def test_taxonomy_rules():
    ing, ov = TA.Ingest(), TA.load_overrides()
    cat = lambda n: TA.classify(ing, n, ov)  # noqa: E731
    assert cat("NSButton") == "widget" and cat("NSWindow") == "widget" and cat("NSCell") == "widget"
    assert cat("NSString") == "primitive" and cat("NSArray") == "primitive" and cat("NSData") == "primitive"
    assert cat("NSAttributedString") == "value", "immutable half of an NSMutable pair"
    assert cat("NSMutableAttributedString") == "object"
    assert cat("NSURL") == "value", "NSCopying, no setters, not main-thread"
    assert cat("NSColor") == "value", "override: one deprecated class-property setter"
    assert cat("NSFileManager") == "object"
    assert cat("NSTimer") == "excluded" and cat("NSRunLoop") == "excluded", "JS timers stay authoritative"


def test_type_mapping_and_naming():
    ing, ov = TA.Ingest(), TA.load_overrides()
    cats = {n: TA.classify(ing, n, ov) for n in ing.classes}
    ty = TA.Types(ing, cats)
    assert ty.slot("Option<Retained<NSString>>") == {"k": "string", "nullable": True}
    assert ty.slot("&NSArray<NSURL>") == {"k": "list", "of": {"k": "value", "t": "NSURL"}}
    assert ty.slot("NSRect")["t"] == "Rect" and ty.slot("NSRange")["t"] == "Range"
    assert ty.slot("NonNull<CGFloat>")["k"] == "blocked"
    assert ty.slot("&NSColorName") == {"k": "string"}, "typedefs resolve"
    assert TA.js_name("setString:forType:", False, False) == "setStringForType"
    assert TA.js_name("contentsOfDirectoryAtPath:error:", True, False) == "contentsOfDirectoryAtPath"
    assert TA.js_name("initWithString:", False, True) == "newWithString"
    shape, res, why = TA.block_shape("&block2::DynBlock<dyn Fn(*mut NSData, *mut NSError)>", ty)
    assert (shape, res, why) == ("oe", {"k": "bytes"}, None)
    assert TA.block_shape("&block2::DynBlock<dyn Fn(NSRect) -> Bool>", ty)[2] == "block-returns"


def test_facts_shape():
    f = TA.load_facts()
    t = f["types"]
    assert {"NSColor", "NSImage", "NSFont", "NSAttributedString", "NSURL", "NSFileManager", "NSPasteboard", "NSUserDefaults",
            "NSUndoManager", "NSItemProvider"} <= {n for n, x in t.items() if x["curated"]}
    load = [m for m in t["NSItemProvider"]["methods"] if m["sel"] == "loadDataRepresentationForTypeIdentifier:completionHandler:"][0]
    assert load["async"]["shape"] == "oe" and [p["name"] for p in load["params"]] == ["typeIdentifier"]
    ls = [m for m in t["NSFileManager"]["methods"] if m["sel"] == "contentsOfDirectoryAtPath:error:"][0]
    assert ls["throws"] and ls["js"] == "contentsOfDirectoryAtPath"
    assert not [m for m in t["NSURL"]["methods"] if m["sel"].startswith("set") and not m["static"]], "value mutators are hidden"


def test_generated_core_table_is_data_only():
    """Section 2.4: generated Rust is const data with no `unsafe`; generated JS calls only the core and host ops."""
    rs = (ROOT / "crates/svelters-appkit/src/generated/types.rs").read_text()
    assert "unsafe" not in rs and "fn " not in rs
    for p in (ROOT / "packages/types/appkit").rglob("*.js"):
        src = p.read_text()
        for imp in re.findall(r"from '([^']+)'", src):
            assert imp == "../../src/core.js" or re.fullmatch(r"\.\.?/(\.\./)?(foundation|appkit)?/?\w+\.js", imp), f"{p.name}: {imp}"
        assert "globalThis" not in src and "__svelters" not in src, p.name


def test_core_stays_small():
    """The hand-written Apple core (types.rs + blocks.rs + the bridge's invoker and conversions) keeps a budget."""
    lines = sum(len((ROOT / f).read_text().splitlines()) for f in
                ["crates/svelters-appkit/src/types.rs", "crates/svelters-appkit/src/types_table.rs", "crates/svelters-apple/src/blocks.rs"])
    assert lines < 900, lines


def test_block_shapes_match_the_core():
    src = (ROOT / "crates/svelters-apple/src/blocks.rs").read_text()
    shapes = set(json.loads("[" + re.search(r"pub const SHAPES: &\[&str\] = &\[(.*?)\];", src).group(1) + "]"))
    assert shapes == TA.BLOCK_SHAPES
