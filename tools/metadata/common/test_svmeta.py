import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import svmeta


def test_merge_by_name_and_marks_manual():
    base = {"properties": [{"name": "label", "origin": "gir", "type": {"ref": "string"}}]}
    over = {"properties": [{"name": "label", "js_name": "text"}, {"name": "extra"}]}
    out = svmeta.merge(base, over)
    assert out["properties"][0]["js_name"] == "text"
    assert out["properties"][0]["origin"] == "gir"
    assert out["properties"][1] == {"name": "extra", "origin": "manual"}


def test_validate_minimal():
    k = {"schema": svmeta.SCHEMA, "toolkit": "gtk", "id": "x", "native": {}, "kind": "object"}
    assert svmeta.validate(k) == []
    assert svmeta.validate({**k, "toolkit": "tk"})


def test_write_json_deterministic(tmp_path):
    p = tmp_path / "a.json"
    assert svmeta.write_json(p, {"b": 1, "a": 2})
    assert not svmeta.write_json(p, {"a": 2, "b": 1})
