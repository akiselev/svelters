#!/usr/bin/env python3
"""Infer Qt annotations from the ingested metadata, the checked-in probe and the rules file.

    python3 tools/metadata/qt/infer.py [--check]

Inputs (a pure function of these; no Qt, display, clock or network):
  metadata/qt/*.json               the ingested facts (qdoc + metaobject + typesystem; never annotated in place)
  metadata/qt/_enums/Qt.json       enum members (an enum sample is a member name)
  metadata/probe/qt/<version>.json tools/metadata/qt/probe.py output (a maintainer step)
  metadata/rules/qt.yaml           accepted patterns, samplers, recipes, denylists
  metadata/overrides/qt/*.yaml     only which classes are exposed as elements, and their tags

Outputs (byte-identical on rerun):
  metadata/inferred/qt/<Class>.yaml           facts of an accepted pattern with confidence >= promote_at, as
                                              `elements: [{tag, props, methods, signals, bindings}]` records of the
                                              override schema (tools/metadata/qt/override.schema.json). The Qt
                                              generator merges them under the manual overrides (tools/metadata/qt/qt_layers.py).
  metadata/inferred/qt/<Class>.evidence.json  confidence and the probe excerpt of every written fact
  metadata/inferred/qt/_review.json           everything else: facts below promote_at or of a pattern the rules do
                                              not accept (middle and low band, by pattern), and skipped counts.
                                              Nothing reads it back: those facts stay unreviewed.

Accept-by-pattern (plans/research/ANNOTATION-AUTOMATION.md section 11, decision 6): only classes a manual override
exposes as an element get facts, and only of the patterns `accept` in metadata/rules/qt.yaml lists:
  prop:round-trip          P1/P3 writable scalar or enum Q_PROPERTY, probe sample read back exactly
  method:read-only-getter  M1 READ accessor of a read-only Q_PROPERTY (probe: getter == property)
  signal:notify-trigger    R1/R2/R3 scalar/enum-argument signal, emitted once per set of the property it NOTIFYs
  signal:property-trigger  the same, emitted by another probed property
  signal:delivery-only     a scalar-argument signal no probed set emits: the test emits it (`verified: delivery`)
  binding:notify           B1 generic `bind:<prop>` through the NOTIFY signal

Confidence: 0.95 probe and rule agree (notify fired, getter equals the property); 0.9 the generated test is the
only check left (a property without NOTIFY, a trigger that is not the NOTIFY); 0.8 delivery-only or an action with
one visible effect; 0.5 the probe disagrees (ignored, normalized, error) or a recipe has no C++ twin.
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
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import naming  # noqa: E402
import svmeta  # noqa: E402

META = ROOT / "metadata/qt"
OVERRIDES = ROOT / "metadata/overrides/qt"
PROBE_DIR = ROOT / "metadata/probe/qt"
RULES = ROOT / "metadata/rules/qt.yaml"
OUT = ROOT / "metadata/inferred/qt"
SCHEMA = HERE / "override.schema.json"

SCALAR_REFS = {"bool", "i32", "u32", "i64", "u64", "f32", "f64", "string"}
# The argument a delivery-only signal is emitted with, by neutral ref.
EMIT_SAMPLE = {"bool": True, "i32": 1, "u32": 1, "i64": 1, "u64": 1, "f32": 1.5, "f64": 1.5, "string": "svl"}
LIFETIME = {"on_native_death": "end_binding", "on_stale_update": "warn_noop", "on_destroy": "idempotent_disconnect"}


def is_enum_ref(ref: str) -> bool:
    return ref.startswith("enum<")


def known_ref(ref: str) -> bool:
    return ref in SCALAR_REFS or is_enum_ref(ref)


def cvalstr(v) -> str:
    """A probe value as the generated tests' `valstr` prints a signal argument (verify.cpp.in)."""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return "%g" % v
    return str(v)


class Rules:
    def __init__(self, path: Path = RULES):
        r = yaml.safe_load(path.read_text())
        self.promote_at = float(r["promote_at"])
        self.accept = set(r["accept"])
        self.scalar_types = dict(r["scalar_types"])
        self.recipes = dict(r["recipes"])
        self.deny = {k: [(re.compile(x["pattern"]), x["reason"]) for x in r.get(f"deny_{k}", [])]
                     for k in ("props", "methods", "signals")}

    def denied(self, section: str, name: str) -> str | None:
        return next((why for rx, why in self.deny[section] if rx.fullmatch(name)), None)


