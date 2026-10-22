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
//! The tables are process-wide (names are names; every egui `Context` of the process shares them).
//!
//! **Nothing here panics on a bad font setup** (WG1b). egui panics on a glyph no installed font has
//! when the `Context`'s `MissingGlyphPolicy` is `Panic` (kittest's default; an app may set it too),
//! so every family and glyph is checked before a `FontId` is built from it:
//! - an `icon` whose family is bound to no font of the `Context`, whose `name` is not in its family's
//!   table, or whose glyph no font of the family has draws a placeholder (an outlined box the size of
//!   the icon) instead;
//! - a weight family that is not bound, or lacks a character of the text, is skipped: the base
//!   family draws the text.
//!
//! Each problem is reported once per process ([`FontWarning`]): printed to stderr and queued for
//! [`take_warnings`] (also drained by `Embed::take_warnings` as `SlotWarning::Font`).
//! [`check_icons`] checks every registered name against a set of font definitions up front.
//!
//! Writer lane: WIDGETS.

use egui::{FontFamily, FontId};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, RwLock};

#[derive(Default)]
struct Table {
    /// (base family, weight, family drawing that weight).
    weights: Vec<(FontFamily, u16, FontFamily)>,
    /// family name → glyph name → glyph text.
    icons: HashMap<String, HashMap<String, String>>,
}

static TABLE: LazyLock<RwLock<Table>> = LazyLock::new(RwLock::default);

/// A font setup problem, found when an `icon` or a weighted text is measured or drawn, or by
/// [`check_icons`]. Nothing panics on it: the node draws a placeholder or the base family.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontWarning {
    /// An `icon`'s `family` is bound to no font of the `Context` (not in its `FontDefinitions`): a
    /// placeholder box draws instead of each of its icons.
    UnboundFamily { family: String },
    /// An `icon`'s `name` is not in its family's name table, and it has no `glyph`: a placeholder.
    UnknownIcon { family: String, name: String },
    /// No font of the family has the glyph (`text`) an icon draws (`name`, else its `glyph`): a
    /// placeholder.
    MissingGlyph { family: String, name: String, text: String },
    /// The family registered for a weight ([`register_weight`]) has no glyph for `ch`, a character of
    /// a text drawn at that weight: the base family draws that text.
    WeightGlyph { family: FontFamily, ch: char },
}

impl std::fmt::Display for FontWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontWarning::UnboundFamily { family } => write!(f, "icon family {family:?} has no fonts in the egui context: its icons draw a placeholder"),
            FontWarning::UnknownIcon { family, name } => write!(f, "icon {name:?} is not in the name table of family {family:?}: drawn as a placeholder"),
            FontWarning::MissingGlyph { family, name, text } => {
                write!(f, "icon {name:?} of family {family:?}: no installed font has its glyph {text:?}: drawn as a placeholder")
            }
            FontWarning::WeightGlyph { family, ch } => {
                write!(f, "weight family {family:?} has no glyph for {ch:?} (U+{:04X}): text with it draws in the base family", *ch as u32)
            }
        }
    }
}

/// Warnings not yet taken are capped (an embedder that never calls [`take_warnings`] loses the rest).
const MAX_QUEUED: usize = 256;

#[derive(Default)]
struct Reported {
    seen: HashSet<FontWarning>,
    queue: Vec<FontWarning>,
}

static REPORTED: LazyLock<Mutex<Reported>> = LazyLock::new(Mutex::default);

/// Reports `w` the first time it is seen in this process: stderr and the [`take_warnings`] queue.
fn report(w: FontWarning) {
    let mut r = REPORTED.lock().unwrap_or_else(|e| e.into_inner());
    if r.seen.contains(&w) {
        return;
    }
    eprintln!("[svelters-egui] {w}");
    if r.queue.len() < MAX_QUEUED {
        r.queue.push(w.clone());
    }
    r.seen.insert(w);
}

/// The font warnings reported since the last call (each problem once per process).
/// `Embed::take_warnings` drains the same queue (as `SlotWarning::Font`).
pub fn take_warnings() -> Vec<FontWarning> {
    std::mem::take(&mut REPORTED.lock().unwrap_or_else(|e| e.into_inner()).queue)
}

/// `family` draws `weight` (CSS 100–900) of `base`. A later registration of the same base and weight
/// replaces the earlier one. A family the `Context` does not have, or that lacks a character of a
/// text, is skipped for that text (the base family draws it).
pub fn register_weight(base: FontFamily, weight: u16, family: FontFamily) {
    let mut t = TABLE.write().unwrap_or_else(|e| e.into_inner());
    t.weights.retain(|(b, w, _)| !(b == &base && *w == weight));
    t.weights.push((base, weight, family));
}

/// Adds glyph names to the name table of icon family `family` (`name → glyph text`, e.g.
/// `("gear", egui_phosphor::regular::GEAR)`). Later names replace earlier ones.
///
/// The fonts are not known here (they belong to an egui `Context`): a name whose glyph the family's
/// fonts lack is found when it is drawn (a placeholder and a [`FontWarning`]), or up front with
/// [`check_icons`].
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

