"""Tests for the dangerous-member layer (tools/metadata/common/dangerous.py, docs/notes/dangerous-members.md)."""
import json
import sys
from pathlib import Path

import jsonschema
import pytest
import yaml

sys.path.insert(0, str(Path(__file__).parent))
import dangerous  # noqa: E402
import svmeta  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]


def kind(name, bases=(), props=(), methods=(), signals=()):
    return {
        "schema": svmeta.SCHEMA, "toolkit": "gtk", "id": f"svelters-api:v1:type:gtk:Gtk:{name}", "native": {}, "kind": "object",
        "bases": list(bases),
        "properties": [{"name": n, "access": acc} for n, acc in props],
        "methods": [{"name": n} for n in methods],
        "signals": [{"name": n} for n in signals],
    }


RW = {"read": True, "write": True}
RO = {"read": True, "write": False}
CO = {"read": True, "write": True, "construct_only": True}


@pytest.fixture
def root(tmp_path):
    meta = tmp_path / "metadata/gtk"
    meta.mkdir(parents=True)
    docs = {
        "Widget": kind("Widget", methods=["set_parent", "show"], props=[("parent", RO)]),
        "Window": kind("Window", ["Gtk.Widget"], props=[("application", RW), ("child", RW), ("title", RW)],
                       methods=["set_child", "set_application", "present"]),
        "ApplicationWindow": kind("ApplicationWindow", ["Gtk.Window", "Gtk.Widget"], methods=["set_child"]),
        "Frame": kind("Frame", ["Gtk.Widget"], props=[("child", RW)], methods=["set_child"]),
        "Page": kind("Page", props=[("child", CO)]),
    }
    for n, d in docs.items():
        (meta / f"{n}.json").write_text(json.dumps(d))
    (tmp_path / "metadata/dangerous").mkdir(parents=True)
    (tmp_path / "metadata/dangerous/gtk.yaml").write_text(yaml.safe_dump({
        "schema": "svelters-dangerous/v1", "toolkit": "gtk", "rules": [
            {"id": "child-setters", "section": "methods", "name": "set_child", "severity": "corrupts-tree", "reason": "re-parents behind the slots"},
            {"id": "tree", "section": "methods", "name": "set_parent", "kinds": ["Widget"], "severity": "corrupts-tree", "reason": "moves a widget behind the backend"},
            {"id": "app", "section": "properties", "name": "application", "kinds": ["Window"], "severity": "owned-by-backend",
             "reason": "the backend sets it at creation", "use_instead": "nothing"},
            {"id": "child-prop", "section": "properties", "name": "child|parent", "severity": "corrupts-tree", "reason": "re-parents behind the slots"},
        ]}))
    ov = tmp_path / "metadata/overrides/gtk"
    ov.mkdir(parents=True)
    (ov / "Frame.yaml").write_text(yaml.safe_dump({"dangerous": {"methods": {"set_child": False}}}))
    (ov / "Window.yaml").write_text(yaml.safe_dump({"dangerous": {"methods": {"set_application": {
        "severity": "owned-by-backend", "reason": "the backend sets the application", "reachable": False}}}}))
    (tmp_path / "metadata/rules").mkdir(parents=True)
    (tmp_path / "metadata/rules/gtk.yaml").write_text(yaml.safe_dump({"deny_methods": [{"pattern": "^(set_child|set_parent|set_application)$", "reason": "x"}]}))
    return tmp_path


def test_rules_match_by_declaring_kind_or_base(root):
    d = dangerous.Dangerous("gtk", root)
    assert d.lookup("Widget", "methods", "set_parent")["source"] == "metadata/dangerous/gtk.yaml#tree"
    assert d.lookup("Widget", "methods", "show") is None
    # set_child has no `kinds`: every declaring kind; ApplicationWindow re-declares it
    assert d.lookup("ApplicationWindow", "methods", "set_child")["severity"] == "corrupts-tree"
    # `kinds: [Window]` matches a subclass's own member through its bases, but not another kind
    assert d.lookup("Window", "properties", "application")["use_instead"] == "nothing"
    assert d.lookup("Frame", "properties", "application") is None
    # names are full matches
    assert d.lookup("Window", "methods", "set_child_x") is None


def test_properties_are_the_write_side(root):
    d = dangerous.Dangerous("gtk", root)
    assert d.lookup("Window", "properties", "child") is not None
    assert d.lookup("Widget", "properties", "parent") is None      # read-only
    assert d.lookup("Page", "properties", "child") is None         # construct-only


def test_override_block_wins_and_false_exempts(root):
    d = dangerous.Dangerous("gtk", root)
    assert d.lookup("Frame", "methods", "set_child") is None
    a = d.lookup("Window", "methods", "set_application")
    assert a == {"severity": "owned-by-backend", "reason": "the backend sets the application", "reachable": False,
                 "source": "metadata/overrides/gtk/Window.yaml"}
    assert d.counts() == {"methods": 4, "properties": 3, "signals": 0}
    assert d.check() == []


