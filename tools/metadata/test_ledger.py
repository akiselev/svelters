"""Tests of the ledger's promotion and parking (tools/metadata/ledger.py)."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ledger  # noqa: E402

NEXTEST = """\
  TRY 1 FAIL [   1.858s] (───) svelters-gtk generated::verify::prop_label_justify
        PASS [   1.000s] (  3/10) svelters-gtk generated::verify::prop_label_label
  TRY 2 FAIL [   3.081s] (1/2) svelters-gtk generated::verify::prop_label_justify
        FLAKY 2/3 [   2.0s] ( 4/10) svelters-gtk generated::verify::prop_label_lines
────────────
     Summary [   4.975s] 2 tests run: 1 passed, 1 failed
  TRY 2 FAIL [   3.081s] (1/2) svelters-gtk generated::verify::prop_label_justify
  stdout ───

    running 1 test

  stderr ───
    thread 'gtk-main' (479418) panicked at crates/svelters-gtk/src/generated/verify.rs:2925:9:
    assertion `left == right` failed: the property reads the value
      left: Bool(false)
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
"""


def test_results_are_the_last_attempt_and_a_failure_keeps_its_panic_message():
    assert ledger.results_of(NEXTEST) == {"generated::verify::prop_label_justify": "fail", "generated::verify::prop_label_label": "pass",
                                          "generated::verify::prop_label_lines": "pass"}
    msgs = ledger.failure_messages(NEXTEST)
    assert list(msgs) == ["generated::verify::prop_label_justify"]
    assert msgs["generated::verify::prop_label_justify"].startswith("assertion `left == right` failed: the property reads the value left: Bool(false)")


def manifest(*facts, untestable=()):
    return {"version": "4.22.4", "facts": list(facts), "untestable": list(untestable)}


def fact(name, origin, tests, section="methods"):
    return {"fact": f"id:{name}", "kind": "Label", "section": section, "name": name, "origin": origin, "tests": tests,
            "fingerprint": "f", "record_fp": f"fp-{name}"}


def test_failing_inferred_fact_is_parked_and_a_failing_manual_one_is_not(tmp_path, monkeypatch):
    monkeypatch.setitem(ledger.PARKED, "gtk", tmp_path / "_parked.json")
    man = manifest(fact("set_a", "inferred", ["t1"]), fact("set_b", "manual", ["t2"]), fact("set_c", "inferred", ["t3"]),
                   untestable=[{"fact": "id:d", "kind": "Label", "section": "methods", "name": "set_d", "record_fp": "fp-d", "message": "generator: nope"}])
    changed = ledger.park("gtk", man, {"t1": "fail", "t2": "fail", "t3": "pass"}, {"t1": "boom"})
    assert changed
    parked = json.loads((tmp_path / "_parked.json").read_text())["parked"]
    assert [(r["name"], r["message"]) for r in parked] == [("set_a", "boom"), ("set_d", "generator: nope")]
    assert parked[0]["record_fp"] == "fp-set_a"
    # nothing new: the list is stable, so refresh stops regenerating
    assert not ledger.park("gtk", man, {"t1": "fail", "t2": "fail", "t3": "pass"}, {"t1": "boom"})


def test_parked_facts_of_another_toolkit_version_are_retried(tmp_path, monkeypatch):
    monkeypatch.setitem(ledger.PARKED, "gtk", tmp_path / "_parked.json")
    ledger.save_parked("gtk", "4.20.0", [{"kind": "Label", "section": "methods", "name": "set_a", "version": "4.20.0", "record_fp": "x"}])
    assert ledger.park("gtk", manifest(), {}, {})
    assert ledger.load_parked("gtk") == []


def test_check_requires_a_probe_for_the_ledger_version(tmp_path, monkeypatch):
    monkeypatch.setattr(ledger, "ROOT", tmp_path)
    assert "no probe for gtk 4.22.4" in ledger.probe_problems("gtk", "4.22.4")[0]
    (tmp_path / "metadata/probe/gtk").mkdir(parents=True)
    (tmp_path / "metadata/probe/gtk/4.22.4.json").write_text(json.dumps({"version": "4.22.3"}))
    assert "not 4.22.4" in ledger.probe_problems("gtk", "4.22.4")[0]
    (tmp_path / "metadata/probe/gtk/4.22.4.json").write_text(json.dumps({"version": "4.22.4"}))
    assert ledger.probe_problems("gtk", "4.22.4") == []
    assert ledger.probe_problems("qt", "6.11.0") == []
    assert "no probe for appkit 26.1" in ledger.probe_problems("appkit", "26.1")[0]


def test_read_results_jsonl_json_and_junit(tmp_path):
    jl = tmp_path / "r.jsonl"
    jl.write_text('{"suite": "generated", "test": "prop_a", "result": "fail", "message": "boom"}\n'
                  '{"suite": "generated", "test": "prop_b", "result": "pass"}\n'
                  '{"suite": "generated", "test": "prop_a", "result": "pass"}\n')
    assert ledger.read_results(jl) == ({"prop_a": "pass", "prop_b": "pass"}, {})
    js = tmp_path / "r.json"
    js.write_text(json.dumps({"results": [{"test": "t", "result": "fail", "message": "m"}]}))
    assert ledger.read_results(js) == ({"t": "fail"}, {"t": "m"})
    xml = tmp_path / "r.xml"
    xml.write_text('<testsuites><testsuite><testcase name="ok1"/><testcase name="bad"><failure message="nope"/></testcase>'
                   '<testcase name="sk"><skipped/></testcase></testsuite></testsuites>')
    assert ledger.read_results(xml) == ({"ok1": "pass", "bad": "fail", "sk": "skip"}, {"bad": "nope"})


def test_import_parks_failing_inferred_facts_and_records_the_rest(tmp_path, monkeypatch):
    facts = tmp_path / "facts.json"
    man = {"toolkit": "appkit", "version": "26.1", "untestable": [],
           "facts": [dict(fact("p_ok", "inferred", ["prop_ok"]), kind="NSView"), dict(fact("p_bad", "inferred", ["prop_bad"]), kind="NSView"),
                     dict(fact("m", "manual", ["kind_nsview"]), kind="NSView")]}
    facts.write_text(json.dumps(man))
    monkeypatch.setitem(ledger.FACTS, "appkit", facts)
    monkeypatch.setitem(ledger.PARKED, "appkit", tmp_path / "_parked.json")
    monkeypatch.setattr(ledger, "LEDGER_DIR", tmp_path / "verified")
    regen = []
    monkeypatch.setattr(ledger, "regenerate", lambda tk: regen.append(tk))
    res = tmp_path / "r.jsonl"
    res.write_text('{"test": "prop_ok", "result": "pass"}\n{"test": "prop_bad", "result": "fail", "message": "left: 1"}\n'
                   '{"test": "kind_nsview", "result": "pass"}\n')
    assert ledger.import_results("appkit", res, dry_run=True) == 0
    assert not (tmp_path / "verified").exists() and not regen
    assert ledger.import_results("appkit", res) == 0
    assert regen == ["appkit"]
    parked = ledger.load_parked("appkit")
    assert [(p["name"], p["message"]) for p in parked] == [("p_bad", "left: 1")]
    entries = {e["test"]: e["result"] for e in ledger.load("appkit")["entries"]}
    assert entries == {"prop_ok": "pass", "prop_bad": "fail", "kind_nsview": "pass"}
    assert "appkit" in ledger.toolkits_all()
    # a failing manual fact fails the import
    res.write_text('{"test": "kind_nsview", "result": "fail"}\n')
    assert ledger.import_results("appkit", res) == 1
