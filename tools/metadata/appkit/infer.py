#!/usr/bin/env python3
"""Infer AppKit annotations from the ingested headers' facts, the checked-in probe and the rules file.

    python3 tools/metadata/appkit/infer.py [--check]

Inputs (a pure function of these; no SDK, AppKit, clock or network, so it runs on Linux):
  metadata/appkit/*.json                 ingested header facts (type, readonly/copy, availability)
  metadata/probe/appkit/<sdk>.json       tools/metadata/appkit/probe.py output (macOS CI or a Mac); optional:
                                         without it every probe-backed fact is held for review
  metadata/rules/appkit.yaml             samples, deny patterns, binding sources, notification names
  metadata/overrides/appkit/*.yaml       read only for the element tags and the manual signals (never the
                                         inferred layer, so rerunning is idempotent)

Outputs (byte-identical on rerun), in the override schema (tools/metadata/appkit/override.schema.json):
  metadata/inferred/appkit/<Class>.yaml           facts with confidence >= promote_at; appkit_layers.py merges them under
                                                  the manual overrides (ingested < inferred < manual)
  metadata/inferred/appkit/<Class>.evidence.json  confidence and the probe excerpt of every fact
  metadata/inferred/appkit/_review.json           facts below promote_at by pattern, and skip counts

Rules:
  P1/P3  a writable prop whose probe round trip was exact gets the probe's sample (0.95 when KVO also fired
         for the programmatic set, else 0.9); normalized / ignored / error / crash are held (0.5).
  B      `bind:<prop>` for the NSControl value props (rules `binding_props`) of an element class, through the
         nearest rules `binding_sources` entry: NSTextField's text-did-change notification, else NSControl's
         target-action. Never bare KVO (plans/research/REACTIVITY.md 3.3), marked `partial` (programmatic
         changes are not announced). Needs an exact round trip on that element, no echo of the change signal
         on a programmatic set, and (0.95) the probe seeing the signal delivered.
  R      notification signals named by the rules for a class, when the probe found the AppKit constant with
         that value; verified by posting (delivery only, 0.9).
Methods and slots are not inferred: AppKit ingest only reads the methods the overrides name.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "common"))
import names  # noqa: E402
import svmeta  # noqa: E402

ROOT = HERE.parents[2]
META = ROOT / "metadata/appkit"
OVERRIDES = ROOT / "metadata/overrides/appkit"
PROBE_DIR = ROOT / "metadata/probe/appkit"
RULES = ROOT / "metadata/rules/appkit.yaml"
OUT = ROOT / "metadata/inferred/appkit"
SCHEMA = HERE / "override.schema.json"
STRUCT_FIELDS = {"rect": ["x", "y", "width", "height"], "size": ["width", "height"], "point": ["x", "y"],
                 "insets": ["top", "right", "bottom", "left"]}
BIND_TYPES = {"bool", "string", "i32", "i64", "u32", "f32", "f64"}


class Inputs:
    def __init__(self, probe: dict | None = None, rules: dict | None = None, meta: dict | None = None, manual: dict | None = None):
        self.meta = meta if meta is not None else {p.stem: json.loads(p.read_text()) for p in sorted(META.glob("NS*.json"))}
        enums = json.loads((META / "_enums.json").read_text()) if meta is None else {"sdk": "MacOSXtest", "enums": []}
        self.version = enums["sdk"].removeprefix("MacOSX")
        self.manual = manual if manual is not None else {p.stem: yaml.safe_load(p.read_text()) or {} for p in sorted(OVERRIDES.glob("*.yaml"))}
        self.rules = rules if rules is not None else yaml.safe_load(RULES.read_text())
        if probe is None and meta is None:
            p = PROBE_DIR / f"{self.version}.json"
            probe = json.loads(p.read_text()) if p.exists() else None
        self.probe = probe

    def chain(self, cls: str) -> list[str]:
        return [cls] + [b for b in self.meta[cls]["bases"] if b in self.meta]

    def tag(self, cls: str) -> str | None:
        return ((self.manual.get(cls) or {}).get("element") or {}).get("tag")

    def find_prop(self, cls: str, js: str) -> tuple[str | None, dict | None]:
        for c in self.chain(cls):
            for p in self.meta[c]["properties"]:
                if p["js_name"] == js:
                    return c, p
        return None, None

    def manual_signal(self, cls: str, name: str) -> dict | None:
        for c in self.chain(cls):
            for s in (self.manual.get(c) or {}).get("signals", []):
                if s["name"] == name:
                    return s
        return None

    def prop_enum(self, cls: str, js: str) -> bool:
        return any(e["prop"] == js for c in self.chain(cls) for e in (self.manual.get(c) or {}).get("enums", []))

    def binding_source(self, cls: str) -> dict | None:
        for c in self.chain(cls):
            for s in self.rules.get("binding_sources", []):
                if s["class"] == c:
                    return s
        return None


def plain_sample(s: dict):
    """The override `sample` (and `expect`) of a typed probe sample."""
    t, v = s["t"], s["v"]
    if t in STRUCT_FIELDS:
        smp = {f: (int(x) if float(x).is_integer() else x) for f, x in zip(STRUCT_FIELDS[t], v)}
    else:
        smp = v
    exp = s["expect"]["v"] if "expect" in s else None
    return smp, exp


def describe(p: dict) -> str:
    t = p["type"]
    return f"{t.get('native', t['ref'])}, {p['memory']}" + (f", since macOS {p['availability']['since']}" if p.get("availability", {}).get("since") else "")


class Kind:
    def __init__(self, inp: Inputs, cls: str):
        self.inp, self.cls = inp, cls
        self.k = ((inp.probe or {}).get("classes") or {}).get(cls)
        self.ov: dict = {}
        self.ev: dict = {}
        self.review: list[dict] = []
        self.skipped: dict[str, int] = {}

    def skip(self, pattern: str) -> None:
        self.skipped[pattern] = self.skipped.get(pattern, 0) + 1

    def emit(self, section: str, name: str, rec: dict, conf: float, evidence, pattern: str) -> None:
        if conf >= self.inp.rules["promote_at"]:
            self.ov.setdefault(section, []).append(rec)
            self.ev[f"{section}:{name}"] = {"confidence": conf, "evidence": evidence}
        else:
            self.hold(section, name, conf, pattern)

    def hold(self, section: str, name: str, conf: float, pattern: str) -> None:
        self.review.append({"kind": self.cls, "section": section, "name": name, "confidence": conf,
                            "band": "middle" if conf >= 0.6 else "low", "pattern": pattern})

    def denied(self, js: str) -> bool:
        """rules deny_props (a probe from before a pattern was added may still list the prop)."""
        chain = self.inp.chain(self.cls)
        return any((not d.get("class") or d["class"] in chain) and re.match(d["pattern"], js) for d in self.inp.rules.get("deny_props", []))

    # -- P1/P3 ----------------------------------------------------------------------------------
    def infer_props(self) -> None:
        if not self.k:
            return
        subject = self.k.get("subject")
        for n, r in sorted((self.k.get("props") or {}).items()):
            _, meta = self.inp.find_prop(self.cls, n)
            if meta is None or not meta["access"]["write"]:
                continue
            if self.denied(n):
                self.skip("prop:denied")
                continue
            outcome = r.get("outcome")
            if outcome != "exact":
                self.hold("props", n, 0.5, f"prop:{outcome}")
                continue
            sample, expect = plain_sample(r["sample"])
            rec: dict = {"name": n, "sample": sample}
            if expect is not None:
                rec["expect"] = expect
            kvo = bool(r.get("kvo"))
            rec["provenance"] = (f"inferred from the probe on <{subject}>: {describe(meta)}; round trip exact"
                                 + ("; KVO fired on the programmatic set" if kvo else "; no KVO notification"))
            self.emit("props", n, rec, 0.95 if kvo else 0.9, r, "prop:round-trip")

    # -- R: notification signals ------------------------------------------------------------------
    def infer_signals(self) -> None:
        for note in self.inp.rules.get("notifications", {}).get(self.cls, []):
            name = names.notification_signal(note)
            if self.inp.manual_signal(self.cls, name):
                self.skip("signal:manual")
                continue
            if not self.k:
                self.hold("signals", name, 0.6, "signal:no-probe")
                continue
            found = ((self.k.get("notifications") or {}).get(note) or {}).get("symbol")
            if found != note:
                self.hold("signals", name, 0.3, "signal:constant-missing" if found is None else "signal:constant-differs")
                continue
            rec = {"name": name, "via": "notification", "notification": note}
            args = self.inp.rules.get("notification_args", {}).get(note)
            if args:
                rec["args"] = list(args)
            rec["verify"] = "post"
            rec["provenance"] = (f"inferred from the rules (metadata/rules/appkit.yaml notifications, {self.cls} header) and the probe: "
                                 f"the AppKit constant {note} exists with that value; the object is the {self.cls}. Verified by posting "
                                 "it (delivery only, not native causation)")
            self.emit("signals", name, rec, 0.9, {"symbol": found}, "signal:notification")

    # -- B: bindings through target-action or text-did-change ------------------------------------
    def infer_bindings(self) -> None:
        inp = self.inp
        tag = inp.tag(self.cls)
        src = inp.binding_source(self.cls)
        if not tag or not src:
            return
        # attach to the topmost element class the source applies to; subclasses inherit the binding
        for c in inp.chain(self.cls)[1:]:
            if inp.tag(c) and inp.binding_source(c) is src:
                self.skip("binding:inherited")
                return
        sig = inp.manual_signal(self.cls, src["signal"])
        if sig is None:
            self.skip("binding:no-change-signal")
            return
        for js in inp.rules.get("binding_props", []):
            _, meta = inp.find_prop(self.cls, js)
            if meta is None or not meta["access"]["write"]:
                continue
            ref = meta["type"]["ref"]
            if inp.prop_enum(self.cls, js):
                self.skip("binding:enum-prop")           # named-integer props (NSButton state) stay manual
                continue
            if ref not in BIND_TYPES:
                self.skip(f"binding:type-{ref}")
                continue
            if not self.k:
                self.hold("bindings", js, 0.6, "binding:no-probe")
                continue
            r = (self.k.get("props") or {}).get(js)
            if not r or r.get("outcome") != "exact":
                self.hold("bindings", js, 0.5, f"binding:prop-{(r or {}).get('outcome', 'unprobed')}")
                continue
            if src["signal"] in (r.get("events") or []):
                self.hold("bindings", js, 0.5, "binding:echo")
                continue
            delivered = ((self.k.get("signals") or {}).get(src["signal"]) or {}).get("delivered")
            conf = 0.95 if delivered else 0.9 if delivered is None else 0.5
            via = (f"the {sig['notification']} notification" if sig["via"] == "notification" else "the control's action")
            rec = {"name": js, "direction": "two_way", "property": js, "type": {"ref": ref, "nullable": False},
                   "change": {"signal": src["signal"], "role": "user_edit", "value": "prop"},
                   "write": {"natively_normalizes": False},
                   "echo": {"strategy": "none", "compare_before_set": True, "read_back": False},
                   "lifetime": {"on_native_death": "end_binding", "on_stale_update": "warn_noop", "on_destroy": "idempotent_disconnect"},
                   "observability": dict(src["observability"]),
                   "provenance": (f"inferred (rules binding_sources {src['class']}): two-way bind:{js} through {via}, value read back. "
                                  f"{src['why']}. Probe on <{tag}>: round trip exact, the programmatic set did not emit {src['signal']}"
                                  + ("; delivery seen" if delivered else ""))}
            self.emit("bindings", js, rec, conf, {"prop": r, "delivered": delivered}, "binding:" + src["signal"])


def run(inp: Inputs) -> tuple[dict[str, str], dict[str, str], dict]:
    files: dict[str, str] = {}
    evid: dict[str, str] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}
    counts: dict[str, int] = {}
    for cls in sorted(inp.meta):
        kd = Kind(inp, cls)
        if kd.k and kd.k.get("crash"):
            kd.skip("class:crash")
        kd.infer_props()
        kd.infer_signals()
        kd.infer_bindings()
        review += kd.review
        for p, c in kd.skipped.items():
            skipped[p] = skipped.get(p, 0) + c
        if not kd.ov:
            continue
        ov = {"class": cls, **{k: kd.ov[k] for k in ("props", "signals", "bindings") if k in kd.ov}}
        head = f"# @generated by tools/metadata/appkit/infer.py (rules + probe {inp.version}). Do not edit: override in metadata/overrides/appkit/{cls}.yaml.\n"
        files[f"{cls}.yaml"] = head + yaml.safe_dump(ov, sort_keys=False, width=140, allow_unicode=True)
        evid[f"{cls}.evidence.json"] = json.dumps(kd.ev, indent=1, sort_keys=True) + "\n"
        for key in kd.ev:
            sec = key.split(":", 1)[0]
            counts[sec] = counts.get(sec, 0) + 1
    by_pattern: dict[str, int] = {}
    for r in review:
        key = f"{r['band']}:{r['pattern']}"
        by_pattern[key] = by_pattern.get(key, 0) + 1
    summary = {"schema": "svelters-inferred-review/v1", "toolkit": "appkit", "version": inp.version,
               "probe": bool(inp.probe), "promote_at": inp.rules["promote_at"],
               "inferred": dict(sorted(counts.items())),
               "unreviewed": {"middle": sum(1 for r in review if r["band"] == "middle"),
                              "low": sum(1 for r in review if r["band"] == "low"),
                              "by_pattern": dict(sorted(by_pattern.items()))},
               "skipped": dict(sorted(skipped.items())),
               "rows": sorted(review, key=lambda r: (r["kind"], r["section"], r["name"]))}
    return files, evid, summary


def record_fingerprints(files: dict[str, str]) -> dict[tuple[str, str, str], str]:
    """(class, section, name) -> the fingerprint appkit_layers.py and the ledger know the inferred record by."""
    out = {}
    for fname, text in files.items():
        doc = yaml.safe_load(text)
        for sec in ("props", "signals", "bindings"):
            for r in doc.get(sec, []):
                out[(doc["class"], sec, r["name"])] = svmeta.fingerprint(r)
    return out


def prune_parked(files: dict[str, str], check: bool) -> bool:
    """Drop parked facts whose inferred record changed or went away (they are tried again). True if stale."""
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
    inp = Inputs()
    files, evid, summary = run(inp)
    errs = validate(files)
    if errs:
        print("infer: the inferred layer breaks the override schema:", *errs[:20], sep="\n  ", file=sys.stderr)
        return 1
    want = {**files, **evid, "_review.json": json.dumps(summary, indent=1, sort_keys=True) + "\n"}
    parked_stale = prune_parked(files, a.check)
    have = {p.name for p in OUT.glob("*") if p.is_file() and p.name != "_parked.json"} if OUT.exists() else set()
    stale = sorted([n for n, t in want.items() if not (OUT / n).exists() or (OUT / n).read_text() != t] + sorted(have - set(want)))
    # The observe layer (rule O7: tools/metadata/appkit/observe_rules.py), milestone R0.
    import observe as obs
    import observe_rules
    orecs, oev, orev, oskip = observe_rules.infer(svmeta.load_yaml(RULES), inp.version)
    ofiles = obs.inferred_files("appkit", orecs, oev, orev, oskip, {k: v["confidence"] for k, v in oev.items()}, {"sdk": inp.version})
    if a.check:
        for n in stale:
            print(f"infer: metadata/inferred/appkit/{n} is stale (run tools/metadata/appkit/infer.py)", file=sys.stderr)
        ostale = obs.sync_dir(OUT / "observe", ofiles, True, "infer (run tools/metadata/appkit/infer.py)")
        if parked_stale:
            print("infer: _parked.json lists facts whose inferred record changed (run tools/metadata/appkit/infer.py)", file=sys.stderr)
        return 1 if stale or parked_stale or ostale else 0
    obs.sync_dir(OUT / "observe", ofiles, False, "infer")
    print("infer appkit: observe " + ", ".join(f"{k} {v}" for k, v in obs.counts({o: {r['prop']: r for r in rs} for o, rs in orecs.items()}).items()),
          file=sys.stderr)
    OUT.mkdir(parents=True, exist_ok=True)
    for n in sorted(have - set(want)):
        (OUT / n).unlink()
    for n, t in want.items():
        if not (OUT / n).exists() or (OUT / n).read_text() != t:
            (OUT / n).write_text(t)
    inf = summary["inferred"]
    print(f"infer: {len(files)} classes; facts " + (", ".join(f"{k} {v}" for k, v in inf.items()) or "none")
          + f"; unreviewed middle {summary['unreviewed']['middle']}, low {summary['unreviewed']['low']}"
          + ("" if inp.probe else f" (no probe for SDK {inp.version}: run tools/metadata/appkit/probe.py on macOS)"), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
