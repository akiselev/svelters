"""The shared naming rule (naming.py): acronyms, prefixes, and agreement with the Rust mirror."""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import naming  # noqa: E402

RUST = HERE.parents[2] / "crates/svelters-apple/src/names.rs"


def test_pairs_match_rust():
    text = RUST.read_text()
    block = text[text.index("pub const PAIRS"):]
    pairs = re.findall(r'\("([^"]+)", "([^"]+)"\)', block[: block.index("];")])
    assert len(pairs) > 10
    for a, b in pairs:
        assert naming.kebab(a) == b, (a, naming.kebab(a), b)


def test_acronyms():
    assert naming.tag_for("qt", "QLCDNumber") == "lcd-number"
    assert naming.tag_for("gtk", "GLArea") == "gl-area"
    assert naming.tag_for("gtk", "GtkGLArea") == "gl-area"
    assert naming.tag_for("qt", "QHBoxLayout") == "h-box-layout"
    assert naming.tag_for("qt", "QOpenGLWidget") == "open-gl-widget"
    assert naming.kebab("URLString") == "url-string"
    assert naming.kebab("Vector3D") == "vector3-d"


def test_tags():
    assert naming.tag_for("qt", "QMainWindow") == "main-window"
    assert naming.tag_for("qt", "QPushButton") == "push-button"
    assert naming.tag_for("qt", "QLineEdit") == "line-edit"
    assert naming.tag_for("gtk", "EventControllerKey") == "event-controller-key"
    assert naming.tag_for("gtk", "GestureClick") == "gesture-click"
    assert naming.tag_for("appkit", "NSStackView") == "stack-view"
    assert naming.tag_for("winui", "Microsoft.UI.Xaml.Controls.StackPanel") == "stack-panel"
    assert naming.tag_for("adw", "AdwHeaderBar") == "ad-header-bar"


def test_members():
    assert naming.kebab("minimumWidth") == "minimum-width"
    assert naming.kebab("textChanged") == "text-changed"
    assert naming.kebab("column-span") == "column-span"
    assert naming.kebab("toolTip") == "tool-tip"
    assert naming.event_attr("text-changed") == "ontext-changed"