def test_check_reports_dead_rules_unknown_members_and_useless_exemptions(root):
    p = root / "metadata/dangerous/gtk.yaml"
    data = yaml.safe_load(p.read_text())
    data["rules"].append({"id": "dead", "section": "signals", "name": "nope", "severity": "other", "reason": "matches nothing at all"})
    p.write_text(yaml.safe_dump(data))
    (root / "metadata/overrides/gtk/Frame.yaml").write_text(yaml.safe_dump({"dangerous": {
        "methods": {"set_child": False, "present": False}, "properties": {"nope": {"severity": "other", "reason": "not a member here"}}}}))
    errs = "\n".join(dangerous.Dangerous("gtk", root).check())
    assert "rule dead: matches no member" in errs
    assert "'nope' is not a member of metadata/gtk/Frame.json" in errs
    assert "'present' is not a member" in errs  # Frame has no present (Window does)


def test_check_rejects_dangerous_members_in_the_inferred_layer(root):
    inf = root / "metadata/inferred/gtk"
    inf.mkdir(parents=True)
    (inf / "Window.yaml").write_text(yaml.safe_dump({"properties": [{"name": "title"}, {"name": "application"}],
                                                     "bindings": [{"name": "child", "property": "child"}]}))
    (inf / "ApplicationWindow.yaml").write_text(yaml.safe_dump({"methods": [{"name": "set_child"}], "dangerous": {}}))
    errs = "\n".join(dangerous.Dangerous("gtk", root).check())
    assert "inferred/gtk/Window.yaml: properties 'application' is a dangerous member" in errs
    assert "inferred/gtk/Window.yaml: bindings 'child' is a dangerous member" in errs
    assert "inferred/gtk/ApplicationWindow.yaml: methods 'set_child' is a dangerous member" in errs
    assert "may not declare `dangerous`" in errs
    assert "'title'" not in errs


def test_gtk_dangerous_methods_must_be_denied_to_inference(root):
    (root / "metadata/rules/gtk.yaml").write_text(yaml.safe_dump({"deny_methods": [{"pattern": "^set_child$", "reason": "x"}]}))
    errs = dangerous.Dangerous("gtk", root).check()
    assert errs == [f"metadata/rules/gtk.yaml: dangerous method {m} is not denied by deny_methods" for m in ("Widget.set_parent", "Window.set_application")]


def test_rules_file_schema(root):
    p = root / "metadata/dangerous/gtk.yaml"
    assert dangerous.validate_rules_file(p) == []
    data = yaml.safe_load(p.read_text())
    data["rules"][0]["severity"] = "scary"
    data["rules"].append(dict(data["rules"][1]))
    data["rules"][2]["name"] = "("
    p.write_text(yaml.safe_dump(data))
    errs = "\n".join(dangerous.validate_rules_file(p))
    assert "'scary' is not one of" in errs and "duplicate rule id tree" in errs and "bad regex" in errs


@pytest.mark.parametrize("tk", dangerous.TOOLKITS)
def test_override_schemas_carry_the_block(tk):
    assert dangerous.check_override_schemas(ROOT) == []
    schema = json.loads((ROOT / f"tools/metadata/{tk}/override.schema.json").read_text())
    block = {"methods": {"x": {"severity": "crashes", "reason": "frees a native object the backend holds"}}, "properties": {"y": False}}
    v = jsonschema.Draft202012Validator(schema["properties"]["dangerous"])
    assert list(v.iter_errors(block)) == []
    assert list(v.iter_errors({"methods": {"x": {"severity": "bad", "reason": "long enough reason"}}}))
    assert list(v.iter_errors({"methods": {"x": True}}))
    assert list(v.iter_errors({"fields": {}}))


def test_severities_agree():
    schema = json.loads(dangerous.SCHEMA_PATH.read_text())
    assert tuple(schema["$defs"]["annotation"]["properties"]["severity"]["enum"]) == svmeta.DANGEROUS_SEVERITIES


def test_svmeta_never_merges_the_read_time_layer():
    base = {"methods": [{"name": "set_child", "origin": "gir"}]}
    out = svmeta.merge(base, {"dangerous": {"methods": {"set_child": {"severity": "other", "reason": "r"}}},
                              "methods": [{"name": "set_child", "dangerous": {"severity": "other"}}]})
    assert "dangerous" not in out and "dangerous" not in out["methods"][0]
    k = {"schema": svmeta.SCHEMA, "toolkit": "gtk", "id": "x", "native": {}, "kind": "object", "methods": [{"name": "a", "dangerous": {}}]}
    assert any("read-time layer" in e for e in svmeta.validate(k))


def test_repository_layer_is_consistent():
    """The checked-in rules, override blocks, inferred layers and GTK deny rules agree (what test:meta runs)."""
    errs = dangerous.check_override_schemas(ROOT)
    for tk in dangerous.TOOLKITS:
        errs += dangerous.Dangerous(tk, ROOT).check()
    assert errs == []
    gtk = dangerous.Dangerous("gtk", ROOT)
    for kind_, name in [("Widget", "set_parent"), ("Window", "set_child"), ("Window", "set_titlebar"), ("Window", "set_application"),
                        ("Window", "set_display"), ("Window", "set_startup_id"), ("Paned", "set_start_child")]:
        assert gtk.lookup(kind_, "methods", name), (kind_, name)
    for name in ("application", "display"):
        assert gtk.lookup("Window", "properties", name)
    # the AI1 review's untestable rejections are not dangerous
    assert gtk.lookup("Widget", "methods", "get_name") is None
    assert gtk.lookup("CheckButton", "methods", "set_group") is None
    assert gtk.lookup("Window", "methods", "present") is None and gtk.lookup("Window", "methods", "close") is None
