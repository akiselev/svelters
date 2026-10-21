#!/usr/bin/env python3
"""Ingest the WinRT non-GUI types (N4, plans/research/NON-GUI-TYPES.md section 3.2) from winmd.

Inputs: the Windows App SDK winmd files (`fetch.py --winmds`) plus the Windows SDK's `Windows.winmd` that
windows-bindgen bundles (`windows-default`), read through `winmd-tool types` (interfaces with their methods in
vtable order, GUIDs, generics, struct fields, delegate signatures, the class attributes).

Output (checked in, byte-identical on rerun; upstream facts only, origin `winmd`):
  metadata/winui/types/ingested/<Namespace>.json   every type of each namespace in NAMESPACES
  metadata/winui/types/ingested/_outside.json      types outside them that their signatures, bases and
                                                   required interfaces mention (so the classifier can say
                                                   what blocks a member, and find widget bases)

Classification is a separate, pure step (`types_winui_classify.py`), so rules change without re-ingesting and
`--check` there needs no winmd.

  python3 tools/metadata/winui/types_winui_ingest.py [--tool target/winmd-tool/release/winmd-tool] [--check]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import fetch  # noqa: E402

OUT = ROOT / "metadata/winui/types/ingested"

# Whole namespaces ingested and classified (the report covers all of them). What is bound is decided by the
# classifier's roots and scope (metadata/overrides/winui/types/winui-types.yaml).
NAMESPACES = [
    "Windows.Foundation",
    "Windows.Foundation.Collections",
    "Windows.Foundation.Numerics",
    "Windows.UI",
    "Windows.Storage",
    "Windows.Storage.Streams",
    "Windows.Storage.FileProperties",
    "Windows.Storage.Search",
    "Windows.ApplicationModel.DataTransfer",
    "Windows.Data.Json",
    "Microsoft.UI",
    "Microsoft.UI.Dispatching",
    "Microsoft.UI.Composition",
    "Microsoft.UI.Windowing",
    "Microsoft.UI.Xaml",
    "Microsoft.UI.Xaml.Media",
    "Microsoft.UI.Xaml.Media.Imaging",
    "Microsoft.UI.Xaml.Media.Animation",
    "Microsoft.UI.Xaml.Documents",
    "Microsoft.UI.Xaml.Data",
    "Microsoft.UI.Xaml.Input",
]


def dump(tool: str, cache: str | None) -> dict:
    winmds = subprocess.run([sys.executable, str(HERE / "fetch.py"), "--winmds"] + (["--cache", cache] if cache else []),
                            check=True, capture_output=True, text=True, cwd=ROOT).stdout.split()
    out = subprocess.run([tool, "types", *NAMESPACES, "--", *winmds], check=True, capture_output=True, text=True, cwd=ROOT).stdout
    return json.loads(out)


def files(d: dict) -> dict[Path, str]:
    by_ns: dict[str, dict] = {ns: {} for ns in NAMESPACES}
    outside: dict = {}
    for name, t in sorted(d["types"].items()):
        t = dict(t)
        t["origin"] = "winmd"
        if t.pop("outside", False) or t.get("namespace") not in by_ns:
            outside[name] = t
        else:
            by_ns[t["namespace"]][name] = t
    pkgs = {pid: ver for pid, ver, _ in fetch.PACKAGES}
    head = {"schema": "svelters-winui-types-ingested/v1", "sources": {"windows-app-sdk": pkgs, "windows-default": "0.100.0"}}
    res: dict[Path, str] = {}
    for ns, types in by_ns.items():
        res[OUT / f"{ns}.json"] = json.dumps({**head, "namespace": ns, "types": types}, indent=1, sort_keys=True) + "\n"
    res[OUT / "_outside.json"] = json.dumps({**head, "namespace": None, "types": outside}, indent=1, sort_keys=True) + "\n"
    return res


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tool", default=str(ROOT / "target/winmd-tool/release/winmd-tool"))
    ap.add_argument("--cache")
    ap.add_argument("--check", action="store_true", help="fail if metadata/winui/types/ingested is stale")
    a = ap.parse_args()
    res = files(dump(a.tool, a.cache))
    bad = []
    for p, text in res.items():
        if a.check:
            if not p.exists() or p.read_text() != text:
                bad.append(str(p.relative_to(ROOT)))
        elif not p.exists() or p.read_text() != text:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
    for p in OUT.glob("*.json"):
        if p not in res:
            if a.check:
                bad.append(f"{p.relative_to(ROOT)} (stale)")
            else:
                p.unlink()
    if bad:
        print("types_winui_ingest: stale: " + ", ".join(bad), file=sys.stderr)
        return 1
    n = sum(len(json.loads(t)["types"]) for t in res.values())
    print(f"types_winui_ingest: {len(res)} files, {n} types {'are current' if a.check else 'written'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
