#!/usr/bin/env python3
"""The `observe` section of the GTK probe (milestone R0, plans/research/REACTIVITY.md sections 3.1 and 4.5).

    python3 tools/metadata/gtk/probe.py --section observe [--libdir DIR] [--girdir DIR] [--out FILE]

Builds tools/metadata/gtk/observe_probe.c, runs it on a private virtual display (tools/dev/headless) over every
GObject type of the metadata (the widget kinds of metadata/gtk/*.json and the non-GUI types of
metadata/gtk/types/*.json), reruns without a type that crashed, and writes
metadata/probe/gtk/observe/<GTK runtime version>.json. `--libdir` runs it against another libgtk-4 (the build
links the installed headers; the runtime library decides the version recorded), so the probe can match the
toolkit version of the ingested metadata. A maintainer step like the rest of the probe: CI never runs it, and
tools/metadata/gtk/infer.py reads the checked-in file.

Per property (keyed `<GIR owner>:<prop>`):
  flags   the GParamSpec flags the type registers (READABLE, WRITABLE, CONSTRUCT_ONLY, EXPLICIT_NOTIFY, ...);
          the GIR has no EXPLICIT_NOTIFY, so this is where the non-GUI types get it
  sweep   a scalar writable property set natively to a new value, then to the value read back:
          {readback: exact|normalized|ignored, notify_change: n, notify_same: n, on: <instance type>}
  seen    side effects: [{cause: set:<owner>:<prop> | scenario:<name>, on: <instance>, notify: n, changed: bool}]
          for every property that changed value or notified while something else was changed
Deterministic: sorted keys and lists; the output depends on the toolkit and the metadata only.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OUT_DIR = ROOT / "metadata/probe/gtk/observe"
FLAG_NAMES = [(1, "READABLE"), (2, "WRITABLE"), (4, "CONSTRUCT"), (8, "CONSTRUCT_ONLY"), (16, "LAX_VALIDATION"),
              (1 << 30, "EXPLICIT_NOTIFY"), (1 << 31, "DEPRECATED")]
MAX_SEEN = 12


def flag_names(v: int) -> list[str]:
    return [n for bit, n in FLAG_NAMES if v & bit]


def get_type_symbols(girdir: Path) -> dict[str, str]:
    """GType name -> its get_type function, from every GIR file (glib:type-name / glib:get-type)."""
    out: dict[str, str] = {}
    rx = re.compile(r'glib:type-name="([^"]+)"\s+glib:get-type="([^"]+)"|glib:get-type="([^"]+)"\s+glib:type-name="([^"]+)"')
    for p in sorted(girdir.glob("*.gir")):
        for m in rx.finditer(p.read_text(errors="replace")):
            name, sym = (m.group(1), m.group(2)) if m.group(1) else (m.group(4), m.group(3))
            out.setdefault(name, sym)
    return out


def subjects() -> list[tuple[str, str]]:
    """(GIR name, GType name) of every GObject type the metadata describes, sorted by GIR name."""
    out: dict[str, str] = {}
    for p in sorted((ROOT / "metadata/gtk").glob("*.json")):
        d = json.loads(p.read_text())
        if d.get("kind") in ("object", "interface"):
            out["Gtk." + d["id"].rsplit(":", 1)[-1]] = d["native"]["gtype"]
    for p in sorted((ROOT / "metadata/gtk/types").glob("*.json")):
        if p.name == "facts.json":
            continue
        d = json.loads(p.read_text())
        for name, t in d["types"].items():
            if t.get("category") in ("object", "model") and t.get("gtype"):
                out.setdefault(f"{d['namespace']}.{name}", t["gtype"])
    return sorted(out.items())


def build(tmp: Path) -> Path:
    exe = tmp / "observe_probe"
    flags = subprocess.run(["pkg-config", "--cflags", "--libs", "gtk4"], capture_output=True, text=True, check=True).stdout.split()
    subprocess.run(["cc", "-O1", "-w", "-o", str(exe), str(HERE / "observe_probe.c"), *flags, "-ldl"], check=True)
    return exe


def run_once(exe: Path, types: Path, skip: list[str], libdir: str | None) -> tuple[int, str]:
    env = dict(os.environ, SVL_SKIP=" ".join(skip), GSK_RENDERER="cairo")
    if libdir:
        env["LD_LIBRARY_PATH"] = libdir + (":" + env["LD_LIBRARY_PATH"] if env.get("LD_LIBRARY_PATH") else "")
    cmd = ([str(ROOT / "tools/dev/headless")] if sys.platform.startswith("linux") else []) + [str(exe), str(types)]
    try:
        p = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, text=True, env=env, timeout=600)
        return p.returncode, p.stdout
    except subprocess.TimeoutExpired as e:
        return -9, (e.stdout.decode() if isinstance(e.stdout, bytes) else e.stdout or "")


def parse(text: str, gir_of: dict[str, str]) -> dict:
    version = None
    types: dict[str, dict] = {}
    props: dict[str, dict] = {}
    notes: list[dict] = []

    def gir(gtype: str) -> str | None:
        return gir_of.get(gtype)

    def prop(owner: str, name: str) -> dict:
        return props.setdefault(f"{owner}:{name}", {})

    for line in text.splitlines():
        f = line.split("\t")
        if f[0] == "#gtk":
            version = f[1]
        elif f[0] == "T" and gir(f[1]):
            types[gir(f[1])] = {"gtype": f[1], "class": f[2]}
        elif f[0] == "F" and gir(f[1]):
            r = prop(gir(f[1]), f[2])
            r["flags"] = flag_names(int(f[3]))
            r["value_type"] = f[4]
        elif f[0] == "S" and gir(f[2]):
            r = prop(gir(f[2]), f[3])
            sw = {"on": gir(f[1]) or f[1], "readback": f[4], "notify_change": int(f[5]), "notify_same": int(f[6])}
            r.setdefault("sweep", sw)
        elif f[0] == "E" and gir(f[3]):
            kind, _, rest = f[2].partition(":")
            if kind == "set":
                o, _, pn = rest.partition(":")
                cause = f"set:{gir(o) or o}:{pn}"
            else:
                cause = f"scenario:{rest}"
            seen = prop(gir(f[3]), f[4]).setdefault("seen", [])
            rec = {"cause": cause, "on": gir(f[1]) or f[1], "notify": int(f[5]), "changed": f[6] == "1"}
            if rec not in seen:
                seen.append(rec)
        elif f[0] == "C":
            notes.append({"scenario": f[1], "note": f[2]})
    for r in props.values():
        if "seen" in r:
            # changed-without-notify first (the evidence a rule must not miss), then by cause
            r["seen"] = sorted(r["seen"], key=lambda s: (not (s["changed"] and not s["notify"]), s["cause"], s["on"]))[:MAX_SEEN]
    return {"version": version, "types": types, "props": props, "notes": notes}


def main(args) -> int:
    girdir = Path(args.girdir)
    syms = get_type_symbols(girdir)
    subj = subjects()
    gir_of = {gt: g for g, gt in subj}
    stdin = "".join(f"{g}\t{gt}\t{syms.get(gt, '')}\n" for g, gt in subj)
    skip: list[str] = []
    with tempfile.TemporaryDirectory() as t:
        exe = build(Path(t))
        types = Path(t) / "types.tsv"
        types.write_text(stdin)
        for _ in range(60):
            rc, out = run_once(exe, types, skip, args.libdir)
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
    doc = parse(out, gir_of)
    doc = {"schema": "svelters-probe-observe/v1", "toolkit": "gtk", "version": doc["version"],
           "built_against": subprocess.run(["pkg-config", "--modversion", "gtk4"], capture_output=True, text=True).stdout.strip(),
           "crashed": sorted(gir_of.get(s, s) for s in skip), "types": doc["types"], "props": doc["props"], "notes": doc["notes"]}
    out_path = Path(args.out) if args.out else OUT_DIR / f"{doc['version']}.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")
    n_sw = sum(1 for r in doc["props"].values() if "sweep" in r)
    print(f"observe probe: GTK {doc['version']}: {len(doc['types'])} types, {len(doc['props'])} props "
          f"({n_sw} swept), {len(skip)} crashed -> {out_path.relative_to(ROOT) if out_path.is_relative_to(ROOT) else out_path}",
          file=sys.stderr)
    return 0
