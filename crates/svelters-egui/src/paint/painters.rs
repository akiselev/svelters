//! Part painters selected in CSS (WIDGET-PARITY §4.3, the user's Q1: the QProxyStyle equivalent).
//!
//! An app registers a Rust painter by name ([`register_painter`], or `Embed::builder().painter`),
//! and CSS picks it per part and state, as the CSS Painting API does:
//!
//! ```css
//! slider::part(thumb)              { background-image: paint(cad.knurl); --knurl-pitch: 2px; }
//! tree-view::part(twistie)         { content: paint(cad.twistie); }
//! slider:focus-visible::part(thumb){ --knurl-color: var(--sv-focus); }
//! ```
//!
//! - `background-image: paint(name)`: the box primitive calls the painter after the background
//!   colour and inset shadows, before the border ([`super::paint_box`]).
//! - `content: paint(name)`: the icon primitive calls it instead of the glyph ([`super::paint_icon`]).
//!
//! The painter gets the part's [`StyleView`] (custom properties pass arguments), its rect, a
//! [`PartInfo`] and widget-specific [`PaintArgs`] (documented per part by each widget). Pure paint:
//! no input, no layout, no scene writes. An unknown name falls back to the default paint and is
//! reported once. OW-CSS parses `paint(<ident> [, <args>]*)` into typed values; until then the
//! raw text is read.

use super::{PaintCx, StyleView};
use crate::provider::StateBits;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// What a painter is told about the part it paints.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct PartInfo<'a> {
    /// The host's tag (`slider`, `cad-gauge`).
    pub tag: &'a str,
    /// The part (`thumb`), `None` for the host's own box.
    pub part: Option<&'a str>,
    pub state: StateBits,
    pub ppp: f32,
}

impl<'a> PartInfo<'a> {
    pub fn new(tag: &'a str, part: Option<&'a str>, state: StateBits, ppp: f32) -> PartInfo<'a> {
        PartInfo { tag, part, state, ppp }
    }
}

/// Orientation of a value part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Widget-specific paint arguments, documented per part (slider thumb: `Value`, progress fill:
/// `Progress`, twistie: `Expanded`, …).
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub enum PaintArgs<'a> {
    #[default]
    None,
    Value { fraction: f32, orientation: Orientation },
    Progress { fraction: Option<f32>, phase: f32 },
    Expanded(bool),
    Custom(&'a dyn std::any::Any),
}

/// A part painter (WIDGET-PARITY §4.3).
pub trait PartPainter: Send + Sync + 'static {
    fn paint(&self, cx: &mut PaintCx, style: &StyleView, rect: egui::Rect, part: &PartInfo<'_>, args: &PaintArgs<'_>);
}

impl<F> PartPainter for F
where
    F: Fn(&mut PaintCx, &StyleView, egui::Rect, &PartInfo<'_>, &PaintArgs<'_>) + Send + Sync + 'static,
{
    fn paint(&self, cx: &mut PaintCx, style: &StyleView, rect: egui::Rect, part: &PartInfo<'_>, args: &PaintArgs<'_>) {
        self(cx, style, rect, part, args)
    }
}

type Registry = RwLock<HashMap<String, Arc<dyn PartPainter>>>;

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::default)
}

/// Registers painter `name` (`cad.knurl`: namespaced like provider kinds). A second registration
/// of a name replaces the first (hot reload of the app's Rust is a restart; tests re-register).
pub fn register_painter(name: &str, p: impl PartPainter) {
    registry().write().unwrap_or_else(|e| e.into_inner()).insert(name.to_string(), Arc::new(p));
}

/// The painter registered as `name`.
pub fn painter(name: &str) -> Option<Arc<dyn PartPainter>> {
    registry().read().unwrap_or_else(|e| e.into_inner()).get(name).cloned()
}

/// `f(<first arg>, …)` in a CSS value: the function's arguments, if `value` is a call of `f`.
pub fn css_call<'v>(value: &'v str, f: &str) -> Option<Vec<&'v str>> {
    let v = value.trim();
    let rest = v.strip_prefix(f)?.trim_start().strip_prefix('(')?.strip_suffix(')')?;
    Some(rest.split(',').map(str::trim).filter(|a| !a.is_empty()).collect())
}

/// The painter that `prop` (`background-image` or `content`) of `s` selects, with its name.
pub fn selected(s: &StyleView, prop: &str) -> Option<(String, Arc<dyn PartPainter>)> {
    let v = s.keyword(prop)?;
    let name = css_call(v, "paint")?.first()?.to_string();
    match painter(&name) {
        Some(p) => Some((name, p)),
        None => {
            report_unknown(&name);
            None
        }
    }
}

fn report_unknown(name: &str) {
    static SEEN: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    let mut seen = SEEN.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if seen.insert(name.to_string()) {
        eprintln!("svelters-egui: paint({name}): no painter registered under that name (the default paint is used)");
    }
}
