"""Unit tests of the Qt inference rules (tools/metadata/qt/infer.py), the layer merge (layers.py) and the
table-driven tests of inferred facts (gen_cpp.py Tests.row_*)."""
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "common"))


def _load(name: str, path: Path):
    # gtk/infer.py is also called `infer`: load the Qt one under its own module name.
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


infer = _load("svelters_qt_infer", HERE / "infer.py")
layers = _load("svelters_qt_layers", HERE / "layers.py")
import gen_cpp  # noqa: E402
import svmeta  # noqa: E402

ENUMS = {"QX::Mode": {"fullname": "QX::Mode", "members": [{"name": "A", "cpp": "QX::A", "value": "0"},
                                                          {"name": "B", "cpp": "QX::B", "value": "1"}]}}


def prop(name, ref, write=True, getter=None, notify=None):
    return {"name": name, "type": {"ref": ref}, "access": {"read": True, "write": write},
            "accessors": {"get": getter or name, "set": ("set" + name[0].upper() + name[1:]) if write else None},
            "metaobject": {"notify": notify}}


def sig(name, params=(), sigs=None):
    ps = [{"name": f"a{i}", "type": {"ref": r}} for i, r in enumerate(params)]
    return {"name": name, "params": ps, "access": "public",
            "metaobject_signatures": sigs or [f"{name}({','.join({'i32': 'int', 'string': 'QString', 'bool': 'bool'}[r] for r in params)})"]}


def doc(props=(), methods=(), signals=()):
    return {"properties": list(props), "methods": list(methods), "signals": list(signals)}


def run(d, probe):
    k = infer.Kind("QX", ["x"], probe, d, ENUMS, infer.Rules())
    k.infer_props()
    k.infer_methods()
    k.infer_signals()
    k.infer_bindings()
    return k


def p_exact(sample, default, typ="int", notify=None, seq=None, **kw):
    r = {"writable": True, "readable": True, "type": typ, "outcome": "exact", "sample": sample, "default": default,
         "notify": notify, "notify_fired": bool(notify), "sequence": seq or {}, **kw}
    if notify and not seq:
        r["sequence"] = {notify: [1, 1, 1]}
        r["sequence_args"] = {notify: [[sample], [default], [sample]]}
    return r


def test_notify_prop_gives_prop_signal_and_binding():
    d = doc([prop("value", "i32", notify="valueChanged(int)")], signals=[sig("valueChanged", ["i32"])])
    k = run(d, {"props": {"value": p_exact(3, 0, notify="valueChanged(int)")},
                "signals": {"valueChanged(int)": {"name": "valueChanged", "params": ["int"]}}})
    (p,) = k.ov["props"]
    assert p == {"js": "value", "native": "value", "sample": 3, "provenance": p["provenance"]}
    (s,) = k.ov["signals"]
    assert s["trigger"] == {"property": "value", "values": [3, 0, 3]} and s["expect_args"] == ["3"]
    (b,) = k.ov["bindings"]
    assert b["change"] == {"signal": "value-changed", "role": "any_change", "value": "arg0"}
    assert b["verify"] == {"values": [3, 0]} and b["echo"]["strategy"] == "none"


def test_ignored_prop_is_low_band_review_only():
    k = run(doc([prop("a", "bool")]), {"props": {"a": {"writable": True, "type": "bool", "outcome": "ignored"}}})
    assert "props" not in k.ov and [(r["band"], r["pattern"]) for r in k.review] == [("low", "prop:ignored")]


def test_scoped_and_flag_enums_are_not_sampled():
    d = doc([prop("mode", "enum<QX::Mode>")])
    for extra in ({"scoped": True}, {"flag": True}):
        k = run(d, {"props": {"mode": p_exact("B", "A", typ="QX::Mode", enum=True, **extra)}})
        assert "props" not in k.ov
    k = run(d, {"props": {"mode": p_exact("B", "A", typ="QX::Mode", enum=True)}})
    assert k.ov["props"][0]["sample"] == "B"


def test_recipe_becomes_setup_and_unknown_recipe_is_held():
    d = doc([prop("currentIndex", "i32")])
    k = run(d, {"props": {"currentIndex": p_exact(1, 0, recipe="populate", recipe_call="addItems")}})
    assert "addItems" in k.ov["props"][0]["setup"]
    k = run(d, {"props": {"currentIndex": p_exact(1, 0, recipe="populate", recipe_call="frobnicate")}})
    assert "props" not in k.ov and k.review[0]["pattern"] == "prop:recipe-populate"


def test_read_only_getter_becomes_a_method():
    d = doc([prop("count", "i32", write=False)],
            [{"name": "count", "access": "public", "params": [], "returns": {"ref": "i32"}}])
    k = run(d, {"props": {"count": {"writable": False, "type": "int", "outcome": "readonly", "value": 0}},
                "methods": {"count": {"class": "accessor_get", "prop": "count", "outcome": "exact"}}})
    assert k.ov["methods"] == [{"js": "count", "native": "count", "provenance": k.ov["methods"][0]["provenance"]}]


