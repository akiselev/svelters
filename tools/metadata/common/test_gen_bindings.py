import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import gen_bindings as g

GOOD = {
    "name": "text",
    "direction": "two_way",
    "property": "text",
    "type": {"ref": "string"},
    "change": {"signal": "changed"},
    "echo": {"strategy": "host_suppressed", "compare_before_set": True, "read_back": False},
    "lifetime": {"on_native_death": "end_binding", "on_destroy": "idempotent_disconnect"},
    "provenance": "test",
}


def test_checked_in_outputs_are_current():
    assert g.main.__module__ == "gen_bindings"
    sys.argv = ["gen_bindings.py", "--check"]
    assert g.main() == 0


def test_fields_are_validated():
    assert g.common_fields("t", GOOD)["signal"] == "changed"
    for bad in (
        {**GOOD, "direction": "sideways"},
        {**GOOD, "type": {"ref": "variant"}},
        {**GOOD, "echo": {"strategy": "magic"}},
        {**GOOD, "change": {}},
        {**GOOD, "provenance": ""},
        {**GOOD, "lifetime": {}},
    ):
        with pytest.raises(g.GenError):
            g.common_fields("t", bad)


def test_toolkits_must_agree():
    a = {"entry": {"text": g.common_fields("t", GOOD)}}
    b = {"entry": {"text": g.common_fields("t", {**GOOD, "property": "other"})}}
    assert g.merge(a, a)["entry"]["text"]["toolkits"] == ["gtk", "qt"]
    variants = g.merge(a, a, None, b)["entry"]["text"]
    assert [v["toolkits"] for v in variants] == [["gtk", "qt"], ["winui"]]
    assert [v["prop"] for v in variants] == ["text", "other"]


def test_toolkits_that_disagree_become_variants():
    f = g.common_fields("t", GOOD)
    other = g.common_fields("t", {**GOOD, "change": {"signal": "text-changed"}})
    m = g.merge({"entry": {"text": f}}, {"entry": {"text": f}}, None, {"entry": {"text": other}})
    assert [v["signal"] for v in m["entry"]["text"]] == ["changed", "text-changed"]


def test_enum_types_travel_as_strings():
    assert g.common_fields("t", {**GOOD, "type": {"ref": "enum<NSControlStateValue>"}})["type"] == "string"


def test_appkit_and_winui_bindings_are_read():
    ak = g.appkit_bindings()
    assert ak["button"]["state"]["signal"] == "action"
    assert ak["button"]["state"]["valueFrom"] == "prop"
    assert ak["text-field"]["string-value"]["valueFrom"] == "arg0"
    wu = g.winui_bindings()
    assert wu["check-box"]["is-checked"]["signal"] == "click"
    assert wu["slider"]["value"]["signal"] == "value-changed"
    assert wu["text-box"]["text"]["signal"] == "text-changed"


def test_binding_validation_against_facts():
    out: dict = {}
    props, signals = {"text": True, "ro": False}, {"changed": 1, "action": 0}
    g._add_binding(out, "e", GOOD, "winui", props, signals)
    assert out["e"]["text"]["signal"] == "changed"
    for bad, msg in (
        ({**GOOD, "name": "n", "property": "missing"}, "unknown property"),
        ({**GOOD, "name": "n", "property": "ro"}, "not writable"),
        ({**GOOD, "name": "n", "change": {"signal": "nope"}}, "unknown signal"),
        ({**GOOD, "name": "n", "change": {"signal": "action"}}, "carries no arguments"),
        (GOOD, "duplicate"),
    ):
        with pytest.raises(g.GenError, match=msg):
            g._add_binding(out, "e", bad, "winui", props, signals)
    ok = {**GOOD, "name": "n", "change": {"signal": "action", "value": "prop"}}
    g._add_binding(out, "e", ok, "winui", props, signals)


def test_kebab_matches_the_winui_generator():
    assert g.kebab("IsChecked") == "is-checked"
    assert g.kebab("StackPanel") == "stack-panel"


def test_text_props_union_in_toolkit_order():
    tp = g.text_props()
    assert tp["button"][0] == "label"
    assert tp["toggle-button"] == ["label"]
    assert tp["push-button"] == ["text"] and tp["button"][-1] == "content"  # Qt QPushButton::text, WinUI Content
    assert list(tp) == sorted(tp)
    assert "export const TEXT_PROPS = {" in g.js_text_props(tp)
