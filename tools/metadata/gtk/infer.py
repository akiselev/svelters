#!/usr/bin/env python3
"""Infer GTK annotations from the ingested metadata, the checked-in probe and the rules file.

    python3 tools/metadata/gtk/infer.py [--check]

Inputs (a pure function of these; no GTK, display, clock or network):
  metadata/gtk/*.json              the ingested facts (records with origin gir/runtime only; annotations
                                   are never read back, so rerunning is idempotent)
  metadata/probe/gtk/<version>.json  tools/metadata/gtk/probe.py output (a maintainer step)
  metadata/rules/gtk.yaml          denylist, reference names, recipes, thresholds
  metadata/overrides/gtk/*.yaml    only to avoid proposing slots for a kind that has hand-written ones, and to check
                                   that a review decision `override` names a manual record
  metadata/inferred/gtk/_reviewed.yaml  review decisions on facts below promote_at (see REVIEWED below)

Outputs (byte-identical on rerun):
  metadata/inferred/gtk/<Kind>.yaml           facts with confidence >= promote_at, in the override schema
                                              (tools/metadata/gtk/override.schema.json). ingest.py merges them
                                              under the manual overrides.
  metadata/inferred/gtk/<Kind>.evidence.json  confidence and the probe excerpt of every fact, for review
  metadata/inferred/gtk/_review.json          facts below promote_at (middle and low band) that no decision in
                                              _reviewed.yaml covers, the counts of decided ones, and the counts of
                                              what was denied or skipped, by pattern.

Rules (plans/research/ANNOTATION-AUTOMATION.md section 2): P1/P3 props with the probe's sample, M1 accessor,
getter and action methods, M2 deny-by-pattern, R1/R2 signal results and triggers (a property set, an action
emit, or a synthetic emission marked delivery-only), S1 one-slots, B1 generic `bind:<prop>` through the
notify signal, V1 version gate for typed facts.

Confidence: 0.95 probe and rule agree; 0.9 the generated test is the only check left (probe-only recipe, a
synthetic emission, a snapshot of a pure getter); 0.8 weaker evidence (identity round trip, ancestor-parented
slot, an action with a visible effect); 0.5 needs a precondition recipe without a Rust twin.
"""
from __future__ import annotations

import argparse
import json
import re
import struct
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import svmeta  # noqa: E402

META = ROOT / "metadata/gtk"
OVERRIDES = ROOT / "metadata/overrides/gtk"
PROBE_DIR = ROOT / "metadata/probe/gtk"
RULES = ROOT / "metadata/rules/gtk.yaml"
OUT = ROOT / "metadata/inferred/gtk"
# Review decisions (hand/agent written, never regenerated). One entry per fact below promote_at:
#   - fact: Kind.section.name          (section: properties, methods, signals, bindings, slots)
#     pattern: middle:prop:round-trip  (band:pattern of the review row; a re-probe that changes it voids the decision)
#     decision: accept | reject | override
#     reason: one line
#     reviewer: who decided
# accept: the candidate record is written to the inferred layer (tested and ledgered like any inferred fact); only
#         facts with a candidate record (not `hold`s) can be accepted. reject: the fact leaves the review queue.
#         override: a manual record in metadata/overrides/gtk/<Kind>.yaml covers the fact (checked).
# A decision that matches no review row (stale, or the fact was promoted by a rule) is an error, as is a malformed one.
REVIEWED = OUT / "_reviewed.yaml"
DECISIONS = ("accept", "reject", "override")
KEEP = ("_parked.json", "_reviewed.yaml")      # files in OUT that infer.py never writes or deletes
SCHEMA = Path(__file__).resolve().parent / "override.schema.json"

INGESTED = ("gir", "runtime")
SCALAR = {"bool", "i32", "u32", "i64", "u64", "f32", "f64", "string"}
PNG_TOKEN = "<sample-png>"
# signal signals that fire when a property is set: not a trigger worth a fact
NOISE = {"state-flags-changed", "direction-changed"}
RUST_ARG = {"gboolean": "&true", "gint": "&1i32", "guint": "&1u32", "glong": "&1i64", "gulong": "&1u64", "gint64": "&1i64",
            "guint64": "&1u64", "gdouble": "&1.5f64", "gfloat": "&1.5f32", "gchararray": '&"x"'}
RUST_RET = {"void": "()", "gboolean": "bool"}
MIN_GTK = (4, 14)


def head(ref: str) -> str:
    return ref.split("<")[0]


def scalar_ref(ref: str) -> bool:
    return head(ref) in SCALAR or head(ref) == "enum"


def ver(v: str | None) -> tuple[int, int] | None:
    if not v:
        return None
    a, b = (v.split(".") + ["0"])[:2]
    return int(a), int(b)


