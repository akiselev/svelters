"""GTK observability rules (milestone R0; plans/research/REACTIVITY.md section 4.4), run by infer.py.

Inputs: the ingested props of metadata/gtk/*.json (widget kinds, ParamSpec flags in `runtime.flags`, `notify`),
the non-GUI types of metadata/gtk/types/<Ns>.json, the observe probe metadata/probe/gtk/observe/<version>.json
(flags of every type, the native set sweep and the side effects of every change; tools/metadata/gtk/probe.py
--section observe), the round-trip probe metadata/probe/gtk/<version>.json (`notify` per set) and the `observe:`
section of metadata/rules/gtk.yaml. Output: one record per readable property per owner (`<Ns>:<Type>`), in the
format of tools/metadata/common/observe.py, with a confidence.

Rules:
  O1  a readable property with a ParamSpec is observed through `notify::<prop>`. Probe evidence decides the class:
      a notify on every observed change -> exact (0.95); a change without notify while other changes notify ->
      partial, missed: cross_prop (or the scenario's cause) (0.95); changes never announced -> none (0.95). With no
      observed change: writable -> exact (0.9, the generated test sets it natively and decides), read-only -> exact
      (0.8: no trigger known, review), unless a review decision of the rules covers it: `accept` -> exact (0.9),
      `constant` (fixed for the object's lifetime) -> constant (0.9, no test), `unreviewed` -> the review row names
      the group and its reason. `fires`: on_set when an identical set re-notifies (probe), or for a
      non-EXPLICIT_NOTIFY property without a probe row (GObject notifies every set); on_change when the probe saw
      no re-notify.
  O2  CONSTRUCT_ONLY -> constant (0.95).
  O8  a property svelters computes (virtual, not forwarded) -> set_only (0.9): nothing native changes it.
  O9  a forwarded property (`virtual.op: via_object`, GtkRange `value` on its adjustment) -> observed through
      `via: {object, prop}`; the class is the target property's.
  O10 derived_from: the properties whose set also notified this one (probe cross-notify); value_cost: proportional
      for the text-content properties the rules list.
The verification hint (`verify`) names the concrete instance type the generated test creates and how it changes
the property: `change` (set it natively to a new value), `cross` (set another property that changed it in the
probe), `via` (set the target), `rust` (a statement from the rules), `silent` (change it and expect no
announcement: none/partial), `connect` (only install and remove the mechanism), `refused` (constant).
"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
META = ROOT / "metadata/gtk"
TYPES = ROOT / "metadata/gtk/types"
OBS_PROBE = ROOT / "metadata/probe/gtk/observe"
SCALAR_GTYPES = {"gboolean", "gint", "guint", "gint64", "guint64", "glong", "gulong", "gdouble", "gfloat", "gchararray"}
INGESTED = ("gir", "runtime")


def gir_owner(doc_id: str) -> str:
    """`svelters-api:v1:type:gtk:Gtk:Entry` -> `Gtk:Entry`."""
    return ":".join(doc_id.rsplit(":", 2)[-2:])


class Owner:
    """One GObject type: its props from the widget metadata or the types facts, its hierarchy."""

    def __init__(self, key: str, gtype: str, abstract: bool, interface: bool, bases: list[str], interfaces: list[str],
                 props: list[dict], element: str | None, since: str | None, source: str):
        self.key, self.gtype, self.abstract, self.interface = key, gtype, abstract, interface
        self.bases, self.interfaces, self.props, self.element, self.since, self.source = bases, interfaces, props, element, since, source


def load_owners() -> dict[str, Owner]:
    out: dict[str, Owner] = {}
    for p in sorted(META.glob("*.json")):
        d = json.loads(p.read_text())
        if d.get("kind") not in ("object", "interface"):
            continue
        key = gir_owner(d["id"])
        props = []
        for pr in d.get("properties", []):
            if pr.get("origin") not in INGESTED and not pr.get("virtual"):
                continue
            a = pr["access"]
            props.append({"name": pr["name"], "read": bool(a["read"]), "write": bool(a["write"]),
                          "construct_only": bool(a.get("construct_only")), "ref": pr["type"]["ref"],
                          "gtype": (pr.get("runtime") or {}).get("gtype") or pr["type"].get("gtype"),
                          "flags": (pr.get("runtime") or {}).get("flags"), "virtual": pr.get("virtual"),
                          "since": pr.get("since") or (pr.get("availability") or {}).get("since"),
                          "deprecated": bool(pr.get("deprecated"))})
        conv = lambda xs: [x.replace(".", ":", 1) for x in xs]  # noqa: E731  Gtk.Widget -> Gtk:Widget
        out[key] = Owner(key, d["native"]["gtype"], bool(d.get("abstract")), d["kind"] == "interface", conv(d.get("bases", [])),
                         conv(d.get("interfaces", [])), props, (d.get("element") or {}).get("tag"),
                         (d.get("availability") or {}).get("since"), "kinds")
    for p in sorted(TYPES.glob("*.json")):
        if p.name == "facts.json":
            continue
        d = json.loads(p.read_text())
        ns = d["namespace"]
        for name, t in d["types"].items():
            key = f"{ns}:{name}"
            if t.get("category") not in ("object", "model") or not t.get("gtype") or key in out:
                continue
            props = [{"name": pr["name"], "read": bool(pr.get("readable", True)), "write": bool(pr.get("writable")),
                      "construct_only": bool(pr.get("construct_only")), "ref": (pr.get("type") or {}).get("k", ""),
                      "gtype": None, "flags": None, "virtual": None, "since": pr.get("since"), "deprecated": False}
                     for pr in t.get("properties", [])]
            parents = []
            cur = t.get("parent")
            if cur:
                parents.append(cur.replace(".", ":", 1))
            out[key] = Owner(key, t["gtype"], bool(t.get("abstract")), t.get("kind") == "interface", parents,
                             [x.replace(".", ":", 1) for x in t.get("interfaces", [])], props, None, t.get("since"), "types")
    return out


def find_observe_probe(version: str) -> dict:
    p = OBS_PROBE / f"{version}.json"
    if not p.exists():
        raise SystemExit(f"infer: no observe probe for GTK {version} ({p.relative_to(ROOT)}); run "
                         "tools/metadata/gtk/probe.py --section observe (with --libdir for that GTK)")
    return json.loads(p.read_text())


class Rules:
    def __init__(self, doc: dict):
        o = doc.get("observe") or {}
        self.promote_at = float(doc["promote_at"])
        self.proportional = set(o.get("proportional", []))
        self.scenario_missed = dict(o.get("scenario_missed") or {})
        self.triggers = dict(o.get("triggers") or {})
        self.setup = dict(o.get("setup") or {})
        self.accept = list(o.get("accept") or [])
        self.constant = list(o.get("constant") or [])
        self.unreviewed = list(o.get("unreviewed") or [])
        self.subjects = dict(doc.get("subjects") or {})


def infer(probe_rt: dict, rules_doc: dict, version: str) -> tuple[dict[str, list[dict]], dict, list[dict], dict[str, int]]:
    """(owner -> records at promote_at or above, evidence by `owner:prop`, review rows, skipped counts)."""
    owners = load_owners()
    obs = find_observe_probe(version)
    rules = Rules(rules_doc)
    ev_props: dict = obs["props"]
    probed_types: dict = obs["types"]
    crashed = set(obs.get("crashed", []))

    def gir_dot(key: str) -> str:
        return key.replace(":", ".", 1)

    def instantiable(key: str) -> bool:
        """The probe built a bare instance of the type (g_object_new without properties) and it did not crash."""
        t = probed_types.get(gir_dot(key))
        return bool(t) and t["class"] == "object" and bool(t.get("instantiated")) and gir_dot(key) not in crashed

    # Concrete types by ancestor/interface, for subjects of abstract owners.
    subclasses: dict[str, list[str]] = {}
    for k, o in owners.items():
        if o.abstract or o.interface or not instantiable(k):
            continue
        for b in o.bases + o.interfaces:
            subclasses.setdefault(b, []).append(k)

    def subject(key: str, sweep_on: str | None) -> str | None:
        """The concrete type a test of `key`'s properties instantiates."""
        o = owners[key]
        if not o.abstract and not o.interface and instantiable(key):
            return key
        if sweep_on and instantiable(sweep_on.replace(".", ":", 1)):
            return sweep_on.replace(".", ":", 1)
        sub = rules.subjects.get(key.split(":", 1)[1]) if key.startswith("Gtk:") else None
        if sub and instantiable(f"Gtk:{sub}"):
            return f"Gtk:{sub}"
        cands = sorted(subclasses.get(key, []), key=lambda k: (owners[k].element is None, k))
        return cands[0] if cands else None

    # Recorded around the round trip of the AI1 probe: kind -> prop -> notify fired.
    ai1 = {}
    p = ROOT / f"metadata/probe/gtk/{version}.json"
    if p.exists():
        for kname, k in json.loads(p.read_text())["kinds"].items():
            for pn, pr in (k.get("props") or {}).items():
                if pr.get("outcome") in ("exact", "normalized"):
                    ai1[f"Gtk:{kname}:{pn}"] = bool(pr.get("notify"))

    out: dict[str, list[dict]] = {}
    evidence: dict[str, dict] = {}
    review: list[dict] = []
    skipped: dict[str, int] = {}

    def skip(why: str) -> None:
        skipped[why] = skipped.get(why, 0) + 1

    def flags_of(key: str, pr: dict) -> list[str] | None:
        if pr.get("flags"):
            return pr["flags"]
        e = ev_props.get(f"{gir_dot(key)}:{pr['name']}") or {}
        return e.get("flags")

    def scalar_writable(owner_key: str, name: str) -> bool:
        o = owners.get(owner_key)
        if o is None:
            return False
        for pr in o.props:
            if pr["name"] == name:
                gt = pr.get("gtype") or (ev_props.get(f"{gir_dot(owner_key)}:{name}") or {}).get("value_type")
                return pr["write"] and not pr["construct_only"] and (gt in SCALAR_GTYPES or pr["ref"].startswith("enum<"))
        return False

    def find_writable(key: str, name: str) -> str | None:
        """The owner (key or an ancestor/interface) declaring a writable scalar property `name`."""
        o = owners[key]
        for k in [key] + o.bases + o.interfaces:
            if k in owners and scalar_writable(k, name):
                return k
        return None

    def decide(key: str, pr: dict) -> tuple[dict | None, float, str, dict]:
        """(record or None for no fact, confidence, review pattern, evidence)."""
        name = pr["name"]
        flags = flags_of(key, pr) or []
        e = ev_props.get(f"{gir_dot(key)}:{name}") or {}
        evid = {k: e[k] for k in ("flags", "sweep", "seen") if k in e}
        fid = f"{key}:{name}"
        notify = {"mechanism": "notify", "signal": f"notify::{name}"}
        rec: dict = {"prop": name}
        if pr.get("virtual"):
            v = pr["virtual"]
            if v.get("op") == "via_object":
                return {**rec, "_via": v}, 0.0, "", evid
            rec.update({"class": "set_only", "provenance": [
                {"origin": "rule", "ref": f"O8: computed by svelters ({v.get('op')}), not a GObject property: only our setter changes it"}]})
            return rec, 0.9, "observe:virtual", evid
        sw = e.get("sweep")
        seen = e.get("seen") or []
        sub = subject(key, (sw or {}).get("on") or next((s["on"] for s in seen if "." in s["on"]), None))
        if pr["construct_only"] or "CONSTRUCT_ONLY" in flags:
            rec.update({"class": "constant", "provenance": [{"origin": "runtime" if flags else "gir",
                                                             "ref": "O2: CONSTRUCT_ONLY: set once at construction"}]})
            if sub:
                rec["verify"] = {"on": sub, "check": "refused"}
            return rec, 0.95, "observe:constant", evid
        ok = [s for s in seen if s["changed"] and s["notify"]]
        miss = [s for s in seen if s["changed"] and not s["notify"]]
        swept_change = bool(sw) and sw["readback"] != "ignored"
        if swept_change:
            (ok if sw["notify_change"] else miss).append({"cause": f"set:{gir_dot(key)}:{name}", "on": sw["on"], "notify": sw["notify_change"], "changed": True})
        ai1_notify = ai1.get(fid)
        if ai1_notify is True and not swept_change:
            ok.append({"cause": f"set:{gir_dot(key)}:{name}", "on": "round-trip probe", "notify": 1, "changed": True})
        if ai1_notify is not None:
            evid["round_trip_notify"] = ai1_notify
        rec["via"] = [notify]
        explicit = "EXPLICIT_NOTIFY" in flags
        if sw and sw["readback"] != "ignored":
            rec["fires"] = "on_set" if sw["notify_same"] else "on_change"
        elif flags and not explicit and pr["write"]:
            rec["fires"] = "on_set"
        derived = sorted({s["cause"].rsplit(":", 1)[1] for s in ok if s["cause"].startswith("set:") and s["cause"].rsplit(":", 1)[1] != name})
        if derived:
            rec["derived_from"] = derived
        if f"{key}:{name}" in rules.proportional:
            rec["value_cost"] = "proportional"
        verify: dict = {"on": sub} if sub else {}
        if miss:
            def cause_kind(s: dict) -> str:
                if s["cause"].startswith("scenario:"):
                    return rules.scenario_missed.get(s["cause"].split(":", 1)[1], "cross_prop")
                return "cross_prop"
            missed = sorted({cause_kind(s) for s in miss})
            causes = ", ".join(sorted({s["cause"] for s in miss})[:4])
            if ok:
                rec.update({"class": "partial", "missed": missed, "provenance": [
                    {"origin": "runtime" if flags else "gir", "ref": "O1: GObject property, notify::" + name},
                    {"origin": "probe", "ref": f"observe probe GTK {obs['version']}: notified on {len(ok)} change(s); "
                                               f"changed without notify on {causes}"}]})
            else:
                rec.pop("via")
                rec.pop("fires", None)
                rec.pop("derived_from", None)
                rec.update({"class": "none", "provenance": [
                    {"origin": "probe", "ref": f"observe probe GTK {obs['version']}: changed without notify on {causes}"}]})
            trig = next((s for s in miss if s["cause"].startswith("set:") and s["on"] and "." in s["on"]), None)
            if fid in rules.triggers:
                verify.update({"check": "silent", "rust": rules.triggers[fid]})
            elif trig:
                tk, tp = trig["cause"][4:].rsplit(":", 1)
                inst = trig["on"].replace(".", ":", 1)
                if instantiable(inst) and find_writable(inst, tp):
                    verify = {"on": inst, "check": "silent", "trigger": tp}
            if rec["class"] == "partial" and verify.get("check") != "silent":
                verify["check"] = "change" if pr["write"] and scalar_writable(key, name) else "connect"
            if verify.get("on") and verify.get("check"):
                rec["verify"] = verify
            return rec, 0.95, f"observe:{rec['class']}-probe", evid
        prov = [{"origin": "runtime" if flags else "gir", "ref": "O1: GObject property, notify::" + name
                 + (" (EXPLICIT_NOTIFY)" if explicit else "")}]
        if ok:
            how = ", ".join(sorted({s["cause"] for s in ok})[:3])
            prov.append({"origin": "probe", "ref": f"observe probe GTK {obs['version']}: notified on every observed change ({how})"})
            rec.update({"class": "exact", "provenance": prov})
            conf, pattern = 0.95, "observe:exact-probe"
        elif pr["write"]:
            how = ("the generated test sets it natively" if scalar_writable(key, name)
                   else "not a scalar: the generated test only installs the mechanism")
            rec.update({"class": "exact", "provenance": prov + [{"origin": "rule", "ref": f"no probe row; {how}"}]})
            conf, pattern = 0.9, "observe:exact-writable-rule"
        elif (con := next((a for a in rules.constant if re.search(a["pattern"], fid)), None)):
            # Read-only and fixed for the object's lifetime: no mechanism and no runnable test (the backend refuses
            # only CONSTRUCT_ONLY properties, so a `refused` row would not hold).
            for k in ("via", "fires", "derived_from"):
                rec.pop(k, None)
            rec.update({"class": "constant", "provenance": [
                {"origin": "runtime" if flags else "gir", "ref": "read-only, never seen changing in the probe"},
                {"origin": "manual", "ref": f"review pattern {con['pattern']}: {con['reason']}"}]})
            return rec, 0.9, "observe:constant-read-only", evid
        elif (acc := next((a for a in rules.accept if re.search(a["pattern"], fid)), None)):
            # `class: none`: a review decision that the change is not announced (the trigger checks it silently).
            cls = acc.get("class", "exact")
            rec.update({"class": cls, "provenance": prov + [{"origin": "manual", "ref": f"review pattern {acc['pattern']}: {acc['reason']}"}]})
            if cls == "none":
                for k in ("via", "fires", "derived_from"):
                    rec.pop(k, None)
                rec["provenance"] = rec["provenance"][1:]
            conf, pattern = 0.9, f"observe:{cls}-read-only-accepted"
        else:
            rec.update({"class": "exact", "provenance": prov + [{"origin": "rule", "ref": "read-only and never seen changing in the probe"}]})
            conf, pattern = 0.8, "observe:exact-read-only-unprobed"
        if verify.get("on"):
            if fid in rules.triggers:
                verify.update({"check": "silent" if rec["class"] == "none" else "cross", "rust": rules.triggers[fid]})
            elif pr["write"] and scalar_writable(key, name) and find_writable(verify["on"], name) and \
                    not (sw and sw["readback"] == "ignored" and verify["on"] not in rules.setup):
                # (a write the probe saw ignored needs a precondition: only with a setup from the rules)
                verify["check"] = "change"
            else:
                cross = next((s for s in ok if s["cause"].startswith("set:") and "." in s["on"]), None)
                inst = cross["on"].replace(".", ":", 1) if cross else None
                tp = cross["cause"].rsplit(":", 1)[1] if cross else None
                if cross and instantiable(inst) and find_writable(inst, tp):
                    verify = {"on": inst, "check": "cross", "trigger": tp}
                else:
                    verify["check"] = "connect"
            rec["verify"] = verify
        return rec, conf, pattern, evid

    decided: dict[str, dict] = {}
    pending_via: list[tuple[str, dict, dict]] = []
    for key, o in sorted(owners.items()):
        for pr in sorted(o.props, key=lambda p: p["name"]):
            if not pr["read"]:
                skip("observe:write-only")
                continue
            rec, conf, pattern, evid = decide(key, pr)
            if rec is None:
                continue
            if "_via" in rec:
                pending_via.append((key, pr, rec))
                continue
            decided[f"{key}:{pr['name']}"] = rec
            evidence[f"{key}:{pr['name']}"] = {"confidence": conf, "pattern": pattern, "evidence": evid}
            if conf >= rules.promote_at:
                out.setdefault(key, []).append(rec)
            else:
                row = {"owner": key, "prop": pr["name"], "class": rec["class"], "confidence": conf,
                       "band": "middle" if conf >= 0.6 else "low", "pattern": pattern}
                grp = next((u for u in rules.unreviewed if re.search(u["pattern"], f"{key}:{pr['name']}")), None)
                row["group"] = grp["group"] if grp else "unclassified"
                if grp:
                    row["reason"] = grp["reason"]
                review.append(row)
    # O9: forwarded properties take the class of the property they forward to.
    for key, pr, rec in pending_via:
        v = rec.pop("_via")
        obj_prop = next((p for p in owners[key].props if p["name"] == v["via"]), None)
        tgt_owner = gir_owner(obj_prop["ref"][len("object<"):-1]) if obj_prop and obj_prop["ref"].startswith("object<") else None
        tgt = decided.get(f"{tgt_owner}:{v['property']}") if tgt_owner else None
        fid = f"{key}:{pr['name']}"
        if tgt is None or tgt["class"] not in ("exact", "partial"):
            review.append({"owner": key, "prop": pr["name"], "class": "?", "confidence": 0.5, "band": "low",
                           "pattern": "observe:via-target-unknown"})
            continue
        r = {"prop": pr["name"], "class": tgt["class"], "via": [{"mechanism": "via", "object": v["via"], "prop": v["property"]}]}
        for k in ("missed", "fires", "value_cost"):
            if k in tgt:
                r[k] = tgt[k]
        if fid in rules.proportional:
            r["value_cost"] = "proportional"
        r["provenance"] = [{"origin": "manual", "ref": f"forwarded to {v['via']}.{v['property']} (metadata virtual.via)"},
                           {"origin": "rule", "ref": f"O9: observed on the {tgt_owner} behind `{v['via']}`, class of {tgt_owner}:{v['property']}"}]
        on = key if owners[key].element and not owners[key].abstract else subject(key, None)
        if on:
            r["verify"] = {"on": on, "check": "via"}
        out.setdefault(key, []).append(r)
        evidence[fid] = {"confidence": 0.95, "pattern": "observe:via", "evidence": {"target": f"{tgt_owner}:{v['property']}"}}
    for k in out:
        out[k].sort(key=lambda r: r["prop"])
        for r in out[k]:
            v = r.get("verify")
            if v and v["check"] not in ("connect", "refused") and v["on"] in rules.setup:
                v["setup"] = rules.setup[v["on"]]
    return out, evidence, review, skipped
