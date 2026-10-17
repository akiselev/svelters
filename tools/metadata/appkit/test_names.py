"""The Python naming rules agree with the Rust ones (same pairs as names::tests::PAIRS)."""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import names  # noqa: E402

RUST = HERE.parents[2] / "crates/svelters-apple/src/names.rs"


def rust_pairs():
    text = RUST.read_text()
    block = text[text.index("pub const PAIRS"):]
    block = block[: block.index("];")]
    return re.findall(r'\("([^"]+)", "([^"]+)"\)', block)


def test_pairs_match_rust():
    pairs = rust_pairs()
    assert len(pairs) > 10
    for a, b in pairs:
        assert names.kebab(a) == b, (a, names.kebab(a), b)


def test_tags_and_selectors():
    assert names.class_tag("NSStackView") == "stack-view"
    assert names.class_tag("NSProgressIndicator") == "progress-indicator"
    assert names.selector_name("insertArrangedSubview:atIndex:") == "insert-arranged-subview-at-index"
    assert names.selector_name("makeKeyAndOrderFront:") == "make-key-and-order-front"
    assert names.notification_signal("NSControlTextDidChangeNotification") == "control-text-did-change"


if __name__ == "__main__":
    test_pairs_match_rust()
    test_tags_and_selectors()
    print("ok")