def rust_v(v) -> str | None:
    """The harness value of a probe sample, or None when it is not a plain scalar."""
    if isinstance(v, bool):
        return f"V::Bool({str(v).lower()})"
    if isinstance(v, int):
        return f"V::I64({v})"
    if isinstance(v, float):
        return f"V::F64({v!r})"
    if isinstance(v, str) and not v.startswith("<"):
        return f"V::Str({json.dumps(v, ensure_ascii=True)}.into())"
    return None


def f32_exact(x: float) -> bool:
    try:
        return struct.unpack("f", struct.pack("f", x))[0] == x
    except (OverflowError, struct.error):
        return False


def sample_ok(ref: str, v) -> bool:
    """A probe sample the generated test can write and compare."""
    h = head(ref)
    if v is None or (isinstance(v, str) and (v == PNG_TOKEN or v.startswith("<"))):
        return False
    if h == "bool":
        return isinstance(v, bool)
    if h in ("i32", "u32", "i64", "u64"):
        return isinstance(v, int) and not isinstance(v, bool)
    if h in ("f32", "f64"):
        return isinstance(v, (int, float)) and not isinstance(v, bool) and (h == "f64" or f32_exact(float(v)))
    return h in ("string", "enum") and isinstance(v, str)


class Rules:
    def __init__(self, path: Path = RULES):
        r = yaml.safe_load(path.read_text())
        self.promote_at = float(r["promote_at"])
        self.baseline = ver(r["typed_baseline"])
        self.deny_methods = [(re.compile(x["pattern"]), x["reason"]) for x in r["deny_methods"]]
        self.deny_signals = [(re.compile(x["pattern"]), x["reason"]) for x in r["deny_signals"]]
        self.reference_names = set(r["reference_names"])
        self.bind_exclude = set(r["bind_exclude_types"])
        self.recipes = r["recipes"]
        self.float_samples = [float(x) for x in r.get("float_samples", [])]

    def denied(self, pats, name: str) -> str | None:
        return next((why for rx, why in pats if rx.match(name)), None)


