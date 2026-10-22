//! RT2 inline rich text (plans/milestones/RT2-rich-text.md; WIDGET-PARITY Q4): the run kinds
//! `span`, `b`, `i`, `code` and `a`, children of a `label` (or of another run). They take no
//! space and paint nothing themselves: the host label lays its runs out as one paragraph
//! (`crate::rich`), each run drawn with its own computed style.
//!
//! - **The markup is the template.** There is no parser: the compiler lowers a label's children
//!   (text, `{expressions}`, run elements, `{#if}`/`{#each}` of those) into run nodes, its bare text
//!   into anonymous `span`s (the spec's `inline_text`).
//! - **Styled by CSS.** Runs are scene nodes, so `label span.warn`, `label a:hover` and scoped
//!   classes match them. The theme's base sheet gives the tag defaults (`b` bold, `i` italic,
//!   `code` monospace on a field fill, `a` in the `link` colour, underlined on hover: DESIGN R18).
//! - **Links.** `a` runs are focusable and clickable (`clicked`; with an `href` and no handler the
//!   URL opens as a `hyperlink`'s does), with the AccessKit role `Link`.

use super::builtin::NONE;
use super::{Def, KindDef, P, PropDef, SignalDef, Ty, prop, signal};

/// Kind ids of the run kinds (what a `label` and a run accept as children).
pub const RUN_IDS: &[&str] = &["Span", "Bold", "Italic", "Code", "Anchor"];

/// The compiler's `inline_text` spec: the kind (tag) and prop bare text in a rich label lowers to.
pub const INLINE_TEXT: (&str, &str) = ("span", "text");

/// What `label` and the runs accept as children: runs only.
pub fn slot_accepts(id: &str) -> Option<&'static [&'static str]> {
    (id == "Label" || id == "Run" || RUN_IDS.contains(&id)).then_some(RUN_IDS)
}

const WEIGHT_DOC: &str = "CSS font weight (100-900) of this run and the runs in it; 0 (the default) inherits the enclosing run's or label's weight. `b` has 700 from the theme's base sheet.";

static RUN_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The run's own text, before its child runs (its text children fold into it)."),
    prop("font-weight", P::FontWeight, Ty::I64, Def::I64(0), WEIGHT_DOC),
];

static ANCHOR_PROPS: &[PropDef] = &[
    prop("href", P::Href, Ty::Str, Def::Str(""), "The link target. Without an `onclicked` handler a click opens it (as `hyperlink`'s `url`); with one, only the handler runs."),
    prop("accessible-name", P::AccessibleName, Ty::Str, Def::Str(""), "AccessKit name of the link; defaults to its text."),
];

static ANCHOR_SIGNALS: &[SignalDef] = &[signal("clicked", NONE, "The link was activated (click, or Enter/Space while it has keyboard focus).")];

const fn run(id: &'static str, tag: &'static str, doc: &'static str) -> KindDef {
    KindDef {
        id,
        tag: Some(tag),
        native: "svelters text run (one section of its label's galley)",
        base: Some("Run"),
        toplevel: false,
        slot: Some("runs"),
        text_content: Some("text"),
        present: None,
        props: &[],
        signals: &[],
        methods: &[],
        doc,
    }
}

/// The abstract base of the runs.
pub(super) const RUN: KindDef = KindDef {
    id: "Run",
    tag: None,
    native: "svelters text run",
    base: None,
    toplevel: false,
    slot: Some("runs"),
    text_content: Some("text"),
    present: None,
    props: RUN_PROPS,
    signals: &[],
    methods: &[],
    doc: "Common properties of the inline runs (`span`, `b`, `i`, `code`, `a`): text inside a `label`, laid out with the label's other runs as one paragraph, never by taffy.",
};

pub(super) const SPAN: KindDef = run("Span", "span", "An inline run of text in a `label`, styled by CSS (`label span.cls`). Bare text in a rich label becomes anonymous spans.");
pub(super) const BOLD: KindDef = run("Bold", "b", "Bold inline text (`font-weight: 700` from the theme's base sheet).");
pub(super) const ITALIC: KindDef = run("Italic", "i", "Italic inline text (`font-style: italic` from the theme's base sheet).");
pub(super) const CODE: KindDef = run("Code", "code", "Inline code: the monospace font on the field fill (theme's base sheet).");
pub(super) const ANCHOR: KindDef = KindDef {
    native: "svelters text run (a link: focusable, clickable, AccessKit `Link`)",
    props: ANCHOR_PROPS,
    signals: ANCHOR_SIGNALS,
    ..run("Anchor", "a", "An inline link: the `link` colour, underlined on hover (DESIGN R18), the pointer cursor (R10); focusable, activated by click, Enter or Space.")
};
