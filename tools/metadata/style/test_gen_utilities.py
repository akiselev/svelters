"""The utility-class generator (gen_utilities.py, lane U0): the checks of UTILITY-CSS §3.6 fire on a
small fixture, and the checked-in outputs are what the real metadata generates."""
import copy
import importlib.util
from pathlib import Path

import pytest

spec_ = importlib.util.spec_from_file_location("gen_utilities", Path(__file__).with_name("gen_utilities.py"))
gen = importlib.util.module_from_spec(spec_)
spec_.loader.exec_module(gen)


def tok(ty, value, layer="shape"):
    return {"layer": layer, "type": ty, "value": value}


SPEC = {
    "kinds": {
        "Box": {"states": [{"name": "hovered", "pseudo": [":hover"], "negated": []}], "parts": [], "provider_states": []},
        "Checkbox": {
            "states": [{"name": "checked", "pseudo": [":checked"], "negated": []}],
            "parts": [{"name": "indicator", "node": False, "doc": ""}],
            "provider_states": [],
        },
    },
    "tags": {"box": "Box", "checkbox": "Checkbox"},
    "style": {
        "properties": {
            "padding": {"via": "css", "decl_type": "insets"},
            "background-color": {"via": "css", "decl_type": "color"},
            "border-width": {"via": "css", "decl_type": "length"},
            "border-color": {"via": "css", "decl_type": "color"},
            "align-items": {"via": "setter", "type": "enum", "values": ["start", "end", "center"]},
            "display": {"via": "css", "decl_type": "enum", "values": ["flex", "none"]},
            "opacity": {"via": "css", "decl_type": "number", "range": {"min": 0.0, "max": 1.0}},
        },
        "pseudo_classes": [
            {"name": "hover", "negated": False, "state": "hovered"},
            {"name": "checked", "negated": False, "state": "checked"},
        ],
        "media_features": [{"name": "prefers-color-scheme", "type": "discrete", "values": ["light", "dark"]}, {"name": "width", "type": "range", "values": []}],
        "provider_states": [],
    },
    "theme": {
        "tokens": {
            "space-1": tok("length", "4px"),
            "space-2": tok("length", "8px"),
            "accent": {"layer": "role", "type": "color", "values": {"dark": "#0078D4", "light": "#005FB8", "hc-dark": "#000", "hc-light": "#fff"}, "vscode": "button.background"},
            "surface": {"layer": "role", "type": "color", "values": {"dark": "#1F1F1F", "light": "#fff", "hc-dark": "#000", "hc-light": "#fff"}, "vscode": "editor.background"},
            "stroke": tok("length", "1px"),
            "breakpoint-sm": tok("length", "640px"),
        }
    },
}

VOCAB = {
    "config": {"layer": "utilities"},
    "scales": {
        "space": {"tokens": ["space-*"]},
        "stroke": {"tokens": [{"token": "stroke", "as": ""}]},
        "colors": {"type": "color"},
        "breakpoint": {"tokens": ["breakpoint-*"]},
    },
    "utilities": [
        {"ns": "p", "family": "spacing", "props": ["padding"], "values": ["space", "arbitrary"]},
        {"utility": "flex", "family": "layout", "decls": {"display": "flex"}},
        {"utility": "hidden", "family": "layout", "decls": {"display": "none"}},
        {"ns": "items", "family": "layout", "props": ["align-items"], "keywords": "table"},
        {"ns": "bg", "family": "color", "props": ["background-color"], "values": ["colors", "arbitrary"]},
        {"ns": "border", "family": "color", "props": ["border-color"], "values": ["colors"]},
        {"ns": "border", "family": "borders", "props": ["border-width"], "values": ["stroke", "int-px"]},
        {"ns": "opacity", "family": "effects", "props": ["opacity"], "values": ["percent"]},
    ],
    "variants": {
        "aliases": {"aria-checked": "checked"},
        "structural": {"first": "&:first-child"},
        "media": {"dark": "(prefers-color-scheme: dark)"},
        "part_prefix": "part-",
        "breakpoints": {"scale": "breakpoint", "min": "(width >= {px})", "max_prefix": "max-", "max": "(width < {px})"},
    },
}

