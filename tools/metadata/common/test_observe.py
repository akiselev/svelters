"""Tests of the observe-fact schema and layers (tools/metadata/common/observe.py, milestone R0)."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import observe  # noqa: E402
import svmeta  # noqa: E402

PROV = [{"origin": "rule", "ref": "test"}]


def rec(**kw) -> dict:
    return {"prop": "text", "class": "exact", "via": [{"mechanism": "notify"}], "provenance": PROV, **kw}


def test_valid_records():
    assert observe.validate(rec()) == []
    assert observe.validate(rec(**{"class": "partial", "missed": ["user_edit"]})) == []
    assert observe.validate({"prop": "x", "class": "set_only", "provenance": PROV}) == []
    assert observe.validate(rec(via=[{"mechanism": "via", "object": "buffer", "prop": "text"}], value_cost="proportional")) == []
    assert observe.validate(rec(verify={"on": "Gtk:Entry", "check": "change"})) == []


def test_invalid_records():
    assert observe.validate(rec(**{"class": "sometimes"}))
    assert observe.validate(rec(via=[]))                                                # exact needs a mechanism
    assert observe.validate({"prop": "x", "class": "none", "via": [{"mechanism": "notify"}], "provenance": PROV})
    assert observe.validate(rec(**{"class": "partial"}))                                 # partial needs `missed`
    assert observe.validate(rec(missed=["user_edit"]))                                   # only partial has `missed`
    assert observe.validate(rec(via=[{"mechanism": "signal"}]))                           # signal needs its name
    assert observe.validate(rec(via=[{"mechanism": "poll"}]))                             # no polling (decision 9)
    assert observe.validate(rec(provenance=[]))
    assert observe.validate(rec(verify={"on": "Gtk:Entry", "check": "silent"}))          # silent needs a trigger
    assert observe.validate(rec(extra=1))


def test_spec_and_mechanism_string():
    r = rec(via=[{"mechanism": "notify", "signal": "changed()", "coarse": True}, {"mechanism": "event", "events": ["Resize", "Move"]}],
            timing="deferred", value_cost="proportional", verify={"on": "Gtk:Entry", "check": "connect"})
    s = observe.spec(r)
    assert s == {"class": "exact", "mechanism": "notify+event:Resize,Move", "coarse": True, "value_cost": "proportional",
                 "timing": "deferred"}
    assert observe.spec({"prop": "x", "class": "set_only", "provenance": PROV}) == {"class": "set_only", "mechanism": ""}


def test_layers_manual_wins_and_parked_inferred_facts_leave(tmp_path: Path):
    inf = tmp_path / "metadata/inferred/gtk/observe"
    man = tmp_path / "metadata/overrides/gtk/observe"
    inf.mkdir(parents=True)
    man.mkdir(parents=True)
    a, b = rec(), rec(prop="label", confidence=0.95)
    (inf / "Gtk.Entry.yaml").write_text(observe.layer_doc("gtk", "Gtk:Entry", [{**a, "confidence": 0.9}, b]))
    (man / "Gtk.Entry.yaml").write_text(observe.layer_doc("gtk", "Gtk:Entry", [{"prop": "text", "class": "none", "provenance": PROV}]))
    lay = observe.Layers("gtk", tmp_path)
    assert lay.errors == []
    assert lay.get("Gtk:Entry", "text")["class"] == "none" and lay.get("Gtk:Entry", "text")["layer"] == "manual"
    assert lay.get("Gtk:Entry", "label")["layer"] == "inferred"
    fp = svmeta.fingerprint(observe.strip(b))
    assert lay.get("Gtk:Entry", "label")["record_fp"] == fp
    (tmp_path / "metadata/inferred/gtk/_parked.json").write_text(json.dumps(
        {"parked": [{"kind": "Gtk:Entry", "section": "observe", "name": "label", "record_fp": fp}]}))
    assert observe.Layers("gtk", tmp_path).get("Gtk:Entry", "label") is None
    # a file must be named after its owner
    (inf / "Gtk.Label.yaml").write_text(observe.layer_doc("gtk", "Gtk:Entry", [a]))
    assert observe.Layers("gtk", tmp_path).errors


def test_checked_in_layers_are_valid():
    for tk in observe.TOOLKITS:
        lay = observe.Layers(tk)
        assert lay.errors == [], lay.errors[:5]
