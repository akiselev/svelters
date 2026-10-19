#!/usr/bin/env python3
"""Infer WinUI annotations from the winmd facts and the rules file (no runtime probe: WinUI does not run on Linux).

    python3 tools/metadata/winui/infer.py [--check]

Inputs (a pure function of these; no network, clock or Windows):
  metadata/winui/*.json, _types.json  the ingested winmd facts (tools/metadata/winui/ingest.py); annotations are
                                      never read back from them, so rerunning is idempotent
  metadata/rules/winui.yaml           samples, deny lists, the no-notify list (O6), triggers, thresholds
  metadata/overrides/winui/*.yaml     only which classes are element kinds (generic bindings go on those)

Outputs (byte-identical on rerun):
  metadata/inferred/winui/<Class>.yaml           facts with confidence >= promote_at, in the override schema
                                                 (tools/metadata/winui/override.schema.json); merged under the
                                                 manual overrides by tools/metadata/winui/layers.py
  metadata/inferred/winui/<Class>.evidence.json  rule, confidence and the winmd excerpt of every fact
  metadata/inferred/winui/_review.json           facts below promote_at and what was skipped, by pattern

Rules (plans/research/ANNOTATION-AUTOMATION.md section 2, plans/research/REACTIVITY.md section 3.4):
  P1  a readable prop of a convertible type (bool, i32, f64, string, enum, Thickness, IReference<bool>) is exposed;
      writable ones get samples by type and name (written and read back in order), read-only ones a read test.
  O6  a writable DependencyProperty gets a `notify::<prop>` event (RegisterPropertyChangedCallback), except the
      DPs that never notify (no_notify: ActualWidth/ActualHeight/ActualTheme).
  B1  a notify prop of a scalar type declared on an element class gets a generic two-way `bind:<prop>`
      (change: notify::<prop>, value read back from the prop, compare before set).
  R1  an event is exposed; if its args class declares a settable Cancel or Handled it is sync and `true` from the
      handler sets it (preventDefault).
  R2  an event's test trigger comes from the rules (a prop set, mounting into a window); otherwise its test only
      connects and disconnects (`verified: connect`).
  M1  zero-argument methods returning nothing or a convertible value are exposed (minus deny_methods, M2);
      methods with convertible arguments are held for review (the arguments need judgement).

Confidence: 0.9 rule only, the generated test on the Windows CI job is the only check left; 0.6 needs a judgement
(an object prop as a string, method arguments).
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent / "common"))
import naming  # noqa: E402
import svmeta  # noqa: E402

META = ROOT / "metadata/winui"
OVERRIDES = ROOT / "metadata/overrides/winui"
RULES = ROOT / "metadata/rules/winui.yaml"
OUT = ROOT / "metadata/inferred/winui"
SCHEMA = HERE / "override.schema.json"

SCALAR = {"bool", "i32", "f64", "string"}
# Binding type refs the renderer table carries (tools/metadata/common/gen_bindings.py TYPES).
BINDABLE = SCALAR | {"nullable<bool>"}


def wire(ref: str, types: dict) -> str | None:
    """The generator's wire class of a type ref (gen_backend.ty_of), or None when it has no conversion."""
    if ref in SCALAR:
        return ref
    if ref == "value<Microsoft.UI.Xaml.Thickness>":
        return "thickness"
    if ref == "nullable<bool>":
        return "nullable_bool"
    m = re.fullmatch(r"enum<(.+)>", ref)
    if m and (types.get(m.group(1)) or {}).get("category") == "enum":
        return "enum"
    return None


class Rules:
    def __init__(self, doc: dict | None = None) -> None:
        d = doc if doc is not None else svmeta.load_yaml(RULES)
        self.promote_at = float(d["promote_at"])
        self.samples = d["samples"]
        self.deny_props = [(re.compile(r["pattern"]), r["reason"]) for r in d.get("deny_props", [])]
        self.no_notify = {r["prop"]: r for r in d.get("no_notify", [])}
        self.deny_methods = [(re.compile(r["pattern"]), r["reason"]) for r in d.get("deny_methods", [])]
        self.deny_events = [(re.compile(r["pattern"]), r["reason"]) for r in d.get("deny_events", [])]
        self.result_props = list(d.get("result_props", []))
        self.triggers = [(re.compile(r["pattern"]), r["trigger"]) for r in d.get("triggers", [])]

    @staticmethod
    def first(pats, name: str):
        return next((x for p, x in pats if p.search(name)), None)

    def sample(self, prop: str, cls_wire: str, ref: str, types: dict) -> list | None:
        for r in self.samples.get("by_name", []):
            if re.search(r["pattern"], prop) and cls_wire in ("f64", "i32", "string"):
                vals = r["values"]
                if cls_wire == "i32":
                    vals = [int(v) for v in vals]
                if all(isinstance(v, str) for v in vals) == (cls_wire == "string"):
                    return list(vals)
        if cls_wire == "enum":
            members = types[ref[len("enum<"):-1]]["members"]
            if len(members) < 2:
                return None
            return [members[1][0], members[0][0]]
        return list(self.samples[cls_wire])


