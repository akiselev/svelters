"""Qt observability rules (milestone R0; plans/research/REACTIVITY.md sections 3.2 and 4.4), run by infer.py.

Inputs: the ingested props of metadata/qt/*.json (qdoc + the PySide6 metaobject record: NOTIFY, CONSTANT, USER),
the non-GUI types of metadata/qt/types/*.json, moc's extract metadata/qt/moc/properties.json (BINDABLE, NOTIFY,
CONSTANT per Q_PROPERTY: tools/metadata/qt/ingest_moc.py), the observe probe metadata/probe/qt/observe/<version>.json
(tools/metadata/qt/probe.py --section observe) and the `observe:` section of metadata/rules/qt.yaml. Output: one
record per readable property per owner class (`<module>:<class>`), with a confidence.

Rules:
  O2  CONSTANT -> constant (0.95).
  O3  a NOTIFY signal -> `notify` (`coarse` when the signal is the NOTIFY of more than one property of the class:
      QAction::changed()); BINDABLE -> `bindable` (QUntypedBindable::addNotifier).
  O4  the QWidget event table of the rules (`enabled` -> EnabledChange, geometry -> Resize/Move, ...) -> `event`.
  O5  a name-paired signal: `<prop>Changed` / `<prop>Edited` among the class's signals, or the rules' explicit
      pairs (`modified` -> modificationChanged / textEdited, selection props -> selectionChanged) -> `signal`.
  O8  no mechanism, writable, not USER, and the probe saw nothing but our setter change it -> set_only (0.9).
      Without a mechanism otherwise -> none (0.9; 0.95 when the probe saw a native change).
  O10 value_cost: proportional for the text-content properties the rules list.
Probe evidence decides between exact and partial: a change the mechanisms announced counts for, one they missed
counts against (cause `set:<own prop>` -> missed: programmatic, a scenario -> the rules' mapping, another
property -> cross_prop). Without evidence a rule mechanism is exact at 0.9 (the generated test decides for NOTIFY;
event, bindable and signal mechanisms wait for the R3 backend).

Verification (`verify`): only what the R1a backend can observe, i.e. NOTIFY. `change` sets the property natively
(QMetaProperty::write of a new value) and expects prop_changed; `silent` sets it and expects nothing (a NOTIFY the
probe saw missed); `refused` constant -> -1; `unsupported` no NOTIFY -> SVL_OBSERVE_UNSUPPORTED (the backend
cannot observe a `none` property); bindable/event/signal-only facts have no row until R3.
"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/qt"
TYPES = ROOT / "metadata/qt/types"
MOC = ROOT / "metadata/qt/moc/properties.json"
OBS_PROBE = ROOT / "metadata/probe/qt/observe"
SCALAR_CPP = {"bool", "int", "uint", "double", "float", "qreal", "QString", "qlonglong", "qint64"}


def owners() -> dict[str, dict]:
    """class -> {key, props: [{name, read, write, notify, constant, user, bindable, cpp, enum}], module}"""
    moc = json.loads(MOC.read_text())["classes"]
    out: dict[str, dict] = {}

    def mocp(cls: str, name: str) -> dict:
        return ((moc.get(cls) or {}).get("properties") or {}).get(name) or {}

    for p in sorted(META.glob("*.json")):
        d = json.loads(p.read_text())
        cls = p.stem
        props = []
        for pr in d.get("properties", []):
            mo = pr.get("metaobject") or {}
            mc = mocp(cls, pr["name"])
            a = pr.get("access") or {}
            props.append({"name": pr["name"], "read": a.get("read", True), "write": bool(a.get("write")),
                          "q_property": bool(mo) or bool(mc), "notify": (pr.get("notify") or {}).get("signal") or mc.get("notify"),
                          "constant": bool(mo.get("constant") or mc.get("constant")), "user": bool(mo.get("user") or mc.get("user")),
                          "bindable": bool(mc.get("bindable")), "cpp": mo.get("type") or mc.get("type") or (pr.get("type") or {}).get("native"),
                          "enum": bool(mo.get("enum")) or (pr.get("type") or {}).get("ref", "").startswith(("enum<", "flags<"))})
        out[cls] = {"key": ":".join(d["id"].rsplit(":", 2)[-2:]), "props": props, "abstract": bool(d.get("abstract")),
                    "bases": d.get("bases", []), "signals": sorted({s["name"] for s in d.get("signals", [])})}
    for p in sorted(TYPES.glob("Qt*.json")):
        d = json.loads(p.read_text())
        for cls, t in d["types"].items():
            if cls in out or not t.get("properties") or not t.get("qobject"):
                continue
            props = []
            for pr in t["properties"]:
                mc = mocp(cls, pr["name"])
                props.append({"name": pr["name"], "read": True, "write": bool(pr.get("writable")), "q_property": bool(mc),
                              "notify": mc.get("notify") or pr.get("notify"), "constant": bool(mc.get("constant")),
                              "user": bool(mc.get("user")), "bindable": bool(mc.get("bindable")),
                              "cpp": mc.get("type") or (pr.get("type") or {}).get("cpp"), "enum": (pr.get("type") or {}).get("k") == "enum"})
            out[cls] = {"key": f"{d['module']}:{cls}", "props": props, "abstract": bool(t.get("abstract")),
                        "bases": t.get("bases", []), "signals": sorted({s["name"] for s in t.get("signals", [])})}
    return out


class Rules:
    def __init__(self, doc: dict):
        o = doc.get("observe") or {}
        self.promote_at = float(doc["promote_at"])
        self.probe = str(o["probe"])
        self.events = {k: list(v) for k, v in (o.get("events") or {}).items()}
        self.deferred = set(o.get("deferred") or [])
        self.signals = {k: list(v) for k, v in (o.get("signals") or {}).items()}
        self.proportional = set(o.get("proportional") or [])
        self.scenario_missed = dict(o.get("scenario_missed") or {})
        # Scenarios whose changes Qt announces later (Resize/Move of a hidden widget at show()): not a miss.
        self.deferred_scenarios = set(o.get("deferred_scenarios") or [])


def sig_name(sig: str | None) -> str | None:
    return sig.split("(", 1)[0] if sig else None


def infer(rules_doc: dict) -> tuple[dict[str, list[dict]], dict, list[dict], dict[str, int]]:
    rules = Rules(rules_doc)
    obs = json.loads((OBS_PROBE / f"{rules.probe}.json").read_text())
    evp = obs["props"]
    probed = obs["types"]
    crashed = set(obs.get("crashed", []))
    own = owners()
    key_of = {c: o["key"] for c, o in own.items()}

    def instantiable(key: str) -> bool:
        return (probed.get(key) or {}).get("class") == "object" and key not in crashed

    # concrete subclasses per class (for the instance a test of an abstract class's props uses)
    subs: dict[str, list[str]] = {}
    for c, o in own.items():
        if instantiable(o["key"]):
            for b in o["bases"]:
                subs.setdefault(b, []).append(c)

    def subject(cls: str, hint: str | None) -> str | None:
        if instantiable(own[cls]["key"]):
            return own[cls]["key"]
        if hint and instantiable(hint):
            return hint
        cands = sorted(subs.get(cls, []))
        return own[cands[0]]["key"] if cands else None

    def writable_on(inst_key: str, name: str) -> bool:
        cls = inst_key.split(":", 1)[1]
        for c in [cls] + own.get(cls, {}).get("bases", []):
            for pr in own.get(c, {}).get("props", []):
                if pr["name"] == name:
                    return pr["write"] and not pr["constant"] and (pr["cpp"] in SCALAR_CPP or pr["enum"])
        return False

    out: dict[str, list[dict]] = {}
    evidence: dict[str, dict] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}

    for cls, o in sorted(own.items()):
        key = o["key"]
        notify_count: dict[str, int] = {}
        for pr in o["props"]:
            if pr["notify"]:
                notify_count[sig_name(pr["notify"])] = notify_count.get(sig_name(pr["notify"]), 0) + 1
        for pr in sorted(o["props"], key=lambda p: p["name"]):
            name = pr["name"]
            fid = f"{key}:{name}"
            if not pr["read"]:
                skipped["observe:write-only"] = skipped.get("observe:write-only", 0) + 1
                continue
            e = evp.get(fid) or {}
            evid = {k: e[k] for k in ("flags", "sweep", "seen") if k in e}
            sw = e.get("sweep")
            seen = list(e.get("seen") or [])
            sub = subject(cls, (sw or {}).get("on") or next((s["on"] for s in seen), None))
            rec: dict = {"prop": name}
            prov = []
            if pr["constant"]:
                rec.update({"class": "constant", "provenance": [{"origin": "moc", "ref": "O2: Q_PROPERTY CONSTANT"}]})
                if sub:
                    rec["verify"] = {"on": sub, "check": "refused"}
                out.setdefault(key, []).append(rec)
                evidence[fid] = {"confidence": 0.95, "pattern": "observe:constant", "evidence": evid}
                continue
            via = []
            nsig = sig_name(pr["notify"])
            if nsig:
                m = {"mechanism": "notify", "signal": pr["notify"]}
                if notify_count.get(nsig, 0) > 1:
                    m["coarse"] = True
                via.append(m)
                prov.append({"origin": "moc" if pr["q_property"] else "metaobject", "ref": f"O3: NOTIFY {pr['notify']}"
                             + (f" (shared by {notify_count[nsig]} properties)" if m.get("coarse") else "")})
            if pr["bindable"]:
                via.append({"mechanism": "bindable"})
                prov.append({"origin": "moc", "ref": "O3: BINDABLE (QUntypedBindable::addNotifier)"})
            evs = rules.events.get(f"{cls}:{name}") or rules.events.get(f"QWidget:{name}") if (cls == "QWidget" or "QWidget" in o["bases"]) \
                else rules.events.get(f"{cls}:{name}")
            if evs and not nsig:
                via.append({"mechanism": "event", "events": evs})
                prov.append({"origin": "rule", "ref": f"O4: QEvent table: {', '.join(evs)}"})
            pairs = rules.signals.get(f"{cls}:{name}") or [s for s in (f"{name}Changed", f"{name}Edited") if s in o["signals"] and s != nsig]
            if not nsig:
                for s in pairs:
                    via.append({"mechanism": "signal", "signal": s})
                if pairs:
                    prov.append({"origin": "rule", "ref": f"O5: name-paired signal {', '.join(pairs)}"})
            # Evidence: which changes the mechanisms announced.
            changes = []
            if sw and sw["readback"] != "ignored":
                changes.append({"cause": f"set:{fid}", "on": sw["on"], "notify": sw["notify_change"], "bindable": sw["bindable_change"],
                                "events": sw["events"], "signals": sw["signals"], "changed": True})
            changes += [s for s in seen if s["changed"] and s["cause"].split(":", 1)[-1] not in rules.deferred_scenarios]

            def announced(c: dict) -> bool:
                if nsig and c["notify"] > 0:
                    return True
                if pr["bindable"] and c["bindable"] > 0:
                    return True
                if evs and set(evs) & set(c["events"]):
                    return True
                return any(s in c["signals"] for s in pairs) and not nsig
            ok = [c for c in changes if announced(c)]
            miss = [c for c in changes if not announced(c)]

            def miss_kind(c: dict) -> str:
                if c["cause"] == f"set:{fid}":
                    return "programmatic"
                if c["cause"].startswith("scenario:"):
                    return rules.scenario_missed.get(c["cause"].split(":", 1)[1], "cross_prop")
                return "cross_prop"
            missed = sorted({miss_kind(c) for c in miss})
            how = lambda cs: ", ".join(sorted({c["cause"] for c in cs})[:3])  # noqa: E731
            verify: dict = {"on": sub} if sub else {}
            if via:
                if miss and ok:
                    rec.update({"class": "partial", "via": via, "missed": missed})
                    prov.append({"origin": "probe", "ref": f"observe probe Qt {obs['version']}: announced {how(ok)}; missed {how(miss)}"})
                    conf, pattern = 0.95, "observe:partial-probe"
                elif miss:
                    rec.update({"class": "partial", "via": via, "missed": missed})
                    prov.append({"origin": "probe", "ref": f"observe probe Qt {obs['version']}: the mechanisms missed {how(miss)}"})
                    conf, pattern = 0.95, "observe:partial-probe"
                elif ok:
                    rec.update({"class": "exact", "via": via})
                    prov.append({"origin": "probe", "ref": f"observe probe Qt {obs['version']}: announced every observed change ({how(ok)})"})
                    conf, pattern = 0.95, "observe:exact-probe"
                else:
                    rec.update({"class": "exact", "via": via})
                    conf = 0.9 if (nsig or pr["bindable"] or evs) else 0.8
                    pattern = "observe:exact-rule" if conf >= 0.9 else "observe:exact-signal-pair-unprobed"
                if sw and nsig and sw["readback"] != "ignored":
                    rec["fires"] = "on_set" if sw["notify_same"] > 0 else "on_change"
                if any(m.get("coarse") for m in via):
                    rec["fires"] = "coarse"
                if nsig and sub:
                    if pr["write"] and writable_on(sub, name) and not (sw and sw["readback"] == "ignored"):
                        silent_self = any(c["cause"] == f"set:{fid}" for c in miss)
                        verify.update({"check": "silent", "trigger": name} if silent_self else {"check": "change"})
                    else:
                        verify["check"] = "connect"
            else:
                native_change = [c for c in changes if c["cause"] != f"set:{fid}"]
                if pr["write"] and not pr["user"] and not native_change:
                    rec["class"] = "set_only"
                    prov.append({"origin": "rule", "ref": "O8: writable, no NOTIFY, BINDABLE, event or paired signal, not USER"
                                 + ("; the probe saw nothing but our setter change it" if obs else "")})
                    conf, pattern = 0.9, "observe:set-only"
                else:
                    rec["class"] = "none"
                    if native_change:
                        prov.append({"origin": "probe", "ref": f"observe probe Qt {obs['version']}: changed unannounced on {how(native_change)}"})
                        conf, pattern = 0.95, "observe:none-probe"
                    else:
                        prov.append({"origin": "rule", "ref": "no NOTIFY, BINDABLE, event or paired signal" + (" (read-only)" if not pr["write"] else " (USER)")})
                        conf, pattern = 0.9, "observe:none-rule"
                if sub and rec["class"] == "none" and pr["q_property"]:
                    verify["check"] = "unsupported"
            if f"{cls}:{name}" in rules.proportional:
                rec["value_cost"] = "proportional"
            if f"{cls}:{name}" in rules.deferred or (cls != "QWidget" and f"QWidget:{name}" in rules.deferred and "QWidget" in o["bases"]):
                rec["timing"] = "deferred"
            if not prov:
                prov.append({"origin": "rule", "ref": "no mechanism"})
            rec["provenance"] = prov
            if verify.get("on") and verify.get("check"):
                rec["verify"] = verify
            evidence[fid] = {"confidence": conf, "pattern": pattern, "evidence": evid}
            if conf >= rules.promote_at:
                out.setdefault(key, []).append(rec)
            else:
                review.append({"owner": key, "prop": name, "class": rec["class"], "confidence": conf,
                               "band": "middle" if conf >= 0.6 else "low", "pattern": pattern})
    for k in out:
        out[k].sort(key=lambda r: r["prop"])
    return out, evidence, review, skipped