class Kind:
    """One probed, exposed class with its ingested record."""

    def __init__(self, cls: str, tags: list[str], probe: dict, doc: dict, enums: dict[str, dict], rules: Rules):
        self.cls, self.tags, self.k, self.d, self.enums, self.rules = cls, tags, probe, doc, enums, rules
        self.props = {p["name"]: p for p in doc["properties"]}
        self.methods: dict[str, list[dict]] = {}
        for m in doc["methods"]:
            self.methods.setdefault(m["name"], []).append(m)
        self.signals = {s["name"]: s for s in doc["signals"]}
        self.ov: dict[str, list] = {}
        self.ev: dict[str, dict] = {}
        self.review: list[dict] = []
        self.skipped: dict[str, int] = {}
        self.written: dict[str, set[str]] = {"props": set(), "signals": set()}

    # -- bookkeeping ---------------------------------------------------------------------------
    def skip(self, pattern: str) -> None:
        self.skipped[pattern] = self.skipped.get(pattern, 0) + 1

    def hold(self, section: str, name: str, conf: float, pattern: str, accepted: bool = True) -> None:
        """A candidate the evidence (or the accept list) does not support yet: a review row, never written."""
        row = {"kind": self.cls, "section": section, "name": name, "confidence": conf,
               "band": "middle" if conf >= 0.6 else "low", "pattern": pattern}
        if not accepted:
            row["accepted"] = False
        self.review.append(row)

    def emit(self, section: str, name: str, rec: dict, conf: float, evidence, pattern: str) -> bool:
        accepted = pattern in self.rules.accept
        if conf >= self.rules.promote_at and accepted:
            self.ov.setdefault(section, []).append(rec)
            self.ev[f"{section}:{name}"] = {"confidence": conf, "pattern": pattern, "evidence": evidence}
            return True
        self.hold(section, name, conf, pattern, accepted)
        return False

    # -- values ----------------------------------------------------------------------------------
    def ref_of(self, pname: str, probe: dict) -> str | None:
        """The neutral ref of a probed property the generated tests can sample, or None."""
        m = self.props.get(pname)
        if m is None:
            return None
        ref = m["type"]["ref"]
        if probe.get("enum"):
            # A scoped enum (`enum class`): the ingested enum table names its members unscoped (no C++ twin yet).
            if probe.get("flag") or probe.get("scoped") or not is_enum_ref(ref):
                return None
            return ref if ref[5:-1] in self.enums else None
        want = self.rules.scalar_types.get(probe.get("type"))
        return ref if want and ref == want else None

    def value_ok(self, ref: str, v) -> bool:
        if v is None:
            return False
        if is_enum_ref(ref):
            return isinstance(v, str) and v in {x["name"] for x in self.enums[ref[5:-1]]["members"]}
        if ref == "bool":
            return isinstance(v, bool)
        if ref in ("i32", "u32", "i64", "u64"):
            return isinstance(v, int) and not isinstance(v, bool)
        if ref in ("f32", "f64"):
            return isinstance(v, (int, float)) and not isinstance(v, bool)
        return isinstance(v, str) and not v.startswith("<")

    def setup_of(self, probe: dict) -> str | None | bool:
        """C++ precondition for a probe recipe: '' none, a statement, or False when the rules have no twin."""
        if not probe.get("recipe"):
            return ""
        return self.rules.recipes.get(probe.get("recipe_call") or "", False)

    def excerpt(self, p: dict) -> dict:
        return {k: p[k] for k in ("outcome", "sample", "default", "read", "notify", "notify_fired", "recipe", "recipe_call",
                                  "value", "stable", "user") if k in p}

    # -- properties (P1/P3) ------------------------------------------------------------------------
    def infer_props(self) -> None:
        for n, p in sorted((self.k.get("props") or {}).items()):
            m = self.props.get(n)
            if m is None or not p.get("writable") or m.get("deprecated") or not m["access"].get("write"):
                continue
            why = self.rules.denied("props", n)
            if why:
                self.skip(f"prop:denied ({why})")
                continue
            outcome = p.get("outcome")
            if outcome in ("ignored", "normalized", "error", "crash"):
                self.hold("props", n, 0.5, f"prop:{outcome}")
                continue
            if outcome != "exact":
                self.skip(f"prop:{outcome}")
                continue
            ref = self.ref_of(n, p)
            if ref is None:
                self.skip("prop:no-sampler" if not p.get("enum") else "prop:enum-not-in-table")
                continue
            if not self.value_ok(ref, p.get("sample")) or not self.value_ok(ref, p.get("default")):
                self.hold("props", n, 0.5, "prop:needs-fixture")
                continue
            setup = self.setup_of(p)
            if setup is False:
                self.hold("props", n, 0.5, f"prop:recipe-{p['recipe']}")
                continue
            js = naming.kebab(n)
            rec: dict = {"js": js, "native": n, "sample": p["sample"]}
            if setup:
                rec["setup"] = setup
            how = "round trip exact" + (f" after recipe {p['recipe']} ({p['recipe_call']})" if p.get("recipe") else "") + \
                  ("; NOTIFY fired" if p.get("notify_fired") else "")
            rec["provenance"] = [{"origin": "probe", "ref": how}]
            conf = 0.95 if (p.get("notify_fired") or not p.get("notify")) else 0.9
            if self.emit("props", js, rec, conf, self.excerpt(p), "prop:round-trip"):
                self.written["props"].add(n)

    # -- methods (M1) ------------------------------------------------------------------------------
    def getter_record(self, name: str) -> dict | None:
        for m in self.methods.get(name, []):
            if m.get("access") == "public" and not m.get("static") and not m.get("deprecated") and not m["params"]:
                return m
        return None

    def infer_methods(self) -> None:
        pm = self.k.get("methods") or {}
        for n, p in sorted((self.k.get("props") or {}).items()):
            m = self.props.get(n)
            if m is None or p.get("writable") or p.get("outcome") != "readonly" or m.get("deprecated"):
                continue
            getter = (m.get("accessors") or {}).get("get")
            g = self.getter_record(getter) if getter else None
            if g is None:
                self.skip("method:read-only-without-getter")
                continue
            why = self.rules.denied("methods", getter)
            if why:
                self.skip(f"method:denied ({why})")
                continue
            if self.ref_of(n, p) is None or g["returns"].get("ref") != m["type"]["ref"]:
                self.skip("method:read-only-non-scalar")
                continue
            probe = pm.get(getter) or {}
            if probe.get("class") == "accessor_get" and probe.get("outcome") == "exact":
                conf, how = 0.95, f"returns property {n} (probe: equal on a new instance)"
            elif probe.get("class") == "accessor_get":
                self.hold("methods", getter, 0.5, f"method:read-only-getter-{probe.get('outcome')}")
                continue
            else:
                conf, how = 0.9, f"READ accessor of read-only property {n}"
            rec = {"js": getter, "native": getter, "provenance": [{"origin": "probe", "ref": how}]}
            self.emit("methods", getter, rec, conf, {"prop": n, **self.excerpt(probe), **self.excerpt(p)}, "method:read-only-getter")
        # Not accepted on Qt (review rows only): actions with one visible effect and pure getters.
        for n, p in sorted(pm.items()):
            cls = p.get("class")
            if cls == "action" and p.get("outcome") == "ok" and len(p.get("changed") or {}) == 1 and not self.rules.denied("methods", n):
                self.hold("methods", n, 0.8, "method:action", accepted=False)
            elif cls == "pure_getter" and p.get("outcome") == "ok" and p.get("stable") and not self.rules.denied("methods", n) \
                    and not any((x.get("accessors") or {}).get("get") == n for x in self.props.values()):
                self.hold("methods", n, 0.9, "method:pure-getter", accepted=False)

    # -- signals (R1/R2/R3) ------------------------------------------------------------------------
    def trigger_for(self, sig: str) -> tuple[str, dict, bool] | None:
        """(property, probe record, is NOTIFY) whose sample -> default -> sample sets emit `sig` once each."""
        found = []
        for n, p in sorted((self.k.get("props") or {}).items()):
            if p.get("outcome") != "exact" or (p.get("sequence") or {}).get(sig) != [1, 1, 1]:
                continue
            m = self.props.get(n)
            ref = self.ref_of(n, p)
            if m is None or m.get("deprecated") or ref is None or self.rules.denied("props", n):
                continue
            if not self.value_ok(ref, p.get("sample")) or not self.value_ok(ref, p.get("default")) or self.setup_of(p) is False:
                continue
            found.append((p.get("notify") == sig, n, p))
        if not found:
            return None
        found.sort(key=lambda t: (not t[0], t[1]))
        notify, n, p = found[0]
        return n, p, notify

    def infer_signals(self) -> None:
        for sig, s in sorted((self.k.get("signals") or {}).items()):
            name = s["name"]
            m = self.signals.get(name)
            if m is None or m.get("deprecated") or m.get("access", "public") != "public":
                continue
            why = self.rules.denied("signals", name)
            if why:
                self.skip(f"signal:denied ({why})")
                continue
            refs = [q["type"]["ref"] for q in m["params"]]
            if not all(known_ref(r) for r in refs) or any(not q.get("name") for q in m["params"]):
                self.skip("signal:argument-shape")
                continue
            if any(is_enum_ref(r) and r[5:-1] not in self.enums for r in refs):
                self.skip("signal:enum-not-in-table")
                continue
            arity = [x for x in m["metaobject_signatures"] if (x.count(",") + 1 if x.split("(", 1)[1] != ")" else 0) == len(refs)]
            if sig not in arity:
                continue                     # R3: an overload with fewer arguments collapses into the full one
            js = naming.kebab(name)
            trig = self.trigger_for(sig)
            if trig is None:
                self.delivery_only(sig, m, js, refs)
                continue
            prop, p, notify = trig
            first = ((p.get("sequence_args") or {}).get(sig) or [None])[0]
            if first is None or len(first) != len(refs):
                self.hold("signals", js, 0.5, "signal:arguments-unobserved")
                continue
            trigger: dict = {"property": prop, "values": [p["sample"], p["default"], p["sample"]]}
            setup = self.setup_of(p)
            if setup:
                trigger["setup"] = setup
            rec: dict = {"js": js, "native": name, "connect": sig}
            others = sorted(x for x in m["metaobject_signatures"] if x != sig)
            if others:
                rec["collapses"] = others
            rec["args"] = [{"name": q["name"], "type": q["type"]["ref"]} for q in m["params"]]
            rec["trigger"] = trigger
            rec["expect_args"] = ["" if is_enum_ref(r) else cvalstr(v) for r, v in zip(refs, first)]
            rec["provenance"] = [{"origin": "metaobject", "ref": f"signal {sig}"},
                                 {"origin": "probe", "ref": f"emitted once per set of {prop} (sample, default, sample)"
                                                            + ("; its NOTIFY signal" if notify else "")}]
            pattern = "signal:notify-trigger" if notify else "signal:property-trigger"
            if self.emit("signals", js, rec, 0.95 if notify else 0.9, {"trigger": prop, "first_args": first}, pattern):
                self.written["signals"].add(sig)

    def delivery_only(self, sig: str, m: dict, js: str, refs: list[str]) -> None:
        """R2 fallback: no property set emits the signal, so the generated test emits it (QMetaMethod::invoke)
        with sampled arguments. Delivery, listen-off and destroy are checked, not native causation."""
        if any(is_enum_ref(r) for r in refs):
            # The generated emission would name enum members in C++, and the ingested table does not say which
            # enums are scoped (`enum class`): left for review.
            self.hold("signals", js, 0.8, "signal:delivery-only-enum-argument")
            return
        args = [EMIT_SAMPLE[r] for r in refs]
        rec: dict = {"js": js, "native": m["name"], "connect": sig}
        others = sorted(x for x in m["metaobject_signatures"] if x != sig)
        if others:
            rec["collapses"] = others
        rec["args"] = [{"name": q["name"], "type": q["type"]["ref"]} for q in m["params"]]
        rec["trigger"] = {"emit": args}
        rec["delivery_only"] = True
        rec["expect_args"] = [cvalstr(v) for v in args]
        rec["provenance"] = [{"origin": "metaobject", "ref": f"signal {sig}"},
                             {"origin": "rule", "ref": "R2: no probed property set emits it; emitted with sampled arguments "
                                                       "(delivery only, no native trigger)"}]
        self.emit("signals", js, rec, 0.9, {"emit": args}, "signal:delivery-only")

    # -- generic bindings (B1) ---------------------------------------------------------------------
    def infer_bindings(self) -> None:
        """Every written writable property whose NOTIFY signal fired (and was written as a signal fact) is
        `bind:`-able: the renderer compares before set, the change signal carries or re-reads the value."""
        for n, p in sorted((self.k.get("props") or {}).items()):
            sig = p.get("notify")
            if n not in self.written["props"] or not sig or not p.get("notify_fired") or sig not in self.written["signals"]:
                continue
            ref = self.ref_of(n, p)
            m = self.signals[(self.k["signals"][sig])["name"]]
            first = (p.get("sequence_args") or {}).get(sig, [None])[0] or []
            arg0 = bool(m["params"]) and m["params"][0]["type"]["ref"] == ref and bool(first) and first[0] == p["sample"]
            js = naming.kebab(n)
            rec = {"name": js, "direction": "two_way", "property": js, "type": {"ref": ref, "nullable": False},
                   "change": {"signal": naming.kebab(m["name"]), "role": "any_change", "value": "arg0" if arg0 else "prop"},
                   "write": {"natively_normalizes": False},
                   "echo": {"strategy": "none", "compare_before_set": True, "read_back": False},
                   "lifetime": dict(LIFETIME),
                   "verify": {"values": [p["sample"], p["default"]]},
                   "provenance": [{"origin": "rule", "ref": f"B1: {self.cls}::{n} is writable, scalar and NOTIFYs {sig}; "
                                                            "the renderer compares before set"}]}
            self.emit("bindings", js, rec, 0.95, {"notify": sig, "arg0": arg0}, "binding:notify")


