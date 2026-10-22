//! SVP6 (plans/research/CADATOMIC.md §2.8): fonts and icons the embedder registers.
//!
//! - **Weights.** egui has no font weights, only families. The embedder registers which family draws
//!   a weight of a base family ([`register_weight`]: cadatomic maps 600 of `Proportional` to its
//!   `Name("semibold")`), and `font-weight` on `label`, `button`, `toggle-button` and `icon` picks the
//!   registered family nearest to the weight. 400, or nothing registered, keeps the base family.
//! - **Icons.** The `icon` kind draws a glyph in a named family (cadatomic ships Phosphor as family
//!   `icons`). [`register_icons`] gives the family a name table, so Svelte writes
//!   `<icon name="gear" />` instead of a code point.
//!
//! The tables are process-wide (names are names; every egui `Context` of the process shares them). A
//! family that is not in the `Context`'s font definitions is never used (egui panics on an unbound
//! `FontFamily::Name`): the base family draws instead.
//!
//! Writer lane: WIDGETS.

use egui::{FontFamily, FontId};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

#[derive(Default)]
struct Table {
    /// (base family, weight, family drawing that weight).
    weights: Vec<(FontFamily, u16, FontFamily)>,
    /// family name → glyph name → glyph text.
    icons: HashMap<String, HashMap<String, String>>,
}

static TABLE: LazyLock<RwLock<Table>> = LazyLock::new(RwLock::default);

/// `family` draws `weight` (CSS 100–900) of `base`. A later registration of the same base and weight
/// replaces the earlier one.
pub fn register_weight(base: FontFamily, weight: u16, family: FontFamily) {
    let mut t = TABLE.write().unwrap_or_else(|e| e.into_inner());
    t.weights.retain(|(b, w, _)| !(b == &base && *w == weight));
    t.weights.push((base, weight, family));
}

/// Adds glyph names to the name table of icon family `family` (`name → glyph text`, e.g.
/// `("gear", egui_phosphor::regular::GEAR)`). Later names replace earlier ones.
pub fn register_icons<I, K, V>(family: &str, names: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let mut t = TABLE.write().unwrap_or_else(|e| e.into_inner());
    let table = t.icons.entry(family.to_string()).or_default();
    for (k, v) in names {
        table.insert(k.into(), v.into());
    }
}

/// The glyph registered as `name` in `family`'s table.
pub fn icon(family: &str, name: &str) -> Option<String> {
    let t = TABLE.read().unwrap_or_else(|e| e.into_inner());
    t.icons.get(family)?.get(name).cloned()
}

/// Forgets every registration (tests).
pub fn clear() {
    *TABLE.write().unwrap_or_else(|e| e.into_inner()) = Table::default();
}

/// True if `family` can be used in `ctx` (the built-in families always can).
pub(crate) fn has_family(ctx: &egui::Context, family: &FontFamily) -> bool {
    match family {
        FontFamily::Proportional | FontFamily::Monospace => true,
        FontFamily::Name(_) => ctx.fonts(|f| f.definitions().families.contains_key(family)),
    }
}

/// The family drawing `weight` of `base`: the registered family whose weight is nearest (ties go to
/// the heavier one from 400 up, the lighter one below), with `base` itself standing for 400.
pub(crate) fn family_for(ctx: &egui::Context, base: &FontFamily, weight: i64) -> FontFamily {
    if weight == 400 {
        return base.clone();
    }
    let t = TABLE.read().unwrap_or_else(|e| e.into_inner());
    let mut best: (i64, i64, FontFamily) = ((weight - 400).abs(), 400, base.clone());
    for (b, w, f) in &t.weights {
        if b != base || !has_family(ctx, f) {
            continue;
        }
        let w = *w as i64;
        let d = (w - weight).abs();
        let better = d < best.0 || (d == best.0 && if weight >= 400 { w > best.1 } else { w < best.1 });
        if better {
            best = (d, w, f.clone());
        }
    }
    best.2
}

/// The text font of a kind with `font-weight` (`size` from the text style).
pub(crate) fn text_font(ctx: &egui::Context, style: &egui::Style, text_style: egui::TextStyle, weight: i64) -> FontId {
    let mut f = text_style.resolve(style);
    if weight != 400 {
        f.family = family_for(ctx, &f.family, weight);
    }
    f
}

/// The font of an `icon`: its family (the proportional one if not registered) at the weight's
/// family, `size` points or the body size.
pub(crate) fn icon_font(ctx: &egui::Context, style: &egui::Style, family: &str, weight: i64, size: Option<f64>) -> FontId {
    let body = egui::TextStyle::Body.resolve(style);
    let named = FontFamily::Name(family.into());
    let base = if !family.is_empty() && has_family(ctx, &named) { named } else { FontFamily::Proportional };
    FontId::new(size.map_or(body.size, |s| s as f32), family_for(ctx, &base, weight))
}

/// The text an `icon` draws: `name` from its family's table, else `glyph`, else `name` itself.
pub(crate) fn icon_text(family: &str, name: &str, glyph: &str) -> String {
    if !name.is_empty()
        && let Some(g) = icon(family, name)
    {
        return g;
    }
    if !glyph.is_empty() { glyph.to_string() } else { name.to_string() }
}
