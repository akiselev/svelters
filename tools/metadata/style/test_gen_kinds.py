"""The checked-in kinds table (packages/style/src/kinds.generated.js) covers every element tag and
agrees with the metadata. Regenerating it needs a display and PyGObject (see gen_kinds.py); this
test needs neither."""
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("gen_kinds", HERE / "gen_kinds.py")
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)


def kinds():
    text = gen.OUT.read_text()
    body = text[text.index("export const KINDS = ") + len("export const KINDS = "):]
    return json.loads(body.rstrip().rstrip(";"))


def test_every_element_tag_is_in_the_table():
    table = kinds()
    assert set(gen.gtk_tags()) | set(gen.qt_tags()) == set(table)


def test_qt_classes_match_the_overrides():
    table = kinds()
    for tag, cls in gen.qt_tags().items():
        assert table[tag]["qt"] == cls


def test_ambiguity_means_a_shared_name():
    table = kinds()
    by_css, by_qt = {}, {}
    for tag, k in table.items():
        if "gtk" in k:
            by_css.setdefault(k["gtk"]["css"], []).append(tag)
        if "qt" in k:
            by_qt.setdefault(k["qt"], []).append(tag)
    for tag, k in table.items():
        want = []
        if "gtk" in k and len(by_css[k["gtk"]["css"]]) > 1:
            want.append("gtk")
        if "qt" in k and len(by_qt[k["qt"]]) > 1:
            want.append("qt")
        assert k.get("ambiguous", []) == want, tag