class Klass:
    """The inferred facts of one class."""

    def __init__(self, k: dict, types: dict, rules: Rules, elements: set[str]) -> None:
        self.k, self.types, self.rules = k, types, rules
        self.cls = k["native"]["name"]
        self.short = k["native"]["short"]
        self.is_element = self.cls in elements
        self.dps = set(k.get("dependency_properties", []))
        self.ov: dict[str, list] = {}
        self.ev: dict[str, dict] = {}
        self.review: list[dict] = []
        self.skipped: dict[str, int] = {}

    def skip(self, pattern: str) -> None:
        self.skipped[pattern] = self.skipped.get(pattern, 0) + 1

    def add(self, sec: str, rec: dict, confidence: float, rule: str, evidence: dict, pattern: str) -> None:
        if confidence < self.rules.promote_at:
            self.review.append({"class": self.short, "section": sec, "name": rec["name"], "confidence": confidence,
                                "band": "middle" if confidence >= 0.6 else "low", "pattern": pattern})
            return
        self.ov.setdefault(sec, []).append(rec)
        self.ev[f"{sec}:{rec['name']}"] = {"confidence": confidence, "rule": rule, "evidence": evidence}

    def is_dp(self, p: dict) -> bool:
        return bool(p.get("dependency_property")) and f"{p['name']}Property" in self.dps

    def infer_props(self) -> None:
        for p in self.k["properties"]:
            full = f"{self.short}.{p['name']}"
            if Rules.first(self.rules.deny_props, full):
                self.skip("prop:denied")
                continue
            ref = p["type"]["ref"]
            w = wire(ref, self.types)
            if not p["access"]["read"]:
                self.skip("prop:write-only")
                continue
            if w is None:
                if ref == "variant" and p["access"]["write"]:
                    # Content, Header, Tag, ...: exposing an object prop as a boxed string (`as: string`) is a choice.
                    self.add("props", {"name": p["name"], "as": "string", "test": ["svelters", "winui"],
                                       "provenance": f"rule P1: {full}: object, as a boxed string (winmd)"},
                             0.6, "P1", {"type": ref}, "prop:object-as-string")
                else:
                    self.skip(f"prop:type:{ref.split('<')[0]}")
                continue
            ev = {"type": ref, "write": p["access"]["write"], "dependency_property": self.is_dp(p), "interface": p["interface"]}
            if p["access"]["write"]:
                vals = self.rules.sample(p["name"], w, ref, self.types)
                if vals is None:
                    self.skip("prop:no-sample")
                    continue
                self.add("props", {"name": p["name"], "test": vals,
                                   "provenance": f"rule P1: {full}: {ref}, read-write{' DP' if ev['dependency_property'] else ''} (winmd); "
                                                 "written and read back"},
                         0.9, "P1", {**ev, "samples": vals}, "prop:read-write")
            else:
                self.add("props", {"name": p["name"], "provenance": f"rule P1: {full}: {ref}, read-only (winmd); read test"},
                         0.9, "P1", ev, "prop:read-only")

    def infer_notify(self) -> None:
        for p in self.k["properties"]:
            full = f"{self.short}.{p['name']}"
            if not self.is_dp(p):
                continue
            if full in self.rules.no_notify:
                self.skip("notify:no-notify-rule")
                continue
            if Rules.first(self.rules.deny_props, full) or wire(p["type"]["ref"], self.types) is None:
                continue
            if not (p["access"]["read"] and p["access"]["write"]):
                self.skip("notify:read-only")       # the generated test changes the value to observe a callback
                continue
            if self.rules.sample(p["name"], wire(p["type"]["ref"], self.types), p["type"]["ref"], self.types) is None:
                self.skip("notify:no-sample")
                continue
            self.add("notify", {"name": p["name"], "provenance": f"rule O6: {full} is a DependencyProperty "
                                                                f"({self.short}.{p['name']}Property, winmd): RegisterPropertyChangedCallback"},
                     0.9, "O6", {"dp": f"{p['name']}Property", "type": p["type"]["ref"]}, "notify:dp")

    def infer_bindings(self) -> None:
        if not self.is_element:
            return
        notify = {r["name"] for r in self.ov.get("notify", [])}
        for p in self.k["properties"]:
            if p["name"] not in notify:
                continue
            ref = p["type"]["ref"]
            if not (ref in BINDABLE or re.fullmatch(r"enum<.+>", ref)):
                self.skip(f"binding:type:{ref.split('<')[0]}")
                continue
            js = naming.kebab(p["name"])
            self.add("bindings", {
                "name": js, "direction": "two_way", "property": js,
                "type": {"ref": "bool" if ref == "nullable<bool>" else ref, "nullable": ref.startswith("nullable<")},
                "change": {"signal": f"notify::{js}", "role": "any_change", "value": "prop"},
                "write": {"natively_normalizes": False},
                # Numbers may be coerced (RangeBase clamps; GTK reads back props with a range): read back after a set.
                "echo": {"strategy": "none", "compare_before_set": True, "read_back": ref in ("i32", "f64")},
                "lifetime": {"on_native_death": "end_binding", "on_stale_update": "warn_noop", "on_destroy": "idempotent_disconnect"},
                "provenance": f"rule B1: {self.short}.{p['name']} is read-write, scalar and a notifying DependencyProperty: "
                              f"notify::{js} carries no value, the renderer reads the prop and compares before set"},
                0.9, "B1", {"notify": f"notify::{js}", "type": ref}, "binding:dp")

    def infer_events(self) -> None:
        for s in self.k["signals"]:
            full = f"{self.short}.{s['name']}"
            if Rules.first(self.rules.deny_events, full):
                self.skip("event:denied")
                continue
            rec: dict = {"name": s["name"]}
            own = {p["name"] for p in s["args_props"] if p["settable"] and p["declared_on"] == s["args"] and p["type"]["ref"] == "bool"}
            res = next((r for r in self.rules.result_props if r in own), None)
            if res:
                rec["sync"] = True
                rec["result"] = {"prop": res}
            trig = Rules.first(self.rules.triggers, full)
            if trig:
                rec["trigger"] = trig
            how = f"trigger {trig}" if trig else "connect test"
            rec["provenance"] = (f"rule R1/R2: {full}: {s['delegate']} (winmd)"
                                 + (f"; {s['args']}.{res} is settable: sync, preventDefault sets it" if res else "") + f"; {how}")
            self.add("events", rec, 0.9, "R1/R2", {"delegate": s["delegate"], "args": s["args"], "result": res, "trigger": trig},
                     "event:triggered" if trig else "event:connect")

    def infer_methods(self) -> None:
        names = [m["name"] for m in self.k["methods"]]
        for m in self.k["methods"]:
            full = f"{self.short}.{m['name']}"
            if names.count(m["name"]) > 1:
                self.skip("method:overloaded")      # windows-rs renames overloads; the generator calls the winmd name
                continue
            if Rules.first(self.rules.deny_methods, full):
                self.skip("method:denied")
                continue
            ret = m["returns"]
            void = ret["native"] == "void"
            if not void and wire(ret["ref"], self.types) not in SCALAR | {"enum"}:
                self.skip(f"method:return:{ret['ref'].split('<')[0]}")
                continue
            if any(wire(p["type"]["ref"], self.types) is None for p in m["params"]):
                self.skip("method:param-type")
                continue
            rec = {"name": m["name"], "provenance": f"rule M1: {full}({', '.join(p['type']['ref'] for p in m['params'])}) -> "
                                                    f"{'void' if void else ret['ref']} (winmd)"}
            if m["params"]:
                self.add("methods", rec, 0.6, "M1", {"params": [p["type"]["ref"] for p in m["params"]]}, "method:scalar-args")
            else:
                self.add("methods", rec, 0.9, "M1", {"returns": None if void else ret["ref"]},
                         "method:action" if void else "method:getter")


