//! OW0 (ARCH-WIDGETS §1.3, §6.2): svelters' own widgets, one file per widget. Each is a
//! `#[svelters::widget(replaces = "<tag>")]` kind composed from behaviour units (`crate::interact`),
//! paint primitives (`crate::paint`) and parts, with its look in the theme's CSS and its UA table.
//!
//! **Migration without a flag day.** A built-in kind's row in `KINDS` (tag, prop and signal ids,
//! spec, typings) is untouched; its own widget declares `replaces = "<tag>"` and the registry
//! routes the row's props and signals to the struct by name (startup check:
//! `KindRegistry::replacement`). Which implementation a new node gets is the own-widget switch
//! ([`KindSet`]): `EguiBackend::set_own_widgets`, or `SVELTERS_OWN_WIDGETS=all|none|default|<tag,…>`
//! read when the backend is created. A kind's default flips to own ([`DEFAULT_OWN`]) in the commit
//! that makes its acceptance pass, and its egui wrapper is deleted in the same lane.
//!
//! "Write a widget" (the guide other lanes follow) is in `plans/milestones/OW0-widget-foundation.md`.
//! Writer lanes: OW0–OW7.

pub mod button;
pub mod calendar;
pub mod checkbox;
pub mod hyperlink;
pub mod label;
pub mod panel;
mod parts;
mod picker_glyphs;
pub mod progress;
pub mod scroll_area;
pub mod scrollbar;
pub mod separator;
pub mod split_view;
pub mod switch;
pub mod toggle_button;

/// OW2: the window's inner chrome (`windows.rs`).
pub use crate::windows::WINDOW_UA;

use crate::scene::W;

/// The built-in tags whose own widget is the default (the egui wrapper otherwise). Grows by one
/// tag per lane as each own widget passes its acceptance (ARCH-WIDGETS §6.2 step 2).
pub const DEFAULT_OWN: &[&str] = &["button"];

/// The built-in tags whose egui wrapper is gone (deleted in the lane whose own widget passed its
/// acceptance, §6.2 step 3) or never existed (`scrollbar`): always svelters' own, whatever the
/// switch says.
pub const OWN_ONLY: &[&str] = &["calendar", "panel", "scroll-area", "scrollbar", "split-view"];

/// Which built-in kinds get svelters' own widget (§6.2: the per-kind switch).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum KindSet {
    /// [`DEFAULT_OWN`].
    #[default]
    Default,
    /// Every kind that has an own widget.
    All,
    /// None: every kind keeps its egui wrapper.
    None,
    /// These tags (others keep their wrapper).
    Tags(Vec<String>),
}

impl KindSet {
    /// These tags.
    pub fn tags<S: AsRef<str>>(tags: impl IntoIterator<Item = S>) -> KindSet {
        KindSet::Tags(tags.into_iter().map(|t| t.as_ref().to_string()).collect())
    }

    /// Whether new `<tag>` nodes get the own widget (when one exists).
    pub fn contains(&self, tag: &str) -> bool {
        match self {
            KindSet::Default => DEFAULT_OWN.contains(&tag),
            KindSet::All => true,
            KindSet::None => false,
            KindSet::Tags(t) => t.iter().any(|x| x == tag),
        }
    }

    /// `all`, `none`, `default`, or tags separated by commas or spaces.
    pub fn parse(v: &str) -> KindSet {
        match v.trim() {
            "all" => KindSet::All,
            "none" => KindSet::None,
            "" | "default" => KindSet::Default,
            t => KindSet::tags(t.split([',', ' ']).filter(|s| !s.is_empty())),
        }
    }

    /// `SVELTERS_OWN_WIDGETS`, if set.
    pub fn from_env() -> Option<KindSet> {
        std::env::var("SVELTERS_OWN_WIDGETS").ok().map(|v| KindSet::parse(&v))
    }
}

/// OW2: an own widget with children (`CONTAINER` and `PAINTS_BOX`): it records its hit-list
/// entry before drawing its children ([`WidgetCx::record_hit`]), so they are above it.
///
/// [`WidgetCx::record_hit`]: crate::provider::WidgetCx::record_hit
pub(crate) fn is_own_container(w: &W) -> bool {
    use crate::provider::Caps;
    matches!(w, W::Ext(x) if x.entry().caps.contains(Caps::CONTAINER) && x.entry().caps.contains(Caps::PAINTS_BOX))
}

/// OW2: a nullable length prop (`panel`'s `size`, `split-view`'s `divider-position`): a value that
/// is not a finite number reads as null (as the egui wrappers' `as_len` did: WG2b's bad-input
/// test).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Len(pub Option<f64>);

impl svelters_host::exports::JsType for Len {
    fn describe(b: &mut svelters_host::exports::__private::TypeBuilder) -> svelters_host::exports::__private::TypeRef {
        <Option<f64> as svelters_host::exports::JsType>::describe(b)
    }
}

impl svelters_host::exports::FromJs for Len {
    fn from_js(v: svelters_host::exports::Value, cx: &svelters_host::exports::CallCx<'_>) -> Result<Self, svelters_host::exports::NativeError> {
        let v = <Option<f64> as svelters_host::exports::FromJs>::from_js(v, cx)?;
        Ok(Len(v.filter(|x| x.is_finite())))
    }
}

impl svelters_host::exports::IntoJs for Len {
    fn into_js(self, cx: &svelters_host::exports::CallCx<'_>) -> Result<svelters_host::exports::Value, svelters_host::exports::NativeError> {
        self.0.into_js(cx)
    }
}

/// OW6: the error a typed prop (`IsoDate`, `HexColor`) refuses a write with.
pub(crate) fn invalid(message: String) -> svelters_host::exports::NativeError {
    svelters_host::exports::NativeError { domain: "svelters:egui".into(), code: svelters_abi::Value::Str("invalid-value".into()), message }
}

/// OW2: an enum prop read leniently (an unknown value is the default).
pub(crate) fn pick<'a>(v: &str, allowed: &[&'a str], default: &'a str) -> &'a str {
    allowed.iter().find(|a| **a == v).copied().unwrap_or(default)
}

/// The payload of a new node of built-in kind `kind` when `set` picks its own widget.
pub(crate) fn own_payload(set: &KindSet, kind: usize) -> Option<W> {
    let reg = crate::kinds::KindRegistry::global();
    let tag = reg.kind(kind).tag?;
    if !set.contains(tag) && !OWN_ONLY.contains(&tag) {
        return None;
    }
    let r = reg.replacement(kind)?;
    Some(W::Ext(crate::provider::ExtWidget::new((r.entry.new)())))
}

impl crate::EguiBackend {
    /// OW0: which built-in kinds new nodes get as svelters' own widget ([`KindSet`]); existing
    /// nodes keep theirs. Overrides `SVELTERS_OWN_WIDGETS`.
    pub fn set_own_widgets(&self, set: KindSet) {
        self.0.scene.borrow_mut().own = set;
    }

    /// The own-widget switch in effect.
    pub fn own_widgets(&self) -> KindSet {
        self.0.scene.borrow().own.clone()
    }
}
