#!/usr/bin/env python3
"""Observability facts (milestone R0; plans/research/REACTIVITY.md sections 4 and 13).

One `observe` record per readable property per platform says whether, and how, a native change of the property can
be observed:

    prop: text                       # the property's native name (GTK kebab, Qt camelCase, AppKit/WinUI as ingested)
    class: exact                     # constant | set_only | exact | partial | none
    via:                             # ordered: the backend installs every entry it supports (primary first)
      - {mechanism: notify}          #   GTK notify::<prop>, Qt NOTIFY, AppKit KVO, WinUI DP callback
      - {mechanism: bindable}        #   Qt QUntypedBindable::addNotifier
      - {mechanism: signal, signal: textEdited}       # another signal of the same object
      - {mechanism: event, events: [EnabledChange]}   # Qt event filter / WinUI routed event
      - {mechanism: notification, name: NSControlTextDidChangeNotification}
      - {mechanism: target_action}   #   AppKit NSControl action (continuous controls)
      - {mechanism: via, object: buffer, prop: length} # a property of another object
    missed: [cross_prop]             # partial only: causes the mechanisms miss
    fires: on_change                 # on_change | on_set (re-announces identical sets) | coarse (shared notifier)
    derived_from: [year]             # props whose set also announces this one (probe cross-notify)
    value_cost: cheap                # cheap | proportional (whole-text props: the host skips read + compare)
    timing: sync                     # sync | deferred
    provenance: [{origin: probe, ref: "..."}]

Classes (REACTIVITY.md 4.1, decisions 13): `exact` every native change is announced by the listed mechanisms (R2
lowers every `bind:` on an exact prop onto observation, so only evidence or a passing test makes a fact exact);
`partial` some causes (`missed`) are not; `set_only` only our setters change it (no native observation, setters
invalidate locally, two-way `bind:` is an error); `constant` never changes after construction; `none` native
changes are not announced (no polling mechanism in v1: decision 9).

Layers, lowest precedence first (the same order as every other fact, ANNOTATION-AUTOMATION.md section 5.2):
  ingested  the raw facts each toolkit's metadata carries (GTK ParamSpec flags + notify, Qt NOTIFY/CONSTANT and the
            moc BINDABLE extract, WinUI DependencyProperty, AppKit KVO evidence); they are inputs of the rules and
            never an `observe` record themselves
  inferred  metadata/inferred/<tk>/observe/<Ns>.<Type>.yaml, written by the toolkit's infer.py (rules O1-O10)
  manual    metadata/overrides/<tk>/observe/<Ns>.<Type>.yaml (same format); a manual record replaces the inferred
            record of the same prop as a whole
Parked inferred facts (metadata/inferred/<tk>/_parked.json, section `observe`) are left out. Both layer files are
`{schema: svelters-observe-layer/v1, toolkit, owner: "<Ns>:<Type>", observe: [records]}`; the inferred ones
carry a `confidence` per record (written only at promote_at or above, the rest go to `_review.json`).

The merged facts are written to metadata/observe/<tk>.json (`--write`; `--check` fails when stale) and read by
gen_semantics.py (ObserveSpec), the verification generators, the ledger and coverage.py. They are kept out of the
property records of metadata/<tk>/*.json on purpose: an observe fact is its own fact
(`svelters-api:v1:type:<tk>:<Ns>:<Type>:prop:<prop>:observe`) with its own fingerprint and test, so changing one
never invalidates the verified property facts.

    python3 tools/metadata/common/observe.py [--toolkit gtk|qt|appkit|winui|all] [--check]
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import svmeta  # noqa: E402

TOOLKITS = ("gtk", "qt", "appkit", "winui")
CLASSES = ("constant", "set_only", "exact", "partial", "none")
MECHANISMS = ("notify", "bindable", "signal", "event", "notification", "target_action", "via")
# `cross_prop`: changed as a side effect of another property set or API call without being announced (the GTK
# probe's TextTag `font` after `family-set`); `programmatic`: a native setter path that does not announce (KVO on
# a non-compliant accessor); the others as REACTIVITY.md 4.2.
MISSED = ("user_edit", "layout", "parent", "wm", "async", "programmatic", "cross_prop")
FIRES = ("on_change", "on_set", "coarse")
VALUE_COST = ("cheap", "proportional")
TIMING = ("sync", "deferred")
PROV_ORIGINS = ("gir", "metaobject", "moc", "winmd", "sdk", "rule", "probe", "manual", "runtime")
KEYS = {"prop", "class", "via", "missed", "fires", "derived_from", "value_cost", "timing", "provenance", "confidence", "verify"}
# How the generated test checks the fact (not part of the semantics; the compiler never sees it):
#   on       the concrete type the test instantiates (`<Ns>:<Type>`); absent: no test can be generated
#   check    change   set the property natively to a new value: announced once per change; `fires` decides an
#                     identical set; unobserved: silent
#            cross    set `trigger` (another property of the same instance) or run `rust`: this property changes
#                     and is announced
#            via      set the forwarded-to object's property natively: announced on the element
#            silent   set `trigger` or run `rust`: this property changes and is NOT announced (none, partial)
#            connect  the mechanism installs and uninstalls (no native trigger known: `verified: connect`)
#            refused  observe is refused (constant)
#            unsupported  observe answers SVL_OBSERVE_UNSUPPORTED: the backend has no mechanism (none)
VERIFY_CHECKS = ("change", "cross", "via", "silent", "connect", "refused", "unsupported")
# Key order of a written record.
ORDER = ("prop", "class", "via", "missed", "fires", "derived_from", "value_cost", "timing", "provenance", "verify", "confidence")
VERIFY_KEYS = {"on", "check", "trigger", "rust", "native", "setup"}
MECH_KEYS = {"notify": {"signal", "coarse"}, "bindable": set(), "signal": {"signal"}, "event": {"events"},
             "notification": {"name"}, "target_action": set(), "via": {"object", "prop"}}
MECH_REQUIRED = {"signal": {"signal"}, "event": {"events"}, "notification": {"name"}, "via": {"object", "prop"}}
LAYER_SCHEMA = "svelters-observe-layer/v1"
MERGED_SCHEMA = "svelters-observe/v1"
OUT_DIR = ROOT / "metadata/observe"


def validate(rec: dict, where: str = "") -> list[str]:
    """Problems of one observe record (empty when valid)."""
    w = f"{where}{rec.get('prop', '?')}: "
    errs = [w + f"unknown key {k!r}" for k in rec if k not in KEYS]
    if not isinstance(rec.get("prop"), str) or not rec.get("prop"):
        errs.append(w + "needs `prop`")
    c = rec.get("class")
    if c not in CLASSES:
        errs.append(w + f"class must be one of {CLASSES}, not {c!r}")
    via = rec.get("via", [])
    if not isinstance(via, list):
        errs.append(w + "via must be a list")
        via = []
    for m in via:
        k = m.get("mechanism") if isinstance(m, dict) else None
        if k not in MECHANISMS:
            errs.append(w + f"unknown mechanism {m!r}")
            continue
        extra = set(m) - {"mechanism"} - MECH_KEYS[k]
        if extra:
            errs.append(w + f"{k}: unknown keys {sorted(extra)}")
        missing = MECH_REQUIRED.get(k, set()) - set(m)
        if missing:
            errs.append(w + f"{k}: needs {sorted(missing)}")
    if c in ("exact", "partial") and not via:
        errs.append(w + f"class {c} needs at least one `via` mechanism")
    if c in ("constant", "set_only", "none") and via:
        errs.append(w + f"class {c} observes nothing: `via` must be empty")
    missed = rec.get("missed", [])
    if any(x not in MISSED for x in missed):
        errs.append(w + f"missed must be among {MISSED}")
    if c == "partial" and not missed:
        errs.append(w + "class partial needs `missed`")
    if c != "partial" and missed:
        errs.append(w + "`missed` is for class partial only")
    for key, allowed in (("fires", FIRES), ("value_cost", VALUE_COST), ("timing", TIMING)):
        if key in rec and rec[key] not in allowed:
            errs.append(w + f"{key} must be one of {allowed}")
    if not isinstance(rec.get("derived_from", []), list):
        errs.append(w + "derived_from must be a list")
    prov = rec.get("provenance")
    if not isinstance(prov, list) or not prov or not all(isinstance(p, dict) and p.get("origin") in PROV_ORIGINS and p.get("ref") for p in prov):
        errs.append(w + f"provenance must be a non-empty list of {{origin: {'|'.join(PROV_ORIGINS)}, ref}}")
    if "confidence" in rec and not (isinstance(rec["confidence"], (int, float)) and 0 <= rec["confidence"] <= 1):
        errs.append(w + "confidence must be in [0, 1]")
    v = rec.get("verify")
    if v is not None:
        if not isinstance(v, dict) or set(v) - VERIFY_KEYS or v.get("check") not in VERIFY_CHECKS or ":" not in str(v.get("on", "")):
            errs.append(w + f"verify must be {{on: <Ns>:<Type>, check: {'|'.join(VERIFY_CHECKS)}, trigger?, rust?, native?}}")
        elif v["check"] == "silent" and not (v.get("trigger") or v.get("rust")):
            errs.append(w + "verify silent needs a trigger or rust")
    return errs


def owner_file(owner: str) -> str:
    """`Gtk:Entry` -> `Gtk.Entry.yaml` (WinUI namespaces have dots already: the colon keeps the split)."""
    ns, typ = owner.rsplit(":", 1)
    return f"{ns}.{typ}.yaml"


def fact_id(tk: str, owner: str, prop: str) -> str:
    return f"svelters-api:v1:type:{tk}:{owner}:prop:{prop}:observe"


def mechanism_str(rec: dict) -> str:
    """One line naming the mechanisms, for diagnostics and docs (`notify+signal:textEdited`)."""
    parts = []
    for m in rec.get("via", []):
        k = m["mechanism"]
        if k in ("signal",):
            parts.append(f"signal:{m['signal']}")
        elif k == "event":
            parts.append("event:" + ",".join(m["events"]))
        elif k == "notification":
            parts.append(f"notification:{m['name']}")
        elif k == "via":
            parts.append(f"via:{m['object']}.{m['prop']}")
        else:
            parts.append(k)
    return "+".join(parts)


def spec(rec: dict) -> dict:
    """The compiler's ObserveSpec (crates/svelters-compiler/src/semantics/spec.rs) of a merged record."""
    out = {"class": rec["class"], "mechanism": mechanism_str(rec)}
    if rec.get("missed"):
        out["missed"] = list(rec["missed"])
    if rec.get("fires") == "coarse" or any(m.get("coarse") for m in rec.get("via", [])):
        out["coarse"] = True
    if rec.get("value_cost") == "proportional":
        out["value_cost"] = "proportional"
    if rec.get("timing") == "deferred":
        out["timing"] = "deferred"
    return out


