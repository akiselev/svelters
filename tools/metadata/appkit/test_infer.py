"""Tests of the AppKit inference pipeline: infer.py rules, layers.py precedence, probe.py plan and crash isolation."""
import json
import sys
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import infer  # noqa: E402
import layers  # noqa: E402
import probe  # noqa: E402

RULES = yaml.safe_load((HERE.parents[2] / "metadata/rules/appkit.yaml").read_text())


def prop(name, ref, write=True, js=None):
    return {"name": name, "js_name": js or name, "type": {"ref": ref, "native": ref}, "access": {"read": True, "write": write},
            "memory": "assign"}


META = {
    "NSView": {"bases": ["NSResponder", "NSObject"], "properties": [prop("hidden", "bool")], "methods": []},
    "NSControl": {"bases": ["NSView", "NSResponder", "NSObject"],
                  "properties": [prop("int-value", "i32"), prop("double-value", "f64"), prop("string-value", "string"),
                                 prop("integer-value", "u64")], "methods": []},
    "NSSlider": {"bases": ["NSControl", "NSView", "NSResponder", "NSObject"], "properties": [prop("vertical", "bool")], "methods": []},
    "NSTextField": {"bases": ["NSControl", "NSView", "NSResponder", "NSObject"], "properties": [prop("editable", "bool")], "methods": []},
    "NSSecureTextField": {"bases": ["NSTextField", "NSControl", "NSView", "NSResponder", "NSObject"], "properties": [], "methods": []},
}
MANUAL = {
    "NSView": {"class": "NSView", "element": {"tag": "view"}},
    "NSControl": {"class": "NSControl", "signals": [{"name": "action", "via": "target_action"}]},
    "NSSlider": {"class": "NSSlider", "element": {"tag": "slider"}},
    "NSTextField": {"class": "NSTextField", "element": {"tag": "text-field"},
                    "signals": [{"name": "control-text-did-change", "via": "notification", "notification": "NSControlTextDidChangeNotification"}]},
    "NSSecureTextField": {"class": "NSSecureTextField", "element": {"tag": "secure-text-field"}},
}


def exact(sample, kvo=True, events=()):
    return {"outcome": "exact", "sample": sample, "kvo": kvo, "events": list(events), "default": None, "read": sample["v"]}


def probe_doc():
    return {"classes": {
        "NSView": {"subject": "view", "props": {"hidden": exact({"t": "bool", "v": True})},
                   "notifications": {"NSViewFrameDidChangeNotification": {"symbol": "NSViewFrameDidChangeNotification"},
                                     "NSViewBoundsDidChangeNotification": {"symbol": None}}},
        "NSSlider": {"subject": "slider", "signals": {"action": {"delivered": True}},
                     "props": {"vertical": {"outcome": "ignored", "sample": {"t": "bool", "v": True}},
                               "int-value": exact({"t": "i64", "v": 1}),
                               "double-value": exact({"t": "f64", "v": 0.5}, events=["action"]),
                               "string-value": {"outcome": "normalized", "sample": {"t": "str", "v": "svl probe"}}}},
        "NSTextField": {"subject": "text-field", "signals": {"control-text-did-change": {"delivered": True}},
                        "props": {"int-value": exact({"t": "i64", "v": 3}, kvo=False),
                                  "editable": exact({"t": "bool", "v": False})}},
        "NSSecureTextField": {"subject": "secure-text-field", "props": {"int-value": exact({"t": "i64", "v": 3})}},
    }}


def run(probe=None):
    inp = infer.Inputs(probe=probe if probe is not None else probe_doc(), rules=RULES, meta=META, manual=MANUAL)
    files, evid, summary = infer.run(inp)
    return {k[:-5]: yaml.safe_load(v) for k, v in files.items()}, evid, summary


def test_exact_round_trips_become_props_with_the_probe_sample():
    docs, evid, summary = run()
    assert docs["NSView"]["props"] == [{"name": "hidden", "sample": True, "provenance": docs["NSView"]["props"][0]["provenance"]}]
    assert "KVO fired" in docs["NSView"]["props"][0]["provenance"]
    tf = {p["name"]: p for p in docs["NSTextField"]["props"]}
    assert tf["int-value"]["sample"] == 3 and "no KVO" in tf["int-value"]["provenance"]
    assert json.loads(evid["NSTextField.evidence.json"])["props:int-value"]["confidence"] == 0.9
    held = {(r["kind"], r["name"]): r["pattern"] for r in summary["rows"]}
    assert held[("NSSlider", "vertical")] == "prop:ignored"
    assert held[("NSSlider", "string-value")] == "prop:normalized"


def test_bindings_use_target_action_or_the_text_notification_never_kvo():
    docs, _, summary = run()
    slider = {b["name"]: b for b in docs["NSSlider"]["bindings"]}
    assert set(slider) == {"int-value"}                       # double-value echoed the action; string-value normalized
    assert slider["int-value"]["change"] == {"signal": "action", "role": "user_edit", "value": "prop"}
    assert slider["int-value"]["observability"] == {"class": "partial", "missed": ["programmatic"]}
    text = {b["name"]: b for b in docs["NSTextField"]["bindings"]}
    assert text["int-value"]["change"]["signal"] == "control-text-did-change"
    # the secure field inherits NSTextField's bindings instead of redefining them
    assert "bindings" not in docs.get("NSSecureTextField", {})
    held = {(r["kind"], r["name"]): r["pattern"] for r in summary["rows"] if r["section"] == "bindings"}
    assert held[("NSSlider", "double-value")] == "binding:echo"
    assert summary["skipped"]["binding:type-u64"] >= 1        # the renderer has no u64 binding type