def load_docs() -> dict[str, dict]:
    return {p.stem: json.loads(p.read_text()) for p in sorted(META.glob("*.json"))}


def exposed() -> dict[str, list[str]]:
    """class -> tags of its plain elements (a composite element, a QWidget with a layout, is not the class)."""
    out: dict[str, list[str]] = {}
    for p in sorted(OVERRIDES.glob("*.yaml")):
        for el in (svmeta.load_yaml(p).get("elements") or []):
            c = el["construct"]
            if c.get("composite") or c.get("args"):
                continue
            out.setdefault(c["class"], []).append(el["tag"])
    return out


def find_probe(version: str) -> dict:
    p = PROBE_DIR / f"{version}.json"
    if not p.exists():
        raise SystemExit(f"infer: no probe for Qt {version} ({p.relative_to(ROOT)}); run tools/metadata/qt/probe.py")
    return json.loads(p.read_text())


def run() -> tuple[dict[str, str], dict[str, str], dict]:
    docs = load_docs()
    version = docs["QWidget"]["source"]["qdoc"]["version"]
    probe = find_probe(version)
    enums = {e["fullname"]: e for e in json.loads((META / "_enums" / "Qt.json").read_text())["enums"]}
    rules = Rules()
    files: dict[str, str] = {}
    evid: dict[str, str] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}
    counts: dict[str, int] = {}
    tags_of = exposed()
    for cls, k in sorted(probe["kinds"].items()):
        d = docs.get(cls)
        if d is None or k.get("crash") or k.get("error") or cls not in tags_of:
            continue
        kd = Kind(cls, tags_of[cls], k, d, enums, rules)
        kd.infer_props()
        kd.infer_methods()
        kd.infer_signals()
        kd.infer_bindings()
        review += kd.review
        for p, c in kd.skipped.items():
            skipped[p] = skipped.get(p, 0) + c
        if not kd.ov:
            continue
        els = []
        for tag in kd.tags:
            el = {"tag": tag}
            for key in ("props", "methods", "signals", "bindings"):
                if key in kd.ov:
                    el[key] = kd.ov[key]
            els.append(el)
        files[f"{cls}.yaml"] = yaml.safe_dump({"elements": els}, sort_keys=False, width=140, allow_unicode=True)
        evid[f"{cls}.evidence.json"] = json.dumps(kd.ev, indent=1, sort_keys=True) + "\n"
        for key in kd.ev:
            sec = key.split(":", 1)[0]
            counts[sec] = counts.get(sec, 0) + len(kd.tags)
    by_pattern: dict[str, int] = {}
    for r in review:
        key = f"{r['band']}:{r['pattern']}"
        by_pattern[key] = by_pattern.get(key, 0) + 1
    summary = {"schema": "svelters-inferred-review/v1", "toolkit": "qt", "version": version,
               "promote_at": rules.promote_at, "accept": sorted(rules.accept),
               "inferred": dict(sorted(counts.items())),
               "unreviewed": {"middle": sum(1 for r in review if r["band"] == "middle"),
                              "low": sum(1 for r in review if r["band"] == "low"),
                              "by_pattern": dict(sorted(by_pattern.items()))},
               "skipped": dict(sorted(skipped.items())),
               "rows": sorted(review, key=lambda r: (r["kind"], r["section"], r["name"]))}
    return files, evid, summary


