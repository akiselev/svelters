"""WinUI observability rules (milestone R0; plans/research/REACTIVITY.md sections 3.4 and 4.4), run by infer.py.

Inputs: the winmd facts metadata/winui/*.json (`dependency_property` per property, the events of each class) and the
`no_notify` list and `observe:` section of metadata/rules/winui.yaml. No probe: WinUI does not run on Linux, and the
backend's `observe` member stays NULL until R4, so every fact here is inferred and unverified (the R4 harness inside
the backend's test binary on the windows-winui CI job verifies them). Output: one record per readable property per
class (`<namespace>:<class>`), with a confidence.

Rules:
  O6  a DependencyProperty -> `notify` (RegisterPropertyChangedCallback, no value; exact 0.9), minus the deny list
      `no_notify` (ActualWidth/ActualHeight/ActualTheme are DPs that never notify): those get the event that
      reports them instead (`use`: SizeChanged, ActualThemeChanged), REACTIVITY.md section 11 item 4.
  O5  a plain (non-DP) property paired with an event: `<Prop>Changed` among the class's events, or the rules' pairs
      (`SelectionStart`/`SelectionLength`/`SelectedText` -> SelectionChanged, `IsLoaded` -> Loaded/Unloaded) ->
      `event` (exact 0.9).
  O8  a plain writable property with no event -> set_only (0.9); a plain read-only one -> none (0.9).
  R4  `observe.verify` names the native trigger of the generated row for props a plain write cannot change
      (`verify: {check: cross, trigger}`; the triggers are `testing::Trig` of crates/svelters-winui).
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/winui"


def infer(rules_doc: dict) -> tuple[dict[str, list[dict]], dict, list[dict], dict[str, int]]:
    o = rules_doc.get("observe") or {}
    promote_at = float(rules_doc["promote_at"])
    no_notify = {x["prop"]: x for x in rules_doc.get("no_notify", [])}
    pairs = {k: list(v) for k, v in (o.get("events") or {}).items()}
    # How the generated row triggers a prop that is not a plain native write (milestone R4): `Class.Prop: trigger`.
    verify = dict(o.get("verify") or {})
    classes = {}
    for p in sorted(META.glob("*.json")):
        if p.name.startswith("_"):
            continue
        d = json.loads(p.read_text())
        classes[d["id"].rsplit(":", 1)[-1]] = d
    out: dict[str, list[dict]] = {}
    evidence: dict[str, dict] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}

    def events_of(cls: str) -> set[str]:
        d = classes[cls]
        names = {s["name"] for s in d.get("signals", [])}
        for b in d.get("bases", []):
            b = b.rsplit(".", 1)[-1]
            if b in classes:
                names |= {s["name"] for s in classes[b].get("signals", [])}
        return names

    for cls, d in sorted(classes.items()):
        key = ":".join(d["id"].rsplit(":", 2)[-2:])
        evs = events_of(cls)
        for pr in sorted(d.get("properties", []), key=lambda p: p["name"]):
            name = pr["name"]
            if not pr["access"]["read"]:
                skipped["observe:write-only"] = skipped.get("observe:write-only", 0) + 1
                continue
            fid = f"{cls}.{name}"
            rec: dict = {"prop": name}
            if pr.get("dependency_property") and fid not in no_notify:
                rec.update({"class": "exact", "via": [{"mechanism": "notify"}], "provenance": [
                    {"origin": "winmd", "ref": f"O6: {name}Property is a DependencyProperty: RegisterPropertyChangedCallback"},
                    {"origin": "rule", "ref": "no probe on Linux; verified by the R4 harness on the windows-winui job"}]})
                conf, pattern = 0.9, "observe:dp-callback"
            else:
                use = (no_notify.get(fid) or {}).get("use")
                ev = [use] if use else pairs.get(fid) or [e for e in (f"{name}Changed",) if e in evs]
                if ev:
                    prov = [{"origin": "rule", "ref": f"O6 deny list: {no_notify[fid]['reason']}"}] if use else \
                           [{"origin": "rule", "ref": f"O5: event {', '.join(ev)}"}]
                    rec.update({"class": "exact", "via": [{"mechanism": "event", "events": ev}], "provenance": prov})
                    conf, pattern = 0.9, "observe:event"
                elif pr["access"]["write"]:
                    rec.update({"class": "set_only", "provenance": [
                        {"origin": "rule", "ref": "O8: a plain (non-DependencyProperty) property with no change event"}]})
                    conf, pattern = 0.9, "observe:set-only"
                else:
                    rec.update({"class": "none", "provenance": [
                        {"origin": "rule", "ref": "a read-only plain property with no change event"}]})
                    conf, pattern = 0.9, "observe:none-rule"
            if fid in verify and rec["class"] in ("exact", "partial"):
                rec["verify"] = {"on": key, "check": "cross", "trigger": verify[fid]}
            evidence[f"{key}:{name}"] = {"confidence": conf, "pattern": pattern,
                                         "evidence": {"dependency_property": bool(pr.get("dependency_property"))}}
            if conf >= promote_at:
                out.setdefault(key, []).append(rec)
            else:
                review.append({"owner": key, "prop": name, "class": rec["class"], "confidence": conf,
                               "band": "middle" if conf >= 0.6 else "low", "pattern": pattern})
    return out, evidence, review, skipped