TAILWIND = {
    "utilities": ["p", "flex", "hidden", "items", "bg", "border", "opacity", {"name": "ring", "reason": "no ring", "use": "outline-*"}],
    "variants": ["hover", "checked", "first", "dark", "sm", "max", "aria-checked", {"name": "print", "reason": "no printing"}],
}


def build(vocab=None, tailwind=None, spec=None):
    return gen.build(copy.deepcopy(vocab or VOCAB), copy.deepcopy(tailwind or TAILWIND), copy.deepcopy(spec or SPEC))


def concrete(m):
    return {n: d for r in m["rows"] for n, d in r["concrete"]}


def test_fixture_builds_from_tokens():
    m = build()
    c = concrete(m)
    assert c["p-2"] == {"padding": "var(--sv-space-2)"}, "spacing is our space-* tokens, not a 4px multiplier"
    assert c["bg-accent"] == {"background-color": "var(--sv-accent)"}
    assert c["border"] == {"border-width": "var(--sv-stroke)"}, "a scale value named '' is the bare utility"
    assert c["items-center"] == {"align-items": "center"}, "keywords: table copies the enum"
    assert "bg-red-500" not in c
    assert m["variants"]["breakpoints"]["sizes"] == [{"name": "sm", "token": "breakpoint-sm", "px": 640.0}]
    assert m["kinds"]["checkbox"]["parts"] == [{"name": "indicator", "node": False}]
    assert m["todo"] == []


def test_a_row_with_a_missing_property_fails():
    v = copy.deepcopy(VOCAB)
    v["utilities"].append({"ns": "gap", "family": "spacing", "props": ["gap"], "values": ["space"]})
    with pytest.raises(gen.UtilError, match="'gap' is not in the egui style table"):
        build(vocab=v, tailwind={**TAILWIND, "utilities": TAILWIND["utilities"] + ["gap"]})


def test_a_row_waiting_for_an_engine_item_is_pending():
    v = copy.deepcopy(VOCAB)
    v["utilities"].append({"ns": "gap", "family": "spacing", "props": ["gap"], "values": ["space"], "requires": "E9"})
    m = build(vocab=v, tailwind={**TAILWIND, "utilities": TAILWIND["utilities"] + ["gap"]})
    row = [r for r in m["rows"] if r["name"] == "gap"][0]
    assert row["pending"] and row["requires"] == "E9"
    assert all(u["label"] != "gap-1" for u in gen.expand(m))


def test_a_keyword_that_is_not_an_enum_value_fails():
    v = copy.deepcopy(VOCAB)
    v["utilities"][3] = {"ns": "items", "family": "layout", "props": ["align-items"], "keywords": {"stretch": "stretch"}}
    with pytest.raises(gen.UtilError, match="'stretch' is not one of align-items's values"):
        build(vocab=v)


def test_a_name_collision_fails():
    v = copy.deepcopy(VOCAB)
    v["utilities"].append({"ns": "bg", "family": "color", "props": ["background-color"], "keywords": {"accent": "transparent"}})
    with pytest.raises(gen.UtilError, match="utility 'bg-accent' comes from two rows"):
        build(vocab=v)


def test_a_concrete_name_under_another_rows_number_pattern_fails():
    s = copy.deepcopy(SPEC)
    s["theme"]["tokens"]["2"] = {"layer": "role", "type": "color", "value": "#fff"}
    with pytest.raises(gen.UtilError, match="'border-2' .* also matches the number pattern"):
        build(spec=s)


def test_an_unlisted_tailwind_namespace_fails():
    t = copy.deepcopy(TAILWIND)
    t["utilities"].append("rotate")
    with pytest.raises(gen.UtilError, match="Tailwind namespace 'rotate' is neither in utilities.yaml nor given a reason"):
        build(tailwind=t)


def test_a_namespace_missing_from_the_tailwind_list_fails_unless_own():
    t = copy.deepcopy(TAILWIND)
    t["utilities"].remove("opacity")
    with pytest.raises(gen.UtilError, match="'opacity' is not a Tailwind v4 name"):
        build(tailwind=t)
    v = copy.deepcopy(VOCAB)
    v["utilities"][7]["own"] = True
    build(vocab=v, tailwind=t)


