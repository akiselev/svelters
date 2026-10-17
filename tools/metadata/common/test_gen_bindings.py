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
    with pytest.raises(g.GenError):
        g.merge(a, b)
