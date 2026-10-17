"""GTK4 specifics for the screenshot generator.

Usage (internal, driven by run.py):
  gtk.py list                  -> JSON {toolkit, kinds: {tag: {meta, recipe}}}
  gtk.py render TAG OUT.png    -> render one kind; failure reason on stderr
"""
import inspect
import json
import os
import sys
from pathlib import Path

os.environ.setdefault("GTK_THEME", "Adwaita")
os.environ.setdefault("GSETTINGS_BACKEND", "memory")
os.environ.setdefault("GTK_A11Y", "none")
os.environ.setdefault("GSK_RENDERER", "cairo")
os.environ["LC_ALL"] = "C"

REPO = Path(__file__).resolve().parents[3]
META_DIR = REPO / "metadata" / "gtk"
MAX_W, MAX_H = 480, 320
MARGIN = 12
WINDOW_SIZE = (360, 220)

# Kinds that cannot be shown as a standalone screenshot.
UNSUPPORTED = {
    "popover": "popover needs a parent widget and a grab",
    "popovermenu": "popover needs a parent widget and a grab",
    "emojichooser": "popover needs a parent widget and a grab",
    "dragicon": "drag icon exists only during a drag operation",
    "glarea": "needs a GL context, unavailable under Xvfb",
    "video": "needs a media backend",
    "mediacontrols": "needs a media stream",
}


def metadata():
    """tag -> (json path, gtype name) for every element kind."""
    out = {}
    for path in sorted(META_DIR.glob("*.json")):
        d = json.loads(path.read_bytes())
        if "element" in d:
            out[d["element"]["tag"]] = (path, d["native"]["gtype"])
    return out


# ---- recipes: r_<tag>(Gtk) -> widget ---------------------------------------

def r_window(Gtk):
    w = Gtk.ApplicationWindow(title="Counter")
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    for side in ("top", "bottom", "start", "end"):
        getattr(box, "set_margin_" + side)(12)
    box.append(Gtk.Label(label="Count: 0"))
    box.append(Gtk.Button(label="Increment"))
    w.set_child(box)
    return w


def r_box(Gtk):
    b = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    b.append(Gtk.Label(label="Hello"))
    b.append(Gtk.Button(label="Click me"))
    return b


def r_label(Gtk):
    return Gtk.Label(label="Hello")


def r_button(Gtk):
    return Gtk.Button(label="Click me")


def r_entry(Gtk):
    e = Gtk.Entry()
    e.set_text("Some text")
    return e


def r_checkbutton(Gtk):
    return Gtk.CheckButton(label="Check me", active=True)


def r_togglebutton(Gtk):
    return Gtk.ToggleButton(label="Toggle", active=True)


def r_switch(Gtk):
    return Gtk.Switch(active=True)


def r_scale(Gtk):
    s = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
    s.set_value(40)
    s.set_size_request(200, -1)
    return s


def r_progressbar(Gtk):
    p = Gtk.ProgressBar(fraction=0.6)
    p.set_size_request(200, -1)
    return p


def r_spinbutton(Gtk):
    return Gtk.SpinButton.new_with_range(0, 100, 1)


def r_headerbar(Gtk):
    h = Gtk.HeaderBar()
    h.set_title_widget(Gtk.Label(label="Title"))
    h.pack_start(Gtk.Button(label="Back"))
    h.set_size_request(320, -1)
    return h


def r_frame(Gtk):
    f = Gtk.Frame(label="Frame")
    f.set_child(Gtk.Label(label="Content"))
    return f


def r_notebook(Gtk):
    n = Gtk.Notebook()
    n.append_page(Gtk.Label(label="First page"), Gtk.Label(label="One"))
    n.append_page(Gtk.Label(label="Second page"), Gtk.Label(label="Two"))
    n.set_size_request(240, 100)
    return n


def r_stack(Gtk):
    s = Gtk.Stack()
    s.add_titled(Gtk.Label(label="Page one"), "one", "One")
    s.add_titled(Gtk.Label(label="Page two"), "two", "Two")
    return s


def r_listbox(Gtk):
    lb = Gtk.ListBox()
    for t in ("First", "Second", "Third"):
        lb.append(Gtk.Label(label=t, xalign=0))
    lb.set_size_request(200, -1)
    return lb


def r_textview(Gtk):
    tv = Gtk.TextView()
    tv.get_buffer().set_text("Some text\nOn two lines")
    tv.set_size_request(200, 80)
    return tv


def r_dropdown(Gtk):
    return Gtk.DropDown.new_from_strings(["Apple", "Banana", "Cherry"])


def r_linkbutton(Gtk):
    return Gtk.LinkButton.new_with_label("https://example.org", "Example")


def r_separator(Gtk):
    s = Gtk.Separator()
    s.set_size_request(160, -1)
    return s


def r_searchentry(Gtk):
    e = Gtk.SearchEntry()
    e.set_text("query")
    return e


def r_passwordentry(Gtk):
    e = Gtk.PasswordEntry(show_peek_icon=True)
    e.set_text("secret")
    return e