def strip(rec: dict) -> dict:
    """The record as a fact: without the layer bookkeeping (confidence), with sorted keys. The fingerprint of an
    inferred record (what the ledger parks it by) is taken over this."""
    return {k: rec[k] for k in sorted(rec) if k != "confidence"}


class Layers:
    """The merged observe facts of one toolkit: inferred < manual, parked inferred facts left out."""

    def __init__(self, tk: str, root: Path = ROOT):
        self.tk = tk
        self.inferred_dir = root / f"metadata/inferred/{tk}/observe"
        self.manual_dir = root / f"metadata/overrides/{tk}/observe"
        self.parked_path = root / f"metadata/inferred/{tk}/_parked.json"
        self.errors: list[str] = []
        self.inferred = self._load(self.inferred_dir)
        self.manual = self._load(self.manual_dir)
        self.parked: dict[tuple[str, str], str] = {}
        if self.parked_path.exists():
            for r in json.loads(self.parked_path.read_text())["parked"]:
                if r["section"] == "observe":
                    self.parked[(r["kind"], r["name"])] = r.get("record_fp")
        self.facts: dict[str, dict[str, dict]] = {}
        for owner in sorted(set(self.inferred) | set(self.manual)):
            out: dict[str, dict] = {}
            for rec in self.inferred.get(owner, []):
                r = strip(rec)
                fp = svmeta.fingerprint(r)
                if self.parked.get((owner, rec["prop"])) == fp:
                    continue
                out[rec["prop"]] = {**r, "layer": "inferred", "record_fp": fp}
            for rec in self.manual.get(owner, []):
                out[rec["prop"]] = {**strip(rec), "layer": "manual"}
            if out:
                self.facts[owner] = dict(sorted(out.items()))

    def _load(self, d: Path) -> dict[str, list[dict]]:
        out: dict[str, list[dict]] = {}
        if not d.exists():
            return out
        for p in sorted(d.glob("*.yaml")):
            doc = svmeta.load_yaml(p)
            if doc.get("schema") != LAYER_SCHEMA or doc.get("toolkit") != self.tk or ":" not in str(doc.get("owner", "")):
                self.errors.append(f"{p}: expected schema {LAYER_SCHEMA}, toolkit {self.tk}, owner <Ns>:<Type>")
                continue
            if owner_file(doc["owner"]) != p.name:
                self.errors.append(f"{p}: owner {doc['owner']} belongs in {owner_file(doc['owner'])}")
            recs = doc.get("observe") or []
            seen = set()
            for r in recs:
                self.errors += validate(r, f"{p.name}: ")
                if r.get("prop") in seen:
                    self.errors.append(f"{p.name}: {r.get('prop')} twice")
                seen.add(r.get("prop"))
            out[doc["owner"]] = recs
        return out

    def get(self, owner: str, prop: str) -> dict | None:
        return self.facts.get(owner, {}).get(prop)