def element_classes() -> set[str]:
    return {svmeta.load_yaml(p)["class"] for p in sorted(OVERRIDES.glob("*.yaml")) if "element" in svmeta.load_yaml(p)}


def run(rules: Rules | None = None) -> tuple[dict[str, str], dict[str, str], dict]:
    rules = rules or Rules()
    types = svmeta.load_json(META / "_types.json")["types"]
    elements = element_classes()
    files, evid = {}, {}
    review: list[dict] = []
    skipped: dict[str, int] = {}
    counts: dict[str, int] = {}
    version = None
    for p in sorted(META.glob("*.json")):
        if p.name.startswith("_"):
            continue
        k = svmeta.load_json(p)
        version = version or k["source"]["package"].rsplit(" ", 1)[1]
        kl = Klass(k, types, rules, elements)
        kl.infer_props()
        kl.infer_notify()
        kl.infer_bindings()
        kl.infer_events()
        kl.infer_methods()
        review += kl.review
        for pat, n in kl.skipped.items():
            skipped[pat] = skipped.get(pat, 0) + n
        if not kl.ov:
            continue
        doc = {"class": kl.cls}
        for sec in ("props", "notify", "events", "methods", "bindings"):
            if sec in kl.ov:
                doc[sec] = sorted(kl.ov[sec], key=lambda r: r["name"])
        files[f"{kl.short}.yaml"] = ("# @generated by tools/metadata/winui/infer.py from metadata/winui and metadata/rules/winui.yaml. "
                                     "Do not edit: override in metadata/overrides/winui.\n"
                                     + yaml.safe_dump(doc, sort_keys=False, width=140, allow_unicode=True))
        evid[f"{kl.short}.evidence.json"] = json.dumps(kl.ev, indent=1, sort_keys=True) + "\n"
        for key in kl.ev:
            sec = key.split(":", 1)[0]
            counts[sec] = counts.get(sec, 0) + 1
    by_pattern: dict[str, int] = {}
    for r in review:
        key = f"{r['band']}:{r['pattern']}"
        by_pattern[key] = by_pattern.get(key, 0) + 1
    summary = {"schema": "svelters-inferred-review/v1", "toolkit": "winui", "version": version, "promote_at": rules.promote_at,
               "inferred": dict(sorted(counts.items())),
               "unreviewed": {"middle": sum(1 for r in review if r["band"] == "middle"),
                              "low": sum(1 for r in review if r["band"] == "low"),
                              "by_pattern": dict(sorted(by_pattern.items()))},
               "skipped": dict(sorted(skipped.items())),
               "rows": sorted(review, key=lambda r: (r["class"], r["section"], r["name"]))}
    return files, evid, summary


