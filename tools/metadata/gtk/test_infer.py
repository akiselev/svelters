"""Unit tests of the GTK inference rules (tools/metadata/gtk/infer.py) and of the inferred layer's merge."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[0] / "common"))
import infer  # noqa: E402
import ingest  # noqa: E402
import svmeta  # noqa: E402


def prop(name, ref, write=True, **kw):
    return {"name": name, "origin": "gir", "type": {"ref": ref}, "access": {"read": True, "write": write, "construct_only": False},
            "range": None, **kw}


def meth(name, params=(), ret="void", **kw):
    return {"name": name, "origin": "gir", "native": f"gtk_x_{name}", "params": [{"name": f"p{i}", "type": t, "native": t} for i, t in enumerate(params)],
            "returns": {"type": ret, "native": ret}, "unsupported_reason": None, **kw}


def sig(name, **kw):
    return {"name": name, "origin": "gir", "js_event": name, **kw}


def kind(props=(), methods=(), signals=(), **kw):
    return {"id": "svelters-api:v1:type:gtk:Gtk:X", "kind": "object", "abstract": False, "bases": ["Gtk.Widget"], "interfaces": [],
            "native": {"gtype": "GtkX"}, "properties": list(props), "methods": list(methods), "signals": list(signals),
            "element": {"tag": "x"}, **kw}


def run(doc, probe):
    k = infer.Kind("X", probe, doc, {"X": doc}, infer.Rules())
    k.infer_props()
    k.infer_methods()
    k.infer_signals()
    k.infer_bindings()
    return k


def test_exact_prop_gets_the_probe_sample_and_a_generic_binding():
    k = run(kind([prop("wrap-mode", "enum<svelters-api:v1:type:gtk:Gtk:WrapMode>")]),
            {"props": {"wrap-mode": {"outcome": "exact", "notify": True, "sample": "word", "default": "none", "signals": []}}})
    (rec,) = k.ov["properties"]
    assert rec["sample"] == "word" and rec["provenance"][0]["origin"] == "probe"
    assert k.ov["bindings"][0]["change"]["signal"] == "notify::wrap-mode"
    assert k.ov["signals"][0]["derived_from"] == {"property": "wrap-mode"}
    assert k.ov["signals"][0]["trigger"]["values"] == ["word", "none", "word"]


def test_ignored_prop_is_held_for_review_not_written():
    k = run(kind([prop("a", "bool")]), {"props": {"a": {"outcome": "ignored"}}})
    assert "properties" not in k.ov
    assert [r["pattern"] for r in k.review] == ["prop:ignored"]


def test_recipe_without_a_rust_twin_is_low_band():
    k = run(kind([prop("page", "i32")]), {"props": {"page": {"outcome": "exact", "sample": 1, "default": 0, "recipe": "populate", "notify": True}}})
    assert "properties" not in k.ov and k.review[0]["band"] == "low"


def test_range_recipe_becomes_a_setup_when_the_kind_has_set_range():
    doc = kind([prop("value", "f64")], [meth("set_range", ["f64", "f64"])])
    k = run(doc, {"props": {"value": {"outcome": "exact", "sample": 1.0, "default": 0.0, "recipe": "range", "notify": True}}})
    assert "set_range" in k.ov["properties"][0]["setup"]


def test_accessor_methods_scalar_only():
    doc = kind([prop("label", "string"), prop("child", "object<svelters-api:v1:type:gtk:Gtk:Widget>")],
               [meth("get_label", [], "string"), meth("set_label", ["string"]),
                meth("get_child", [], "object<svelters-api:v1:type:gtk:Gtk:Widget>"), meth("set_child", ["object<x>"])])
    probe = {"methods": {
        "get_label": {"class": "accessor_get", "prop": "label", "outcome": "exact"},
        "set_label": {"class": "accessor_set", "prop": "label", "outcome": "exact", "sample": "svl probe"},
        "get_child": {"class": "accessor_get", "prop": "child", "outcome": "exact"},
        "set_child": {"class": "accessor_set", "prop": "child", "outcome": "no_sample"}}}
    k = run(doc, probe)
    names = {m["name"]: m for m in k.ov["methods"]}
    assert set(names) == {"get_label", "set_label"}
    assert names["get_label"]["expect"] == 'r == h.get(o, "label")'
    assert names["set_label"]["expect"] == 'h.get(o, "label") == V::Str("svl probe".into())'
    assert names["set_label"]["dispatch"] == "dynamic" and names["set_label"]["args"] == ["svl probe"]


def test_enum_setter_sample_is_a_nick_not_a_number():
    doc = kind([prop("ellipsize", "enum<svelters-api:v1:type:gtk:Pango:EllipsizeMode>")],
               [meth("set_ellipsize", ["enum<svelters-api:v1:type:gtk:Pango:EllipsizeMode>"])])
    k = run(doc, {"methods": {"set_ellipsize": {"class": "accessor_set", "prop": "ellipsize", "outcome": "exact", "sample": "middle"}}})
    assert k.ov["methods"][0]["expect"] == 'h.get(o, "ellipsize") == V::Str("middle".into())'
    k = run(doc, {"methods": {"set_ellipsize": {"class": "accessor_set", "prop": "ellipsize", "outcome": "exact", "sample": 3}}})
    assert "methods" not in k.ov          # a number where the harness compares nicks: held, not drafted


def test_pure_getter_snapshot_needs_a_stable_value():
    doc = kind(methods=[meth("get_n_pages", [], "i32"), meth("get_id", [], "u32")])
    probe = {"methods": {"get_n_pages": {"class": "pure_getter", "outcome": "ok", "ret": 0, "stable": True},
                         "get_id": {"class": "pure_getter", "outcome": "ok", "ret": 3, "stable": False}}}
    k = run(doc, probe)
    (rec,) = k.ov["methods"]
    assert rec["name"] == "get_n_pages" and rec["expect"] == "r == V::I64(0)"


def test_object_and_gunichar_accessors_are_skipped():
    doc = kind([prop("invisible-char", "u32")], [meth("get_invisible_char", [], "u32", returns={"type": "u32", "native": "gunichar"})])
    k = run(doc, {"methods": {"get_invisible_char": {"class": "accessor_get", "prop": "invisible-char", "outcome": "exact"}}})
    assert "methods" not in k.ov


def test_denied_method_pattern_is_counted_not_written():
    doc = kind(methods=[meth("grab_focus")])
    k = run(doc, {"methods": {"grab_focus": {"class": "action", "outcome": "ok", "changed": {"has-focus": [False, True]}}}})
    assert "methods" not in k.ov and any(p.startswith("method:denied") for p in k.skipped)


def test_signal_triggers():
    doc = kind([prop("active", "bool")], signals=[sig("toggled"), sig("activate"), sig("state-set"), sig("move", params=[1])])
    probe = {"props": {"active": {"outcome": "exact", "notify": True, "sample": True, "default": False, "signals": ["toggled"]}},
             "signals": {
                 "toggled": {"returns": "void", "handler_result": "none", "sync_required": False, "run": "last", "params": []},
                 "activate": {"returns": "void", "handler_result": "none", "sync_required": False, "run": "last", "params": [], "emit": "delivered"},
                 "state-set": {"returns": "gboolean", "handler_result": "bool_stop", "sync_required": True, "run": "last", "params": ["gboolean"],
                               "synthetic": {"args": [True], "emit": "delivered"}},
                 "move": {"returns": "void", "handler_result": "none", "sync_required": False, "run": "last", "params": ["GObject"]}}}
    k = run(doc, probe)
    by = {s["name"]: s for s in k.ov["signals"] if not s.get("derived_from")}
    assert by["toggled"]["trigger"] == {"property": "active", "values": [True, False, True]}
    assert "trigger" not in by["activate"]
    assert by["state-set"]["delivery_only"] is True and "emit_by_name::<bool>" in by["state-set"]["trigger"]["rust"]
    assert "move" not in by and k.skipped.get("signal:no-trigger") == 1


def test_inferred_output_validates_against_the_override_schema(tmp_path):
    doc = kind([prop("label", "string")], [meth("get_label", [], "string")])
    k = run(doc, {"props": {"label": {"outcome": "exact", "notify": True, "sample": "x", "default": "", "signals": []}},
                  "methods": {"get_label": {"class": "accessor_get", "prop": "label", "outcome": "exact"}}})
    import yaml
    (tmp_path / "X.yaml").write_text(yaml.safe_dump({key: k.ov[key] for key in k.ov}, sort_keys=False))
    assert svmeta.check_override(tmp_path / "X.yaml", infer.SCHEMA) == []


# ---- the merge layers -------------------------------------------------------------------------------------------

def base_doc():
    return {"id": "svelters-api:v1:type:gtk:Gtk:X", "properties": [{"name": "label", "origin": "gir"}], "signals": [], "methods": [],
            "children": {"policy": "unknown", "default_slot": None, "slots": []}}


def test_manual_wins_over_inferred_over_ingested():
    errors = []
    inferred = {"properties": [{"name": "label", "sample": "a", "provenance": "probe"}],
                "bindings": [{"name": "label", "direction": "two_way", "property": "label", "type": {"ref": "string"},
                              "change": {"signal": "notify::label"}, "echo": {"strategy": "none"}, "lifetime": {}, "provenance": "rule"}]}
    manual = {"properties": [{"name": "label", "sample": "b", "provenance": "docs"}],
              "bindings": [{"name": "label", "direction": "read_only", "property": "label", "provenance": "docs"}]}
    doc = ingest.apply_override(base_doc(), inferred, errors, "inferred")
    assert doc["properties"][0]["annotated"]["origin"] == "inferred" and doc["properties"][0]["annotated"]["fp"]
    assert doc["bindings"][0]["origin"] == "inferred" and doc["bindings"][0]["fp"]
    doc = ingest.apply_override(doc, manual, errors)
    assert errors == []
    assert doc["properties"][0]["sample"] == "b" and doc["properties"][0]["annotated"]["origin"] == "manual"
    assert doc["bindings"][0]["direction"] == "read_only" and doc["bindings"][0]["origin"] == "manual"
    assert "fp" not in doc["bindings"][0]
    assert doc["bindings"][0]["property"] == "label"          # fields the manual record does not set survive


def test_inferred_layer_may_not_write_exposure():
    errors = []
    ingest.apply_override(base_doc(), {"unexposed": {"reason": "no"}}, errors, "inferred")
    assert any("inferred layer may not write" in e for e in errors)


def test_inferred_record_must_match_an_ingested_one():
    errors = []
    ingest.apply_override(base_doc(), {"properties": [{"name": "nope", "provenance": "probe"}]}, errors, "inferred")
    assert any("matches no ingested record" in e for e in errors)


def test_parked_and_denied_facts_leave_the_layer():
    rec = {"name": "set_label", "dispatch": "dynamic", "provenance": "probe"}
    other = {"name": "get_label", "dispatch": "dynamic", "provenance": "probe"}
    ov = {"methods": [rec, other],
          "bindings": [{"name": "x", "provenance": "rule"}],
          "signals": [{"name": "notify::x", "derived_from": {"property": "x"}, "provenance": "rule"}]}
    parked = {("X", "methods", "set_label"): svmeta.fingerprint(rec)}
    out = ingest.drop_parked_and_denied("X", ov, parked, [])
    assert [r["name"] for r in out["methods"]] == ["get_label"]
    out = ingest.drop_parked_and_denied("X", ov, {}, ["methods:get_*", "bindings:x"])
    assert [r["name"] for r in out["methods"]] == ["set_label"] and "bindings" not in out and "signals" not in out
    # a changed record is tried again
    assert len(ingest.drop_parked_and_denied("X", ov, {("X", "methods", "set_label"): "stale"}, [])["methods"]) == 2


# ---- rule changes from the AI1 review (docs/notes/ai1-gtk-review.md) ---------------------------------------------

def test_unsampled_float_gets_a_sample_from_the_gir_range():
    doc = kind([prop("xalign", "f32", range={"min": 0.0, "max": 1.0}, default=0.5)],
               [meth("set_xalign", ["f32"])])
    k = run(doc, {"props": {"xalign": {"outcome": "identity"}},
                  "methods": {"set_xalign": {"class": "accessor_set", "prop": "xalign", "outcome": "no_sample"}}})
    (p,) = k.ov["properties"]
    assert p["sample"] == 0.25 and p["provenance"][0]["origin"] == "rule"
    (m,) = k.ov["methods"]
    assert m["args"] == [0.25] and m["expect"] == 'h.get(o, "xalign") == V::F64(0.25)'
    assert k.review == []


def test_unranged_float_identity_is_still_held():
    k = run(kind([prop("ratio", "f32")]), {"props": {"ratio": {"outcome": "identity"}}})
    assert "properties" not in k.ov and [r["pattern"] for r in k.review] == ["prop:scalar-identity"]


def test_non_scalar_accessor_without_a_sample_is_skipped_not_held():
    doc = kind([prop("tabs", "boxed<svelters-api:v1:type:gtk:Pango:TabArray>"), prop("name", "string")],
               [meth("set_tabs", ["boxed<svelters-api:v1:type:gtk:Pango:TabArray>"]), meth("get_name", [], "string")])
    k = run(doc, {"methods": {"set_tabs": {"class": "accessor_set", "prop": "tabs", "outcome": "no_sample"},
                              "get_name": {"class": "accessor_get", "prop": "name", "outcome": "differs"}}})
    assert k.skipped.get("method:set-non-scalar") == 1
    assert [r["name"] for r in k.review] == ["get_name"]          # a scalar accessor that differs still needs review


def test_child_setters_are_denied():
    k = run(kind(methods=[meth("set_child", ["object<svelters-api:v1:type:gtk:Gtk:Widget>"]), meth("set_reveal_child", ["bool"])]),
            {"methods": {"set_child": {"class": "accessor_set", "prop": "child", "outcome": "no_sample"},
                         "set_reveal_child": {"class": "accessor_set", "prop": "reveal-child", "outcome": "no_sample"}}})
    assert sum(c for p, c in k.skipped.items() if p.startswith("method:denied (re-parents")) == 1


def test_side_effect_trigger_with_an_unsampled_default_falls_back_to_delivery_only():
    doc = kind([prop("im-module", "string")], signals=[sig("preedit-changed")])
    k = run(doc, {"props": {"im-module": {"outcome": "exact", "notify": True, "sample": "x", "default": None, "signals": ["preedit-changed"]}},
                  "signals": {"preedit-changed": {"returns": "void", "handler_result": "none", "sync_required": False, "run": "last",
                                                  "params": ["gchararray"], "synthetic": {"args": ["x"], "emit": "delivered"}}}})
    (s,) = [s for s in k.ov["signals"] if not s.get("derived_from")]
    assert s["delivery_only"] is True and "property" not in s["trigger"]


# ---- review decisions (_reviewed.yaml) ----------------------------------------------------------------------------

def decided(doc, probe, decisions):
    k = infer.Kind("X", probe, doc, {"X": doc}, infer.Rules(), {f: dict(d) for f, d in decisions.items()})
    k.infer_props()
    k.infer_methods()
    return k


IDENT = {"props": {"tags": {"outcome": "identity"}}}


def test_accept_writes_the_candidate_with_the_review_as_provenance():
    doc = kind([prop("tags", "list<string>")])
    k = decided(doc, IDENT, {"X.properties.tags": {"pattern": "middle:prop:round-trip", "decision": "accept", "reason": "ok", "reviewer": "t"}})
    (rec,) = k.ov["properties"]
    assert rec["provenance"][-1] == {"origin": "manual", "ref": "review accept (t): ok"}
    assert k.ev["properties:tags"]["review"]["decision"] == "accept"
    assert k.review == [] and k.errors == [] and k.decided[0]["decision"] == "accept"


def test_reject_leaves_the_queue_and_writes_nothing():
    k = decided(kind([prop("tags", "list<string>")]), IDENT,
                {"X.properties.tags": {"pattern": "middle:prop:round-trip", "decision": "reject", "reason": "no", "reviewer": "t"}})
    assert "properties" not in k.ov and k.review == [] and k.errors == []


def test_a_changed_pattern_voids_the_decision():
    k = decided(kind([prop("tags", "list<string>")]), IDENT,
                {"X.properties.tags": {"pattern": "low:prop:ignored", "decision": "accept", "reason": "ok", "reviewer": "t"}})
    assert "properties" not in k.ov and len(k.review) == 1 and "review it again" in k.errors[0]


def test_accept_needs_a_candidate_and_override_needs_a_manual_record(tmp_path, monkeypatch):
    monkeypatch.setattr(infer, "OVERRIDES", tmp_path)
    doc = kind([prop("a", "bool"), prop("b", "bool")])
    probe = {"props": {"a": {"outcome": "ignored"}, "b": {"outcome": "ignored"}}}
    dec = {"X.properties.a": {"pattern": "low:prop:ignored", "decision": "accept", "reason": "ok", "reviewer": "t"},
           "X.properties.b": {"pattern": "low:prop:ignored", "decision": "override", "reason": "ok", "reviewer": "t"}}
    k = decided(doc, probe, dec)
    assert any("needs a candidate record" in e for e in k.errors) and any("has no properties record 'b'" in e for e in k.errors)
    (tmp_path / "X.yaml").write_text("properties:\n  - name: b\n    provenance: docs\n")
    k = decided(doc, probe, dec)
    assert [e for e in k.errors if "'b'" in e] == []


def test_load_decisions_rejects_malformed_and_duplicate_entries(tmp_path):
    import yaml
    p = tmp_path / "_reviewed.yaml"
    good = {"fact": "X.properties.a", "pattern": "low:prop:ignored", "decision": "reject", "reason": "why", "reviewer": "tester"}
    p.write_text(yaml.safe_dump({"schema": "svelters-inferred-reviewed/v1", "toolkit": "gtk",
                                 "decisions": [good, good, {**good, "fact": "X.b", "decision": "maybe"}]}))
    out, errs = infer.load_decisions(p)
    assert list(out) == ["X.properties.a"] and len(errs) == 2


def test_checked_in_decisions_are_well_formed_and_all_used():
    _, _, summary, errors = infer.run()
    assert errors == []
    assert summary["unreviewed"]["middle"] + summary["unreviewed"]["low"] + sum(summary["reviewed"][d] for d in infer.DECISIONS) > 0
