//! WG2b display kinds (plans/milestones/WG2b-tier0-controls.md): `image`, `progress-bar`,
//! `spinner` and `canvas`. Their entries are appended to [`KINDS`](super::KINDS); painting and
//! measuring are `crate::elements::media`, the canvas replay `crate::elements::canvas`.

use super::builtin::NONE;
use super::{Def, KindDef, P, PropDef, SignalDef, Ty, optional, prop, signal};

pub const FIT: &[&str] = &["contain", "cover", "fill", "none", "scale-down"];

static IMAGE_PROPS: &[PropDef] = &[
    prop(
        "source",
        P::Source,
        Ty::Str,
        Def::Str(""),
        "What to show: a file path, a `file://` uri, or any uri an egui loader serves (`bytes://` from `Context::include_bytes`, `http(s)://` with egui_extras' loaders). PNG decodes built in.",
    ),
    prop(
        "fit",
        P::Fit,
        Ty::Enum(FIT),
        Def::Str("contain"),
        "How the image fills its rect (CSS `object-fit`): `contain` keeps the aspect ratio inside it, `cover` fills it and crops, `fill` stretches, `none` keeps the image's size, `scale-down` is the smaller of `none` and `contain`.",
    ),
];

static PROGRESS_BAR_PROPS: &[PropDef] = &[
    prop("fraction", P::Fraction, Ty::F64, Def::F64(0.0), "Progress from 0 to 1 (GTK's `fraction`)."),
    prop("text", P::Text, Ty::Str, Def::Str(""), "Text inside the bar; empty for none."),
    prop("show-percentage", P::ShowPercentage, Ty::Bool, Def::Bool(false), "Shows the percentage inside the bar when `text` is empty."),
    prop("animate", P::Animate, Ty::Bool, Def::Bool(false), "Animates the bar and shows a spinner in it (busy look; repaints while shown)."),
];

static SPINNER_PROPS: &[PropDef] = &[
    prop("spinning", P::Spinning, Ty::Bool, Def::Bool(true), "Spins (and repaints while shown); false keeps its space empty."),
    optional("size", P::Size, Ty::F64, "Diameter in points; null for the row height (`interact_size.y`)."),
];

static CANVAS_PROPS: &[PropDef] = &[
    PropDef {
        name: "ops",
        key: P::Ops,
        ty: Ty::Str,
        default: Def::Null,
        nullable: true,
        native: super::Native::Never,
        layout: false,
        doc: "The display list (X8): a JSON array of `[name, ...args]` arrays as `@svelters/canvas` records it. Replayed with egui's painter each frame; no JS runs while it paints.",
    },
    prop("content-width", P::ContentWidth, Ty::F64, Def::F64(0.0), "Natural width in points (GTK's `content-width`)."),
    prop("content-height", P::ContentHeight, Ty::F64, Def::F64(0.0), "Natural height in points (GTK's `content-height`)."),
];

static CANVAS_SIGNALS: &[SignalDef] = &[
    signal("resize", &[("width", Ty::F64), ("height", Ty::F64)], "Its laid-out size changed (and once after the first layout); `draw()` of `@svelters/canvas` records for it."),
    signal("clicked", NONE, "Pressed and released on it."),
];

static IMAGE_SIGNALS: &[SignalDef] = &[signal("clicked", NONE, "Pressed and released on it (it senses clicks only while listened).")];

pub(super) const IMAGE: KindDef = KindDef {
    id: "Image",
    tag: Some("image"),
    native: "egui::Image",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: IMAGE_PROPS,
    signals: IMAGE_SIGNALS,
    methods: &[],
    doc: "A picture from `source`, at its own size unless `width`/`height` say otherwise. AccessKit role `Image`, named by `accessible-name` or `tooltip`.",
};

pub(super) const PROGRESS_BAR: KindDef = KindDef {
    id: "ProgressBar",
    tag: Some("progress-bar"),
    native: "egui::ProgressBar",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: PROGRESS_BAR_PROPS,
    signals: &[],
    methods: &[],
    doc: "A horizontal bar filled to `fraction`, as wide as its rect.",
};

pub(super) const SPINNER: KindDef = KindDef {
    id: "Spinner",
    tag: Some("spinner"),
    native: "egui::Spinner",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: SPINNER_PROPS,
    signals: &[],
    methods: &[],
    doc: "A busy indicator.",
};

pub(super) const CANVAS: KindDef = KindDef {
    id: "Canvas",
    tag: Some("canvas"),
    native: "egui::Painter (X8 display list replay)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: CANVAS_PROPS,
    signals: CANVAS_SIGNALS,
    methods: &[],
    doc: "A drawing surface replaying the X8 display list (`ops`) that `@svelters/canvas` records, as GTK's and Qt's `canvas` do. Paths fill as simple polygons (no holes, nonzero = even-odd), `clip` cuts to the path's bounding box, `clearRect` paints the window background.",
};
