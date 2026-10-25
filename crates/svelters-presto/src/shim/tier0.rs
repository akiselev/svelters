//! The Tier 0 kinds' specs (shim for presto-widgets `controls`, plus `text-edit` from `text` and the
//! base `Widget` and `Window` from presto-core), transcribed from `metadata/semantics/egui.json` so the
//! JS-facing names, types and defaults are egui's. Change classes, passivity and cancelability are
//! presto's (architecture §4.3, §11.5). Generated once by the lane's transcription script; the
//! consolidation replaces this file with the kinds' own `KindSpec`s.

use super::*;
use presto_base::change::ChangeClass;

static WIDGET_PROPS: &[PropSpec] = &[
    PropSpec { name: "accessible-name", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "AccessKit name (`Response::accessible_name`); inputs default to their hint or text." },
    PropSpec { name: "align-self", ty: ValueType::Enum(&["auto", "start", "end", "center", "stretch", "baseline"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Cross-axis alignment in the parent box, overriding its `align-items`." },
    PropSpec { name: "anchor", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "A name unique in its embedded root: the node's egui id derives from it (stable across runs, not only across reorders), it is the AccessKit author id, and `Embed::anchor_rect` finds it. Changing it drops the node's egui memory." },
    PropSpec { name: "anchor-scope", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Set by the runtime on the top nodes of a keyed `{#each}` item: the item's key, and the keys of the keyed items around it (outermost first, separated by U+001F). An `anchor` inside it takes its egui id from these keys and the anchors of its anchored ancestors, so repeated anchors in a keyed list get one id per row; `key=` selectors match it. Not for apps to set." },
    PropSpec { name: "autofocus", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Takes keyboard focus when it first appears (in a `dialog` scope: when the dialog opens)." },
    PropSpec { name: "bottom", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Inset from the containing block's bottom edge in points; null for auto." },
    PropSpec { name: "dialog-role", ty: ValueType::Enum(&["auto", "accept", "reject", "destructive", "apply", "reset", "help", "action"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Its place in a `dialog-buttons` row (QDialogButtonBox roles): `accept`, `reject`, `destructive`, `apply`, `reset`, `help`, `action`; `auto` is `accept` for a `default` button, `reject` for a `cancel` one, else `action`. Also the `[dialog-role=…]` selector." },
    PropSpec { name: "drag-data", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "The drag payload's data (JSON for structured data); `drop` delivers it." },
    PropSpec { name: "drag-type", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Makes the node a drag source: the payload's type, a MIME-like string (`cad/part`); empty for none." },
    PropSpec { name: "drop-types", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Makes the node a drop target: the drag types it accepts, separated by spaces or commas (`*` any, `cad/*` a prefix); empty for none." },
    PropSpec { name: "enabled", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Paint, native: false, doc: "Takes input when true; drawn greyed out otherwise (`Ui::add_enabled`)." },
    PropSpec { name: "flex-basis", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Main-axis size before growing or shrinking; null for auto." },
    PropSpec { name: "flex-grow", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Share of the parent's free main-axis space." },
    PropSpec { name: "flex-shrink", ty: ValueType::F64, default: DefaultValue::F64(1.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Share of the parent's main-axis overflow to give up." },
    PropSpec { name: "height", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Height in points; null for auto." },
    PropSpec { name: "invalid", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Marks the value as invalid: an outline in the error colour (`Visuals::error_fg_color`) and AccessKit `invalid`. The app sets it; the widget never does." },
    PropSpec { name: "left", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Inset from the containing block's left edge in points (`position: absolute` or `relative`); null for auto." },
    PropSpec { name: "margin", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Outer spacing on all four sides, in points." },
    PropSpec { name: "max-height", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Maximum height in points; null for none." },
    PropSpec { name: "max-width", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Maximum width in points; null for none." },
    PropSpec { name: "min-height", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Minimum height in points; null for auto." },
    PropSpec { name: "min-width", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Minimum width in points; null for auto." },
    PropSpec { name: "position", ty: ValueType::Enum(&["static", "relative", "absolute"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "`absolute` takes the node out of the flow and places it by `left`/`top`/`right`/`bottom` inside its containing block: the nearest `relative` or `absolute` ancestor, else the root." },
    PropSpec { name: "right", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Inset from the containing block's right edge in points; null for auto." },
    PropSpec { name: "source-loc", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Set by the runtime in dev builds: where the node was declared, as `file#offset` (the template element's byte offset in its `.svelte` file), then the component call sites it renders inside, innermost first, separated by U+001F. The visual designer and the inspector map a node to its source with it (`NodeRef::source_loc`, `source=` selectors). Not for apps to set." },
    PropSpec { name: "status-section", ty: ValueType::Enum(&["temporary", "start", "end"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Its place in a `status-bar` (QStatusBar): `temporary` (after the start sections; hidden while a message or status tip shows), `start` (permanent, at the start), `end` (permanent, pushed to the end). Also the `[status-section=…]` selector." },
    PropSpec { name: "status-tip", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Text for the status bar while the pointer is over the node (enabled or not; the innermost node with one wins) or, with nothing hovered, while it has keyboard focus: every `status-bar` of the viewport shows it in `::part(message)`, and `svelters_egui::status_tip` reads it. Empty for none." },
    PropSpec { name: "tooltip", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Hover text (`Response::on_hover_text`); empty for none." },
    PropSpec { name: "top", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Inset from the containing block's top edge in points; null for auto." },
    PropSpec { name: "visible", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shown and laid out when true (`display: none` otherwise)." },
    PropSpec { name: "width", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Width in points; null for auto." },
];
static WIDGET_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "animationend", args: &[("id", ValueType::I64)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "A native animation (TR2) finished; carries its id. The runtime listens to it itself." },
    SignalSpec { name: "click", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "The primary button was pressed and released; targets the innermost node that holds both points (EV1). `preventDefault` keeps the click from the widget." },
    SignalSpec { name: "contextmenu", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "The secondary button went down (EV1), after `pointerdown`. `preventDefault` keeps the press from egui (no context menu)." },
    SignalSpec { name: "dblclick", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A second click on the same node in a short time, after its `click` (EV1)." },
    SignalSpec { name: "drag-begin", args: &[], passive: true, cancelable: false, sync: false, bubbles: false, doc: "Drag source: the user started dragging this node (`drag-type` set)." },
    SignalSpec { name: "drag-end", args: &[("dropped", ValueType::Bool), ("action", ValueType::Str)], passive: true, cancelable: false, sync: false, bubbles: false, doc: "Drag source: the drag ended; `dropped` is true when a target took it, `action` how (`move`, `copy`, `link`; `none` when not dropped)." },
    SignalSpec { name: "drag-enter", args: &[("type", ValueType::Str)], passive: true, cancelable: false, sync: false, bubbles: false, doc: "Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type." },
    SignalSpec { name: "drag-leave", args: &[], passive: true, cancelable: false, sync: false, bubbles: false, doc: "Drop target: the drag left, was cancelled, or was dropped here (before `drop`)." },
    SignalSpec { name: "drop", args: &[("type", ValueType::Str), ("data", ValueType::Str), ("x", ValueType::F64), ("y", ValueType::F64), ("action", ValueType::Str)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Drop target: a drag it accepts was released over it; carries the type, the data, the point in the node's own points and the drop action from the modifiers (`move`; `copy` with Ctrl, Option on macOS; `link` with Ctrl+Shift, Cmd+Option on macOS). Files dragged in from the OS arrive as `text/uri-list` (one `file://` URI per line) with `copy` on a target that accepts that type." },
    SignalSpec { name: "focusin", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("related", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "This node or a descendant took keyboard focus (EV1; after the frame that moved it). `related` is the node that lost it, 0 for none." },
    SignalSpec { name: "focusout", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("related", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "This node or a descendant lost keyboard focus (EV1). `related` is the node that took it, 0 for none." },
    SignalSpec { name: "keydown", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("key", ValueType::Str), ("repeat", ValueType::Bool), ("modifiers", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A key went down while this node or a descendant had focus (EV1); `key` is egui's key name. `preventDefault` keeps the key (and the text it types) from the focused widget." },
    SignalSpec { name: "keyup", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("key", ValueType::Str), ("repeat", ValueType::Bool), ("modifiers", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A key went up while this node or a descendant had focus (EV1)." },
    SignalSpec { name: "pointerdown", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A pointer button went down over this node or a descendant (EV1: capture, target, bubble). `preventDefault` keeps the press from egui (no click, drag or focus)." },
    SignalSpec { name: "pointermove", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: true, cancelable: true, sync: true, bubbles: false, doc: "The pointer moved over this node or a descendant (EV1 propagation; CV1): one per frame at the last point, `button` -1, `buttons` the held ones, `offsetX`/`offsetY` relative to the target. Not cancelable: egui always sees the move." },
    SignalSpec { name: "pointerup", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A pointer button went up over this node or a descendant (EV1). `preventDefault` cancels the click." },
    SignalSpec { name: "wheel", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("dx", ValueType::F64), ("dy", ValueType::F64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64)], passive: true, cancelable: true, sync: true, bubbles: true, doc: "The mouse wheel or a trackpad scrolled over this node (EV1); `dx`, `dy` in points. `preventDefault` keeps it from scrolling." },
];
static WIDGET_METHODS: &[MethodSpec] = &[
    MethodSpec { name: "scroll-into-view", params: &[], ret: None, doc: "`scroll-into-view(align?)`: scrolls the scroll areas around the node so it is visible on the next frame, innermost first; `align` is `nearest` (the default: the least scrolling, keeping the area's `scroll-margin`) or `center`. Qt's `ensureWidgetVisible`, the DOM's `scrollIntoView`." },
];
static WIDGET_SLOTS: &[SlotSpec] = &[
];
pub static WIDGET: KindSpec = KindSpec {
    name: "Widget",
    tag: None,
    extends: None,
    doc: "Common properties of everything placed in a window: state, tooltip, accessibility and its taffy item style.",
    props: WIDGET_PROPS,
    signals: WIDGET_SIGNALS,
    methods: WIDGET_METHODS,
    slots: WIDGET_SLOTS,
    a11y: A11ySpec { role: "", name_from: &["accessible-name"], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static WINDOW_PROPS: &[PropSpec] = &[
    PropSpec { name: "default-height", ty: ValueType::F64, default: DefaultValue::F64(360.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Initial inner height in points." },
    PropSpec { name: "default-width", ty: ValueType::F64, default: DefaultValue::F64(480.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Initial inner width in points." },
    PropSpec { name: "gap", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Space between the window's children, in points." },
    PropSpec { name: "padding", ty: ValueType::F64, default: DefaultValue::F64(8.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Inner spacing of the content area, in points." },
    PropSpec { name: "source-loc", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Set by the runtime in dev builds: where the window was declared (`file#offset`, then the component call sites around it, U+001F-separated). Not for apps to set." },
    PropSpec { name: "title", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Window title (`ViewportCommand::Title`)." },
];
static WINDOW_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "click", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "The primary button was pressed and released; targets the innermost node that holds both points (EV1). `preventDefault` keeps the click from the widget." },
    SignalSpec { name: "close-request", args: &[], passive: false, cancelable: true, sync: true, bubbles: false, doc: "The user asked to close the window; return true to keep it open. Runs before the frame's pass." },
    SignalSpec { name: "contextmenu", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "The secondary button went down (EV1), after `pointerdown`. `preventDefault` keeps the press from egui (no context menu)." },
    SignalSpec { name: "dblclick", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A second click on the same node in a short time, after its `click` (EV1)." },
    SignalSpec { name: "focusin", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("related", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "This node or a descendant took keyboard focus (EV1; after the frame that moved it). `related` is the node that lost it, 0 for none." },
    SignalSpec { name: "focusout", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("related", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "This node or a descendant lost keyboard focus (EV1). `related` is the node that took it, 0 for none." },
    SignalSpec { name: "keydown", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("key", ValueType::Str), ("repeat", ValueType::Bool), ("modifiers", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A key went down while this node or a descendant had focus (EV1); `key` is egui's key name. `preventDefault` keeps the key (and the text it types) from the focused widget." },
    SignalSpec { name: "keyup", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("key", ValueType::Str), ("repeat", ValueType::Bool), ("modifiers", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A key went up while this node or a descendant had focus (EV1)." },
    SignalSpec { name: "pointerdown", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A pointer button went down over this node or a descendant (EV1: capture, target, bubble). `preventDefault` keeps the press from egui (no click, drag or focus)." },
    SignalSpec { name: "pointermove", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: true, cancelable: true, sync: true, bubbles: false, doc: "The pointer moved over this node or a descendant (EV1 propagation; CV1): one per frame at the last point, `button` -1, `buttons` the held ones, `offsetX`/`offsetY` relative to the target. Not cancelable: egui always sees the move." },
    SignalSpec { name: "pointerup", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("button", ValueType::I64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64), ("buttons", ValueType::I64)], passive: false, cancelable: true, sync: true, bubbles: true, doc: "A pointer button went up over this node or a descendant (EV1). `preventDefault` cancels the click." },
    SignalSpec { name: "wheel", args: &[("target", ValueType::I64), ("phase", ValueType::I64), ("x", ValueType::F64), ("y", ValueType::F64), ("dx", ValueType::F64), ("dy", ValueType::F64), ("modifiers", ValueType::I64), ("ox", ValueType::F64), ("oy", ValueType::F64)], passive: true, cancelable: true, sync: true, bubbles: true, doc: "The mouse wheel or a trackpad scrolled over this node (EV1); `dx`, `dy` in points. `preventDefault` keeps it from scrolling." },
];
static WINDOW_METHODS: &[MethodSpec] = &[
    MethodSpec { name: "close", params: &[], ret: None, doc: "Closes the window as the user would (no close-request)." },
    MethodSpec { name: "present", params: &[], ret: None, doc: "Shows the window (the first one presented is the root viewport)." },
];
static WINDOW_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "content", accepts: &["Widget"], many: true },
];
pub static WINDOW: KindSpec = KindSpec {
    name: "Window",
    tag: Some("window"),
    extends: None,
    doc: "A native window (the root viewport, or an immediate viewport); its children are laid out as a flex column.",
    props: WINDOW_PROPS,
    signals: WINDOW_SIGNALS,
    methods: WINDOW_METHODS,
    slots: WINDOW_SLOTS,
    a11y: A11ySpec { role: "Window", name_from: &[], name_required: false },
    text_content: None,
    toplevel: true,
    native_state: &[],
};

static BOX_PROPS: &[PropSpec] = &[
    PropSpec { name: "align-items", ty: ValueType::Enum(&["start", "end", "center", "stretch", "baseline"]), default: DefaultValue::Enum(3), nullable: false, change: ChangeClass::Layout, native: false, doc: "Cross-axis alignment of the children." },
    PropSpec { name: "column-gap", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Space between columns in points; null to use `gap`." },
    PropSpec { name: "flex-direction", ty: ValueType::Enum(&["column", "row", "column-reverse", "row-reverse"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Main axis of the box." },
    PropSpec { name: "flex-wrap", ty: ValueType::Enum(&["nowrap", "wrap", "wrap-reverse"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Whether children wrap onto more lines." },
    PropSpec { name: "focus-scope", ty: ValueType::Enum(&["none", "contain", "dialog"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "`contain`: Tab and Shift+Tab cycle inside while focus is in it; `dialog`: also takes focus when it appears (`autofocus` node, else the first) and gives it back to the opener when it goes away." },
    PropSpec { name: "gap", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Space between children on both axes, in points (row-gap and column-gap override it)." },
    PropSpec { name: "justify-content", ty: ValueType::Enum(&["start", "end", "center", "space-between", "space-around", "space-evenly"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Main-axis distribution of the children." },
    PropSpec { name: "padding", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Inner spacing on all four sides, in points." },
    PropSpec { name: "row-gap", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Space between rows in points; null to use `gap`." },
];
static BOX_SIGNALS: &[SignalSpec] = &[
];
static BOX_METHODS: &[MethodSpec] = &[
];
static BOX_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "children", accepts: &["Widget"], many: true },
];
pub static BOX: KindSpec = KindSpec {
    name: "Box",
    tag: Some("box"),
    extends: Some(&WIDGET),
    doc: "A flexbox container laid out by taffy; draws nothing itself.",
    props: BOX_PROPS,
    signals: BOX_SIGNALS,
    methods: BOX_METHODS,
    slots: BOX_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static FRAME_PROPS: &[PropSpec] = &[
    PropSpec { name: "checkable", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "WIDGET-PARITY §2.16 (QGroupBox `checkable`): a check before the caption; unchecked\ndisables the content." },
    PropSpec { name: "checked", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Paint, native: true, doc: "Checked (`bind:checked`), when `checkable`." },
    PropSpec { name: "frame-style", ty: ValueType::Enum(&["group", "canvas", "window", "menu", "popup", "panel", "none"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "The egui frame drawn around the content: `egui::Frame::group`, `canvas`, `window`, `menu`, `popup`, `side_top_panel` (`panel`), or no chrome (`none`). Its inner margin adds to `padding`." },
    PropSpec { name: "label", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "A caption above the content (GTK's `Gtk.Frame:label`); empty for none." },
];
static FRAME_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("value", ValueType::Bool)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user toggled the check; carries the new state." },
];
static FRAME_METHODS: &[MethodSpec] = &[
];
static FRAME_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "children", accepts: &["Widget"], many: true },
];
pub static FRAME: KindSpec = KindSpec {
    name: "Frame",
    tag: Some("frame"),
    extends: Some(&BOX),
    doc: "A flex box drawn inside an egui frame (background, border, rounding, inner margin), with an optional caption.",
    props: FRAME_PROPS,
    signals: FRAME_SIGNALS,
    methods: FRAME_METHODS,
    slots: FRAME_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &["checked"],
};

static LABEL_PROPS: &[PropSpec] = &[
    PropSpec { name: "elide", ty: ValueType::Enum(&["none", "start", "middle", "end"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "WG6 QW13: cut one line with `…` to the width: `none`, `start`, `middle`, `end`." },
    PropSpec { name: "font-weight", ty: ValueType::I64, default: DefaultValue::I64(400), nullable: false, change: ChangeClass::Layout, native: false, doc: "CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face." },
    PropSpec { name: "for", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "The `anchor` of the control this label names (Qt's buddy): a click focuses it." },
    PropSpec { name: "selectable", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "The text can be selected and copied." },
    PropSpec { name: "selection-end", ty: ValueType::I64, default: DefaultValue::I64(0), nullable: false, change: ChangeClass::Paint, native: true, doc: "The selection's end, in chars (`bind:`); a negative end is the end of the text." },
    PropSpec { name: "selection-start", ty: ValueType::I64, default: DefaultValue::I64(0), nullable: false, change: ChangeClass::Paint, native: true, doc: "The selection's start, in chars of the shown text (`bind:`)." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The text." },
    PropSpec { name: "wrap-mode", ty: ValueType::Enum(&["wrap", "truncate", "extend"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "What a line longer than the width does (`egui::TextWrapMode`)." },
];
static LABEL_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released (`Response::clicked`)." },
];
static LABEL_METHODS: &[MethodSpec] = &[
];
static LABEL_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "runs", accepts: &["Span", "Bold", "Italic", "Code", "Anchor"], many: true },
];
pub static LABEL: KindSpec = KindSpec {
    name: "Label",
    tag: Some("label"),
    extends: Some(&WIDGET),
    doc: "Static text; with inline run children (`<b>`, `<i>`, `<code>`, `<a>`, `<span>`) one paragraph of styled runs (RT2).",
    props: LABEL_PROPS,
    signals: LABEL_SIGNALS,
    methods: LABEL_METHODS,
    slots: LABEL_SLOTS,
    a11y: A11ySpec { role: "Label", name_from: &["accessible-name", "text"], name_required: false },
    text_content: Some("text"),
    toplevel: false,
    native_state: &["selection-end", "selection-start"],
};

static BUTTON_PROPS: &[PropSpec] = &[
    PropSpec { name: "cancel", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Escape activates it while focus is in its focus scope (or root)." },
    PropSpec { name: "default", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Enter activates it while focus is in its focus scope (or root) and the focused node does not take Enter itself." },
    PropSpec { name: "font-weight", ty: ValueType::I64, default: DefaultValue::I64(400), nullable: false, change: ChangeClass::Layout, native: false, doc: "CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face." },
    PropSpec { name: "frame", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Layout, native: false, doc: "Draws the button background and border." },
    PropSpec { name: "icon", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "An icon before the caption: an icon key (`name` in the `icons` family, or `family:name`).\nIts size, place and visibility are CSS on `::part(icon)` (P3)." },
    PropSpec { name: "repeat", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Held down, it activates again and again (`clicked` after 300 ms, then every 100 ms;\nQAbstractButton `autoRepeat`)." },
    PropSpec { name: "selected", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Drawn as selected (a toggle look); clicking does not change it." },
    PropSpec { name: "small", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "No vertical padding (`Button::small`)." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The caption." },
];
static BUTTON_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released (`Response::clicked`)." },
    SignalSpec { name: "double-clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Double click (`Response::double_clicked`)." },
    SignalSpec { name: "press", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The primary button went down on it (QAbstractButton `pressed`)." },
    SignalSpec { name: "release", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "It came up again, over the button or not (`released`)." },
    SignalSpec { name: "secondary-clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Right click (`Response::secondary_clicked`)." },
];
static BUTTON_METHODS: &[MethodSpec] = &[
];
static BUTTON_SLOTS: &[SlotSpec] = &[
];
pub static BUTTON: KindSpec = KindSpec {
    name: "Button",
    tag: Some("button"),
    extends: Some(&WIDGET),
    doc: "A push button.",
    props: BUTTON_PROPS,
    signals: BUTTON_SIGNALS,
    methods: BUTTON_METHODS,
    slots: BUTTON_SLOTS,
    a11y: A11ySpec { role: "Button", name_from: &["accessible-name", "text", "tooltip (when `text` is empty)"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &[],
};

static TOGGLEBUTTON_PROPS: &[PropSpec] = &[
    PropSpec { name: "checked", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: true, doc: "Pressed state; a click toggles it (with `radio`, a click only sets it)." },
    PropSpec { name: "font-weight", ty: ValueType::I64, default: DefaultValue::I64(400), nullable: false, change: ChangeClass::Layout, native: false, doc: "CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face." },
    PropSpec { name: "frame", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Layout, native: false, doc: "Draws the button background and border." },
    PropSpec { name: "group", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "WIDGET-PARITY §2.1 (QButtonGroup): the sibling toggle buttons with the same `group` form an\nexclusive set (checking one unchecks the others) and one Tab stop the arrow keys move in." },
    PropSpec { name: "radio", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "One of a segmented group: a click checks it and never unchecks it (`Ui::selectable_value`); AccessKit role `RadioButton`. The app unchecks the others." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The caption." },
];
static TOGGLEBUTTON_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("checked", ValueType::Bool)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user toggled it; carries the new state." },
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released (also when a `radio` click left it checked)." },
];
static TOGGLEBUTTON_METHODS: &[MethodSpec] = &[
];
static TOGGLEBUTTON_SLOTS: &[SlotSpec] = &[
];
pub static TOGGLEBUTTON: KindSpec = KindSpec {
    name: "ToggleButton",
    tag: Some("toggle-button"),
    extends: Some(&WIDGET),
    doc: "A button that stays pressed (`checked`). With `radio`, a row of them is a segmented control: a click checks one, the app unchecks the rest.",
    props: TOGGLEBUTTON_PROPS,
    signals: TOGGLEBUTTON_SIGNALS,
    methods: TOGGLEBUTTON_METHODS,
    slots: TOGGLEBUTTON_SLOTS,
    a11y: A11ySpec { role: "Button", name_from: &["accessible-name", "text", "tooltip (when `text` is empty)"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &["checked"],
};

static CHECKBOX_PROPS: &[PropSpec] = &[
    PropSpec { name: "checked", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: true, doc: "Checked state; clicking toggles it." },
    PropSpec { name: "icon", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "An icon before the caption (a key, or `family:name`)." },
    PropSpec { name: "indeterminate", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: true, doc: "WIDGET-PARITY §2.2: partially checked (`bind:indeterminate`), shown as a dash. The app\nwrites it; a click clears it and checks the box (Qt, GTK)." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The caption." },
    PropSpec { name: "tristate", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "The user cycles unchecked → checked → indeterminate (Qt's tristate)." },
];
static CHECKBOX_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("checked", ValueType::Bool)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user toggled it; carries the new state." },
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released." },
];
static CHECKBOX_METHODS: &[MethodSpec] = &[
];
static CHECKBOX_SLOTS: &[SlotSpec] = &[
];
pub static CHECKBOX: KindSpec = KindSpec {
    name: "Checkbox",
    tag: Some("checkbox"),
    extends: Some(&WIDGET),
    doc: "A check box with a caption.",
    props: CHECKBOX_PROPS,
    signals: CHECKBOX_SIGNALS,
    methods: CHECKBOX_METHODS,
    slots: CHECKBOX_SLOTS,
    a11y: A11ySpec { role: "CheckBox", name_from: &["accessible-name", "text", "tooltip (when `text` is empty)"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &["checked", "indeterminate"],
};

static RADIOBUTTON_PROPS: &[PropSpec] = &[
    PropSpec { name: "checked", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: true, doc: "Chosen. A click checks it and unchecks its sibling radio buttons (Qt's auto-exclusive siblings); it never unchecks itself. With `value`, it follows `group == value`." },
    PropSpec { name: "group", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: true, doc: "The group's chosen value (`bind:group={choice}` on every button of the group, each with its own `value`). A click sets it on this button and its siblings." },
    PropSpec { name: "icon", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "WIDGET-PARITY §2.3: an icon before the caption (a key, or `family:name`)." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The caption." },
    PropSpec { name: "value", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "What this button stands for in its group: a click writes it into `group`." },
];
static RADIOBUTTON_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("checked", ValueType::Bool), ("group", ValueType::Str)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user's click changed it (or a sibling's click unchecked it); carries the new state and group value." },
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released." },
];
static RADIOBUTTON_METHODS: &[MethodSpec] = &[
];
static RADIOBUTTON_SLOTS: &[SlotSpec] = &[
];
pub static RADIOBUTTON: KindSpec = KindSpec {
    name: "RadioButton",
    tag: Some("radio-button"),
    extends: Some(&WIDGET),
    doc: "One choice of several. Siblings are exclusive: a click checks it and unchecks the other radio buttons of the same parent. Bind `checked` per button, or `group` on every button with a `value` each.",
    props: RADIOBUTTON_PROPS,
    signals: RADIOBUTTON_SIGNALS,
    methods: RADIOBUTTON_METHODS,
    slots: RADIOBUTTON_SLOTS,
    a11y: A11ySpec { role: "RadioButton", name_from: &["accessible-name", "text", "tooltip (when `text` is empty)"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &["checked", "group"],
};

static SWITCH_PROPS: &[PropSpec] = &[
    PropSpec { name: "active", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: true, doc: "On; a click or Space toggles it." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Caption after the switch (also its accessible name)." },
];
static SWITCH_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("active", ValueType::Bool)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user toggled it; carries the new state." },
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released." },
];
static SWITCH_METHODS: &[MethodSpec] = &[
];
static SWITCH_SLOTS: &[SlotSpec] = &[
];
pub static SWITCH: KindSpec = KindSpec {
    name: "Switch",
    tag: Some("switch"),
    extends: Some(&WIDGET),
    doc: "An on/off switch with a sliding knob (GTK's `switch`; WinUI's `toggle-switch`).",
    props: SWITCH_PROPS,
    signals: SWITCH_SIGNALS,
    methods: SWITCH_METHODS,
    slots: SWITCH_SLOTS,
    a11y: A11ySpec { role: "Switch", name_from: &["accessible-name", "text", "tooltip (when `text` is empty)"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &["active"],
};

static SLIDER_PROPS: &[PropSpec] = &[
    PropSpec { name: "commit", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Commit-only mode (Qt `tracking=false`): `value` moves at the release, not while dragging." },
    PropSpec { name: "inverted", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Max at the start (Qt `invertedAppearance`)." },
    PropSpec { name: "marks", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Marks on the rail, drawn as ticks (with labels) instead of the `tick-interval` ticks:\nvalues separated by commas, each optionally `value:label` (`\"0:Low, 50, 100:High\"`)." },
    PropSpec { name: "max", ty: ValueType::F64, default: DefaultValue::F64(100.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Upper bound." },
    PropSpec { name: "min", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Lower bound." },
    PropSpec { name: "orientation", ty: ValueType::Enum(&["horizontal", "vertical"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "`horizontal` (default) or `vertical` (max at the top)." },
    PropSpec { name: "page-step", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "PageUp/PageDown and `track-click=\"page\"` move this much (Qt `pageStep`); 0 for ten steps." },
    PropSpec { name: "scale", ty: ValueType::Enum(&["linear", "log"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Paint, native: false, doc: "`linear` (default) or `log` (both ends > 0)." },
    PropSpec { name: "show-value", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shows the value box next to the slider." },
    PropSpec { name: "snap-to-marks", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Dragging snaps to the nearest mark." },
    PropSpec { name: "step", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Value granularity; 0 for continuous." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Caption after the slider." },
    PropSpec { name: "tick-interval", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Between ticks; 0 for `page-step` (Qt `tickInterval`)." },
    PropSpec { name: "ticks", ty: ValueType::Enum(&["none", "before", "after", "both"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Tick marks: `none` (default), `before` (above or left), `after`, `both` (Qt `tickPosition`)." },
    PropSpec { name: "track-click", ty: ValueType::Enum(&["jump", "page"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Paint, native: false, doc: "A press on the track off the thumb: `jump` there (default) or `page` toward it." },
    PropSpec { name: "value", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: true, doc: "The value; dragging or the arrow keys change it." },
    PropSpec { name: "wheel", ty: ValueType::Enum(&["focused", "hover", "off"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Paint, native: false, doc: "When the wheel steps: `focused` (default), `hover`, `off`." },
];
static SLIDER_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("value", ValueType::F64)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user moved it; carries the new value." },
    SignalSpec { name: "commit", args: &[("value", ValueType::F64)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "" },
    SignalSpec { name: "drag-started", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "A drag on the slider began." },
    SignalSpec { name: "drag-stopped", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "A drag on the slider ended." },
    SignalSpec { name: "input", args: &[("value", ValueType::F64)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "" },
    SignalSpec { name: "submit", args: &[("value", ValueType::F64)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "" },
];
static SLIDER_METHODS: &[MethodSpec] = &[
];
static SLIDER_SLOTS: &[SlotSpec] = &[
];
pub static SLIDER: KindSpec = KindSpec {
    name: "Slider",
    tag: Some("slider"),
    extends: Some(&WIDGET),
    doc: "A horizontal slider with an optional value box.",
    props: SLIDER_PROPS,
    signals: SLIDER_SIGNALS,
    methods: SLIDER_METHODS,
    slots: SLIDER_SLOTS,
    a11y: A11ySpec { role: "Slider", name_from: &["accessible-name", "text", "tooltip"], name_required: true },
    text_content: None,
    toplevel: false,
    native_state: &["value"],
};

static PROGRESSBAR_PROPS: &[PropSpec] = &[
    PropSpec { name: "animate", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Animates the bar and shows a spinner in it (busy look; repaints while shown)." },
    PropSpec { name: "format", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The label's text when `text` is empty: `%p` percent, `%v` value, `%m` maximum (`%%` a\npercent sign); shown when set or with `show-percentage` (`%p%`)." },
    PropSpec { name: "fraction", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Progress from 0 to 1 (GTK's `fraction`)." },
    PropSpec { name: "indeterminate", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Busy, as `animate` (and `:indeterminate`); also when `value` is set and `min == max`." },
    PropSpec { name: "inverted", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Fills from the end (right, or top when vertical) instead." },
    PropSpec { name: "max", ty: ValueType::F64, default: DefaultValue::F64(100.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "" },
    PropSpec { name: "min", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Paint, native: false, doc: "" },
    PropSpec { name: "orientation", ty: ValueType::Enum(&["horizontal", "vertical"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "The bar's axis (P7: `[orientation=\"vertical\"]` styles it)." },
    PropSpec { name: "show-percentage", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shows the percentage inside the bar when `text` is empty." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Text inside the bar; empty for none." },
    PropSpec { name: "value", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Paint, native: false, doc: "WIDGET-PARITY §2.9: the value in `min..max`, the alternative to `fraction` (Qt's form)." },
];
static PROGRESSBAR_SIGNALS: &[SignalSpec] = &[
];
static PROGRESSBAR_METHODS: &[MethodSpec] = &[
];
static PROGRESSBAR_SLOTS: &[SlotSpec] = &[
];
pub static PROGRESSBAR: KindSpec = KindSpec {
    name: "ProgressBar",
    tag: Some("progress-bar"),
    extends: Some(&WIDGET),
    doc: "A horizontal bar filled to `fraction`, as wide as its rect.",
    props: PROGRESSBAR_PROPS,
    signals: PROGRESSBAR_SIGNALS,
    methods: PROGRESSBAR_METHODS,
    slots: PROGRESSBAR_SLOTS,
    a11y: A11ySpec { role: "ProgressIndicator", name_from: &["accessible-name", "text", "tooltip"], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static SPINNER_PROPS: &[PropSpec] = &[
    PropSpec { name: "size", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Diameter in points; null for the row height (`interact_size.y`)." },
    PropSpec { name: "spinning", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Paint, native: false, doc: "Spins (and repaints while shown); false keeps its space empty." },
];
static SPINNER_SIGNALS: &[SignalSpec] = &[
];
static SPINNER_METHODS: &[MethodSpec] = &[
];
static SPINNER_SLOTS: &[SlotSpec] = &[
];
pub static SPINNER: KindSpec = KindSpec {
    name: "Spinner",
    tag: Some("spinner"),
    extends: Some(&WIDGET),
    doc: "A busy indicator.",
    props: SPINNER_PROPS,
    signals: SPINNER_SIGNALS,
    methods: SPINNER_METHODS,
    slots: SPINNER_SLOTS,
    a11y: A11ySpec { role: "ProgressIndicator", name_from: &["accessible-name", "tooltip"], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static SEPARATOR_PROPS: &[PropSpec] = &[
    PropSpec { name: "orientation", ty: ValueType::Enum(&["horizontal", "vertical"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "A horizontal line (in a column) or a vertical one (in a row)." },
];
static SEPARATOR_SIGNALS: &[SignalSpec] = &[
];
static SEPARATOR_METHODS: &[MethodSpec] = &[
];
static SEPARATOR_SLOTS: &[SlotSpec] = &[
];
pub static SEPARATOR: KindSpec = KindSpec {
    name: "Separator",
    tag: Some("separator"),
    extends: Some(&WIDGET),
    doc: "A thin line between items; fills its rect along its orientation.",
    props: SEPARATOR_PROPS,
    signals: SEPARATOR_SIGNALS,
    methods: SEPARATOR_METHODS,
    slots: SEPARATOR_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static ICON_PROPS: &[PropSpec] = &[
    PropSpec { name: "family", ty: ValueType::Str, default: DefaultValue::Str("icons"), nullable: false, change: ChangeClass::Layout, native: false, doc: "The registered font family (`egui::FontFamily::Name`) holding the glyphs; empty for the proportional family. A family with no fonts, a name not in its table or a glyph no font has draws an outlined placeholder box (`svelters_egui::fonts::FontWarning`)." },
    PropSpec { name: "font-weight", ty: ValueType::I64, default: DefaultValue::I64(400), nullable: false, change: ChangeClass::Layout, native: false, doc: "CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face." },
    PropSpec { name: "frame", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "Draws it as a button (an icon button: role `Button`, takes focus)." },
    PropSpec { name: "glyph", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The glyph itself (text), when `name` is empty or not in the table." },
    PropSpec { name: "name", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "An icon registry key (`file.save`, `debug.step-over`; `svelters_egui::icons`) when `family` is `icons` and the key is registered: drawn from its SVG masters (optical size by physical pixels), tinted by CSS `color`, with the `--icon-subject`/`--icon-accent`/`--icon-badge` colour slots. Otherwise the glyph's name in the family's name table (`svelters_egui::fonts::register_icons`). Also the default accessible name." },
    PropSpec { name: "selected", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "With `frame`: drawn as selected (a toggle look)." },
    PropSpec { name: "size", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Font size in points; null for the body size (16 for a registry icon)." },
    PropSpec { name: "source", ty: ValueType::Str, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "An image instead of the glyph: an `asset://` ref (an imported SVG or PNG), a file path or any uri an egui loader serves. Measured `size` (else the body size) square and fitted inside; when it does not load the placeholder box is drawn." },
    PropSpec { name: "tint", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Paint, native: false, doc: "With `source`: tints the image with the text colour (SVG `currentColor` icons render white, then tint like a glyph); false keeps its own colours." },
];
static ICON_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released (`Response::clicked`)." },
    SignalSpec { name: "double-clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Double click (`Response::double_clicked`)." },
    SignalSpec { name: "secondary-clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Right click (`Response::secondary_clicked`)." },
];
static ICON_METHODS: &[MethodSpec] = &[
];
static ICON_SLOTS: &[SlotSpec] = &[
];
pub static ICON: KindSpec = KindSpec {
    name: "Icon",
    tag: Some("icon"),
    extends: Some(&WIDGET),
    doc: "An icon from the icon registry by key (ICR1), or a glyph from an icon font the embedder registered (SVP6), by name or as text. With `frame` it is an icon button. Its accessible name is `accessible-name`, else `tooltip`, else `name`.",
    props: ICON_PROPS,
    signals: ICON_SIGNALS,
    methods: ICON_METHODS,
    slots: ICON_SLOTS,
    a11y: A11ySpec { role: "Image", name_from: &["accessible-name", "tooltip", "name"], name_required: true },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static IMAGE_PROPS: &[PropSpec] = &[
    PropSpec { name: "fit", ty: ValueType::Enum(&["contain", "cover", "fill", "none", "scale-down"]), default: DefaultValue::Enum(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "How the image fills its rect (CSS `object-fit`): `contain` keeps the aspect ratio inside it, `cover` fills it and crops, `fill` stretches, `none` keeps the image's size, `scale-down` is the smaller of `none` and `contain`." },
    PropSpec { name: "source", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "What to show: a file path, a `file://` uri, or any uri an egui loader serves (`bytes://` from `Context::include_bytes`, `http(s)://` with egui_extras' loaders). PNG decodes built in." },
];
static IMAGE_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released on it (it senses clicks only while listened)." },
];
static IMAGE_METHODS: &[MethodSpec] = &[
];
static IMAGE_SLOTS: &[SlotSpec] = &[
];
pub static IMAGE: KindSpec = KindSpec {
    name: "Image",
    tag: Some("image"),
    extends: Some(&WIDGET),
    doc: "A picture from `source`, at its own size unless `width`/`height` say otherwise. AccessKit role `Image`, named by `accessible-name` or `tooltip`.",
    props: IMAGE_PROPS,
    signals: IMAGE_SIGNALS,
    methods: IMAGE_METHODS,
    slots: IMAGE_SLOTS,
    a11y: A11ySpec { role: "Image", name_from: &["accessible-name", "tooltip"], name_required: true },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static HYPERLINK_PROPS: &[PropSpec] = &[
    PropSpec { name: "new-tab", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Paint, native: false, doc: "Asks for a new browser tab (`OpenUrl::new_tab`). A middle click always does." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "The link text; the url when empty." },
    PropSpec { name: "url", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Paint, native: false, doc: "Opened on a click (`Context::open_url`: eframe opens the browser, an embedder reads `PlatformOutput::commands`); empty for none." },
];
static HYPERLINK_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Pressed and released (before the url opens)." },
    SignalSpec { name: "secondary-clicked", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Right click." },
];
static HYPERLINK_METHODS: &[MethodSpec] = &[
];
static HYPERLINK_SLOTS: &[SlotSpec] = &[
];
pub static HYPERLINK: KindSpec = KindSpec {
    name: "Hyperlink",
    tag: Some("hyperlink"),
    extends: Some(&WIDGET),
    doc: "A link: text that opens `url` when clicked (AccessKit role `Link`).",
    props: HYPERLINK_PROPS,
    signals: HYPERLINK_SIGNALS,
    methods: HYPERLINK_METHODS,
    slots: HYPERLINK_SLOTS,
    a11y: A11ySpec { role: "Link", name_from: &["accessible-name", "text"], name_required: true },
    text_content: Some("text"),
    toplevel: false,
    native_state: &[],
};

static STACK_PROPS: &[PropSpec] = &[
    PropSpec { name: "homogeneous", ty: ValueType::Bool, default: DefaultValue::Bool(true), nullable: false, change: ChangeClass::Layout, native: false, doc: "Sized to the largest child (GTK's `hhomogeneous`/`vhomogeneous`); false: to the visible child." },
    PropSpec { name: "padding", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Inner spacing on all four sides, in points." },
    PropSpec { name: "visible-child-name", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shows the child whose `anchor` is this name (GTK's `visible-child-name`; the child names itself with `anchor`). Empty: `visible-index` decides." },
    PropSpec { name: "visible-index", ty: ValueType::I64, default: DefaultValue::I64(0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shows the child at this index when `visible-child-name` is empty or names no child (Qt's `current-index`)." },
];
static STACK_SIGNALS: &[SignalSpec] = &[
];
static STACK_METHODS: &[MethodSpec] = &[
];
static STACK_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "children", accepts: &["Widget"], many: true },
];
pub static STACK: KindSpec = KindSpec {
    name: "Stack",
    tag: Some("stack"),
    extends: Some(&WIDGET),
    doc: "Shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one anchored `visible-child-name`, else the one at `visible-index`. The others are neither drawn nor hit.",
    props: STACK_PROPS,
    signals: STACK_SIGNALS,
    methods: STACK_METHODS,
    slots: STACK_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static SIDES_PROPS: &[PropSpec] = &[
    PropSpec { name: "align-items", ty: ValueType::Enum(&["start", "end", "center", "stretch", "baseline"]), default: DefaultValue::Enum(3), nullable: false, change: ChangeClass::Layout, native: false, doc: "Cross-axis alignment of the two sides (`center` is egui's look)." },
    PropSpec { name: "gap", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Minimum space between the two sides, in points." },
    PropSpec { name: "padding", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Inner spacing on all four sides, in points." },
];
static SIDES_SIGNALS: &[SignalSpec] = &[
];
static SIDES_METHODS: &[MethodSpec] = &[
];
static SIDES_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "children", accepts: &["Widget"], many: true },
];
pub static SIDES: KindSpec = KindSpec {
    name: "Sides",
    tag: Some("sides"),
    extends: Some(&WIDGET),
    doc: "A row with its first child at the start and its second at the end (wrap several items in a `box`); with more children, the space is shared between them.",
    props: SIDES_PROPS,
    signals: SIDES_SIGNALS,
    methods: SIDES_METHODS,
    slots: SIDES_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static GRID_PROPS: &[PropSpec] = &[
    PropSpec { name: "align-items", ty: ValueType::Enum(&["start", "end", "center", "stretch", "baseline"]), default: DefaultValue::Enum(3), nullable: false, change: ChangeClass::Layout, native: false, doc: "Vertical alignment of each child in its cell." },
    PropSpec { name: "column-gap", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Space between columns in points; null to use `gap`." },
    PropSpec { name: "gap", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Space between rows and between columns, in points (row-gap and column-gap override it)." },
    PropSpec { name: "grid-template-columns", ty: ValueType::Str, default: DefaultValue::Str("auto 1fr"), nullable: false, change: ChangeClass::Layout, native: false, doc: "The column tracks, CSS syntax: `auto`, `min-content`, `max-content`, `<n>fr`, `<n>px` (or a bare number: points), `<n>%`, `minmax(a, b)`, `fit-content(<len>)`, `repeat(<n>, tracks)`. Rows are implicit (`auto`). The default is a form: a label column and a field column." },
    PropSpec { name: "justify-items", ty: ValueType::Enum(&["start", "end", "center", "stretch"]), default: DefaultValue::Enum(3), nullable: false, change: ChangeClass::Layout, native: false, doc: "Horizontal alignment of each child in its cell." },
    PropSpec { name: "padding", ty: ValueType::F64, default: DefaultValue::F64(0.0), nullable: false, change: ChangeClass::Layout, native: false, doc: "Inner spacing on all four sides, in points." },
    PropSpec { name: "row-gap", ty: ValueType::F64, default: DefaultValue::Null, nullable: true, change: ChangeClass::Layout, native: false, doc: "Space between rows in points; null to use `gap`." },
];
static GRID_SIGNALS: &[SignalSpec] = &[
];
static GRID_METHODS: &[MethodSpec] = &[
];
static GRID_SLOTS: &[SlotSpec] = &[
    SlotSpec { name: "children", accepts: &["Widget"], many: true },
];
pub static GRID: KindSpec = KindSpec {
    name: "Grid",
    tag: Some("grid"),
    extends: Some(&WIDGET),
    doc: "A CSS grid container laid out by taffy: children fill the cells row by row (auto-placement); draws nothing itself.",
    props: GRID_PROPS,
    signals: GRID_SIGNALS,
    methods: GRID_METHODS,
    slots: GRID_SLOTS,
    a11y: A11ySpec { role: "", name_from: &[], name_required: false },
    text_content: None,
    toplevel: false,
    native_state: &[],
};

static TEXTEDIT_PROPS: &[PropSpec] = &[
    PropSpec { name: "char-limit", ty: ValueType::I64, default: DefaultValue::I64(0), nullable: false, change: ChangeClass::Paint, native: false, doc: "Maximum number of characters; 0 for none." },
    PropSpec { name: "hint-text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: false, doc: "Shown while the text is empty; also the default accessible name." },
    PropSpec { name: "multiline", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "Several lines (`TextEdit::multiline`); Enter inserts a newline." },
    PropSpec { name: "password", ty: ValueType::Bool, default: DefaultValue::Bool(false), nullable: false, change: ChangeClass::Layout, native: false, doc: "Hides the characters." },
    PropSpec { name: "text", ty: ValueType::Str, default: DefaultValue::Str(""), nullable: false, change: ChangeClass::Layout, native: true, doc: "The edited text; typing changes it." },
];
static TEXTEDIT_SIGNALS: &[SignalSpec] = &[
    SignalSpec { name: "changed", args: &[("text", ValueType::Str)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user edited the text; carries the new text (once per frame)." },
    SignalSpec { name: "commit", args: &[("text", ValueType::Str)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "The user committed the text: Enter in a one-line field (also when a default button takes it), or losing focus after an edit since the last commit; carries the text." },
    SignalSpec { name: "gained-focus", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Took keyboard focus." },
    SignalSpec { name: "lost-focus", args: &[], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Lost keyboard focus." },
    SignalSpec { name: "submit", args: &[("text", ValueType::Str)], passive: false, cancelable: false, sync: false, bubbles: false, doc: "Enter in a one-line field (also when a default button takes it); carries the text. Fires after `commit`." },
];
static TEXTEDIT_METHODS: &[MethodSpec] = &[
    MethodSpec { name: "request-focus", params: &[], ret: None, doc: "Moves keyboard focus here on the next frame." },
];
static TEXTEDIT_SLOTS: &[SlotSpec] = &[
];
pub static TEXTEDIT: KindSpec = KindSpec {
    name: "TextEdit",
    tag: Some("text-edit"),
    extends: Some(&WIDGET),
    doc: "A single- or multi-line text field.",
    props: TEXTEDIT_PROPS,
    signals: TEXTEDIT_SIGNALS,
    methods: TEXTEDIT_METHODS,
    slots: TEXTEDIT_SLOTS,
    a11y: A11ySpec { role: "TextInput", name_from: &["accessible-name", "hint-text", "tooltip"], name_required: true },
    text_content: None,
    toplevel: false,
    native_state: &["text"],
};

/// Every kind here, bases first.
pub static ALL: &[&KindSpec] = &[&WIDGET, &WINDOW, &BOX, &FRAME, &LABEL, &BUTTON, &TOGGLEBUTTON, &CHECKBOX, &RADIOBUTTON, &SWITCH, &SLIDER, &PROGRESSBAR, &SPINNER, &SEPARATOR, &ICON, &IMAGE, &HYPERLINK, &STACK, &SIDES, &GRID, &TEXTEDIT];

/// A registry of the Tier 0 kinds (shim for `presto_widgets::registry()`).
pub fn registry() -> KindRegistry {
    let mut r = KindRegistry::new();
    for k in ALL {
        r.register(k);
    }
    r
}