def test_notification_signals_need_the_constant_found_by_the_probe():
    docs, evid, summary = run()
    sig = docs["NSView"]["signals"][0]
    assert sig == {"name": "view-frame-did-change", "via": "notification", "notification": "NSViewFrameDidChangeNotification",
                   "verify": "post", "provenance": sig["provenance"]}
    held = {(r["kind"], r["name"]): r["pattern"] for r in summary["rows"] if r["section"] == "signals"}
    assert held[("NSView", "view-bounds-did-change")] == "signal:constant-missing"


def test_without_a_probe_everything_is_held_and_the_run_is_deterministic():
    docs, _, summary = run(probe={})
    assert docs == {} and summary["unreviewed"]["middle"] > 0
    assert run() == run()


def test_inferred_layer_validates_against_the_override_schema():
    inp = infer.Inputs(probe=probe_doc(), rules=RULES, meta=META, manual=MANUAL)
    files, _, _ = infer.run(inp)
    assert infer.validate(files) == []


def test_layers_manual_wins_deny_and_parked(tmp_path):
    import svmeta
    inferred = {"class": "NSSlider", "props": [{"name": "a", "sample": 1, "provenance": "inf"}, {"name": "b", "sample": 2, "provenance": "inf"},
                                               {"name": "c", "sample": 3, "provenance": "inf"}],
                "bindings": [{"name": "x"}]}
    manual = {"class": "NSSlider", "element": {"tag": "slider"}, "props": [{"name": "b", "sample": 9, "provenance": "man"}],
              "deny": ["bindings:*"]}
    parked = {("NSSlider", "props", "c", svmeta.fingerprint(inferred["props"][2]))}
    origin = {}
    out = layers.merge(manual, inferred, "NSSlider", parked, origin)
    assert [(p["name"], p["sample"]) for p in out["props"]] == [("b", 9), ("a", 1)]
    assert "bindings" not in out and out["element"] == {"tag": "slider"}
    assert origin[("NSSlider", "props", "b")] == ("manual", None)
    assert origin[("NSSlider", "props", "a")][0] == "inferred"
    # a parked fact whose record changed is merged again (and retried)
    changed = dict(inferred, props=inferred["props"][:2] + [{"name": "c", "sample": 4, "provenance": "inf"}])
    assert [p["name"] for p in layers.merge(manual, changed, "NSSlider", parked, {})["props"]] == ["b", "a", "c"]


def test_probe_crash_isolation_marks_the_prop_in_flight_and_resumes_after_it():
    c = {"class": "NSWindow", "subject": "window", "props": [{"name": "a"}, {"name": "b"}, {"name": "c"}]}
    calls = []

    def runner(binary, plan, cls, after, timeout, skip_class=False):
        calls.append(after)
        if after is None:
            return [{"class": cls, "created": True, "native_class": "NSWindow", "signals": {}, "notifications": {}},
                    {"class": cls, "prop": "a", "start": True}, {"class": cls, "prop": "a", "result": {"outcome": "exact"}},
                    {"class": cls, "prop": "b", "start": True}], "signal 11: Segmentation fault at /Users/x/y"
        return [{"class": cls, "prop": "c", "start": True}, {"class": cls, "prop": "c", "result": {"outcome": "ignored"}}, {"done": True}], None

    rec = probe.probe_class(Path("bin"), Path("plan"), c, 5, runner=runner)
    assert calls == [None, "b"]
    assert rec["props"]["a"]["outcome"] == "exact" and rec["props"]["c"]["outcome"] == "ignored"
    assert rec["props"]["b"]["outcome"] == "crash" and "<path>" in rec["props"]["b"]["error"]
    assert rec["created"] and "crash" not in rec


def test_probe_records_a_class_level_crash_and_still_probes_the_props():
    c = {"class": "NSBox", "subject": "box", "props": [{"name": "a"}]}

    def runner(binary, plan, cls, after, timeout, skip_class=False):
        if not skip_class:
            return [], "signal 6"
        return [{"class": cls, "prop": "a", "start": True}, {"class": cls, "prop": "a", "result": {"outcome": "exact"}}, {"done": True}], None

    rec = probe.probe_class(Path("bin"), Path("plan"), c, 5, runner=runner)
    assert rec["crash"] == "signal 6" and rec["props"]["a"]["outcome"] == "exact"


def test_probe_plan_covers_writable_probeable_props_and_binding_props():
    import gen_backend
    plan = probe.build_plan(gen_backend.Model(), RULES)
    by = {c["class"]: c for c in plan["classes"]}
    slider = {p["name"]: p for p in by["NSSlider"]["props"]}
    assert slider["int-value"]["inherited"] and slider["int-value"]["samples"][0] == {"t": "i64", "v": 3}
    assert "action" in by["NSSlider"]["listen"]
    assert by["NSControl"]["subject"] == "button"
    window = {p["name"] for p in by["NSWindow"]["props"]}
    assert "level" not in window and "style-mask" not in window          # rules deny_props
    flags = [p for c in plan["classes"] for p in c["props"] if p["samples"][0]["t"] == "names"]
    assert all("expect" in s for p in flags for s in p["samples"])
