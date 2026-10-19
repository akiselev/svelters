"""Unit tests of the WinUI inference rules (tools/metadata/winui/infer.py), the layer merge (winui_layers.py) and the
ledger import of CI results (tools/metadata/ledger.py import)."""
import importlib.util
import json
import sys
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "common"))
sys.path.insert(0, str(HERE.parent))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


infer = load("svelters_winui_infer_t", HERE / "infer.py")
layers = load("svelters_winui_layers_t", HERE / "winui_layers.py")
ledger = load("svelters_ledger_t", HERE.parent / "ledger.py")

TYPES = {"X.Mode": {"category": "enum", "members": [["First", 0], ["Second", 1]]},
         "X.One": {"category": "enum", "members": [["Only", 0]]}}


def prop(name, ref, write=True, dp=True):
    return {"name": name, "type": {"ref": ref}, "access": {"read": True, "write": write}, "dependency_property": dp,
            "interface": "X.IThing", "origin": "winmd"}


def event(name, args="X.Args", settable=()):
    return {"name": name, "delegate": f"Windows.Foundation.TypedEventHandler<X.Thing, {args}>", "args": args, "sender": "X.Thing",
            "args_props": [{"name": n, "type": {"ref": "bool"}, "settable": True, "declared_on": d} for n, d in settable],
            "args_methods": [], "interface": "X.IThing", "origin": "winmd"}


def method(name, params=(), ret="void"):
    return {"name": name, "params": [{"name": f"p{i}", "type": {"ref": t}} for i, t in enumerate(params)],
            "returns": {"ref": ret, "native": ret}, "interface": "X.IThing", "origin": "winmd"}


def klass(short="Thing", props=(), signals=(), methods=()):
    dps = [f"{p['name']}Property" for p in props if p["dependency_property"]]
    return {"native": {"name": f"Microsoft.UI.Xaml.{short}", "short": short}, "properties": list(props), "signals": list(signals),
            "methods": list(methods), "dependency_properties": dps}


def rules(**over):
    doc = {"promote_at": 0.9,
           "samples": {"by_name": [{"pattern": "^Opacity$", "values": [0.5, 0.25]}], "bool": [True, False], "i32": [3, 7],
                       "f64": [12.0, 24.0], "string": ["a", "b"], "thickness": ["1,2,3,4", "5"], "nullable_bool": [True, False]},
           "deny_props": [{"pattern": r"^Thing\.Tag$", "reason": "object plumbing"}],
           "no_notify": [{"prop": "Thing.ActualWidth", "use": "SizeChanged", "reason": "never notifies"},
                         {"prop": "Thing.Silent", "use": "Changed", "reason": "never notifies"}],
           "deny_methods": [{"pattern": r"\.Paste$", "reason": "clipboard"}],
           "deny_events": [], "result_props": ["Cancel", "Handled"],
           "triggers": [{"pattern": r"^Thing\.Loaded$", "trigger": "mount"}]}
    doc.update(over)
    return infer.Rules(doc)


def run(k, element=True):
    kl = infer.Klass(k, TYPES, rules(), {k["native"]["name"]} if element else set())
    kl.infer_props()
    kl.infer_notify()
    kl.infer_bindings()
    kl.infer_events()
    kl.infer_methods()
    return kl


def names(kl, sec):
    return [r["name"] for r in kl.ov.get(sec, [])]


def test_writable_dp_gets_samples_a_notify_event_and_a_generic_binding():
    kl = run(klass(props=[prop("IsOpen", "bool"), prop("Mode", "enum<X.Mode>"), prop("Opacity", "f64")]))
    recs = {r["name"]: r for r in kl.ov["props"]}
    assert recs["IsOpen"]["test"] == [True, False]
    assert recs["Mode"]["test"] == ["Second", "First"]             # the second member, then the first
    assert recs["Opacity"]["test"] == [0.5, 0.25]                   # by name before by type
    assert names(kl, "notify") == ["IsOpen", "Mode", "Opacity"]
    b = {r["name"]: r for r in kl.ov["bindings"]}
    assert b["is-open"]["change"] == {"signal": "notify::is-open", "role": "any_change", "value": "prop"}
    assert b["is-open"]["echo"]["read_back"] is False and b["opacity"]["echo"]["read_back"] is True
    assert b["mode"]["type"] == {"ref": "enum<X.Mode>", "nullable": False}