def r_levelbar(Gtk):
    lb = Gtk.LevelBar.new_for_interval(0, 10)
    lb.set_value(6)
    lb.set_size_request(200, -1)
    return lb


def r_expander(Gtk):
    e = Gtk.Expander(label="Details", expanded=True)
    e.set_child(Gtk.Label(label="Hidden content"))
    return e


def r_grid(Gtk):
    g = Gtk.Grid(row_spacing=6, column_spacing=6)
    for i, t in enumerate(("A", "B", "C", "D")):
        g.attach(Gtk.Button(label=t), i % 2, i // 2, 1, 1)
    return g


def r_paned(Gtk):
    p = Gtk.Paned()
    p.set_start_child(Gtk.Label(label="Start"))
    p.set_end_child(Gtk.Label(label="End"))
    p.set_size_request(240, 60)
    return p


def r_comboboxtext(Gtk):
    c = Gtk.ComboBoxText()
    for t in ("Apple", "Banana", "Cherry"):
        c.append_text(t)
    c.set_active(0)
    return c


def r_scrolledwindow(Gtk):
    s = Gtk.ScrolledWindow()
    s.set_child(Gtk.Label(label="Scrolled content"))
    s.set_size_request(200, 80)
    return s


def r_statusbar(Gtk):
    s = Gtk.Statusbar()
    s.push(s.get_context_id("demo"), "Ready")
    s.set_size_request(200, -1)
    return s


def r_image(Gtk):
    return Gtk.Image.new_from_icon_name("document-open-symbolic")


def r_actionbar(Gtk):
    a = Gtk.ActionBar()
    a.pack_start(Gtk.Button(label="Action"))
    a.set_size_request(240, -1)
    return a


RECIPES = {
    name[2:]: fn for name, fn in sorted(globals().items())
    if name.startswith("r_") and callable(fn)
}


def recipe_source(tag):
    fn = RECIPES.get(tag)
    return inspect.getsource(fn) if fn else "generic"


def toolkit_version():
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import Gtk
    return "gtk-%d.%d.%d" % (Gtk.get_major_version(), Gtk.get_minor_version(),
                             Gtk.get_micro_version())


def build(Gtk, GObject, tag, gtype):
    if tag in RECIPES:
        return RECIPES[tag](Gtk)
    cls = getattr(Gtk, gtype[3:], None)
    if cls is None or not issubclass(cls, Gtk.Widget):
        raise RuntimeError("not a widget type: " + gtype)
    w = GObject.new(cls)
    if hasattr(w, "set_child") and not isinstance(w, Gtk.Window):
        w.set_child(Gtk.Label(label="Hello"))
    return w


def render(tag, out_png):
    import gi
    gi.require_version("Gtk", "4.0")
    gi.require_version("Graphene", "1.0")
    from gi.repository import GLib, GObject, Graphene, Gtk

    meta = metadata()
    if tag not in meta:
        raise RuntimeError("unknown tag: " + tag)
    if tag in UNSUPPORTED:
        raise RuntimeError(UNSUPPORTED[tag])
    if not Gtk.init_check():
        raise RuntimeError("no display")
    Gtk.Settings.get_default().set_property("gtk-enable-animations", False)

    widget = build(Gtk, GObject, tag, meta[tag][1])
    if isinstance(widget, Gtk.Window):
        win = widget
        win.set_default_size(*WINDOW_SIZE)
    else:
        for side in ("top", "bottom", "start", "end"):
            getattr(widget, "set_margin_" + side)(MARGIN)
        widget.set_halign(Gtk.Align.START)
        widget.set_valign(Gtk.Align.START)
        win = Gtk.Window(decorated=False)
        win.set_child(widget)
    win.present()

    # Pump until mapped, then keep iterating so layout and first frames land.
    ctx = GLib.MainContext.default()
    deadline = GLib.get_monotonic_time() + 5_000_000
    mapped_at = None
    while GLib.get_monotonic_time() < deadline:
        while ctx.pending():
            ctx.iteration(False)
        now = GLib.get_monotonic_time()
        if win.get_mapped() and mapped_at is None:
            mapped_at = now
        if mapped_at is not None and now - mapped_at > 300_000:
            break
        GLib.usleep(5000)
    else:
        raise RuntimeError("window never became ready")

    w = min(win.get_width(), MAX_W)
    h = min(win.get_height(), MAX_H)
    if w <= 0 or h <= 0:
        raise RuntimeError("empty window allocation")
    snap = Gtk.Snapshot()
    Gtk.WidgetPaintable.new(win).snapshot(snap, w, h)
    node = snap.to_node()
    if node is None:
        raise RuntimeError("empty render node")
    renderer = win.get_native().get_renderer()
    texture = renderer.render_texture(node, Graphene.Rect().init(0, 0, w, h))
    if not texture.save_to_png(str(out_png)):
        raise RuntimeError("save_to_png failed")


def main(argv):
    if argv and argv[0] == "list":
        meta = metadata()
        print(json.dumps({
            "toolkit": toolkit_version(),
            "kinds": {t: {"meta": str(p.relative_to(REPO)), "recipe": recipe_source(t)}
                      for t, (p, _g) in meta.items()},
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
