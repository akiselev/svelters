#!/usr/bin/env python3
"""Dangerous native members: reachable from app JS at run time, but the backend owns what they change.

A dangerous member is a method, a writable property (the write side) or a signal that app code can still
reach (a GI / selector / KVC call through `NativeElement.call`, `setProp`, an element attribute) although
calling it can corrupt the tree svelters manages, crash, or fight the backend's lifecycle. They are not
blocked at run time; they are marked so the docs and the typings warn about them (docs/notes/dangerous-members.md).

Two declaration places, lowest precedence first (the layer is read here, never merged into the facts, so no
fact record or fingerprint changes):

  metadata/dangerous/<tk>.yaml           pattern rules for whole families (schema svelters-dangerous/v1,
                                         tools/metadata/common/dangerous.schema.json)
  metadata/overrides/<tk>/<Kind>.yaml    a `dangerous:` block: {methods|properties|signals: {<native name>:
                                         annotation | false}}; `false` exempts the member from every rule

Member names are the native names of metadata/<tk>/<Kind>.json (`name`). The inferred layer may never hold
a dangerous member (`--check`).

    python3 tools/metadata/common/dangerous.py --check     # schema, references, inferred layer, GTK rules
    python3 tools/metadata/common/dangerous.py --list      # every dangerous member, per toolkit
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import naming  # noqa: E402
import svmeta  # noqa: E402

SCHEMA_PATH = HERE / "dangerous.schema.json"
FILE_SCHEMA = "svelters-dangerous/v1"
TOOLKITS = ("gtk", "qt", "appkit", "winui")
SECTIONS = ("methods", "properties", "signals")
SEVERITIES = svmeta.DANGEROUS_SEVERITIES
ANNOTATION_KEYS = ("severity", "reason", "use_instead", "reachable", "evidence")
# record lists of the override / inferred files and the member section they name
LIST_SECTION = {"properties": "properties", "props": "properties", "setters": "properties", "methods": "methods",
                "signals": "signals", "events": "signals", "bindings": "properties"}


def short(name: str) -> str:
    """`Gtk.Window` / `Microsoft.UI.Xaml.Controls.Panel` / `svelters-api:...:Gtk:Window` -> `Window`."""
    return re.split(r"[.:]", name)[-1]


def rules_path(tk: str, root: Path = ROOT) -> Path:
    return Path(root) / "metadata/dangerous" / f"{tk}.yaml"


def override_block_schema() -> dict:
    """The `dangerous:` block schema with its refs resolved (each override schema carries a copy)."""
    s = json.loads(SCHEMA_PATH.read_text())
    ann = s["$defs"]["annotation"]
    member = {"oneOf": [ann, {"const": False}]}
    block = json.loads(json.dumps(s["$defs"]["override_block"]))
    for sec in SECTIONS:
        block["properties"][sec]["additionalProperties"] = member
    return block


def validate_rules_file(path: Path) -> list[str]:
    import jsonschema

    schema = json.loads(SCHEMA_PATH.read_text())
    data = svmeta.load_yaml(path)
    errs = []
    for e in sorted(jsonschema.Draft202012Validator(schema).iter_errors(data), key=lambda e: [str(p) for p in e.absolute_path]):
        errs.append(f"{Path(path).name}: {'/'.join(map(str, e.absolute_path)) or '<root>'}: {e.message[:200]}")
    ids = [r.get("id") for r in data.get("rules", []) or []]
    errs += [f"{Path(path).name}: duplicate rule id {i}" for i in sorted({i for i in ids if ids.count(i) > 1})]
    for r in data.get("rules", []) or []:
        try:
            re.compile(r.get("name", ""))
        except re.error as e:
            errs.append(f"{Path(path).name}: rule {r.get('id')}: bad regex: {e}")
    return errs


class Members:
    """The members of every kind of one toolkit, from metadata/<tk>/*.json."""

    def __init__(self, tk: str, root: Path = ROOT):
        self.tk = tk
        self.kinds: dict[str, dict] = {}
        for p in sorted((Path(root) / "metadata" / tk).glob("*.json")):
            if p.name.startswith("_"):
                continue
            d = svmeta.load_json(p)
            if "id" not in d:
                continue
            props = {}
            for r in d.get("properties", []) or []:
                acc = r.get("access") or {}
                props[r["name"]] = {"js": r.get("js_name") or naming.kebab(r["name"]), "write": bool(acc.get("write", True)) and not acc.get("construct_only")}
            self.kinds[p.stem] = {
                "bases": [short(b) for b in d.get("bases", []) or []],
                "methods": {r["name"]: {"js": r.get("js_name") or r["name"]} for r in d.get("methods", []) or []},
                "properties": props,
                "signals": {r["name"]: {"js": r.get("js_event") or naming.kebab(r["name"])} for r in d.get("signals", []) or []},
            }

    def chain(self, kind: str) -> list[str]:
        return [kind] + (self.kinds.get(kind) or {}).get("bases", [])


def rule_matches(rule: dict, kind: str, bases: list[str], section: str, name: str) -> bool:
    if rule["section"] != section or not re.fullmatch(rule["name"], name):
        return False
    kinds = rule.get("kinds")
    return not kinds or any(k in kinds for k in [kind] + list(bases))


def _annotation(src: dict, source: str) -> dict:
    out = {k: src[k] for k in ANNOTATION_KEYS if k in src}
    out["source"] = source
    return out


class Dangerous:
    """Resolves the dangerous annotation of one toolkit's members (rules < override `dangerous:` blocks)."""

    def __init__(self, tk: str, root: Path = ROOT, members: Members | None = None):
        self.tk, self.root = tk, Path(root)
        self.members = members or Members(tk, root)
        p = rules_path(tk, root)
        self.rules = (svmeta.load_yaml(p).get("rules") or []) if p.exists() else []
        self.manual: dict[tuple[str, str, str], Any] = {}
        self.manual_files: dict[tuple[str, str, str], str] = {}
        for f in sorted((self.root / "metadata/overrides" / tk).glob("*.yaml")):
            block = svmeta.load_yaml(f).get("dangerous") or {}
            for sec in SECTIONS:
                for name, ann in (block.get(sec) or {}).items():
                    self.manual[(f.stem, sec, name)] = ann
                    self.manual_files[(f.stem, sec, name)] = f"metadata/overrides/{tk}/{f.name}"

    def lookup(self, kind: str, section: str, name: str) -> dict | None:
        """The annotation of `kind`'s own member `name` (`section`: methods | properties | signals), or None.
        A rule on a property applies only to a writable one (reading is harmless)."""
        key = (kind, section, name)
        if key in self.manual:
            ann = self.manual[key]
            return _annotation(ann, self.manual_files[key]) if ann else None
        bases = (self.members.kinds.get(kind) or {}).get("bases", [])
        if section == "properties":
            rec = ((self.members.kinds.get(kind) or {}).get("properties") or {}).get(name)
            if rec is not None and not rec["write"]:
                return None
        for r in self.rules:
            if rule_matches(r, kind, bases, section, name):
                return _annotation(r, f"metadata/dangerous/{self.tk}.yaml#{r['id']}")
        return None

    def all(self) -> list[tuple[str, str, str, dict]]:
        """(kind, section, name, annotation) of every dangerous member declared in metadata/<tk>, sorted."""
        out = []
        for kind, k in sorted(self.members.kinds.items()):
            for sec in SECTIONS:
                for name in sorted(k[sec]):
                    ann = self.lookup(kind, sec, name)
                    if ann:
                        out.append((kind, sec, name, ann))
        return out

    def counts(self) -> dict[str, int]:
        c = {sec: 0 for sec in SECTIONS}
        for _, sec, _, _ in self.all():
            c[sec] += 1
        return c

    # ---- checks -------------------------------------------------------------------------------

    def check(self) -> list[str]:
        errs: list[str] = []
        p = rules_path(self.tk, self.root)
        if p.exists():
            errs += validate_rules_file(p)
            if svmeta.load_yaml(p).get("toolkit") != self.tk:
                errs.append(f"{p.name}: toolkit must be {self.tk}")
        found = self.all()
        # every rule marks something; every manual entry names a real member
        for r in self.rules:
            if not any(a["source"].endswith("#" + r["id"]) for _, _, _, a in found):
                errs.append(f"dangerous/{self.tk}.yaml rule {r['id']}: matches no member of metadata/{self.tk}")
        for (kind, sec, name), ann in sorted(self.manual.items()):
            k = self.members.kinds.get(kind)
            if k is None or name not in k[sec]:
                errs.append(f"{self.manual_files[(kind, sec, name)]}: dangerous {sec} {name!r} is not a member of metadata/{self.tk}/{kind}.json")
            elif ann is False and not any(rule_matches(r, kind, k["bases"], sec, name) for r in self.rules):
                errs.append(f"{self.manual_files[(kind, sec, name)]}: dangerous {sec} {name!r}: false exempts nothing (no rule matches)")
        errs += self.check_inferred(found)
        if self.tk == "gtk":
            errs += self.check_gtk_rules(found)
        return errs

    def names_of(self, found) -> dict[str, dict[str, set[str]]]:
        """kind -> section -> native and JS names of its dangerous members."""
        out: dict[str, dict[str, set[str]]] = {}
        for kind, sec, name, _ in found:
            js = self.members.kinds[kind][sec][name]["js"]
            out.setdefault(kind, {s: set() for s in SECTIONS})[sec] |= {name, js}
        return out

    def check_inferred(self, found) -> list[str]:
        """No inferred record (props, methods, signals, bindings) of a kind or its bases is a dangerous member."""
        names = self.names_of(found)
        errs = []
        d = self.root / "metadata/inferred" / self.tk
        for f in sorted(d.glob("*.yaml")) if d.is_dir() else []:
            if f.name.startswith("_"):
                continue
            data = svmeta.load_yaml(f)
            if "dangerous" in data:
                errs.append(f"inferred/{self.tk}/{f.name}: the inferred layer may not declare `dangerous`")
            chain = self.members.chain(data.get("class") or f.stem)
            bad = {s: set().union(*[names.get(c, {}).get(s, set()) for c in chain]) for s in SECTIONS}

            def walk(v, lst=None):
                if isinstance(v, dict):
                    for k, x in v.items():
                        walk(x, k if isinstance(x, list) else None)
                elif isinstance(v, list):
                    sec = LIST_SECTION.get(lst or "")
                    for r in v:
                        if sec and isinstance(r, dict):
                            ids = {r.get(k) for k in ("name", "native", "js", "property")} - {None}
                            hit = ids & bad[sec]
                            if hit:
                                errs.append(f"inferred/{self.tk}/{f.name}: {lst} {sorted(hit)[0]!r} is a dangerous member "
                                            f"(metadata/dangerous/{self.tk}.yaml or an override `dangerous:` block); deny it in the rules")
                        walk(r)

            walk(data)
        return errs

    def check_gtk_rules(self, found) -> list[str]:
        """GTK infers every GI method no `deny_methods` pattern denies: each dangerous method must be denied, so
        a re-probe cannot make it an inferred fact (metadata/rules/gtk.yaml)."""
        rules = svmeta.load_yaml(self.root / "metadata/rules/gtk.yaml")
        pats = [re.compile(d["pattern"]) for d in rules.get("deny_methods", [])]
        return [f"metadata/rules/gtk.yaml: dangerous method {kind}.{name} is not denied by deny_methods"
                for kind, sec, name, _ in found if sec == "methods" and not any(p.fullmatch(name) for p in pats)]


SCHEMA_LINE = '    "dangerous": {block},\n'
SCHEMA_ANCHOR = '\n  "properties": {\n'


def check_override_schemas(root: Path = ROOT) -> list[str]:
    """Every toolkit's override schema carries the same top-level `dangerous` block as dangerous.schema.json."""
    want = override_block_schema()
    errs = []
    for tk in TOOLKITS:
        s = json.loads((Path(root) / f"tools/metadata/{tk}/override.schema.json").read_text())
        if s.get("properties", {}).get("dangerous") != want:
            errs.append(f"tools/metadata/{tk}/override.schema.json: properties.dangerous must equal dangerous.override_block_schema() "
                        f"(run: python3 tools/metadata/common/dangerous.py --sync-schemas)")
    return errs


def sync_override_schemas(root: Path = ROOT) -> None:
    """Write the block as the first top-level property of each override schema, one line (the files are hand formatted)."""
    line = SCHEMA_LINE.format(block=json.dumps(override_block_schema(), ensure_ascii=False))
    for tk in TOOLKITS:
        p = Path(root) / f"tools/metadata/{tk}/override.schema.json"
        text = p.read_text()
        text = re.sub(r'(?m)^    "dangerous": .*\n', "", text, count=1)
        if SCHEMA_ANCHOR not in text:
            raise SystemExit(f"{p}: no top-level `\"properties\": {{` line to anchor the dangerous block")
        text = text.replace(SCHEMA_ANCHOR, SCHEMA_ANCHOR + line, 1)
        if p.read_text() != text:
            p.write_text(text)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--sync-schemas", action="store_true", help="copy the override block into the four override schemas")
    a = ap.parse_args()
    if a.sync_schemas:
        sync_override_schemas()
    errs = check_override_schemas() if a.check else []
    for tk in TOOLKITS:
        d = Dangerous(tk)
        if a.check:
            errs += d.check()
        if a.list:
            rows = d.all()
            c = d.counts()
            print(f"## {tk}: {len(rows)} ({', '.join(f'{v} {k}' for k, v in c.items())})")
            for kind, sec, name, ann in rows:
                print(f"  {kind}.{sec}.{name}  [{ann['severity']}]  {ann['source']}")
    if errs:
        print("dangerous members check failed:\n  " + "\n  ".join(errs), file=sys.stderr)
        return 1
    if a.check:
        print("dangerous members: ok (" + ", ".join(f"{tk} {len(Dangerous(tk).all())}" for tk in TOOLKITS) + ")")
    return 0


if __name__ == "__main__":
    sys.exit(main())