class Kind:
    """One probed kind with its ingested record."""

    def __init__(self, name: str, probe: dict, doc: dict, docs: dict[str, dict], rules: Rules, decisions: dict | None = None):
        self.name, self.k, self.d, self.docs, self.rules = name, probe, doc, docs, rules
        self.decisions = decisions or {}
        self.decided: list[dict] = []
        self.errors: list[str] = []
        self.props = {p["name"]: p for p in doc["properties"] if p.get("origin") in INGESTED}
        self.methods = {m["name"]: m for m in doc["methods"] if m.get("origin") in INGESTED}
        self.signals = {s["name"]: s for s in doc["signals"] if s.get("origin") in INGESTED}
        self.method_by_native = {m["native"]: m for m in self.methods.values()}
        self.ov: dict = {}
        self.ev: dict = {}
        self.review: list[dict] = []
        self.skipped: dict[str, int] = {}

    # -- bookkeeping ---------------------------------------------------------------------------
    def skip(self, pattern: str) -> None:
        self.skipped[pattern] = self.skipped.get(pattern, 0) + 1

    def emit(self, section: str, name: str, rec: dict, conf: float, evidence, pattern: str) -> None:
        if conf >= self.rules.promote_at:
            self.ov.setdefault(section, []).append(rec)
            self.ev[f"{section}:{name}"] = {"confidence": conf, "evidence": evidence}
        elif self.queue(section, name, conf, pattern, rec, evidence) == "accept":
            self.ov.setdefault(section, []).append(rec)

    def hold(self, section: str, name: str, conf: float, pattern: str) -> None:
        """A fact the evidence does not support yet: listed for review, never written (a decision may settle it)."""
        self.queue(section, name, conf, pattern, None)

    def queue(self, section: str, name: str, conf: float, pattern: str, rec: dict | None, evidence=None) -> str | None:
        """A fact below promote_at: a review row, or settled by its decision in _reviewed.yaml. Returns the decision.
        An accepted record gets the review as provenance and an evidence entry; the caller writes it."""
        row = {"kind": self.name, "section": section, "name": name, "confidence": conf,
               "band": "middle" if conf >= 0.6 else "low", "pattern": pattern}
        fact = f"{self.name}.{section}.{name}"
        d = self.decisions.get(fact)
        if d is None:
            self.review.append(row)
            return None
        d["_used"] = True
        if d["pattern"] != f"{row['band']}:{pattern}":
            self.errors.append(f"{fact}: decided as {d['pattern']} but the review row is now {row['band']}:{pattern}; review it again")
            self.review.append(row)
            return None
        if d["decision"] == "accept":
            if rec is None:
                self.errors.append(f"{fact}: `accept` needs a candidate record, and {pattern} has none; write a manual record "
                                   "in metadata/overrides/gtk and decide `override`")
                return None
            rec["provenance"] = list(rec["provenance"]) + [{"origin": "manual", "ref": f"review accept ({d['reviewer']}): {d['reason']}"}]
            self.ev[f"{section}:{name}"] = {"confidence": conf, "evidence": evidence,
                                            "review": {"decision": "accept", "reason": d["reason"], "reviewer": d["reviewer"]}}
        elif d["decision"] == "override" and not manual_record(self.name, section, name):
            self.errors.append(f"{fact}: decided `override`, but metadata/overrides/gtk/{self.name}.yaml has no {section} record {name!r}")
        self.decided.append({**row, "decision": d["decision"]})
        return d["decision"]

    # -- lookups --------------------------------------------------------------------------------
    def chain(self) -> list[dict]:
        out = [self.d]
        for x in self.d.get("bases", []) + self.d.get("interfaces", []):
            doc = self.docs.get(x.split(".", 1)[1]) if x.startswith("Gtk.") else None
            if doc:
                out.append(doc)
        return out

    def prop_meta(self, name: str) -> dict | None:
        for doc in self.chain():
            for p in doc["properties"]:
                if p["name"] == name and p.get("origin") in INGESTED:
                    return p
        return None

    def newer_than(self, limit: tuple[int, int], *recs) -> bool:
        vs = [ver(r.get("since") or (r.get("availability") or {}).get("since")) for r in recs if r]
        return any(v and v > limit for v in vs)

    def recipe_setup(self, recipe: str | None) -> str | None | bool:
        """Rust `setup` for a probe recipe: '' for none, a statement, or False when there is no Rust twin."""
        if not recipe:
            return ""
        if recipe == "range" and "set_range" in self.methods:
            return self.rules.recipes["range"]
        if recipe == "text" and ("text" in self.props or "Gtk.Editable" in self.d.get("interfaces", [])):
            return self.rules.recipes["text"]
        return False

    # -- properties (P1/P3) ---------------------------------------------------------------------
    def infer_props(self) -> None:
        self.fired_by: dict[str, tuple[str, dict]] = {}
        for n, p in sorted(self.k.get("props", {}).items()):
            m = self.props.get(n)
            if m is None or m.get("deprecated"):
                continue
            outcome = p.get("outcome")
            if outcome in ("ignored", "error", "crash", "normalized"):
                self.hold("properties", n, 0.5, f"prop:{outcome}")
                continue
            if outcome not in ("exact", "identity"):
                continue
            acc = m["access"]
            if not (acc["read"] and acc["write"]) or acc.get("construct_only"):
                continue
            ref = m["type"]["ref"]
            rec: dict = {"name": n}
            if scalar_ref(ref):
                if outcome != "exact":
                    smp = self.float_sample(m)
                    if smp is None:
                        self.hold("properties", n, 0.8, "prop:scalar-identity")
                        continue
                    # The probe does not sample floats; the generated test sets one taken from the GIR range.
                    rec["sample"] = smp
                    rec["provenance"] = [{"origin": "rule", "ref": f"float sample {smp} from the GIR range "
                                          f"[{m['range']['min']}, {m['range']['max']}]; the probe left it unsampled"}]
                    self.emit("properties", n, rec, 0.9, p, "prop:float-range-sample")
                    continue
                if not sample_ok(ref, p.get("sample")):
                    self.hold("properties", n, 0.5, "prop:needs-fixture")
                    continue
                rec["sample"] = p["sample"]
                conf = 0.95 if p.get("notify") else 0.9
            else:
                conf = 0.9 if outcome == "exact" else 0.8
            setup = self.recipe_setup(p.get("recipe"))
            if setup is False:
                self.hold("properties", n, 0.5, f"prop:recipe-{p['recipe']}")
                continue
            if setup:
                rec["setup"] = setup
            how = f"round trip {outcome}" + (f" after recipe {p['recipe']}" if p.get("recipe") else "") + \
                  ("; notify fired" if p.get("notify") else "")
            rec["provenance"] = [{"origin": "probe", "ref": how}]
            self.emit("properties", n, rec, conf, p, "prop:round-trip")
            if outcome == "exact" and scalar_ref(ref):
                self.fired_by_add(n, p, ref, setup)

    def float_sample(self, pm: dict | None) -> float | None:
        """A float sample from the property's GIR range: the first of `float_samples` inside it, not the default,
        exact as an f32. None when the property is not a ranged float."""
        if pm is None or head(pm["type"]["ref"]) not in ("f32", "f64"):
            return None
        rng = pm.get("range") or {}
        lo, hi = rng.get("min"), rng.get("max")
        if lo is None or hi is None:
            return None
        return next((x for x in self.rules.float_samples if lo <= x <= hi and x != pm.get("default") and f32_exact(x)), None)

    def fired_by_add(self, n: str, p: dict, ref: str, setup: str) -> None:
        for s in p.get("signals") or []:
            if s not in NOISE and not s.startswith("notify::") and s not in self.fired_by:
                self.fired_by[s] = (n, {"sample": p["sample"], "default": p.get("default"), "ref": ref, "setup": setup})

    # -- methods (M1/M2) ------------------------------------------------------------------------
    def method_ok(self, name: str, m: dict) -> bool:
        if m.get("deprecated") or m.get("unsupported_reason") or m.get("throws"):
            return False
        if any(x.get("native") == "gunichar" for x in m["params"]) or (m["returns"].get("native") == "gunichar"):
            return False                     # the harness has no gunichar value
        return True

    def infer_methods(self) -> None:
        for n, mp in sorted(self.k.get("methods", {}).items()):
            m = self.methods.get(n)
            cls = mp.get("class")
            if m is None or not self.method_ok(n, m):
                continue
            why = self.rules.denied(self.rules.deny_methods, n)
            if why:
                self.skip(f"method:denied ({why})")
                continue
            if mp.get("log") and cls != "accessor_set":
                self.hold("methods", n, 0.5, "method:glib-warning")
                continue
            prov = None
            rec: dict = {"name": n, "dispatch": "dynamic"}
            if cls == "accessor_get" and mp.get("outcome") == "exact":
                pm = self.prop_meta(mp["prop"])
                ret = m["returns"]["type"]
                if pm is None or not scalar_ref(ret) or head(pm["type"]["ref"]) != head(ret) or ret.startswith("void"):
                    self.skip("method:get-non-scalar")
                    continue
                rec.update({"args": [], "expect": f'r == h.get(o, "{mp["prop"]}")'})
                prov, conf = f"equals property {mp['prop']}", 0.95
            elif cls == "accessor_set" and mp.get("outcome") == "exact":
                pm = self.prop_meta(mp["prop"])
                ptype = m["params"][0]["type"] if len(m["params"]) == 1 else ""
                if pm is None or not scalar_ref(ptype) or head(pm["type"]["ref"]) != head(ptype):
                    self.skip("method:set-non-scalar")
                    continue
                sample = mp.get("sample")
                if not sample_ok(ptype, sample):
                    self.hold("methods", n, 0.5, "method:needs-fixture")
                    continue
                setup = self.recipe_setup(mp.get("recipe"))
                if setup is False:
                    self.hold("methods", n, 0.5, f"method:recipe-{mp['recipe']}")
                    continue
                if setup:
                    rec["setup"] = setup
                rec.update({"args": [sample], "expect": f'h.get(o, "{mp["prop"]}") == {rust_v(sample)}'})
                prov, conf = f"set then read property {mp['prop']}: exact", 0.95
            elif cls in ("accessor_get", "accessor_set"):
                # A non-scalar accessor is skipped whatever the probe outcome (as an exact one is): the generated
                # test has no literal for its value, so a review row could never become a fact.
                pm = self.prop_meta(mp.get("prop") or "")
                vt = m["returns"]["type"] if cls == "accessor_get" else (m["params"][0]["type"] if len(m["params"]) == 1 else "")
                if not scalar_ref(vt):
                    self.skip("method:get-non-scalar" if cls == "accessor_get" else "method:set-non-scalar")
                    continue
                smp = self.float_sample(pm) if cls == "accessor_set" and mp.get("outcome") == "no_sample" \
                    and pm is not None and head(pm["type"]["ref"]) == head(vt) and not mp.get("log") else None
                if smp is None:
                    self.hold("methods", n, 0.5, f"method:{cls}-{mp.get('outcome')}")
                    continue
                rec.update({"args": [smp], "expect": f'h.get(o, "{mp["prop"]}") == {rust_v(smp)}'})
                prov, conf = f"set then read property {mp['prop']}; float sample {smp} from the GIR range, the probe left it unsampled", 0.9
            elif cls == "pure_getter" and mp.get("outcome") == "ok":
                ret, val = m["returns"]["type"], mp.get("ret")
                if not scalar_ref(ret) or head(ret) in ("f32", "f64") or rust_v(val) is None or not sample_ok(ret, val) \
                        or mp.get("stable") is not True:
                    self.skip("method:getter-unsnapshotable")
                    continue
                rec.update({"args": [], "expect": f"r == {rust_v(val)}"})
                prov, conf = f"called on a new instance, returned {val!r}", 0.9
            elif cls == "action" and mp.get("outcome") == "ok":
                ch = mp.get("changed") or {}
                if len(ch) != 1 or m["params"]:
                    self.skip("method:action-no-visible-effect")
                    continue
                (prop, (_, after)), = ch.items()
                pm = self.prop_meta(prop)
                if pm is None or not scalar_ref(pm["type"]["ref"]) or not sample_ok(pm["type"]["ref"], after):
                    self.skip("method:action-no-visible-effect")
                    continue
                rec.update({"args": [], "expect": f'h.get(o, "{prop}") == {rust_v(after)}'})
                prov, conf = f"changes {prop} to {after!r}", 0.8
            else:
                continue
            rec["provenance"] = [{"origin": "rule" if "GIR range" in prov else "probe", "ref": prov}]
            self.emit("methods", n, rec, conf, mp, f"method:{cls}")

    # -- signals (R1/R2) ------------------------------------------------------------------------
    def infer_signals(self) -> None:
        for n, s in sorted(self.k.get("signals", {}).items()):
            m = self.signals.get(n)
            if m is None or m.get("deprecated") or s.get("emit") == "crash":
                continue
            rec: dict = {"name": n, "handler_result": s["handler_result"], "sync_required": s["sync_required"]}
            prov = [{"origin": "runtime", "ref": f"returns {s['returns']}, run-{s['run']}"}]
            conf = 0.95
            fired = self.fired_by.get(n)
            why = self.rules.denied(self.rules.deny_signals, n)
            unsampled = bool(fired and not why and not sample_ok(fired[1]["ref"], fired[1]["default"]))
            if unsampled:
                # A property whose default the test cannot write back (im-module: NULL) fires the signal only as a
                # side effect; a synthetic emission below still gives a delivery-only fact.
                fired = None
            if fired and not why:
                prop, f = fired
                trig = {"property": prop, "values": [f["sample"], f["default"], f["sample"]]}
                if f["setup"]:
                    trig["setup"] = f["setup"]
                rec["trigger"] = trig
                prov.append({"origin": "probe", "ref": f"emitted when {prop} is set"})
                pattern = "signal:property-trigger"
            elif s.get("emit") == "delivered" and not why:
                prov.append({"origin": "probe", "ref": "action signal, emitted and delivered"})
                pattern = "signal:action"
            elif (syn := s.get("synthetic")) and syn.get("emit") == "delivered" and not why and \
                    all(t in RUST_ARG for t in s["params"]) and s["returns"] in RUST_RET:
                args = ", ".join(RUST_ARG[t] for t in s["params"])
                call = f'o.emit_by_name::<{RUST_RET[s["returns"]]}>({json.dumps(n)}, &[{args}]);'
                rec["trigger"] = {"rust": ("let _ = " + call) if s["returns"] == "gboolean" else call, "values": ["a", "b", "c"]}
                rec["delivery_only"] = True
                prov.append({"origin": "probe", "ref": "emitted synthetically with sampled arguments; delivery only, no native trigger"})
                conf, pattern = 0.9, "signal:delivery-only"
            elif unsampled:
                self.hold("signals", n, 0.5, "signal:trigger-default-unsampled")
                continue
            else:
                self.skip("signal:no-trigger" if not why else f"signal:denied ({why})")
                continue
            rec["provenance"] = prov
            self.emit("signals", n, rec, conf, s, pattern)

    # -- slots (S1) -----------------------------------------------------------------------------
    def infer_slots(self, manual_children: bool) -> None:
        slots = []
        for n, s in sorted(self.k.get("slots", {}).items()):
            if s.get("role") == "reference" or n.startswith("<"):
                continue
            if s.get("role") != "slot":
                continue
            pm = self.props.get(n)
            if pm is None:
                continue
            if n in self.rules.reference_names:
                self.skip("slot:reference-name")
                continue
            get_m = self.method_by_native.get((pm["accessors"] or {}).get("get"))
            set_m = self.method_by_native.get((pm["accessors"] or {}).get("set"))
            if get_m is None or set_m is None or self.d.get("abstract"):
                self.skip("slot:no-accessors")
                continue
            if manual_children:
                self.skip("slot:kind-has-manual-children")
                continue
            if self.newer_than(self.rules.baseline, self.d, pm, get_m, set_m):
                self.skip("slot:newer-than-typed-baseline")       # V1: the typed gtk4-rs call would not compile
                continue
            if not (s.get("readback") and s.get("clear_unparents")):
                self.hold("slots", n, 0.5, "slot:no-readback-or-clear")
                continue
            gtype = self.d["native"]["gtype"]
            name = re.sub(r"-(child|widget)$", "", n)
            by = "set parents the child (%s%s); old child %s on replace; set(None) unparents" % (
                s["parent"], f", revealed by {s['revealed_by']}" if s.get("revealed_by") else "", s["old_on_replace"])
            rec = {"name": name, "content": "widget", "accepts": ["Gtk.Widget"], "route_by_kind": False,
                   "cardinality": "one", "order": "none", "default": {"kind": "none"}, "on_remove": "empty",
                   "attrs": [], "requires": [],
                   "ops": {"get": {"native": f"{gtype}::{get_m['name']}() const"},
                           "set": {"native": f"{gtype}::{set_m['name']}(GtkWidget*)", "args": ["child"],
                                   "effects": {"child": "parented", "old": "unparented"}},
                           "remove": {"native": f"{gtype}::{set_m['name']}(GtkWidget*)", "args": [None],
                                      "effects": {"child": "unparented"}}},
                   "verify": {}, "provenance": [{"origin": "probe", "ref": by}]}
            conf = 0.95 if s["parent"] == "direct" else 0.8
            slots.append((name, rec, conf, s))
        keep = []
        for name, rec, conf, s in slots:
            if conf >= self.rules.promote_at:
                self.ev[f"slots:{name}"] = {"confidence": conf, "evidence": s}
                keep.append(rec)
            elif self.queue("slots", name, conf, "slot:ancestor-parented", rec, s) == "accept":
                keep.append(rec)
        if keep:
            self.ov["children"] = {
                "policy": "single_child" if len(keep) == 1 and keep[0]["name"] == "child" else "named_slots",
                "default_slot": next((r["name"] for r in keep if r["name"] in ("child", "content")), keep[0]["name"]),
                "slots": keep}

    # -- generic bindings (B1) ------------------------------------------------------------------
    def infer_bindings(self) -> None:
        """Every writable scalar property with a notify is `bind:`-able through its notify signal; one generated
        echo test per property (gen_backend.binding_test). Curation only picks the primary name."""
        for n, p in sorted(self.k.get("props", {}).items()):
            m = self.props.get(n)
            if m is None or m.get("deprecated") or p.get("outcome") != "exact" or not p.get("notify"):
                continue
            ref = m["type"]["ref"]
            if not scalar_ref(ref) or head(ref) in self.rules.bind_exclude:
                continue
            if not sample_ok(ref, p.get("sample")) or not sample_ok(ref, p.get("default")):
                continue
            setup = self.recipe_setup(p.get("recipe"))
            if setup is False:
                continue
            sig = f"notify::{n}"
            srec = {"name": sig, "derived_from": {"property": n}, "js_event": sig,
                    "trigger": {"property": n, "values": [p["sample"], p["default"], p["sample"]]},
                    "params": [], "returns": {"type": "void"}, "handler_result": "none", "sync_required": False,
                    "phase": "first", "flags": ["detailed"], "connection": {"disconnect_on": ["handle_drop", "native_destroy"]},
                    "provenance": [{"origin": "rule", "ref": f"GObject::notify for {self.d['native']['gtype']}:{n}"}]}
            if setup:
                srec["trigger"]["setup"] = setup
            brec = {"name": n, "direction": "two_way", "property": n, "type": {"ref": ref, "nullable": False},
                    "change": {"signal": sig, "role": "any_change", "value": "prop"},
                    "write": {"natively_normalizes": False},
                    "echo": {"strategy": "none", "compare_before_set": True, "read_back": bool(m.get("range"))},
                    "lifetime": {"on_native_death": "end_binding", "on_stale_update": "warn_noop", "on_destroy": "idempotent_disconnect"},
                    "provenance": [{"origin": "rule", "ref": f"B1: {self.d['native']['gtype']}:{n} is writable, scalar and notifies; the renderer compares before set"}]}
            self.ov.setdefault("signals", []).append(srec)
            self.ov.setdefault("bindings", []).append(brec)
            self.ev[f"bindings:{n}"] = {"confidence": 0.95, "evidence": {"notify": True, "outcome": p["outcome"], "sample": p["sample"]}}


