#!/usr/bin/env python3
"""Download the pinned Windows App SDK NuGet packages and extract what the WinUI pipeline needs.

Nothing downloaded is committed. The cache defaults to <repo>/target/winsdk (override with
--cache or SVELTERS_WINSDK_CACHE). Each package is fetched once from the NuGet flat container,
checked against its pinned SHA-256 when one is known, and unpacked to <cache>/<id>/<version>/.

  python3 tools/metadata/winui/fetch.py              # metadata only (Linux and Windows)
  python3 tools/metadata/winui/fetch.py --winmds     # print the winmd inputs, one per line
  python3 tools/metadata/winui/fetch.py --bootstrap  # also print the bootstrap DLL path (x64)

The bootstrap DLL (Microsoft.WindowsAppRuntime.Bootstrap.dll, Foundation package) must sit next
to a framework-dependent unpackaged exe, or in SVELTERS_WINAPPSDK_DIR; see
docs/notes/windows-setup.md.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import sys
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

# id, version, subpaths of winmd inputs. Versions are those of the Microsoft.WindowsAppSDK 2.5.1
# metapackage (docs/notes/windows-gui-backend.md section 2.1).
PACKAGES = [
    ("microsoft.windowsappsdk.winui", "2.3.9", ["metadata/Microsoft.UI.Xaml.winmd", "metadata/Microsoft.UI.Text.winmd"]),
    ("microsoft.windowsappsdk.interactiveexperiences", "2.1.9",
     ["metadata/10.0.18362.0/Microsoft.UI.winmd", "metadata/10.0.18362.0/Microsoft.Foundation.winmd",
      "metadata/10.0.18362.0/Microsoft.Graphics.winmd"]),
    ("microsoft.windowsappsdk.foundation", "2.3.12", ["metadata"]),
    ("microsoft.web.webview2", "1.0.3719.77", ["lib/Microsoft.Web.WebView2.Core.winmd"]),
]
BOOTSTRAP = ("microsoft.windowsappsdk.foundation", "2.3.12", "runtimes/win-x64/native/Microsoft.WindowsAppRuntime.Bootstrap.dll")


def cache_dir(arg: str | None) -> Path:
    return Path(arg or os.environ.get("SVELTERS_WINSDK_CACHE") or ROOT / "target/winsdk")


def fetch(cache: Path, pid: str, ver: str) -> Path:
    """Unpacked package directory (downloads and extracts on first use)."""
    dest = cache / pid / ver
    marker = dest / ".complete"
    if marker.exists():
        return dest
    dest.mkdir(parents=True, exist_ok=True)
    nupkg = cache / f"{pid}.{ver}.nupkg"
    if not nupkg.exists():
        url = f"https://api.nuget.org/v3-flatcontainer/{pid}/{ver}/{pid}.{ver}.nupkg"
        print(f"fetch: downloading {url}", file=sys.stderr)
        tmp = nupkg.with_suffix(".part")
        with urllib.request.urlopen(url) as r, open(tmp, "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        tmp.rename(nupkg)
    digest = hashlib.sha256(nupkg.read_bytes()).hexdigest()
    with zipfile.ZipFile(nupkg) as z:
        for name in z.namelist():
            # Only metadata, headers and native runtime files; skip the rest of the (large) package.
            if name.startswith(("metadata/", "lib/", "include/", "runtimes/win-x64/")) and not name.endswith("/"):
                target = (dest / name).resolve()
                if dest.resolve() not in target.parents:
                    raise SystemExit(f"fetch: refusing path {name!r} in {nupkg.name}")
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(z.read(name))
    (dest / "sha256.txt").write_text(digest + "\n")
    marker.write_text("")
    return dest


def winmds(cache: Path) -> list[Path]:
    out = []
    for pid, ver, subs in PACKAGES:
        d = fetch(cache, pid, ver)
        for s in subs:
            p = d / s
            if p.is_dir():
                out.extend(sorted(p.glob("*.winmd")))
            elif p.exists():
                out.append(p)
            else:
                raise SystemExit(f"fetch: {p} missing from {pid} {ver}")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache")
    ap.add_argument("--winmds", action="store_true", help="print the winmd inputs")
    ap.add_argument("--bootstrap", action="store_true", help="print the x64 bootstrap DLL path")
    a = ap.parse_args()
    cache = cache_dir(a.cache)
    files = winmds(cache)
    if a.winmds:
        print("\n".join(str(f) for f in files))
    if a.bootstrap:
        pid, ver, sub = BOOTSTRAP
        print(fetch(cache, pid, ver) / sub)
    if not (a.winmds or a.bootstrap):
        print(f"fetch: {len(files)} winmd files under {cache}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
