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
pub mod checkbox;
pub mod collapsing;
pub mod frame;
pub mod hyperlink;
pub mod label;
mod parts;
pub mod progress;
pub mod radio;
pub mod separator;
pub mod switch;
pub mod toggle_button;

use crate::scene::W;

/// The built-in tags whose own widget is the default (the egui wrapper otherwise). Grows by one
/// tag per lane as each own widget passes its acceptance (ARCH-WIDGETS §6.2 step 2).
pub const DEFAULT_OWN: &[&str] = &["button"];

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

/// The payload of a new node of built-in kind `kind` when `set` picks its own widget.
pub(crate) fn own_payload(set: &KindSet, kind: usize) -> Option<W> {
    let reg = crate::kinds::KindRegistry::global();
    let tag = reg.kind(kind).tag?;
    if !set.contains(tag) {
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
