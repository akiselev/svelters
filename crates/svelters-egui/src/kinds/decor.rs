//! WG1 display kinds: `separator` and `icon` (SVP6: a glyph of a registered icon family, see
//! `crate::fonts`). Their entries are appended to [`KINDS`](super::KINDS).

use super::builtin::{BUTTON_SIGNALS, FONT_WEIGHT_DOC};
use super::{Def, KindDef, P, PropDef, Ty, optional, prop};

pub const ORIENTATION: &[&str] = &["horizontal", "vertical"];

static SEPARATOR_PROPS: &[PropDef] = &[prop(
    "orientation",
    P::Orientation,
    Ty::Enum(ORIENTATION),
    Def::Str("horizontal"),
    "A horizontal line (in a column) or a vertical one (in a row).",
)];

static ICON_PROPS: &[PropDef] = &[
    prop(
        "name",
        P::Name,
        Ty::Str,
        Def::Str(""),
        "The glyph's name in the family's name table (`svelters_egui::fonts::register_icons`); also the default accessible name.",
    ),
    prop("glyph", P::Glyph, Ty::Str, Def::Str(""), "The glyph itself (text), when `name` is empty or not in the table."),
    prop(
        "family",
        P::Family,
        Ty::Str,
        Def::Str("icons"),
        "The registered font family (`egui::FontFamily::Name`) holding the glyphs; empty for the proportional family. A family with no fonts, a name not in its table or a glyph no font has draws an outlined placeholder box (`svelters_egui::fonts::FontWarning`).",
    ),
    optional("size", P::Size, Ty::F64, "Font size in points; null for the body size."),
    prop("font-weight", P::FontWeight, Ty::I64, Def::I64(400), FONT_WEIGHT_DOC),
    prop("frame", P::Frame, Ty::Bool, Def::Bool(false), "Draws it as a button (an icon button: role `Button`, takes focus)."),
    prop("selected", P::Selected, Ty::Bool, Def::Bool(false), "With `frame`: drawn as selected (a toggle look)."),
];

pub(super) const SEPARATOR: KindDef = KindDef {
        id: "Separator",
        tag: Some("separator"),
        native: "egui::Separator",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: SEPARATOR_PROPS,
        signals: &[],
        methods: &[],
        doc: "A thin line between items; fills its rect along its orientation.",
    };

pub(super) const ICON: KindDef = KindDef {
        id: "Icon",
        tag: Some("icon"),
        native: "egui::Label / egui::Button (icon font glyph)",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: ICON_PROPS,
        signals: BUTTON_SIGNALS,
        methods: &[],
        doc: "A glyph from an icon font the embedder registered (SVP6), by name or as text. With `frame` it is an icon button. Its accessible name is `accessible-name`, else `tooltip`, else `name`.",
    };