def test_dps_that_never_notify_and_non_dps_get_no_notify_or_binding():
    kl = run(klass(props=[prop("ActualWidth", "f64", write=False), prop("Silent", "bool"), prop("Plain", "bool", dp=False)]))
    assert names(kl, "props") == ["ActualWidth", "Silent", "Plain"]
    assert "test" not in kl.ov["props"][0]                          # read-only: a read test
    assert "notify" not in kl.ov and "bindings" not in kl.ov
    assert kl.skipped["notify:no-notify-rule"] == 2


def test_bindings_only_on_element_classes_and_unconvertible_or_denied_props_are_skipped():
    kl = run(klass(props=[prop("IsOpen", "bool"), prop("Brush", "object<X.Brush>"), prop("Tag", "variant"),
                          prop("Only", "enum<X.One>")]), element=False)
    assert names(kl, "props") == ["IsOpen"] and names(kl, "notify") == ["IsOpen"] and "bindings" not in kl.ov
    assert kl.skipped["prop:type:object"] == 1 and kl.skipped["prop:denied"] == 1 and kl.skipped["prop:no-sample"] == 1


def test_object_props_as_strings_wait_for_review():
    kl = run(klass(props=[prop("Header", "variant")]))
    assert "props" not in kl.ov
    assert [(r["band"], r["pattern"]) for r in kl.review] == [("middle", "prop:object-as-string")]


def test_events_sync_on_their_own_settable_cancel_or_handled_and_take_rule_triggers():
    kl = run(klass(signals=[event("Closing", settable=[("Handled", "X.Args"), ("Cancel", "X.Args")]),
                            event("Moved", args="X.Derived", settable=[("Handled", "X.Base")]), event("Loaded")]))
    ev = {r["name"]: r for r in kl.ov["events"]}
    assert ev["Closing"]["sync"] and ev["Closing"]["result"] == {"prop": "Cancel"}   # Cancel before Handled
    assert "sync" not in ev["Moved"]                                # Handled declared on a base args class
    assert ev["Loaded"]["trigger"] == "mount" and "trigger" not in ev["Closing"]


def test_methods_zero_arg_inferred_args_reviewed_denied_and_overloads_skipped():
    kl = run(klass(methods=[method("Undo"), method("CanGo", ret="bool"), method("Paste"), method("Select", ["i32", "i32"]),
                            method("Bring"), method("Bring", ["object<X.Options>"]), method("Rect", ret="value<X.Rect>")]))
    assert names(kl, "methods") == ["Undo", "CanGo"]
    assert [r["name"] for r in kl.review] == ["Select"]
    assert kl.skipped == {"method:denied": 1, "method:overloaded": 2, "method:return:value": 1}


def write_yaml(path, doc):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(yaml.safe_dump(doc))