def test_signal_without_trigger_is_delivery_only_and_enum_args_are_held():
    d = doc(signals=[sig("activated", ["i32"]), sig("moved", ["enum<QX::Mode>"], ["moved(QX::Mode)"])])
    k = run(d, {"signals": {"activated(int)": {"name": "activated"}, "moved(QX::Mode)": {"name": "moved"}}})
    (s,) = k.ov["signals"]
    assert s["js"] == "activated" and s["delivery_only"] and s["trigger"] == {"emit": [1]} and s["expect_args"] == ["1"]
    assert [r["pattern"] for r in k.review] == ["signal:delivery-only-enum-argument"]


def test_shorter_overload_collapses_into_the_full_signature():
    d = doc(signals=[sig("triggered", ["bool"], ["triggered(bool)", "triggered()"])])
    k = run(d, {"signals": {"triggered(bool)": {"name": "triggered"}, "triggered()": {"name": "triggered"}}})
    (s,) = k.ov["signals"]
    assert s["connect"] == "triggered(bool)" and s["collapses"] == ["triggered()"]


def test_rules_accept_only_listed_patterns():
    r = infer.Rules()
    k = infer.Kind("QX", ["x"], {}, doc(), ENUMS, r)
    assert not k.emit("methods", "m", {}, 0.99, {}, "method:pure-getter")
    assert k.review[0]["accepted"] is False


# ---- layers ------------------------------------------------------------------------------------

def test_manual_wins_deny_and_parked_drop_inferred_records(tmp_path):
    files = {"QX": {"elements": [{"tag": "x", "construct": {"class": "QX"}, "props": [{"js": "a", "provenance": "m"}],
                                  "deny": ["props:c*"]}]},
             "QBase": {"inherit": {"props": [{"js": "b", "provenance": "m"}]}}}
    (tmp_path / "QX.json").write_text(json.dumps({"bases": ["QBase"]}))
    rec = lambda js: {"js": js, "native": js, "provenance": "p"}  # noqa: E731
    inferred = {"QX": {"elements": [{"tag": "x", "props": [rec("a"), rec("b"), rec("c1"), rec("d"), rec("e")]}]}}
    parked = {("QX", "props", "e"): svmeta.fingerprint(rec("e"))}
    out = layers.merge(files, inferred, parked, tmp_path)
    props = out["QX"]["elements"][0]["props"]
    assert [p["js"] for p in props] == ["a", "d"]
    assert props[0].get("_origin") is None and props[1]["_origin"] == "inferred" and props[1]["_fp"] == svmeta.fingerprint(rec("d"))
    assert files["QX"]["elements"][0]["props"] == [{"js": "a", "provenance": "m"}]   # inputs untouched


def test_parked_fact_with_a_changed_record_is_retried(tmp_path):
    files = {"QX": {"elements": [{"tag": "x", "construct": {"class": "QX"}}]}}
    (tmp_path / "QX.json").write_text("{}")
    out = layers.merge(files, {"QX": {"elements": [{"tag": "x", "props": [{"js": "e", "provenance": "new"}]}]}},
                       {("QX", "props", "e"): "stale-fp"}, tmp_path)
    assert [p["js"] for p in out["QX"]["elements"][0]["props"]] == ["e"]


# ---- generator ---------------------------------------------------------------------------------

def test_checked_in_inferred_layer_becomes_table_rows_not_functions():
    m = gen_cpp.Model(ROOT / "metadata/qt", ROOT / "metadata/overrides/qt", [])
    tmpl = (HERE / "templates/verify.cpp.in").read_text()
    src, man = gen_cpp.gen_verify(m, "d", tmpl)
    inferred = [f for f in man["facts"] if f.get("origin") == "inferred"]
    assert inferred and all(f["record_fp"] and f["kind"] for f in inferred)
    for f in inferred:
        (t,) = f["tests"]
        assert f'{{"{t}", ' in src                                      # a row ...
        assert "bool t_" + gen_cpp.cident(t) + "(" not in src            # ... and no function
    assert {f["section"] for f in inferred} >= {"props", "methods", "signals", "bindings"}
    assert any(f.get("strength") == "delivery" for f in inferred)


def test_unresolvable_inferred_fact_is_untestable_not_an_error(tmp_path):
    inf = tmp_path / "inferred"
    inf.mkdir()
    (inf / "QSpinBox.yaml").write_text("elements:\n- tag: spin-box\n  props:\n  - {js: no-such, native: noSuch, sample: 1, provenance: x}\n")
    m = gen_cpp.Model(ROOT / "metadata/qt", ROOT / "metadata/overrides/qt", [], inf)
    assert [(u["kind"], u["section"], u["name"]) for u in m.untestable] == [("QSpinBox", "props", "no-such")]
    assert "no-such" not in {p["js"] for p in m.by_tag["spin-box"]["props"]}