/// Checks every registered icon name against `fonts` (what the embedder passes to
/// `Context::set_fonts`): an unbound family, or a name whose glyph no font of its family has. The
/// problems are returned (sorted) and reported like the ones found when drawing.
pub fn check_icons(fonts: &egui::FontDefinitions) -> Vec<FontWarning> {
    let tables: Vec<(String, Vec<(String, String)>)> = {
        let t = TABLE.read().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<_> = t
            .icons
            .iter()
            .map(|(f, names)| {
                let mut n: Vec<_> = names.iter().map(|(k, g)| (k.clone(), g.clone())).collect();
                n.sort();
                (f.clone(), n)
            })
            .collect();
        v.sort();
        v
    };
    let mut out = Vec::new();
    let mut atlas: Option<egui::epaint::text::Fonts> = None;
    for (family, names) in tables {
        let fam = icon_family(&family);
        if !fonts.families.contains_key(&fam) {
            out.push(FontWarning::UnboundFamily { family });
            continue;
        }
        let atlas = atlas.get_or_insert_with(|| egui::epaint::text::Fonts::new(Default::default(), fonts.clone()));
        let id = FontId::new(14.0, fam);
        for (name, text) in names {
            if !text.chars().filter(|c| !c.is_control()).all(|c| atlas.has_glyph(&id, c)) {
                out.push(FontWarning::MissingGlyph { family: family.clone(), name, text });
            }
        }
    }
    for w in &out {
        report(w.clone());
    }
    out
}

/// The glyph registered as `name` in `family`'s table.
pub fn icon(family: &str, name: &str) -> Option<String> {
    let t = TABLE.read().unwrap_or_else(|e| e.into_inner());
    t.icons.get(family)?.get(name).cloned()
}

/// Forgets every registration and every reported warning (tests).
pub fn clear() {
    *TABLE.write().unwrap_or_else(|e| e.into_inner()) = Table::default();
    *REPORTED.lock().unwrap_or_else(|e| e.into_inner()) = Reported::default();
}

/// True if `family` can be used in `ctx` (the built-in families always can).
pub(crate) fn has_family(ctx: &egui::Context, family: &FontFamily) -> bool {
    match family {
        FontFamily::Proportional | FontFamily::Monospace => true,
        FontFamily::Name(_) => ctx.fonts(|f| f.definitions().families.contains_key(family)),
    }
}

/// The first character of `text` that no font of `family` has (control characters never count:
/// fonts have no glyphs for them, and egui does not panic on them).
fn missing_char(ctx: &egui::Context, family: &FontFamily, text: &str) -> Option<char> {
    let id = FontId::new(14.0, family.clone()); // `has_glyph` only reads the family
    ctx.fonts_mut(|f| text.chars().filter(|c| !c.is_control()).find(|&c| !f.has_glyph(&id, c)))
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

/// [`family_for`], unless that family lacks a character of `text`: then `base` (reported once per
/// family and character).
fn weighted_family(ctx: &egui::Context, base: &FontFamily, weight: i64, text: &str) -> FontFamily {
    let fam = family_for(ctx, base, weight);
    if fam == *base {
        return fam;
    }
    match missing_char(ctx, &fam, text) {
        None => fam,
        Some(ch) => {
            report(FontWarning::WeightGlyph { family: fam, ch });
            base.clone()
        }
    }
}

/// The font `text` of a kind with `font-weight` draws in (`size` from the text style).
pub(crate) fn text_font(ctx: &egui::Context, style: &egui::Style, text_style: egui::TextStyle, weight: i64, text: &str) -> FontId {
    let mut f = text_style.resolve(style);
    if weight != 400 {
        f.family = weighted_family(ctx, &f.family, weight, text);
    }
    f
}

/// What an `icon` draws.
pub(crate) enum IconLook {
    /// This text in this font.
    Glyph(FontId, String),
    /// An outlined square of this side (points) in place of a glyph that cannot be drawn.
    Placeholder(f32),
}

/// `FontFamily::Name(family)`, or Proportional for an empty `family`.
fn icon_family(family: &str) -> FontFamily {
    if family.is_empty() { FontFamily::Proportional } else { FontFamily::Name(family.into()) }
}

/// What an `icon` draws: its text (see [`icon_text`]) in its family at the weight's family, `size`
/// points or the body size; a placeholder of that size when the family is not bound, the name is
/// unknown or no font of the family has the glyph.
pub(crate) fn icon_look(ctx: &egui::Context, style: &egui::Style, family: &str, name: &str, glyph: &str, weight: i64, size: Option<f64>) -> IconLook {
    let pt = size.map_or_else(|| egui::TextStyle::Body.resolve(style).size, |s| s as f32);
    let base = icon_family(family);
    if !has_family(ctx, &base) {
        report(FontWarning::UnboundFamily { family: family.to_string() });
        return IconLook::Placeholder(pt);
    }
    let Some(text) = icon_text(family, name, glyph) else {
        report(FontWarning::UnknownIcon { family: family.to_string(), name: name.to_string() });
        return IconLook::Placeholder(pt);
    };
    let fam = weighted_family(ctx, &base, weight, &text);
    if fam == base && missing_char(ctx, &base, &text).is_some() {
        let name = if name.is_empty() { glyph } else { name };
        report(FontWarning::MissingGlyph { family: family.to_string(), name: name.to_string(), text });
        return IconLook::Placeholder(pt);
    }
    IconLook::Glyph(FontId::new(pt, fam), text)
}

/// The text an `icon` draws: `name` from its family's table, else `glyph`, else `name` itself when
/// the family has no table. `None`: `name` is not in the family's table (a typo, or a name the
/// embedder did not register).
pub(crate) fn icon_text(family: &str, name: &str, glyph: &str) -> Option<String> {
    let t = TABLE.read().unwrap_or_else(|e| e.into_inner());
    let table = t.icons.get(family);
    if !name.is_empty()
        && let Some(g) = table.and_then(|n| n.get(name))
    {
        return Some(g.clone());
    }
    if !glyph.is_empty() {
        return Some(glyph.to_string());
    }
    if name.is_empty() || table.is_none() { Some(name.to_string()) } else { None }
}