def merged(tk: str, version: str | None = None) -> dict:
    lay = Layers(tk)
    if lay.errors:
        raise SystemExit("observe: " + "\n  ".join(lay.errors[:40]))
    facts = {}
    for owner, props in lay.facts.items():
        facts[owner] = {p: {**r, "fact": fact_id(tk, owner, p)} for p, r in props.items()}
    return {"schema": MERGED_SCHEMA, "toolkit": tk, "facts": facts}


def load(tk: str) -> dict[str, dict[str, dict]]:
    """owner -> prop -> merged record, from the checked-in metadata/observe/<tk>.json (empty when absent)."""
    p = OUT_DIR / f"{tk}.json"
    return json.loads(p.read_text())["facts"] if p.exists() else {}


def counts(facts: dict[str, dict[str, dict]]) -> dict[str, int]:
    out = {c: 0 for c in CLASSES}
    for props in facts.values():
        for r in props.values():
            out[r["class"]] += 1
    return out


def layer_doc(tk: str, owner: str, recs: list[dict]) -> str:
    """The YAML text of one layer file (infer.py writes them through this, so the format stays one)."""
    import yaml
    return yaml.safe_dump({"schema": LAYER_SCHEMA, "toolkit": tk, "owner": owner, "observe": recs},
                          sort_keys=False, width=140, allow_unicode=True)