def test_layers_manual_wins_by_name_deny_drops_and_parked_facts_are_left_out(tmp_path):
    cls = "Microsoft.UI.Xaml.Controls.Thing"
    write_yaml(tmp_path / "ov/Thing.yaml", {"class": cls, "element": {}, "deny": ["events:Pointer*"],
                                            "props": [{"name": "Text", "test": "x", "provenance": "manual"}]})
    write_yaml(tmp_path / "inf/Thing.yaml", {"class": cls, "props": [{"name": "Text", "test": ["a", "b"], "provenance": "rule"},
                                                                    {"name": "Width", "test": [1.0, 2.0], "provenance": "rule"},
                                                                    {"name": "Height", "test": [1.0, 2.0], "provenance": "rule"}],
                                             "events": [{"name": "PointerMoved", "provenance": "rule"}, {"name": "Loaded", "provenance": "rule"}]})
    (tmp_path / "inf/_parked.json").write_text(json.dumps({"parked": [{"kind": "Thing", "section": "props", "name": "Height"}]}))
    lay = layers.Layers(tmp_path / "ov", tmp_path / "inf")
    m = lay.merged[cls]
    assert [(p["name"], p["test"]) for p in m["props"]] == [("Text", "x"), ("Width", [1.0, 2.0])]
    assert [e["name"] for e in m["events"]] == ["Loaded"] and "deny" not in m and m["element"] == {}
    assert lay.origin(cls, "props", "Text") == "manual" and lay.origin(cls, "props", "Width") == "inferred"
    assert lay.record_fp(cls, "props", "Width") and lay.record_fp(cls, "props", "Text") is None
    assert lay.denied == [("Thing", "events", "PointerMoved")]


def test_ci_results_a_test_without_a_result_is_a_crash(tmp_path):
    run_dir = tmp_path / "artifact/logs/winui-ledger"
    run_dir.mkdir(parents=True)
    lines = [{"test": "a", "result": "start", "message": ""}, {"test": "a", "result": "pass", "message": ""},
             {"test": "b", "result": "start", "message": ""}, {"test": "b", "result": "fail", "message": "boom"},
             {"test": "c", "result": "start", "message": ""}]
    (run_dir / "winui-results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))
    (run_dir / "facts.json").write_text(json.dumps({"version": "2.3.9", "facts": []}))
    res, msgs, snap = ledger.read_ci_results(tmp_path / "artifact")
    assert res == {"a": "pass", "b": "fail", "c": "crash"}
    assert msgs["b"] == "boom" and "inside this test" in msgs["c"] and "a" not in msgs
    assert snap["version"] == "2.3.9"


def test_import_records_tested_facts_parks_failing_inferred_ones_and_leaves_untested_pending(tmp_path, monkeypatch):
    def fact(name, origin, test, fp="f"):
        return {"fact": f"id:{name}", "kind": "Thing", "section": "props", "name": name, "origin": origin, "tests": [test],
                "fingerprint": fp, **({"record_fp": f"r-{name}"} if origin == "inferred" else {})}
    now = {"toolkit": "winui", "version": "2.3.9", "untestable": [],
           "facts": [fact("a", "manual", "ta"), fact("b", "inferred", "tb"), fact("c", "inferred", "tc"),
                     fact("d", "inferred", "td", fp="new")]}
    ran = {**now, "facts": [*now["facts"][:3], fact("d", "inferred", "td", fp="old")]}
    art = tmp_path / "art"
    art.mkdir()
    (art / "facts.json").write_text(json.dumps(ran))
    # c started but the process died; d ran with an older fingerprint
    lines = [("ta", "pass"), ("tb", "fail"), ("td", "fail"), ("tc", "start")]
    (art / "results.jsonl").write_text("".join(json.dumps({"test": t, "result": r, "message": "m"}) + "\n" for t, r in lines))
    after_park = {**now, "facts": [f for f in now["facts"] if f["name"] not in ("b", "c")]}
    state = {"man": now}
    monkeypatch.setattr(ledger, "manifest", lambda tk: state["man"])
    monkeypatch.setattr(ledger, "regenerate", lambda tk: state.update(man=after_park))
    monkeypatch.setattr(ledger, "LEDGER_DIR", tmp_path / "verified")
    monkeypatch.setitem(ledger.PARKED, "winui", tmp_path / "_parked.json")
    assert ledger.import_ci("winui", art) == 0
    parked = json.loads((tmp_path / "_parked.json").read_text())["parked"]
    assert sorted((r["name"], r["test"]) for r in parked) == [("b", "tb"), ("c", "tc")]
    led = ledger.load("winui")
    assert [(e["fact"], e["result"]) for e in led["entries"]] == [("id:a", "pass")]   # d: stale in the run, pending
    assert led["version"] == "2.3.9"
