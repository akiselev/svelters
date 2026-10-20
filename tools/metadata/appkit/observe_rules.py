"""AppKit observability rules (milestone R0; plans/research/REACTIVITY.md sections 3.3 and 4.4), run by infer.py.

Inputs: the SDK facts metadata/appkit/*.json, the AppKit probe metadata/probe/appkit/<SDK>.json (run on the macOS CI
runner: per probed property whether a KVO observer on its KVC key fired on a programmatic set) and the `observe:`
section of metadata/rules/appkit.yaml. The backend's `observe` member stays NULL until R4, so every fact is inferred
and unverified; the R4 harness runs inside the backend's test binary on the macos-appkit job. Output: one record per
readable property per class (`AppKit:<class>`), with a confidence.

Rules (O7):
  - KVO fired on the probed set -> `notify` (KVO on the key). NSControl value properties (`value_props`) are
    `partial, missed: [user_edit]`: while the user edits, the field editor or the tracking loop owns the value and KVO
    stays silent. The rules' fallbacks complete them to `exact` per class (NSTextField values through
    NSControlTextDidChangeNotification, NSSlider values and NSButton state through the action, NSWindow frame through
    NSWindowDidResize/DidMove): such a fact is written on the class the fallback belongs to, even for an inherited
    property, so the element's chain finds it first.
  - KVO did not fire -> none for a value property, set_only for configuration (O8).
  - Not probed: KVO compliance is "only if documented"; writable -> set_only (O8, 0.9), read-only -> none (0.9).
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/appkit"
PROBE_DIR = ROOT / "metadata/probe/appkit"


def infer(rules_doc: dict, version: str) -> tuple[dict[str, list[dict]], dict, list[dict], dict[str, int]]:
    o = rules_doc.get("observe") or {}
    promote_at = float(rules_doc["promote_at"])
    value_props = set(o.get("value_props") or [])
    fallbacks = {k: list(v) for k, v in (o.get("fallbacks") or {}).items()}
    complete = set(o.get("fallback_complete") or [])
    probe_path = PROBE_DIR / f"{version}.json"
    probe = json.loads(probe_path.read_text())["classes"] if probe_path.exists() else {}
    classes = {}
    for p in sorted(META.glob("NS*.json")):
        d = json.loads(p.read_text())
        classes[d["id"].rsplit(":", 1)[-1]] = d
    out: dict[str, list[dict]] = {}
    evidence: dict[str, dict] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}

    def kvo_of(cls: str, js: str) -> bool | None:
        """KVO evidence for `js`: probed on this class, else on a subclass (the probe runs on elements)."""
        seen = [(c, (probe.get(c) or {}).get("props", {}).get(js)) for c in [cls] + sorted(k for k, d in classes.items() if cls in d["bases"])]
        for c, r in seen:
            if r and r.get("outcome") == "exact" and r.get("kvo") is not None:
                return bool(r["kvo"])
        return None

    def decide(cls: str, pr: dict, owner_cls: str) -> tuple[dict, float, str]:
        js = pr["js_name"]
        rec: dict = {"prop": js}
        kvo = kvo_of(cls, js)
        fb = fallbacks.get(f"{owner_cls}:{js}")
        # A value the user edits: NSControl subclasses only (an NSProgressIndicator's value is the app's).
        is_value = js in value_props and (cls == "NSControl" or "NSControl" in classes[cls]["bases"])
        if kvo:
            via = [{"mechanism": "notify"}]
            prov = [{"origin": "probe", "ref": f"AppKit probe (SDK {version}): a KVO observer on `{pr['name']}` fired on a programmatic set"}]
            if fb:
                via += fb
                rec.update({"class": "exact", "via": via})
                prov.append({"origin": "rule", "ref": "O7: user edits reach the value through " + ", ".join(m.get("name") or m["mechanism"] for m in fb)})
            elif is_value:
                rec.update({"class": "partial", "via": via, "missed": ["user_edit"]})
                prov.append({"origin": "rule", "ref": "O7: an NSControl value: KVO misses user edits (field editor, tracking loop)"})
            else:
                rec.update({"class": "exact", "via": via})
            rec["provenance"] = prov
            return rec, 0.9, "observe:kvo-probe"
        if kvo is False:
            rec.update({"class": "none" if is_value else "set_only", "provenance": [
                {"origin": "probe", "ref": f"AppKit probe (SDK {version}): no KVO notification on a programmatic set"}]})
            return rec, 0.9, "observe:kvo-silent"
        if fb and f"{owner_cls}:{js}" in complete:
            rec.update({"class": "exact", "via": fb, "provenance": [
                {"origin": "rule", "ref": "O7: not probed for KVO; the notifications report programmatic and user changes alike"}]})
            return rec, 0.9, "observe:fallback-complete"
        if fb:
            rec.update({"class": "partial", "via": fb, "missed": ["programmatic"], "provenance": [
                {"origin": "rule", "ref": "O7: not probed for KVO; the fallback reports user changes only"}]})
            return rec, 0.9, "observe:fallback-only"
        if pr["access"]["write"]:
            rec.update({"class": "set_only", "provenance": [
                {"origin": "rule", "ref": "O8: not probed; AppKit properties are KVO-compliant only if documented: configuration"}]})
            return rec, 0.9, "observe:set-only"
        rec.update({"class": "none", "provenance": [
            {"origin": "rule", "ref": "a read-only property, not probed; KVO-compliant only if documented"}]})
        return rec, 0.9, "observe:none-rule"

    for cls, d in sorted(classes.items()):
        key = f"AppKit:{cls}"
        props = {pr["js_name"]: pr for pr in d.get("properties", []) if pr["access"]["read"]}
        skipped["observe:write-only"] = skipped.get("observe:write-only", 0) + sum(1 for pr in d.get("properties", []) if not pr["access"]["read"])
        # Fallback facts for inherited properties live on this class (NSTextField:string-value of NSControl).
        for k in fallbacks:
            c, js = k.split(":", 1)
            if c == cls and js not in props:
                for b in d["bases"]:
                    inh = next((x for x in (classes.get(b) or {}).get("properties", []) if x["js_name"] == js), None)
                    if inh:
                        props[js] = inh
                        break
        for js, pr in sorted(props.items()):
            rec, conf, pattern = decide(cls, pr, cls)
            evidence[f"{key}:{js}"] = {"confidence": conf, "pattern": pattern, "evidence": {"kvo": kvo_of(cls, js)}}
            if conf >= promote_at:
                out.setdefault(key, []).append(rec)
            else:
                review.append({"owner": key, "prop": js, "class": rec["class"], "confidence": conf,
                               "band": "middle" if conf >= 0.6 else "low", "pattern": pattern})
    return out, evidence, review, skipped