def test_an_unsupported_prefix_over_our_utility_fails_unless_allowed():
    t = copy.deepcopy(TAILWIND)
    t["utilities"].append({"name": "bg-acc", "reason": "x"})
    build(tailwind=t)  # a prefix ends at a dash: bg-accent isn't under bg-acc
    t["utilities"][t["utilities"].index("border")] = {"name": "border", "reason": "x"}
    with pytest.raises(gen.UtilError, match="has a reason but utilities.yaml renders it"):
        build(tailwind=t)
    s = copy.deepcopy(SPEC)
    s["theme"]["tokens"]["accent-wash"] = {"layer": "role", "type": "color", "value": "#fff"}
    t = copy.deepcopy(TAILWIND)
    t["utilities"].append({"name": "bg-accent", "reason": "x"})
    with pytest.raises(gen.UtilError, match="our 'bg-accent-wash' falls under unsupported 'bg-accent'"):
        build(tailwind=t, spec=s)


def test_value_sources_must_fit_the_property():
    v = copy.deepcopy(VOCAB)
    v["utilities"][0]["values"] = ["colors"]
    with pytest.raises(gen.UtilError, match="scale colors .* doesn't fit padding"):
        build(vocab=v)
    v = copy.deepcopy(VOCAB)
    v["utilities"][1]["decls"] = {"display": "var(--sv-nope)"}
    with pytest.raises(gen.UtilError, match="not a enum value|names no token"):
        build(vocab=v)


def test_variants_are_checked():
    v = copy.deepcopy(VOCAB)
    v["variants"]["aliases"]["aria-pressed"] = "pressed"
    with pytest.raises(gen.UtilError, match="not a state variant"):
        build(vocab=v)
    v = copy.deepcopy(VOCAB)
    v["variants"]["media"]["mac"] = "(-sv-platform: macos)"
    with pytest.raises(gen.UtilError, match="not a media feature"):
        build(vocab=v)
    v["variants"]["media"]["mac"] = {"query": "(-sv-platform: macos)", "requires": "E7"}
    t = copy.deepcopy(TAILWIND)
    assert build(vocab=v, tailwind=t)["variants"]["media"]["mac"]["pending"]
    t["variants"].append("rtl")
    with pytest.raises(gen.UtilError, match="Tailwind variant 'rtl' is neither ours nor given a reason"):
        build(tailwind=t)


def test_a_scale_pattern_that_matches_nothing_fails():
    v = copy.deepcopy(VOCAB)
    v["scales"]["radius"] = {"tokens": ["radius-*"]}
    with pytest.raises(gen.UtilError, match="matches no token"):
        build(vocab=v)


def test_checked_in_outputs_are_current():
    assert gen.main(["--check"]) == 0, "run python3 tools/metadata/style/gen_utilities.py"


def test_the_real_vocabulary_uses_our_tokens():
    m = gen.build(*gen.load_inputs())
    c = concrete(m)
    assert c["p-7"] == {"padding": "var(--sv-space-7)"}, "space-7 = 32px, not Tailwind's 28px"
    assert c["text-body"] == {"font-size": "var(--sv-font-size)"}
    assert c["text-label1"] == {"font-size": "var(--sv-font-size-label1)"}
    assert c["rounded"] == {"border-radius": "var(--sv-radius)"}
    assert c["bg-list-hover"] == {"background-color": "var(--sv-list-hover)"}
    assert c["h-row"] == {"height": "var(--sv-row-h)"}
    assert c["shadow-popup"] == {"box-shadow": "var(--sv-popup-shadow)"}
    assert not any(n.startswith(("bg-red", "text-base", "text-lg")) for n in c), "no Tailwind palette or type scale"
    assert "p-0-5" in c and "p-0.5" not in c
    assert {"name": "indicator", "node": False} in m["kinds"]["checkbox"]["parts"]
    assert "sv.overflow-top" in m["kinds"]["scroll-area"]["provider_states"]
    assert [s["name"] for s in m["variants"]["breakpoints"]["sizes"]] == ["sm", "md", "lg", "xl"]
    assert "focus-visible" in {s["name"] for s in m["variants"]["states"]}
    assert m["config"]["opt_in"] is True and m["config"]["prefix"] == "optional"