SECTION = {"props": "js", "methods": "js", "signals": "js", "bindings": "name"}


def observe_layer() -> tuple[dict[str, str], dict[str, int]]:
    """The inferred observe layer (rules O2-O5, O8, O10: tools/metadata/qt/observe_rules.py): file name -> text for
    metadata/inferred/qt/observe/, and the counts per class."""
    import observe as obs
    import observe_rules
    rules = yaml.safe_load(RULES.read_text())
    recs, evidence, review, skipped = observe_rules.infer(rules)
    conf = {k: v["confidence"] for k, v in evidence.items()}
    files = obs.inferred_files("qt", recs, evidence, review, skipped, conf, {"probe": rules["observe"]["probe"]})
    return files, obs.counts({o: {r["prop"]: r for r in rs} for o, rs in recs.items()})


def record_fingerprints(files: dict[str, str], observe_files: dict[str, str] | None = None) -> dict[tuple[str, str, str], str]:
    """(class, section, name) -> the fingerprint qt_layers.py puts on the inferred record (and the ledger parks it by).
    Observe facts: (owner, "observe", prop) -> the fingerprint of the stripped record (observe.Layers)."""
    import observe as obs
    out = obs.layer_fingerprints(observe_files or {})
    for fname, text in files.items():
        cls = fname[:-len(".yaml")]
        for el in yaml.safe_load(text).get("elements", []):
            for sec, key in SECTION.items():
                for r in el.get(sec, []):
                    out[(cls, sec, r[key])] = svmeta.fingerprint(r)
    return out


