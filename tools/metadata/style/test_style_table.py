"""The style table checker (style_table.py): setter targets must be writable on every tag they
expand to, types must fit, child attributes must exist on the parents."""
import importlib.util
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location("style_table", Path(__file__).with_name("style_table.py"))
st = importlib.util.module_from_spec(spec)
spec.loader.exec_module(st)

TAGS = {"box": "Box", "button": "Button", "label": "Label"}
KINDS = {
    "Widget": {"chain": ["Widget"], "props": {"widthRequest": {"js": "widthRequest", "native": "width-request", "type": "i32", "writable": True}}},
    "Box": {"chain": ["Box", "Widget"], "props": {"spacing": {"js": "spacing", "native": "spacing", "type": "i32", "writable": True},
                                                  "mode": {"js": "mode", "native": "mode", "type": "enum", "writable": True, "values": ["a", "b"]}},
            "children": {"slots": {"children": {"attrs": {"stretch": {"type": "i32"}}}}}},
    "Button": {"chain": ["Button", "Widget"], "props": {"label": {"js": "label", "native": "label", "type": "string", "writable": False}}},
    "Label": {"chain": ["Label", "Widget"], "props": {}},
}


def doc(props, dialect="css"):
    return {"toolkit": "t", "dialect": dialect, "setter_selectors": ["type", "class", "id"],
            "units": {"px": {"scale": 1, "provenance": "x"}}, "properties": props}


def build(props, dialect="css"):
    return st.build("t", doc(props, dialect), TAGS, KINDS)


def test_setters_expand_to_tags_with_native_or_js_names():
    t = build({"width-request": {"via": "setter", "prop": "width-request", "type": "length", "kinds": ["Widget"]},
               "mode": {"via": "setter", "prop": "mode", "type": "enum", "kinds": ["Box"]},
               "stretch": {"via": "child-attr", "attr": "stretch", "type": "int", "parents": ["Box"]},
               "color": {"via": "css"}})
    p = t["properties"]
    assert p["width-request"]["prop"] == "widthRequest" and p["width-request"]["tags"] == ["box", "button", "label"]
    assert p["mode"]["values"] == ["a", "b"]
    assert p["stretch"]["tags"] == ["box"]
    assert p["color"] == {"via": "css"}


@pytest.mark.parametrize("entry, msg", [
    ({"via": "setter", "prop": "spacing", "type": "length", "kinds": ["Widget"]}, "not a writable property of <button>, <label>"),
    ({"via": "setter", "prop": "label", "type": "string", "kinds": ["Button"]}, "not a writable property"),
    ({"via": "setter", "prop": "spacing", "type": "bool", "kinds": ["Box"]}, "type bool cannot target"),
    ({"via": "setter", "prop": "spacing", "type": "length", "kinds": ["Nope"]}, "no element of t is a Nope"),
    ({"via": "child-attr", "attr": "row", "type": "int", "parents": ["Box"]}, "has no child attribute 'row'"),
    ({"via": "sideways"}, "via must be one of"),
])
def test_mistakes_are_errors(entry, msg):
    with pytest.raises(st.StyleError, match=msg):
        build({"x": entry})


def test_a_setter_only_toolkit_has_no_css_entries():
    assert build({"spacing": {"via": "setter", "prop": "spacing", "type": "length", "kinds": ["Box"]}}, dialect="none")["dialect"] == "none"
    with pytest.raises(st.StyleError, match="no native styling dialect"):
        build({"color": {"via": "css"}}, dialect="none")


def test_names_are_kebab_case():
    with pytest.raises(st.StyleError, match="kebab-case"):
        build({"minWidth": {"via": "css"}})


def test_optional_range_on_numeric_rows():
    t = build({"width-request": {"via": "setter", "prop": "width-request", "type": "length", "kinds": ["Widget"], "range": {"min": 0}},
               "stretch": {"via": "child-attr", "attr": "stretch", "type": "int", "parents": ["Box"], "range": {"min": 0, "max": 10}}})
    assert t["properties"]["width-request"]["range"] == {"min": 0}
    assert t["properties"]["stretch"]["range"] == {"min": 0, "max": 10}
    with pytest.raises(st.StyleError, match="only for"):
        build({"mode": {"via": "setter", "prop": "mode", "type": "enum", "kinds": ["Box"], "range": {"min": 0}}})
    with pytest.raises(st.StyleError, match="min > range.max"):
        build({"width-request": {"via": "setter", "prop": "width-request", "type": "length", "kinds": ["Widget"], "range": {"min": 2, "max": 1}}})
    with pytest.raises(st.StyleError, match="min and/or max"):
        build({"width-request": {"via": "setter", "prop": "width-request", "type": "length", "kinds": ["Widget"], "range": {"low": 0}}})
