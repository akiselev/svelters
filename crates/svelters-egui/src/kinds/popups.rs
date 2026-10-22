//! WG2a overlays (plans/milestones/WG2a-menus-popups.md): `popup`, `modal` and `inner-window`.
//! They derive from `Box`: their children are laid out by taffy as a flex box, but out of their
//! parent's flow (`display: none` there) and on their own, in the egui area that shows them
//! (`layout::layout_overlay`). Entries appended to [`KINDS`](super::KINDS); painting is
//! `crate::overlays`.

use super::builtin::NONE;
use super::{Def, KindDef, MethodDef, P, PropDef, SignalDef, Ty, input, optional, prop, signal};

pub const PLACEMENT: &[&str] = &["bottom", "top", "left", "right"];

static POPUP_PROPS: &[PropDef] = &[
    input(
        "open",
        P::Open,
        Ty::Bool,
        Def::Bool(false),
        "Shown when true; the user closing it (Escape, a click outside with `autohide`) sets it false.",
    ),
    prop(
        "placement",
        P::Placement,
        Ty::Enum(PLACEMENT),
        Def::Str("bottom"),
        "The side of its owner it opens on (egui moves it to another side when it does not fit).",
    ),
    prop(
        "autohide",
        P::Autohide,
        Ty::Bool,
        Def::Bool(true),
        "A click outside closes it; with false only Escape, `popdown` or the app does.",
    ),
];

static MODAL_PROPS: &[PropDef] = &[
    input(
        "open",
        P::Open,
        Ty::Bool,
        Def::Bool(true),
        "Shown when true (so `{#if}` alone can show it); the user dismissing it sets it false.",
    ),
    prop(
        "title",
        P::Title,
        Ty::Str,
        Def::Str(""),
        "A heading above the content; also the dialog's accessible name.",
    ),
    prop(
        "dismissable",
        P::Dismissable,
        Ty::Bool,
        Def::Bool(true),
        "Escape or a click on the backdrop closes it; with false only the app does.",
    ),
];

static INNER_WINDOW_PROPS: &[PropDef] = &[
    input(
        "open",
        P::Open,
        Ty::Bool,
        Def::Bool(true),
        "Shown when true; the close button sets it false.",
    ),
    prop(
        "title",
        P::Title,
        Ty::Str,
        Def::Str(""),
        "The title bar text.",
    ),
    prop(
        "default-width",
        P::DefaultWidth,
        Ty::F64,
        Def::F64(320.0),
        "Initial width in points (a resizable window's content fills it).",
    ),
    prop(
        "default-height",
        P::DefaultHeight,
        Ty::F64,
        Def::F64(240.0),
        "Initial height in points.",
    ),
    optional(
        "default-x",
        P::DefaultX,
        Ty::F64,
        "Initial left edge in points (viewport coordinates); null: egui places it.",
    ),
    optional(
        "default-y",
        P::DefaultY,
        Ty::F64,
        "Initial top edge in points (viewport coordinates); null: egui places it.",
    ),
    prop(
        "closable",
        P::Closable,
        Ty::Bool,
        Def::Bool(true),
        "Shows a close button in the title bar.",
    ),
    prop(
        "collapsible",
        P::Collapsible,
        Ty::Bool,
        Def::Bool(true),
        "The title bar can collapse it to the title.",
    ),
    prop(
        "resizable",
        P::Resizable,
        Ty::Bool,
        Def::Bool(true),
        "The user can resize it; the content is laid out at its size. Without it the window is as big as its content.",
    ),
];

static OVERLAY_SIGNALS: &[SignalDef] = &[
    signal(
        "changed",
        &[("open", Ty::Bool)],
        "The user (or `popup`/`popdown`) opened or closed it; carries `open`.",
    ),
    signal("closed", NONE, "The user (or `popdown`) closed it."),
];

static POPUP_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "popup",
        doc: "Opens it (announced like a user open).",
    },
    MethodDef {
        name: "popdown",
        doc: "Closes it (announced like a user close).",
    },
];

pub(super) const POPUP: KindDef = KindDef {
    id: "Popup",
    tag: Some("popup"),
    native: "egui::Popup",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: POPUP_PROPS,
    signals: OVERLAY_SIGNALS,
    methods: POPUP_METHODS,
    doc: "A popover anchored to its owner: the element it is declared in (attached), or the box it is placed in. Its children are a flex box sized to their content (or `width`/`height`), out of the owner's flow. GTK's `popover`.",
};

/// WG6 (QA5): a rich tooltip (Qt's rich-text tooltip, `{#snippet tooltip()}` in the survey).
pub(super) const TOOLTIP: KindDef = KindDef {
    id: "Tooltip",
    tag: Some("tooltip"),
    native: "egui::Tooltip",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: &[],
    signals: &[],
    methods: &[],
    doc: "A rich tooltip: declared in an element (attached), its children are shown as that element's tooltip on hover, enabled or disabled (`Tooltip::for_enabled`/`for_disabled`). A flex box sized to its content, out of the owner's flow. Render a snippet into it for a reusable tooltip: `<tooltip>{@render tip()}</tooltip>`. A `tooltip` prop on the same element wins over it (one tooltip per layer).",
};

pub(super) const MODAL: KindDef = KindDef {
    id: "Modal",
    tag: Some("modal"),
    native: "egui::Modal",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: MODAL_PROPS,
    signals: OVERLAY_SIGNALS,
    methods: &[],
    doc: "An in-window modal dialog: a backdrop over the viewport that blocks the rest of the UI, its content centred, sized to fit (or `width`/`height`). The egui form of a modal dialog (no OS-modal windows on egui: EGUI.md §5).",
};

pub(super) const INNER_WINDOW: KindDef = KindDef {
    id: "InnerWindow",
    tag: Some("inner-window"),
    native: "egui::Window",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: INNER_WINDOW_PROPS,
    signals: OVERLAY_SIGNALS,
    methods: &[],
    doc: "A floating window inside the viewport (`egui::Window`): movable, optionally resizable, collapsible and closable; its children are a flex box.",
};