def prune_parked(files: dict[str, str], check: bool, observe_files: dict[str, str] | None = None) -> bool:
    """Drop parked facts whose inferred record changed or went away: they are tried again. True if stale."""
    path = OUT / "_parked.json"
    if not path.exists():
        return False
    doc = json.loads(path.read_text())
    fps = record_fingerprints(files, observe_files)
    keep = [r for r in doc["parked"] if fps.get((r["kind"], r["section"], r["name"])) == r.get("record_fp")]
    if len(keep) == len(doc["parked"]):
        return False
    if not check:
        doc["parked"] = keep
        path.write_text(json.dumps(doc, indent=1, sort_keys=True, ensure_ascii=False) + "\n")
    return True


def validate(files: dict[str, str]) -> list[str]:
    """Each inferred element, completed with its construct, must be a valid override element."""
    import tempfile
    errs = []
    with tempfile.TemporaryDirectory() as t:
        for name, text in files.items():
            cls = name[:-len(".yaml")]
            doc = yaml.safe_load(text)
            full = {"elements": [{**el, "construct": {"class": cls}} for el in doc["elements"]]}
            p = Path(t) / name
            p.write_text(yaml.safe_dump(full, sort_keys=False))
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
    import observe as obs
    ofiles, ocounts = observe_layer()
    parked_stale = prune_parked(files, a.check, ofiles)
    have = {p.name for p in OUT.glob("*") if p.is_file() and p.name != "_parked.json"} if OUT.exists() else set()
    stale = sorted([n for n, t in want.items() if not (OUT / n).exists() or (OUT / n).read_text() != t] + sorted(have - set(want)))
    if a.check:
        for n in stale:
            print(f"infer: metadata/inferred/qt/{n} is stale (run tools/metadata/qt/infer.py)", file=sys.stderr)
        ostale = obs.sync_dir(OUT / "observe", ofiles, True, "infer (run tools/metadata/qt/infer.py)")
        if parked_stale:
            print("infer: metadata/inferred/qt/_parked.json lists facts whose inferred record changed (run tools/metadata/qt/infer.py)",
                  file=sys.stderr)
        return 1 if stale or parked_stale or ostale else 0
    obs.sync_dir(OUT / "observe", ofiles, False, "infer")
    print("infer qt: observe " + ", ".join(f"{k} {v}" for k, v in ocounts.items()), file=sys.stderr)
    OUT.mkdir(parents=True, exist_ok=True)
    for n in sorted(have - set(want)):
        (OUT / n).unlink()
    for n, t in want.items():
        if not (OUT / n).exists() or (OUT / n).read_text() != t:
            (OUT / n).write_text(t)
    inf = summary["inferred"]
    print(f"infer qt: {len(files)} classes; facts " + ", ".join(f"{k} {v}" for k, v in inf.items())
          + f"; unreviewed middle {summary['unreviewed']['middle']}, low {summary['unreviewed']['low']}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