def load_docs() -> dict[str, dict]:
    return {p.stem: json.loads(p.read_text()) for p in sorted(META.glob("*.json"))}


def manual_children(kind: str) -> bool:
    p = OVERRIDES / f"{kind}.yaml"
    return p.exists() and bool((yaml.safe_load(p.read_text()) or {}).get("children"))


def manual_record(kind: str, section: str, name: str) -> bool:
    """metadata/overrides/gtk/<kind>.yaml has a record `name` in `section` (slots: children.slots)."""
    p = OVERRIDES / f"{kind}.yaml"
    if not p.exists():
        return False
    doc = yaml.safe_load(p.read_text()) or {}
    recs = (doc.get("children") or {}).get("slots", []) if section == "slots" else doc.get(section, [])
    return any(r.get("name") == name for r in recs or [])


FIELDS = ("fact", "pattern", "decision", "reason", "reviewer")


def load_decisions(path: Path | None = None) -> tuple[dict[str, dict], list[str]]:
    """fact -> decision from _reviewed.yaml, and the format errors."""
    path = path or REVIEWED
    if not path.exists():
        return {}, []
    doc = yaml.safe_load(path.read_text()) or {}
    errs = []
    if doc.get("schema") != "svelters-inferred-reviewed/v1" or doc.get("toolkit") != "gtk":
        errs.append(f"{path.name}: expected schema svelters-inferred-reviewed/v1, toolkit gtk")
    out: dict[str, dict] = {}
    for i, d in enumerate(doc.get("decisions") or []):
        fact = str(d.get("fact", ""))
        bad = [k for k in FIELDS if not isinstance(d.get(k), str) or len(d[k].strip()) < 2] + [k for k in d if k not in FIELDS]
        if bad or d["decision"] not in DECISIONS or len(fact.split(".")) != 3:
            errs.append(f"{path.name}: decision #{i + 1} ({fact or '?'}) is malformed ({', '.join(bad) or 'decision or fact'})")
            continue
        if fact in out:
            errs.append(f"{path.name}: {fact} is decided twice")
            continue
        out[fact] = dict(d)
    return out, errs


