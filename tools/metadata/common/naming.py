"""The one native-name -> JS-name rule, shared by every pipeline (GTK, Qt, AppKit, WinUI ingest and
generators, the semantics generator, the style kind table and the docs generator).

- Element tag: the native class name without the framework prefix, in kebab-case
  (`QMainWindow` -> `main-window`, `GtkDrawingArea` -> `drawing-area`, `NSStackView` -> `stack-view`).
  A toolkit whose tags would collide with another family's carries a tag prefix instead
  (libadwaita, later: `AdwHeaderBar` -> `ad-header-bar`); see TAG_PREFIX.
- Props, events (signals), child attributes and `bind:` names: the native name in kebab-case
  (`minimumWidth` -> `minimum-width`, `textChanged` -> `text-changed`); GObject names are already kebab.
- Event handler attributes are `on<event>` (`ontext-changed`).

Acronym rule (kebab): a run of capitals is one word, except that its last capital starts the next
word when a lowercase letter follows. So `QLCDNumber` -> `lcd-number`, `GtkGLArea` -> `gl-area`,
`QHBoxLayout` -> `h-box-layout`, `URLString` -> `url-string`, `QOpenGLWidget` -> `open-gl-widget`.
Digits stay with the preceding word and a capital after a digit starts a new one (`Vector3D` ->
`vector3-d`). `_`, `:` and `-` are word separators. `crates/svelters-apple/src/names.rs` implements the
same function in Rust; both are tested against its `PAIRS`.
"""
from __future__ import annotations

# Native class prefix stripped from a class name to get its tag, and the prefix the tag gets.
# libadwaita (not wired yet) keeps its widgets apart from GTK's with an `ad-` tag prefix.
CLASS_PREFIX = {"gtk": "Gtk", "qt": "Q", "appkit": "NS", "winui": "", "adw": "Adw"}
TAG_PREFIX = {"adw": "ad-"}


def kebab(ident: str) -> str:
    """camelCase / PascalCase / snake_case -> kebab-case (see the module docstring for acronyms)."""
    out: list[str] = []
    for i, c in enumerate(ident):
        if c in "_:-":
            if out and out[-1] != "-":
                out.append("-")
            continue
        if c.isascii() and c.isupper() and i > 0 and out and out[-1] != "-":
            prev = ident[i - 1]
            nxt = ident[i + 1] if i + 1 < len(ident) else ""
            if (prev.isascii() and (prev.islower() or prev.isdigit())) or (prev.isupper() and nxt.isascii() and nxt.islower()):
                out.append("-")
        out.append(c.lower() if c.isascii() else c)
    return "".join(out).rstrip("-")


def strip_prefix(cls: str, prefix: str) -> str:
    """`cls` without `prefix`, only when the prefix is followed by an uppercase letter (`QLabel` ->
    `Label`, but `Quaternion` stays). Dotted (WinUI) names keep only their last segment."""
    cls = cls.rsplit(".", 1)[-1]
    if prefix and cls.startswith(prefix) and len(cls) > len(prefix) and cls[len(prefix)].isupper():
        return cls[len(prefix):]
    return cls


def class_tag(cls: str, prefix: str = "") -> str:
    """Tag of a native class with the given class prefix: `class_tag("QPushButton", "Q")` -> `push-button`."""
    return kebab(strip_prefix(cls, prefix))


def tag_for(toolkit: str, cls: str) -> str:
    """Element tag of a native class of `toolkit` (`gtk`, `qt`, `appkit`, `winui`, `adw`). GTK GIR names
    come without the `Gtk` prefix (`DrawingArea`), which is accepted too."""
    return TAG_PREFIX.get(toolkit, "") + class_tag(cls, CLASS_PREFIX[toolkit])


def event_attr(event: str) -> str:
    """Svelte handler attribute of an event: `text-changed` -> `ontext-changed`."""
    return "on" + event
