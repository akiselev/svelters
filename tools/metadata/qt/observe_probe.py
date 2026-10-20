#!/usr/bin/env python3
"""The `observe` section of the Qt probe (milestone R0, plans/research/REACTIVITY.md sections 3.2 and 4.5).

    python3 tools/metadata/qt/probe.py --section observe [--qtdir <qt>/gcc_64] [--out FILE]

Writes classes.inc (one `PROBE(QClass)` per QObject class of the metadata: the classes of metadata/qt/*.json and
the non-GUI types of metadata/qt/types/*.json, as far as moc's extract metadata/qt/moc/properties.json knows them
as QObjects), builds tools/metadata/qt/observe_probe.cpp against the Qt SDK in --qtdir (default: the Qt 6.11.3
install of docs/notes/nt2-qt-types.md; without one, the system Qt through pkg-config), runs it offscreen, reruns
without a class that crashed, and writes metadata/probe/qt/observe/<Qt version>.json. A maintainer step: CI never
runs it; tools/metadata/qt/infer.py reads the checked-in file. No PySide6 needed.

Per property (keyed `<owner class>:<prop>`):
  flags   readable, writable, constant, bindable, user, notify (the NOTIFY signature) from the metaobject
  sweep   a scalar writable property set to a new value, then to the value read back: {on, readback, notify_change,
          notify_same, bindable_change, events, signals}
  seen    side effects: [{cause: set:<owner>:<prop> | scenario:<name>, on, notify, changed, bindable, events,
          signals}] for every property that changed, notified or had its bindable notifier called meanwhile
Deterministic: sorted keys and lists.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OUT_DIR = ROOT / "metadata/probe/qt/observe"
DEFAULT_QT = Path.home() / ".cache/claude-svelters/nt2/qt/6.11.3/gcc_64"
MAX_SEEN = 12


def subjects() -> dict[str, str]:
    """Qt class -> owner key (`<module>:<class>`), for every QObject class the metadata describes."""
    moc = json.loads((ROOT / "metadata/qt/moc/properties.json").read_text())["classes"]
    out: dict[str, str] = {}
    for p in sorted((ROOT / "metadata/qt").glob("*.json")):
        d = json.loads(p.read_text())
        cls = p.stem
        if (moc.get(cls) or {}).get("object"):
            out[cls] = ":".join(d["id"].rsplit(":", 2)[-2:])
    for p in sorted((ROOT / "metadata/qt/types").glob("Qt*.json")):
        d = json.loads(p.read_text())
        for cls, t in d["types"].items():
            if t.get("qobject") and (moc.get(cls) or {}).get("object") and cls not in out:
                out[cls] = f"{d['module']}:{cls}"
    return dict(sorted(out.items()))


def build(tmp: Path, qtdir: Path | None, classes: list[str]) -> tuple[Path, dict]:
    (tmp / "classes.inc").write_text("".join(f"PROBE({c})\n" for c in classes))
    exe = tmp / "observe_probe"
    env = dict(os.environ)
    if qtdir:
        inc, lib = qtdir / "include", qtdir / "lib"
        flags = [f"-I{inc}"] + [f"-I{inc / m}" for m in ("QtCore", "QtGui", "QtWidgets", "QtTest")] + \
                [f"-L{lib}", "-lQt6Widgets", "-lQt6Gui", "-lQt6Test", "-lQt6Core", f"-Wl,-rpath,{lib}",
                 # QtTest's widget overloads (keyClicks on a QWidget) need the module defines pkg-config gives
                 "-DQT_WIDGETS_LIB", "-DQT_GUI_LIB", "-DQT_TESTLIB_LIB", "-DQT_CORE_LIB"]
        env["QT_PLUGIN_PATH"] = str(qtdir / "plugins")
    else:
        flags = subprocess.run(["pkg-config", "--cflags", "--libs", "Qt6Widgets", "Qt6Test"], capture_output=True, text=True,
                               check=True).stdout.split()
    subprocess.run(["g++", "-std=c++17", "-fPIC", "-O1", "-w", "-I", str(tmp), "-o", str(exe), str(HERE / "observe_probe.cpp"), *flags],
                   check=True)
    env["QT_QPA_PLATFORM"] = "offscreen"
    return exe, env


def parse(text: str, owner_of: dict[str, str]) -> dict:
    version = None
    types: dict[str, dict] = {}
    props: dict[str, dict] = {}
    notes = []

    def own(cls: str) -> str | None:
        return owner_of.get(cls)

    def lst(s: str) -> list[str]:
        return sorted(x for x in s.split(",") if x) if s else []

    for line in text.splitlines():
        f = line.split("\t")
        if f[0] == "#qt":
            version = f[1]
        elif f[0] == "T" and own(f[1]):
            types[own(f[1])] = {"class": f[2]}
        elif f[0] == "F" and own(f[1]):
            r = props.setdefault(f"{own(f[1])}:{f[2]}", {})
            r["flags"] = {"readable": f[3] == "1", "writable": f[4] == "1", "constant": f[5] == "1", "bindable": f[6] == "1",
                          "user": f[7] == "1", "notify": None if f[8] == "-" else f[8]}
        elif f[0] == "S" and own(f[2]):
            r = props.setdefault(f"{own(f[2])}:{f[3]}", {})
            r.setdefault("sweep", {"on": own(f[1]) or f[1], "readback": f[4], "notify_change": int(f[5]), "notify_same": int(f[6]),
                                   "bindable_change": int(f[7]), "events": lst(f[8]), "signals": lst(f[9] if len(f) > 9 else "")})
        elif f[0] == "E" and own(f[3]):
            kind, _, rest = f[2].partition(":")
            if kind == "set":
                c, _, pn = rest.partition(":")
                cause = f"set:{own(c) or c}:{pn}"
            else:
                cause = f"scenario:{rest}"
            seen = props.setdefault(f"{own(f[3])}:{f[4]}", {}).setdefault("seen", [])
            rec = {"cause": cause, "on": own(f[1]) or f[1], "notify": int(f[5]), "changed": f[6] == "1", "bindable": int(f[7]),
                   "events": lst(f[8]), "signals": lst(f[9] if len(f) > 9 else "")}
            if rec not in seen:
                seen.append(rec)
        elif f[0] == "C":
            notes.append({"scenario": f[1], "note": f[2]})
    for r in props.values():
        if "seen" in r:
            r["seen"] = sorted(r["seen"], key=lambda s: (not (s["changed"] and s["notify"] <= 0 and s["bindable"] <= 0),
                                                         s["cause"], s["on"]))[:MAX_SEEN]
    return {"version": version, "types": types, "props": props, "notes": notes}


def main(args) -> int:
    owner_of = subjects()
    qtdir = Path(args.qtdir) if args.qtdir else (DEFAULT_QT if DEFAULT_QT.exists() else None)
    skip: list[str] = []
    with tempfile.TemporaryDirectory() as t:
        exe, env = build(Path(t), qtdir, list(owner_of))
        for _ in range(40):
            env["SVL_SKIP"] = " ".join(skip)
            try:
                p = subprocess.run([str(exe)], capture_output=True, text=True, env=env, timeout=900, stdin=subprocess.DEVNULL)
                rc, out = p.returncode, p.stdout
            except subprocess.TimeoutExpired as e:
                rc, out = -9, (e.stdout.decode() if isinstance(e.stdout, bytes) else e.stdout or "")
            if rc == 0 and "#end" in out:
                break
            begins = [ln.split("\t")[1] for ln in out.splitlines() if ln.startswith("#begin\t")]
            if not begins or begins[-1] in skip:
                sys.stderr.write(out[-3000:])
                raise SystemExit(f"observe probe: failed (rc {rc}) without a culprit")
            print(f"observe probe: {begins[-1]} crashed (rc {rc}); rerunning without it", file=sys.stderr)
            skip.append(begins[-1])
        else:
            raise SystemExit("observe probe: too many crashes")
    doc = parse(out, owner_of)
    doc = {"schema": "svelters-probe-observe/v1", "toolkit": "qt", "version": doc["version"],
           "crashed": sorted(owner_of.get(s, s) for s in skip), "types": doc["types"], "props": doc["props"], "notes": doc["notes"]}
    path = Path(args.out) if args.out else OUT_DIR / f"{doc['version']}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")
    print(f"observe probe: Qt {doc['version']}: {len(doc['types'])} classes, {len(doc['props'])} props "
          f"({sum(1 for r in doc['props'].values() if 'sweep' in r)} swept), {len(skip)} crashed -> {path}", file=sys.stderr)
    return 0