def find_probe(version: str) -> dict:
    p = PROBE_DIR / f"{version}.json"
    if not p.exists():
        raise SystemExit(f"infer: no probe for GTK {version} ({p.relative_to(ROOT)}); run tools/metadata/gtk/probe.py")
    return json.loads(p.read_text())


def run() -> tuple[dict[str, str], dict[str, str], dict, list[str]]:
    docs = load_docs()
    version = next(iter(docs.values()))["source"]["gir"]["version"]
    probe = find_probe(version)
    rules = Rules()
    decisions, errors = load_decisions()
    decided: list[dict] = []
    files: dict[str, str] = {}
    evid: dict[str, str] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}
    counts: dict[str, int] = {}
    for name, k in sorted(probe["kinds"].items()):
        d = docs.get(name)
        subject = k.get("subject")
        if d is None or k.get("crash") or k.get("error"):
            continue
        if subject:
            sd = docs.get(subject)
            if sd is None or "element" not in sd:
                continue
        elif "element" not in d:
            continue                         # a kind without an element is not exposed (deny-by-pattern applies to methods)
        kd = Kind(name, k, d, docs, rules, decisions)
        kd.infer_props()
        kd.infer_methods()
        if subject:
            # Abstract kinds and interfaces are tested on a concrete subject; their signals, slots and the
            # generic bindings (which every subclass would inherit) are left to the concrete kinds.
            kd.ov["verify_subject"] = {"tag": sd["element"]["tag"]}
        else:
            kd.infer_signals()
            kd.infer_slots(manual_children(name))
            kd.infer_bindings()
        review += kd.review
        decided += kd.decided
        errors += kd.errors
        for p, c in kd.skipped.items():
            skipped[p] = skipped.get(p, 0) + c
        if not kd.ov:
            continue
        ov = {key: kd.ov[key] for key in ("verify_subject", "properties", "signals", "methods", "children", "bindings") if key in kd.ov}
        if list(ov) == ["verify_subject"]:
            continue
        files[f"{name}.yaml"] = yaml.safe_dump(ov, sort_keys=False, width=140, allow_unicode=True)
        evid[f"{name}.evidence.json"] = json.dumps(kd.ev, indent=1, sort_keys=True) + "\n"
        for key in kd.ev:
            sec = key.split(":", 1)[0]
            counts[sec] = counts.get(sec, 0) + 1
    errors += [f"{f} matches no review row (promoted by a rule, or gone); remove the decision"
               for f, d in sorted(decisions.items()) if not d.get("_used")]
    by_pattern: dict[str, int] = {}
    for r in review:
        key = f"{r['band']}:{r['pattern']}"
        by_pattern[key] = by_pattern.get(key, 0) + 1
    rev_by: dict[str, dict[str, int]] = {}
    for r in decided:
        c = rev_by.setdefault(f"{r['band']}:{r['pattern']}", {})
        c[r["decision"]] = c.get(r["decision"], 0) + 1
    summary = {"schema": "svelters-inferred-review/v1", "toolkit": "gtk", "version": version,
               "promote_at": rules.promote_at,
               "inferred": dict(sorted(counts.items())),
               "unreviewed": {"middle": sum(1 for r in review if r["band"] == "middle"),
                              "low": sum(1 for r in review if r["band"] == "low"),
                              "by_pattern": dict(sorted(by_pattern.items()))},
               "reviewed": {**{dec: sum(1 for r in decided if r["decision"] == dec) for dec in DECISIONS},
                            "by_pattern": {k: dict(sorted(v.items())) for k, v in sorted(rev_by.items())}},
               "skipped": dict(sorted(skipped.items())),
               "rows": sorted(review, key=lambda r: (r["kind"], r["section"], r["name"]))}
    return files, evid, summary, errors