def record_fingerprints(files: dict[str, str]) -> dict[tuple[str, str, str], str]:
    """(class short name, section, name) -> the fingerprint layers.py gives the inferred record (the ledger parks by it)."""
    out = {}
    for fname, text in files.items():
        doc = yaml.safe_load(text)
        for sec in ("props", "notify", "events", "methods", "bindings"):
            for r in doc.get(sec, []):
                out[(fname[:-len(".yaml")], sec, r["name"])] = svmeta.fingerprint(r)
    return out


def prune_parked(files: dict[str, str], check: bool) -> bool:
    """Drop parked facts whose inferred record changed or went away: they are tried again. True if stale."""
    path = OUT / "_parked.json"
    if not path.exists():
        return False
    doc = json.loads(path.read_text())
    fps = record_fingerprints(files)
    keep = [r for r in doc["parked"] if fps.get((r["kind"], r["section"], r["name"])) == r.get("record_fp")]
    if len(keep) == len(doc["parked"]):
        return False
    if not check:
        doc["parked"] = keep
        path.write_text(json.dumps(doc, indent=1, sort_keys=True, ensure_ascii=False) + "\n")
    return True


def validate(files: dict[str, str]) -> list[str]:
    import tempfile
    errs = []
    with tempfile.TemporaryDirectory() as t:
        for name, text in files.items():
            p = Path(t) / name
            p.write_text(text)
            errs += svmeta.check_override(p, SCHEMA)
    return errs


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if the checked-in inferred layer is stale")
    a = ap.parse_args()
    files, evid, summary = run()
    errs = validate(files)
    if errs:
        print("infer: the inferred layer breaks the override schema:", *errs[:20], sep="\n  ", file=sys.stderr)
        return 1
    want = {**files, **evid, "_review.json": json.dumps(summary, indent=1, sort_keys=True) + "\n"}
    parked_stale = prune_parked(files, a.check)
    have = {p.name for p in OUT.glob("*") if p.is_file() and p.name != "_parked.json"} if OUT.exists() else set()
    stale = sorted([n for n, t in want.items() if not (OUT / n).exists() or (OUT / n).read_text() != t] + sorted(have - set(want)))
    if a.check:
        for n in stale:
            print(f"infer: metadata/inferred/winui/{n} is stale (run tools/metadata/winui/infer.py)", file=sys.stderr)
        if parked_stale:
            print("infer: metadata/inferred/winui/_parked.json lists facts whose inferred record changed "
                  "(run tools/metadata/winui/infer.py)", file=sys.stderr)
        return 1 if stale or parked_stale else 0
    OUT.mkdir(parents=True, exist_ok=True)
    for n in sorted(have - set(want)):
        (OUT / n).unlink()
    for n, t in want.items():
        if not (OUT / n).exists() or (OUT / n).read_text() != t:
            (OUT / n).write_text(t)
    inf = summary["inferred"]
    print(f"infer: {len(files)} classes; facts " + ", ".join(f"{k} {v}" for k, v in inf.items())
          + f"; unreviewed middle {summary['unreviewed']['middle']}, low {summary['unreviewed']['low']}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
