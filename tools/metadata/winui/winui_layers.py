"""The WinUI annotation layers merged per class: inferred < manual (ingested facts stay in metadata/winui).

    metadata/inferred/winui/<Class>.yaml   rules over the winmd (tools/metadata/winui/infer.py)
    metadata/overrides/winui/<Class>.yaml  hand/agent annotations; win record by record (same name), and
                                           `deny: ["<section>:<glob>"]` drops inferred records
    metadata/inferred/winui/_parked.json   inferred facts whose generated test failed on the Windows CI job
                                           (tools/metadata/ledger.py import): left out

Both files use the override schema (tools/metadata/winui/override.schema.json). Every merged record says where it
came from: `origins[(class, section, name)]` is `manual` or `inferred`, and `record_fps[...]` is the fingerprint of
the inferred record (what the ledger parks it by). Readers: gen_backend.py (tables, tests, facts), and
tools/metadata/common/gen_bindings.py (renderer binding table). A manual `dangerous:` block (members the backend owns)
is not merged: tools/metadata/common/dangerous.py reads it from the override file.
"""
from __future__ import annotations

import fnmatch
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent / "common"))
import svmeta  # noqa: E402

OVERRIDES = ROOT / "metadata/overrides/winui"
INFERRED = ROOT / "metadata/inferred/winui"
PARKED = INFERRED / "_parked.json"
# Sections a class record can have, and the key that names a record.
SECTIONS = {"props": "name", "events": "name", "methods": "name", "bindings": "name", "notify": "name",
            "slots": "name", "attached": "name"}


class Layers:
    def __init__(self, overrides: Path = OVERRIDES, inferred: Path = INFERRED, parked: Path | None = None) -> None:
        parked = parked if parked is not None else inferred / "_parked.json"
        self.manual: dict[str, dict] = {}
        self.files: dict[str, str] = {}
        self.paths: list[Path] = []
        for p in sorted(overrides.glob("*.yaml")):
            o = svmeta.load_yaml(p)
            self.manual[o["class"]] = o
            self.files[o["class"]] = p.name
            self.paths.append(p)
        self.inferred: dict[str, dict] = {}
        self.inferred_paths: list[Path] = []
        if inferred.exists():
            for p in sorted(inferred.glob("*.yaml")):
                o = svmeta.load_yaml(p)
                self.inferred[o["class"]] = o
                self.files.setdefault(o["class"], f"inferred/winui/{p.name}")
                self.inferred_paths.append(p)
        self.parked: set[tuple[str, str, str]] = set()
        if parked.exists():
            for r in json.loads(parked.read_text())["parked"]:
                self.parked.add((r["kind"], r["section"], r["name"]))
        self.origins: dict[tuple[str, str, str], str] = {}
        self.record_fps: dict[tuple[str, str, str], str] = {}
        self.denied: list[tuple[str, str, str]] = []
        self.merged: dict[str, dict] = {}
        for cls in sorted(set(self.manual) | set(self.inferred)):
            self.merged[cls] = self._merge(cls)

    def _merge(self, cls: str) -> dict:
        man, inf = self.manual.get(cls, {}), self.inferred.get(cls, {})
        short = cls.rsplit(".", 1)[1]
        deny = man.get("deny", [])
        out = {k: v for k, v in man.items() if k not in SECTIONS and k != "deny" and k not in svmeta.READ_TIME_KEYS}
        out["class"] = cls
        for sec, key in SECTIONS.items():
            recs: list[dict] = []
            names = {r[key] for r in man.get(sec, [])}
            for r in inf.get(sec, []):
                ident = (short, sec, r[key])
                if r[key] in names:
                    continue
                if any(fnmatch.fnmatchcase(f"{sec}:{r[key]}", d) for d in deny):
                    self.denied.append(ident)
                    continue
                if ident in self.parked:
                    continue
                recs.append(r)
                self.origins[ident] = "inferred"
                self.record_fps[ident] = svmeta.fingerprint(r)
            for r in man.get(sec, []):
                recs.append(r)
                self.origins[(short, sec, r[key])] = "manual"
            if recs:
                # Manual records first, in file order, then the inferred ones (sorted by infer.py): the manual
                # tables keep their ids when an inferred layer is added.
                out[sec] = [r for r in recs if self.origins[(short, sec, r[key])] == "manual"] + \
                           [r for r in recs if self.origins[(short, sec, r[key])] == "inferred"]
        return out

    def origin(self, cls: str, sec: str, name: str) -> str:
        return self.origins.get((cls.rsplit(".", 1)[1], sec, name), "manual")

    def record_fp(self, cls: str, sec: str, name: str) -> str | None:
        return self.record_fps.get((cls.rsplit(".", 1)[1], sec, name))