def observe_layer() -> tuple[dict[str, str], dict[str, int]]:
    """The inferred observe layer (rules O1-O10, tools/metadata/gtk/observe_rules.py): file name -> text for
    metadata/inferred/gtk/observe/, and the counts per class."""
    import observe as obs
    import observe_rules
    version = json.loads(next(iter(sorted(META.glob("*.json")))).read_text())["source"]["gir"]["version"]
    recs, evidence, review, skipped = observe_rules.infer(None, yaml.safe_load(RULES.read_text()), version)
    conf = {k: v["confidence"] for k, v in evidence.items()}
    files = obs.inferred_files("gtk", recs, evidence, review, skipped, conf, {"version": version})
    return files, obs.counts({o: {r["prop"]: r for r in rs} for o, rs in recs.items()})


def record_fingerprints(files: dict[str, str], observe_files: dict[str, str] | None = None) -> dict[tuple[str, str, str], str]:
    """(kind, section, name) -> the fingerprint ingest.py puts on the inferred record (and the ledger parks it by).
    Observe facts: (owner, "observe", prop) -> the fingerprint of the stripped record (observe.Layers)."""
    import observe as obs
    out = {}
    for fname, text in (observe_files or {}).items():
        if fname.endswith(".yaml"):
            doc = yaml.safe_load(text)
            for r in doc["observe"]:
                out[(doc["owner"], "observe", r["prop"])] = svmeta.fingerprint(obs.strip(r))
    for fname, text in files.items():
        doc = yaml.safe_load(text)
        kind = fname[:-len(".yaml")]
        for sec in ("properties", "signals", "methods", "bindings"):
            for r in doc.get(sec, []):
                out[(kind, sec, r["name"])] = svmeta.fingerprint(r)
        for r in (doc.get("children") or {}).get("slots", []):
            out[(kind, "slots", r["name"])] = svmeta.fingerprint(r)
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
    files, evid, summary, rev_errs = run()
    if rev_errs:
        print("infer: review decisions (metadata/inferred/gtk/_reviewed.yaml):", *rev_errs, sep="\n  ", file=sys.stderr)
        return 1
    errs = validate(files)
    if errs:
        print("infer: the inferred layer breaks the override schema:", *errs[:20], sep="\n  ", file=sys.stderr)
        return 1
    want = {**files, **evid, "_review.json": json.dumps(summary, indent=1, sort_keys=True) + "\n"}
    import observe as obs
    ofiles, ocounts = observe_layer()
    parked_stale = prune_parked(files, a.check, ofiles)
    have = {p.name for p in OUT.glob("*") if p.is_file() and p.name not in KEEP} if OUT.exists() else set()
    stale = sorted([n for n, t in want.items() if not (OUT / n).exists() or (OUT / n).read_text() != t] + sorted(have - set(want)))
    if a.check:
        for n in stale:
            print(f"infer: metadata/inferred/gtk/{n} is stale (run tools/metadata/gtk/infer.py)", file=sys.stderr)
        ostale = obs.sync_dir(OUT / "observe", ofiles, True, "infer (run tools/metadata/gtk/infer.py)")
        if parked_stale:
            print("infer: _parked.json lists facts whose inferred record changed (run tools/metadata/gtk/infer.py)", file=sys.stderr)
        return 1 if stale or parked_stale or ostale else 0
    obs.sync_dir(OUT / "observe", ofiles, False, "infer")
    print("infer: observe " + ", ".join(f"{k} {v}" for k, v in ocounts.items()), file=sys.stderr)
    OUT.mkdir(parents=True, exist_ok=True)
    for n in sorted(have - set(want)):
        (OUT / n).unlink()
    for n, t in want.items():
        if not (OUT / n).exists() or (OUT / n).read_text() != t:
            (OUT / n).write_text(t)
    inf = summary["inferred"]
    print(f"infer: {len(files)} kinds; facts " + ", ".join(f"{k} {v}" for k, v in inf.items())
          + f"; unreviewed middle {summary['unreviewed']['middle']}, low {summary['unreviewed']['low']}"
          + "; reviewed " + ", ".join(f"{d} {summary['reviewed'][d]}" for d in DECISIONS), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