def inferred_files(tk: str, recs: dict[str, list[dict]], evidence: dict, review: list[dict], skipped: dict[str, int],
                   confidence: dict[str, float], extra: dict | None = None) -> dict[str, str]:
    """name -> text of the inferred observe layer of a toolkit (metadata/inferred/<tk>/observe/): one file per owner,
    `_evidence.json` (confidence, rule pattern and probe excerpt per fact) and `_review.json` (facts below
    promote_at, by pattern, plus counts per class). `confidence`: `owner:prop` -> confidence of a written record."""
    files = {}
    by_class: dict[str, int] = {c: 0 for c in CLASSES}
    for owner, rs in sorted(recs.items()):
        out = []
        for r in rs:
            errs = validate(r, f"{owner}: ")
            if errs:
                raise SystemExit("infer: bad observe record: " + "; ".join(errs))
            by_class[r["class"]] += 1
            r = {**r, "confidence": confidence[f"{owner}:{r['prop']}"]}
            out.append({k: r[k] for k in ORDER if k in r})
        files[owner_file(owner)] = layer_doc(tk, owner, out)
    by_pattern: dict[str, int] = {}
    for r in review:
        k = f"{r['band']}:{r['pattern']}"
        by_pattern[k] = by_pattern.get(k, 0) + 1
    files["_evidence.json"] = json.dumps(evidence, indent=1, sort_keys=True, ensure_ascii=False) + "\n"
    files["_review.json"] = json.dumps({
        "schema": "svelters-observe-review/v1", "toolkit": tk, **(extra or {}),
        "inferred": by_class, "unreviewed": {"middle": sum(1 for r in review if r["band"] == "middle"),
                                             "low": sum(1 for r in review if r["band"] == "low"),
                                             "by_pattern": dict(sorted(by_pattern.items()))},
        "skipped": dict(sorted(skipped.items())),
        "rows": sorted(review, key=lambda r: (r["owner"], r["prop"]))}, indent=1, sort_keys=True, ensure_ascii=False) + "\n"
    return files


