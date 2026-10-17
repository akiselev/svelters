"""Qt (PySide6) specifics for the screenshot generator.

Usage (internal, driven by run.py):
  qt.py list                   -> JSON {toolkit, kinds: {tag: {meta, recipe}}}
  qt.py render TAG OUT.png     -> render one kind; failure reason on stderr
"""
import inspect
import json
import os
import re
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "xcb" if os.environ.get("DISPLAY") else "offscreen")
os.environ["LC_ALL"] = "C"

REPO = Path(__file__).resolve().parents[3]
OVERRIDES = REPO / "metadata" / "overrides" / "qt"
MAX_W, MAX_H = 480, 320
MARGIN = 12
WINDOW_SIZE = (360, 220)

_TAG_RE = re.compile(r"^  - tag: ([\w-]+)\s*$", re.M)


def metadata():
    """tag -> override yaml path, for every top-level element entry."""
    out = {}
    for path in sorted(OVERRIDES.glob("*.yaml")):
        for tag in _TAG_RE.findall(path.read_text()):
            out[tag] = path
    return out


# ---- recipes: r_<tag with - as _>(QtWidgets) -> widget ----------------------

def r_main_window(W):
    win = W.QMainWindow()
    win.setWindowTitle("Counter")
    central = W.QWidget()
    lay = W.QBoxLayout(W.QBoxLayout.TopToBottom, central)
    lay.addWidget(W.QLabel("Count: 0"))
    lay.addWidget(W.QPushButton("Increment"))
    win.setCentralWidget(central)
    win.resize(*WINDOW_SIZE)
    return win


def r_box_layout(W):
    box = W.QWidget()
    lay = W.QBoxLayout(W.QBoxLayout.TopToBottom, box)
    lay.setSpacing(8)
    lay.addWidget(W.QLabel("Hello"))
    lay.addWidget(W.QPushButton("Click me"))
    return box


def r_label(W):
    return W.QLabel("Hello")


def r_push_button(W):
    return W.QPushButton("Click me")


def r_line_edit(W):
    e = W.QLineEdit()
    e.setText("Some text")
    return e


def r_scroll_area(W):
    s = W.QScrollArea()
    s.setWidget(W.QLabel("Scrolled content"))
    s.setFixedSize(200, 80)
    return s


def r_status_bar(W):
    s = W.QStatusBar()
    s.showMessage("Ready")
    s.setFixedWidth(200)
    return s


def r_menu_bar(W):
    m = W.QMenuBar()
    m.addMenu("File")
    m.addMenu("Edit")
    m.setFixedWidth(200)
    return m


RECIPES = {
    name[2:].replace("_", "-"): fn for name, fn in sorted(globals().items())
    if name.startswith("r_") and callable(fn)
}


def recipe_source(tag):
    fn = RECIPES.get(tag)
    return inspect.getsource(fn) if fn else ""


def toolkit_version():
    import PySide6
    return "qt-pyside6-" + PySide6.__version__


def render(tag, out_png):
    from PySide6 import QtWidgets

    if tag not in RECIPES:
        raise RuntimeError("no recipe for tag: " + tag)
    app = QtWidgets.QApplication([])
    app.setStyle("Fusion")
    widget = RECIPES[tag](QtWidgets)
    if not isinstance(widget, QtWidgets.QMainWindow):
        # Wrap so the standalone widget gets a margin like the GTK renders.
        holder = QtWidgets.QWidget()
        lay = QtWidgets.QVBoxLayout(holder)
        lay.setContentsMargins(MARGIN, MARGIN, MARGIN, MARGIN)
        lay.setSizeConstraint(QtWidgets.QLayout.SetFixedSize)
        lay.addWidget(widget, 0)
        widget = holder
    widget.show()
    for _ in range(5):
        app.processEvents()
    widget.adjustSize()
    pix = widget.grab()
    if pix.isNull() or pix.width() == 0:
        raise RuntimeError("empty grab")
    pix = pix.copy(0, 0, min(pix.width(), MAX_W), min(pix.height(), MAX_H))
    if not pix.save(str(out_png), "PNG"):
        raise RuntimeError("save failed")


def main(argv):
    if argv and argv[0] == "list":
        meta = metadata()
        print(json.dumps({
            "toolkit": toolkit_version(),
            "kinds": {t: {"meta": str(p.relative_to(REPO)), "recipe": recipe_source(t)}
                      for t, p in meta.items()},
        }))
        return 0
    if len(argv) == 3 and argv[0] == "render":
        try:
            render(argv[1], argv[2])
        except Exception as e:  # the runner records this text as the reason
            print("%s: %s" % (type(e).__name__, e), file=sys.stderr)
            return 1
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