def layer_fingerprints(files: dict[str, str]) -> dict[tuple[str, str, str], str]:
    """(owner, "observe", prop) -> the fingerprint Layers gives an inferred record (what the ledger parks it by), for
    the layer files `inferred_files` returns. infer.py prunes parked facts whose record changed with it."""
    import yaml
    out = {}
    for name, text in files.items():
        if name.endswith(".yaml"):
            doc = yaml.safe_load(text)
            for r in doc["observe"]:
                out[(doc["owner"], "observe", r["prop"])] = svmeta.fingerprint(strip(r))
    return out


def sync_dir(d: Path, files: dict[str, str], check: bool, label: str) -> list[str]:
    """Make directory `d` hold exactly `files` (check: only report). Returns the stale names."""
    have = {p.name for p in d.glob("*") if p.is_file()} if d.exists() else set()
    stale = sorted([n for n, t in files.items() if not (d / n).exists() or (d / n).read_text() != t] + sorted(have - set(files)))
    if check:
        for n in stale:
            print(f"{label}: {d.relative_to(ROOT)}/{n} is stale", file=sys.stderr)
        return stale
    d.mkdir(parents=True, exist_ok=True)
    for n in sorted(have - set(files)):
        (d / n).unlink()
    for n, t in files.items():
        if not (d / n).exists() or (d / n).read_text() != t:
            (d / n).write_text(t)
    return stale


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--toolkit", default="all", choices=TOOLKITS + ("all",))
    ap.add_argument("--check", action="store_true", help="fail if metadata/observe/<tk>.json is stale")
    a = ap.parse_args()
    rc = 0
    for tk in (TOOLKITS if a.toolkit == "all" else (a.toolkit,)):
        doc = merged(tk)
        text = json.dumps(doc, indent=1, sort_keys=True, ensure_ascii=False) + "\n"
        path = OUT_DIR / f"{tk}.json"
        if a.check:
            if not path.exists() or path.read_text() != text:
                print(f"observe: {path.relative_to(ROOT)} is stale (run tools/metadata/common/observe.py)", file=sys.stderr)
                rc = 1
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists() or path.read_text() != text:
            path.write_text(text)
        c = counts(doc["facts"])
        print(f"observe {tk}: " + ", ".join(f"{k} {v}" for k, v in c.items()), file=sys.stderr)
    return rc


if __name__ == "__main__":
    sys.exit(main())
