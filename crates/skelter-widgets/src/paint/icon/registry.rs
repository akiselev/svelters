//! The icon registry and how registry icons are drawn (plans/research/ICON-STYLE.md §5–§6), ported
//! from svelters-egui `icons.rs` onto display lists.
//!
//! An [`IconRegistry`] maps a **key** (`file.save`, `debug.step-over`, `mcad.extrude+remove`) to an
//! [`Icon`]: an SVG with one drawing per optical size (12, 14, 16, 20, 24, 32+), or a glyph of an
//! icon font. Rust registers icons (the host or the app); Svelte looks them up by key
//! (`<icon name="file.save">`, `content: icon(check)`, `::part(icon)`). One registry is the
//! process's ([`IconRegistry::global`]); [`set_global`] swaps it (an embed that keeps its icons
//! apart).
//!
//! Keys are lowercase dotted segments of kebab-case (`[a-z0-9]+(-[a-z0-9]+)*`), with an optional
//! `+<badge>` suffix for IC1's composed ids. The `ext.` prefix belongs to extensions: an extension
//! registers only under `ext.<its id>.`, and nothing else registers there. That is a namespace rule
//! with a register-time diagnostic, not a security check.
//!
//! Drawing ([`paint`]) follows ICON-STYLE: the box snaps to whole device pixels (Z1) and the master is
//! chosen by the physical size (Z2, [`choose_master`]). Above [`VECTOR_THRESHOLD_PX`](super::vector::VECTOR_THRESHOLD_PX) physical
//! pixels a master that vectorises is drawn as `Prim::Path`s ([`super::vector`]); at or below it,
//! and for an icon the vector path cannot draw (reported once, [`IconError::Fallback`]), from a raster.
//! Colours are palette slots the compositor can change at draw time (`PaletteSlot`: 0 is `color`,
//! 1–3 the slots):
//! - no slot set: the SVG is rasterised to an `A8` coverage mask (`currentColor` white) and drawn as
//!   [`Prim::Icon`] tinted by `color` from slot 0 (a glyph tints the same way);
//! - a slot set (`--icon-subject`, `--icon-accent`, `--icon-badge`): one `A8` mask per slot its
//!   layers use (`vector::slot_masks`), each tinted from its slot, so new colours need no raster; an
//!   icon that does not vectorise is rasterised to premultiplied RGBA with `color` as the root colour
//!   and the slots injected as a usvg style sheet on the classes `subject`, `accent` and `badge` (only
//!   `color` declarations), drawn untinted (`color: None`; those slots are not live).
//!
//! Rasters enter the commit through [`Resources::image`](crate::shim::core::Resources::image) under
//! a key made of the registry key, its registration, the master, the physical size and the mask's
//! slot (with baked slots, the colours): one raster per distinct key; the commit builder owns their
//! lifetime. Vector paths enter through [`Resources::path`](crate::shim::core::Resources::path),
//! keyed by the SVG's content.
//!
//! usvg resolves `currentColor` at the element that *declares* the paint, so a classed group whose
//! stroke is inherited from the root would keep the root's colour. Before rasterising with slots,
//! [`slot_svg`] gives each classed element an explicit `fill`/`stroke="currentColor"` where it
//! inherits one, which makes its own (slot) `color` apply (ICON-STYLE §6's open question).
//!
//! Icon fonts: skelter-text owns fonts. This module keeps the families registered as icon fonts
//! ([`register_icon_font`], with an optional name → glyph table, [`register_icon_names`]); loading
//! the font into skelter-text under that family is the caller's.

use super::super::{PaintCx, StyleView};
use super::vector::{IconSlot, MaskChannel, NotVector, PaintSource, VectorArt, is_vector_size, mask_channels, slot_masks};
use skelter_base::display::PaletteSlot;
use crate::shim::text::{RichText, TextShaper, TextStyle};
use skelter_base::color::Color;
use skelter_base::commit::{ImageData, ImageFormat};
use skelter_base::display::{Icon as IconPrim, Prim};
use skelter_base::geom::{DeviceSize, Point, Rect, Size, Vec2};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, RwLock};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

// ---- keys ----

/// The prefix extensions register under: `ext.<id>.<name>`.
pub const EXTENSION_PREFIX: &str = "ext.";

fn segment_ok(s: &str) -> bool {
    !s.is_empty() && s.split('-').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// Checks a key: dotted kebab-case segments, optionally `+<badge>` (one kebab segment).
pub fn validate_key(key: &str) -> Result<(), IconError> {
    let bad = |why: &str| Err(IconError::BadKey { key: key.to_string(), why: why.to_string() });
    let (base, badge) = match key.split_once('+') {
        Some((b, g)) => (b, Some(g)),
        None => (key, None),
    };
    if base.is_empty() {
        return bad("empty");
    }
    if !base.split('.').all(segment_ok) {
        return bad("segments are lowercase kebab-case ([a-z0-9]+(-[a-z0-9]+)*) separated by '.'");
    }
    if let Some(g) = badge
        && !segment_ok(g)
    {
        return bad("the badge after '+' is one lowercase kebab-case segment");
    }
    Ok(())
}

/// Who registers an icon.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    /// The host or the app: any key outside `ext.`.
    Host,
    /// An extension by id (one kebab segment): only `ext.<id>.<name>`.
    Extension(String),
}

impl Owner {
    /// Whether this owner may register `key` (already a valid key).
    fn check(&self, key: &str) -> Result<(), IconError> {
        let refuse = |why: String| Err(IconError::Namespace { key: key.to_string(), owner: self.clone(), why });
        match self {
            Owner::Host if key.starts_with(EXTENSION_PREFIX) => refuse("`ext.*` keys belong to extensions".into()),
            Owner::Host => Ok(()),
            Owner::Extension(id) if !segment_ok(id) => refuse(format!("extension id {id:?} is not one kebab-case segment")),
            Owner::Extension(id) => {
                let prefix = format!("{EXTENSION_PREFIX}{id}.");
                match key.strip_prefix(&prefix) {
                    Some(rest) if !rest.is_empty() && !rest.starts_with('+') => Ok(()),
                    _ => refuse(format!("an extension registers only under `{prefix}<name>`")),
                }
            }
        }
    }
}

/// Why a registration was refused, or what drawing found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconError {
    BadKey { key: String, why: String },
    Namespace { key: String, owner: Owner, why: String },
    /// An icon with no master, or a master of size 0.
    BadIcon { key: String, why: String },
    /// Drawing: no icon is registered under the key (reported once per key).
    Missing { key: String },
    /// Drawing: the SVG did not load or rasterise, or a glyph's font is not registered (reported
    /// once per key and reason).
    Load { key: String, why: String },
    /// Drawing: an SVG icon above the vector threshold uses what the vector path cannot draw yet (a
    /// filter, a mask, a gradient…) and is drawn from its raster instead (reported once per key and
    /// reason; [`vector`](super::vector)).
    Fallback { key: String, why: String },
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::BadKey { key, why } => write!(f, "icon key {key:?}: {why}"),
            IconError::Namespace { key, owner, why } => write!(f, "icon key {key:?} refused for {owner:?}: {why}"),
            IconError::BadIcon { key, why } => write!(f, "icon {key:?}: {why}"),
            IconError::Missing { key } => write!(f, "no icon is registered under {key:?}"),
            IconError::Load { key, why } => write!(f, "icon {key:?} did not load: {why}"),
            IconError::Fallback { key, why } => write!(f, "icon {key:?} draws as a raster, not vectors: {why}"),
        }
    }
}

impl std::error::Error for IconError {}

#[derive(Default)]
struct Reported {
    seen: HashSet<String>,
    queue: Vec<IconError>,
}

static REPORTED: LazyLock<Mutex<Reported>> = LazyLock::new(Mutex::default);

fn report(e: IconError) {
    let mut r = lock(&REPORTED);
    let once = match &e {
        IconError::Missing { key } => Some(format!("missing:{key}")),
        IconError::Load { key, why } => Some(format!("load:{key}:{why}")),
        IconError::Fallback { key, why } => Some(format!("fallback:{key}:{why}")),
        _ => None,
    };
    if let Some(k) = once
        && !r.seen.insert(k)
    {
        return;
    }
    r.queue.push(e);
}

/// The registration refusals and drawing problems since the last call (the embedder logs or shows
/// them; tests assert on them). A missing key or a failed load is reported once.
pub fn take_diagnostics() -> Vec<IconError> {
    std::mem::take(&mut lock(&REPORTED).queue)
}

// ---- icons ----

/// An SVG drawing's bytes.
#[derive(Clone)]
pub enum IconData {
    /// Embedded in the binary (`include_bytes!`).
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl fmt::Debug for IconData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconData::Static(b) => write!(f, "Static({} bytes)", b.len()),
            IconData::Shared(b) => write!(f, "Shared({} bytes)", b.len()),
        }
    }
}

impl IconData {
    pub fn bytes(&self) -> &[u8] {
        match self {
            IconData::Static(b) => b,
            IconData::Shared(b) => b,
        }
    }
}

impl From<&'static [u8]> for IconData {
    fn from(b: &'static [u8]) -> IconData {
        IconData::Static(b)
    }
}

impl From<Arc<[u8]>> for IconData {
    fn from(b: Arc<[u8]>) -> IconData {
        IconData::Shared(b)
    }
}

impl From<Vec<u8>> for IconData {
    fn from(b: Vec<u8>) -> IconData {
        IconData::Shared(b.into())
    }
}

/// What a key draws.
#[derive(Clone, Debug)]
pub enum Icon {
    /// One SVG per optical size (pixels at 1x), and optionally High Contrast copies (IC1's `hc/`:
    /// the subject tone at .6).
    Svg { masters: BTreeMap<u32, IconData>, hc: BTreeMap<u32, IconData>, colors: IconColors },
    /// A glyph of a registered icon font ([`register_icon_font`]), sized by the box.
    Glyph { family: String, text: String },
}

/// How an SVG icon's colours are drawn (a per-icon registration choice, never inferred).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconColors {
    /// A monochrome icon: its shapes are coverage tinted by `color` (literal colours keep only their
    /// alpha), unless slots are set.
    #[default]
    Tint,
    /// A multicolour icon: literal colours are drawn as they are, at every size; `currentColor` and
    /// slot layers stay live (palette slots).
    Own,
}

impl Icon {
    /// An SVG icon with its `size` master, tinted ([`IconColors::Tint`]).
    pub fn svg(size: u32, data: impl Into<IconData>) -> Icon {
        Icon::Svg { masters: BTreeMap::from([(size, data.into())]), hc: BTreeMap::new(), colors: IconColors::Tint }
    }

    /// A multicolour SVG icon with its `size` master ([`IconColors::Own`]).
    pub fn multicolour(size: u32, data: impl Into<IconData>) -> Icon {
        Icon::svg(size, data).colors(IconColors::Own)
    }

    /// Sets how an SVG icon's colours are drawn; no-op on a glyph.
    pub fn colors(mut self, c: IconColors) -> Icon {
        if let Icon::Svg { colors, .. } = &mut self {
            *colors = c;
        }
        self
    }

    /// Adds (or replaces) the `size` master of an SVG icon; no-op on a glyph.
    pub fn master(mut self, size: u32, data: impl Into<IconData>) -> Icon {
        if let Icon::Svg { masters, .. } = &mut self {
            masters.insert(size, data.into());
        }
        self
    }

    /// Adds the High Contrast copy of the `size` master; no-op on a glyph.
    pub fn hc_master(mut self, size: u32, data: impl Into<IconData>) -> Icon {
        if let Icon::Svg { hc, .. } = &mut self {
            hc.insert(size, data.into());
        }
        self
    }

    /// A glyph of icon font `family`.
    pub fn glyph(family: impl Into<String>, text: impl Into<String>) -> Icon {
        Icon::Glyph { family: family.into(), text: text.into() }
    }

    /// The optical sizes (SVG masters); empty for a glyph.
    pub fn sizes(&self) -> Vec<u32> {
        match self {
            Icon::Svg { masters, .. } => masters.keys().copied().collect(),
            Icon::Glyph { .. } => Vec::new(),
        }
    }
}

// ---- master choice (ICON-STYLE §5.3) ----

/// How a master fills a box of `P` physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// A master of exactly `P` (crisp).
    Exact,
    /// Master `m` drawn at `2m = P` (crisp).
    Double,
    /// The largest master (or ×2 master) at most 2 pixels smaller, at its own size, centred.
    Padded,
    /// The nearest master (or ×2 master), scaled to `P` (soft, accepted).
    Scaled,
}

/// Which master draws a box of `p` physical pixels, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    /// The master's optical size.
    pub master: u32,
    /// The raster's side in physical pixels.
    pub px: u32,
    /// The raster's offset from the box's top left, in physical pixels (both axes).
    pub offset: u32,
    pub fit: Fit,
}

/// ICON-STYLE rule Z2: a master of exactly `p`; else `p = 2m`; else the largest master or ×2
/// master `m ≤ p` with `p − m ≤ 2`, centred with `floor((p − m) / 2)`; else the nearest master or
/// ×2 master, scaled (a tie goes to the larger). `None` when there are no masters.
pub fn choose_master(masters: &[u32], p: u32) -> Option<Choice> {
    let p = p.max(1);
    if masters.contains(&p) {
        return Some(Choice { master: p, px: p, offset: 0, fit: Fit::Exact });
    }
    if p.is_multiple_of(2) && masters.contains(&(p / 2)) {
        return Some(Choice { master: p / 2, px: p, offset: 0, fit: Fit::Double });
    }
    // (drawn size, master) for every master and its ×2
    let drawn: Vec<(u32, u32)> = masters.iter().flat_map(|&m| [(m, m), (2 * m, m)]).collect();
    if let Some(&(d, m)) = drawn.iter().filter(|(d, _)| *d <= p && p - d <= 2).max_by_key(|(d, m)| (*d, *m)) {
        return Some(Choice { master: m, px: d, offset: (p - d) / 2, fit: Fit::Padded });
    }
    let &(_, m) = drawn.iter().min_by_key(|(d, m)| (d.abs_diff(p), std::cmp::Reverse(*d), std::cmp::Reverse(*m)))?;
    Some(Choice { master: m, px: p, offset: 0, fit: Fit::Scaled })
}

// ---- the registry ----

struct Entry {
    icon: Icon,
    owner: Owner,
    generation: u64,
}

/// Keys to icons. One is the process's ([`IconRegistry::global`], swapped by [`set_global`]).
#[derive(Default)]
pub struct IconRegistry {
    entries: RwLock<HashMap<String, Entry>>,
    generation: AtomicU64,
}

impl fmt::Debug for IconRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IconRegistry").field("keys", &self.keys().len()).finish()
    }
}

static GLOBAL: LazyLock<RwLock<Arc<IconRegistry>>> = LazyLock::new(|| RwLock::new(Arc::new(IconRegistry::default())));

/// Makes `registry` the process's (what [`IconRegistry::global`] returns from now on); returns the
/// one it replaces.
pub fn set_global(registry: Arc<IconRegistry>) -> Arc<IconRegistry> {
    std::mem::replace(&mut *GLOBAL.write().unwrap_or_else(|p| p.into_inner()), registry)
}

impl IconRegistry {
    pub fn new() -> Arc<IconRegistry> {
        Arc::new(IconRegistry::default())
    }

    /// The process's registry: what every kind draws from.
    pub fn global() -> Arc<IconRegistry> {
        GLOBAL.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Registers `icon` under `key` for `owner`, replacing an earlier one. A bad key, a key outside
    /// the owner's namespace or an icon without masters is refused, returned and reported
    /// ([`take_diagnostics`]).
    pub fn register(&self, owner: Owner, key: &str, icon: Icon) -> Result<(), IconError> {
        let r = self.try_register(owner, key, icon);
        if let Err(e) = &r {
            report(e.clone());
        }
        r
    }

    fn try_register(&self, owner: Owner, key: &str, icon: Icon) -> Result<(), IconError> {
        validate_key(key)?;
        owner.check(key)?;
        if let Icon::Svg { masters, .. } = &icon
            && (masters.is_empty() || masters.contains_key(&0))
        {
            return Err(IconError::BadIcon { key: key.to_string(), why: "an SVG icon needs at least one master of a nonzero size".into() });
        }
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.entries.write().unwrap_or_else(|p| p.into_inner()).insert(key.to_string(), Entry { icon, owner, generation });
        Ok(())
    }

    /// Removes `key` if `owner` registered it.
    pub fn unregister(&self, owner: &Owner, key: &str) -> bool {
        let mut e = self.entries.write().unwrap_or_else(|p| p.into_inner());
        if e.get(key).is_some_and(|x| &x.owner == owner) {
            e.remove(key);
            self.generation.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Removes every icon of `owner` (an extension unloading); returns how many.
    pub fn unregister_owner(&self, owner: &Owner) -> usize {
        let mut e = self.entries.write().unwrap_or_else(|p| p.into_inner());
        let before = e.len();
        e.retain(|_, x| &x.owner != owner);
        before - e.len()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.read().unwrap_or_else(|p| p.into_inner()).contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<Icon> {
        self.entries.read().unwrap_or_else(|p| p.into_inner()).get(key).map(|e| e.icon.clone())
    }

    /// Every key, sorted.
    pub fn keys(&self) -> Vec<String> {
        let mut k: Vec<_> = self.entries.read().unwrap_or_else(|p| p.into_inner()).keys().cloned().collect();
        k.sort();
        k
    }

    fn entry(&self, key: &str) -> Option<(Icon, u64)> {
        self.entries.read().unwrap_or_else(|p| p.into_inner()).get(key).map(|e| (e.icon.clone(), e.generation))
    }

    /// The master that draws `key` in a box of `box_pt` points at `scale` device pixels per point
    /// (P = round(box_pt × scale), rule Z2). `None` for an unknown key or a glyph icon.
    pub fn resolve(&self, key: &str, box_pt: f32, scale: f32) -> Option<Choice> {
        let (icon, _) = self.entry(key)?;
        choose_master(&icon.sizes(), physical(box_pt, scale))
    }
}

/// `round(pt × scale)`, at least 1.
fn physical(pt: f32, scale: f32) -> u32 {
    (pt * scale).round().max(1.0) as u32
}

/// The box side of an `icon` with no `size`: 16 points, the default chrome size (ICON-STYLE §5.1).
pub const DEFAULT_SIZE: f32 = 16.0;

/// The global registry when it has `key` (the default family, a registered key).
pub fn registered(key: &super::IconKey) -> Option<Arc<IconRegistry>> {
    let k = key.registry_key().filter(|k| !k.is_empty())?;
    let reg = IconRegistry::global();
    reg.contains(k).then_some(reg)
}

// ---- icon fonts ----

/// Icon font families, each with its name → glyph table (empty: names are ligatures).
static FONTS: LazyLock<RwLock<HashMap<String, HashMap<String, String>>>> = LazyLock::new(RwLock::default);

/// Registers `family` as an icon font: `family:name` icon keys and [`Icon::Glyph`]s of it draw as
/// text in that family. Loading the font into skelter-text under `family` is the caller's.
pub fn register_icon_font(family: &str) {
    FONTS.write().unwrap_or_else(|p| p.into_inner()).entry(family.to_string()).or_default();
}

/// Registers `family` as an icon font with a name → glyph table (codicons' `check` → `"\u{eab2}"`),
/// added to any it has.
pub fn register_icon_names(family: &str, names: impl IntoIterator<Item = (String, String)>) {
    FONTS.write().unwrap_or_else(|p| p.into_inner()).entry(family.to_string()).or_default().extend(names);
}

/// Forgets icon font `family`; whether it was registered.
pub fn unregister_icon_font(family: &str) -> bool {
    FONTS.write().unwrap_or_else(|p| p.into_inner()).remove(family).is_some()
}

/// Whether an icon font of `family` is registered.
pub fn has_font(family: &str) -> bool {
    FONTS.read().unwrap_or_else(|p| p.into_inner()).contains_key(family)
}

/// The text an icon of `family` draws: `name` from the family's table, else `glyph`, else `name`
/// itself when the family has no table (a ligature font, or no icon font). `None`: `name` is not in
/// the family's table (a typo, or a name the embedder did not register).
pub fn icon_text(family: &str, name: &str, glyph: &str) -> Option<String> {
    let fonts = FONTS.read().unwrap_or_else(|p| p.into_inner());
    let table = fonts.get(family).filter(|t| !t.is_empty());
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

// ---- colours ----

/// The named colour slots (ICON-STYLE §6); `None` keeps the part on `currentColor`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slots {
    pub subject: Option<Color>,
    pub accent: Option<Color>,
    pub badge: Option<Color>,
}

impl Slots {
    /// The colour of `slot`, when set.
    pub fn get(&self, slot: IconSlot) -> Option<Color> {
        match slot {
            IconSlot::Subject => self.subject,
            IconSlot::Accent => self.accent,
            IconSlot::Badge => self.badge,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.subject.is_none() && self.accent.is_none() && self.badge.is_none()
    }

    /// The slots a style sets (`--icon-subject`, `--icon-accent`, `--icon-badge`; custom
    /// properties inherit, so a part's computed style carries its host's).
    pub fn of(s: &StyleView) -> Slots {
        let c = s.computed();
        let get = |n: &str| c.and_then(|c| c.custom(n));
        Slots::from_css(get("--icon-subject"), get("--icon-accent"), get("--icon-badge"))
    }

    /// From CSS custom property values (`--icon-subject` etc., as computed text); a value that is
    /// not a colour is ignored.
    pub fn from_css(subject: Option<&str>, accent: Option<&str>, badge: Option<&str>) -> Slots {
        Slots { subject: subject.and_then(parse_css_color), accent: accent.and_then(parse_css_color), badge: badge.and_then(parse_css_color) }
    }
}

/// A CSS colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, a named colour) as svgtypes
/// reads it, through usvg (no direct svgtypes dependency): a one-pixel SVG with the colour as
/// `color`, read back.
pub fn parse_css_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.is_empty() || s.contains(['"', '<', '>', ';', '{', '}']) {
        return None;
    }
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" color="{s}"><rect width="1" height="1" fill="currentColor"/></svg>"#);
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let resvg::usvg::Node::Path(p) = tree.root().children().first()? else { return None };
    let f = p.fill()?;
    let resvg::usvg::Paint::Color(c) = f.paint() else { return None };
    let a = (f.opacity().get() * 255.0).round() as u8;
    // an unparsable `color` falls back to black in usvg: accept black only when asked for
    if (c.red, c.green, c.blue, a) == (0, 0, 0, 255) && !is_black(s) {
        return None;
    }
    Some(Color::from_rgba8(c.red, c.green, c.blue, a))
}

fn is_black(s: &str) -> bool {
    let s = s.to_ascii_lowercase().replace(' ', "");
    matches!(s.as_str(), "black" | "#000" | "#000000" | "#000f" | "#000000ff" | "rgb(0,0,0)" | "rgba(0,0,0,1)")
}

/// How an icon is drawn: its box side in points, the tint (`color`), the slots and the theme's
/// contrast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconStyle {
    pub size: f32,
    pub color: Color,
    pub slots: Slots,
    /// Use the High Contrast masters when the icon has them.
    pub high_contrast: bool,
}

impl IconStyle {
    pub fn new(size: f32, color: Color) -> IconStyle {
        IconStyle { size, color, slots: Slots::default(), high_contrast: false }
    }
}

fn rgba8(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn css_rgba(c: Color) -> String {
    let [r, g, b, a] = rgba8(c);
    format!("rgba({r},{g},{b},{:.3})", a as f32 / 255.0)
}

/// The usvg style sheet for `color` and `slots`: `svg{color:…}` (opaque) plus one `color`
/// declaration per set slot on its class. Nothing but `color` ever reaches an icon (ICON-STYLE §6,
/// L3/L4).
pub fn slot_style_sheet(color: Color, slots: &Slots) -> String {
    let mut s = format!("svg{{color:{}}}", css_rgba(color.with_alpha(1.0)));
    for (class, c) in [("subject", slots.subject), ("accent", slots.accent), ("badge", slots.badge)] {
        if let Some(c) = c {
            s.push_str(&format!(".{class}{{color:{}}}", css_rgba(c)));
        }
    }
    s
}

/// `svg` with an explicit `fill`/`stroke="currentColor"` on each element of class `subject`,
/// `accent` or `badge` that inherits that paint as `currentColor` from an ancestor, so usvg
/// resolves it against the element's own (slot) `color`. Unchanged when nothing needs it.
pub fn slot_svg(svg: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(svg).map_err(|e| e.to_string())?;
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for n in doc.descendants().filter(|n| n.is_element()) {
        let classed = n.attribute("class").is_some_and(|c| c.split_whitespace().any(|c| matches!(c, "subject" | "accent" | "badge")));
        if !classed {
            continue;
        }
        let mut add = String::new();
        for attr in ["fill", "stroke"] {
            if n.attribute(attr).is_some() {
                continue;
            }
            let inherited = n.ancestors().skip(1).filter(|a| a.is_element()).find_map(|a| a.attribute(attr));
            if inherited.is_some_and(|v| v.trim().eq_ignore_ascii_case("currentcolor")) {
                add.push_str(&format!(r#" {attr}="currentColor""#));
            }
        }
        if !add.is_empty() {
            // after `<tag`
            let start = n.range().start;
            let name_len = text[start + 1..].find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(0);
            inserts.push((start + 1 + name_len, add));
        }
    }
    if inserts.is_empty() {
        return Ok(svg.to_vec());
    }
    let mut out = text.to_string();
    inserts.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    for (at, s) in inserts {
        out.insert_str(at, &s);
    }
    Ok(out.into_bytes())
}

// ---- rasterising ----

/// The style sheet of a tinted (monochrome) raster: `currentColor` white.
pub const MONO_STYLE: &str = "svg { color: #fff }";

/// usvg options: the injected `style_sheet`, and no reads outside the document.
fn svg_options(style_sheet: &str) -> resvg::usvg::Options<'static> {
    let mut o = resvg::usvg::Options { style_sheet: Some(style_sheet.to_string()), ..Default::default() };
    o.image_href_resolver.resolve_string = Box::new(|_, _| None);
    o
}

/// The size of an SVG drawing in its own units (points), or why it does not parse.
pub fn svg_size(svg: &[u8]) -> Result<Size, String> {
    let tree = resvg::usvg::Tree::from_data(svg, &svg_options(MONO_STYLE)).map_err(|e| e.to_string())?;
    let s = tree.size();
    if s.width() <= 0.0 || s.height() <= 0.0 {
        return Err("empty svg".into());
    }
    Ok(Size::new(s.width(), s.height()))
}

/// Renders `svg` under `style_sheet` into `w`×`h` pixels: stretched to fill them, or (`keep_aspect`)
/// scaled to fit them keeping its aspect ratio, centred. Premultiplied RGBA.
fn render(svg: &[u8], w: u32, h: u32, style_sheet: &str, keep_aspect: bool) -> Result<ImageData, String> {
    use resvg::tiny_skia::{Pixmap, Transform};
    let tree = resvg::usvg::Tree::from_data(svg, &svg_options(style_sheet)).map_err(|e| e.to_string())?;
    let (sw, sh) = (tree.size().width(), tree.size().height());
    if sw <= 0.0 || sh <= 0.0 {
        return Err("empty svg".into());
    }
    let (w, h) = (w.max(1), h.max(1));
    let mut pixmap = Pixmap::new(w, h).ok_or_else(|| format!("svg of {w}x{h} pixels cannot be rasterised"))?;
    let t = if keep_aspect {
        let k = (w as f32 / sw).min(h as f32 / sh);
        Transform::from_row(k, 0.0, 0.0, k, (w as f32 - sw * k) / 2.0, (h as f32 - sh * k) / 2.0)
    } else {
        Transform::from_scale(w as f32 / sw, h as f32 / sh)
    };
    resvg::render(&tree, t, &mut pixmap.as_mut());
    Ok(ImageData { size: DeviceSize { w, h }, format: ImageFormat::Rgba8Premul, pixels: pixmap.take() })
}

/// Renders an image `svg` stretched to `w`×`h` pixels as it is (its own colours), premultiplied
/// RGBA; the `image` kind's SVG sources.
pub fn render_svg(svg: &[u8], w: u32, h: u32) -> Result<ImageData, String> {
    render(svg, w, h, "", false)
}

/// Renders `svg` stretched to `w`×`h` pixels with `currentColor` white, as an `A8` coverage mask
/// (to be tinted).
pub fn render_svg_mask(svg: &[u8], w: u32, h: u32) -> Result<ImageData, String> {
    render(svg, w, h, MONO_STYLE, false).map(|i| coverage(&i))
}

/// The alpha of a premultiplied RGBA image as an `A8` coverage mask (an `A8` image is kept).
pub fn coverage(img: &ImageData) -> ImageData {
    match img.format {
        ImageFormat::A8 => img.clone(),
        ImageFormat::Rgba8Premul => ImageData { size: img.size, format: ImageFormat::A8, pixels: img.pixels.chunks_exact(4).map(|p| p[3]).collect() },
    }
}

/// Rasterises one master into a `px`-pixel square (the drawing fitted and centred): an `A8`
/// coverage mask of white `currentColor` (to be tinted) when `slots` is empty, else premultiplied
/// RGBA in `color` (its alpha applied to the whole raster) and the slots, drawn untinted.
pub fn rasterize(svg: &[u8], px: u32, color: Color, slots: &Slots) -> Result<ImageData, String> {
    rasterize_colors(svg, px, color, slots, IconColors::Tint)
}

/// [`rasterize`] for an icon registered with `colors`: an [`IconColors::Own`] icon is premultiplied
/// RGBA in its own colours (with `color` and the slots) even when no slot is set.
pub fn rasterize_colors(svg: &[u8], px: u32, color: Color, slots: &Slots, colors: IconColors) -> Result<ImageData, String> {
    let px = px.max(1);
    if slots.is_empty() && colors == IconColors::Tint {
        return render(svg, px, px, MONO_STYLE, true).map(|i| coverage(&i));
    }
    let mut img = render(&slot_svg(svg)?, px, px, &slot_style_sheet(color, slots), true)?;
    let a = color.a.clamp(0.0, 1.0);
    if a < 1.0 {
        for v in &mut img.pixels {
            *v = (*v as f32 * a).round() as u8;
        }
    }
    Ok(img)
}

// ---- drawing ----

/// The [`Resources::image`](crate::shim::core::Resources::image) key of a raster: the registry key,
/// its registration, the master (and whether High Contrast), the physical size, and the colours
/// when slots are set (a tinted mask does not depend on them).
fn raster_key(key: &str, generation: u64, choice: &Choice, hc: bool, style: &IconStyle, literal: bool) -> String {
    let mut k = format!("icon:{key}#{generation}@{}{}/{}", choice.master, if hc { "hc" } else { "" }, choice.px);
    if literal {
        let hex = |c: Option<Color>| c.map_or("-".to_string(), |c| rgba8(c).iter().map(|b| format!("{b:02x}")).collect());
        k.push_str(&format!(";{},{},{},{}", hex(Some(style.color)), hex(style.slots.subject), hex(style.slots.accent), hex(style.slots.badge)));
    }
    k
}

/// What [`paint`] drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Painted {
    /// The drawn rect in points (on the device pixel grid).
    pub rect: Rect,
    /// The master and fit, for an SVG icon.
    pub choice: Option<Choice>,
    /// The tint the raster or glyph was painted with (`None`: the raster's own colours, slots set).
    pub tint: Option<Color>,
    /// Drawn as vectors (`Prim::Path`s), not from a raster.
    pub vector: bool,
}

fn glyph_style(family: &str, size: f32) -> TextStyle {
    TextStyle { family: Arc::from(family), size, ..TextStyle::default() }
}

/// The size of `key`'s box at `size_pt` points: a square for SVG icons, the glyph's size for a
/// glyph icon (a square when its font is not registered). `None` for an unknown key.
pub fn size(shaper: &mut dyn TextShaper, registry: &IconRegistry, key: &str, size_pt: f32) -> Option<Vec2> {
    match registry.get(key)? {
        Icon::Svg { .. } => Some(Vec2::new(size_pt, size_pt)),
        Icon::Glyph { family, text } => {
            if !has_font(&family) {
                return Some(Vec2::new(size_pt, size_pt));
            }
            let l = shaper.layout(&RichText::plain(&text, glyph_style(&family, size_pt)), None);
            Some(Vec2::new(l.size().w, l.size().h))
        }
    }
}

/// Paints `key` from `registry` centred in `rect` (rules Z1 and Z2). `None` when the key is not
/// registered (reported once) or the icon cannot be drawn (reported once; nothing is painted).
pub fn paint(cx: &mut PaintCx, registry: &IconRegistry, key: &str, rect: Rect, style: &IconStyle) -> Option<Painted> {
    let Some((icon, generation)) = registry.entry(key) else {
        report(IconError::Missing { key: key.to_string() });
        return None;
    };
    let center = Point::new((rect.min.x + rect.max.x) / 2.0, (rect.min.y + rect.max.y) / 2.0);
    match icon {
        Icon::Glyph { family, text } => {
            if !has_font(&family) {
                report(IconError::Load { key: key.to_string(), why: format!("font family {family:?} is not registered as an icon font") });
                return None;
            }
            let l = cx.text.layout(&RichText::plain(&text, glyph_style(&family, style.size)), None);
            let pos = cx.snap_point(Point::new(center.x - l.size().w / 2.0, center.y - l.size().h / 2.0));
            let color = style.color;
            super::super::paint_runs(cx, &l, pos, &|_| color);
            Some(Painted { rect: Rect::from_origin_size(pos, l.size()), choice: None, tint: Some(color), vector: false })
        }
        Icon::Svg { masters, hc, colors } => {
            // Z1: the box on whole device pixels, centred in `rect`
            let scale = cx.scale;
            let p = physical(style.size, scale);
            let origin = Point::new((center.x * scale - p as f32 / 2.0).round(), (center.y * scale - p as f32 / 2.0).round());
            let sizes: Vec<u32> = masters.keys().copied().collect();
            let choice = choose_master(&sizes, p)?;
            let use_hc = style.high_contrast && hc.contains_key(&choice.master);
            let data = if use_hc { &hc[&choice.master] } else { &masters[&choice.master] };
            let side = choice.px as f32 / scale;
            let min = Point::new((origin.x + choice.offset as f32) / scale, (origin.y + choice.offset as f32) / scale);
            let r = Rect::from_origin_size(min, Size::new(side, side));
            // literal colours drawn as they are: a multicolour icon, or slots set
            let literal = colors == IconColors::Own || !style.slots.is_empty();
            let tint = (!literal).then_some(style.color);
            let art = match VectorArt::parse(data.bytes()) {
                Ok(a) => Some(a),
                Err(NotVector::Unsupported(why)) => {
                    if is_vector_size(choice.px) {
                        report(IconError::Fallback { key: key.to_string(), why });
                    }
                    None
                }
                Err(_) => None,
            };
            // above the threshold: the layers as paths, in live palette colours
            if let Some(art) = &art
                && is_vector_size(choice.px)
                && art.record(cx, art.fit(r), &|src, op| layer_paint(style, literal, src, op))
            {
                return Some(Painted { rect: r, choice: Some(choice), tint, vector: true });
            }
            // literal colours or slots on a vectorisable icon: one coverage mask per palette slot,
            // tinted at draw time, and one RGBA raster of the literal colours
            if let Some(art) = art.as_ref().filter(|_| literal) {
                // the masks depend on which slots are set and on the colour mode, not on the colours
                let set: String = IconSlot::ALL.iter().map(|s| if style.slots.get(*s).is_some() { '1' } else { '0' }).collect();
                let base = format!("{};m{}{set}", raster_key(key, generation, &choice, use_hc, style, false), if colors == IconColors::Own { "o" } else { "t" });
                let channel = |src: PaintSource| mask_channel(style, literal, src);
                let mut masks: Option<Vec<(MaskChannel, ImageData)>> = None;
                let mut drawn = Vec::new();
                for ch in mask_channels(art, &channel) {
                    let name = match ch {
                        MaskChannel::Literal => "lit".to_string(),
                        MaskChannel::Slot(s) => format!("s{s}"),
                    };
                    let image = cx.res.image(&format!("{base};{name}"), &mut || {
                        let all = masks.get_or_insert_with(|| slot_masks(art, choice.px, &channel));
                        all.iter().find(|(c, _)| *c == ch).map(|(_, i)| i.clone())
                    });
                    let Some(image) = image else { break };
                    drawn.push((ch, image));
                }
                if !drawn.is_empty() {
                    for (ch, image) in drawn {
                        let (color, slot) = match ch {
                            MaskChannel::Literal => (None, None),
                            MaskChannel::Slot(s) => (Some(slot_color(style, s)), Some(PaletteSlot::new(s, 1.0))),
                        };
                        cx.push(Prim::Icon(IconPrim { image, rect: r, color, slot }), r);
                    }
                    return Some(Painted { rect: r, choice: Some(choice), tint, vector: false });
                }
            }
            let ck = raster_key(key, generation, &choice, use_hc, style, literal);
            let mut failed = None;
            let image = cx.res.image(&ck, &mut || match rasterize_colors(data.bytes(), choice.px, style.color, &style.slots, colors) {
                Ok(i) => Some(i),
                Err(why) => {
                    failed = Some(why);
                    None
                }
            });
            let Some(image) = image else {
                report(IconError::Load { key: key.to_string(), why: failed.unwrap_or_else(|| "the raster was not registered".into()) });
                return None;
            };
            // a tinted mask's colour is palette slot 0; slots baked into an RGBA raster are not live
            let slot = tint.map(|_| PaletteSlot::new(0, 1.0));
            cx.push(Prim::Icon(IconPrim { image, rect: r, color: tint, slot }), r);
            Some(Painted { rect: r, choice: Some(choice), tint, vector: false })
        }
    }
}

/// The colour of palette slot `index` under `style`: 0 is `color`, 1–3 the set slots (an unset slot
/// is `color`).
fn slot_color(style: &IconStyle, index: u8) -> Color {
    IconSlot::ALL.iter().find(|s| s.index() == index).and_then(|s| style.slots.get(*s)).unwrap_or(style.color)
}

/// The palette slot a layer of `src` draws from under `style`: its slot when the style sets it, else
/// `currentColor` (0); a literal colour is its own colour when `literal` (a multicolour icon, or slots
/// set), else a slot-0 coverage (a monochrome icon, as the `A8` raster draws it).
fn layer_slot(style: &IconStyle, literal: bool, src: PaintSource) -> Option<u8> {
    match src {
        PaintSource::Slot(s) if style.slots.get(s).is_some() => Some(s.index()),
        PaintSource::Color(_) if literal => None,
        _ => Some(0),
    }
}

/// A vector layer's colour and palette slot under `style`.
fn layer_paint(style: &IconStyle, literal: bool, src: PaintSource, opacity: f32) -> (Color, Option<PaletteSlot>) {
    let lit = match src {
        PaintSource::Color(c) => c,
        _ => Color::WHITE,
    };
    match layer_slot(style, literal, src) {
        Some(i) => {
            let slot = PaletteSlot::new(i, opacity * lit.a);
            (slot.resolve(slot_color(style, i)), Some(slot))
        }
        None => (lit.with_alpha(lit.a * opacity), None),
    }
}

/// Where a layer of `src` goes in the per-slot masks under `style`.
fn mask_channel(style: &IconStyle, literal: bool, src: PaintSource) -> MaskChannel {
    match layer_slot(style, literal, src) {
        Some(i) => MaskChannel::Slot(i),
        None => MaskChannel::Literal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::vector::{LayerKind, VECTOR_THRESHOLD_PX};
    use crate::shim::testing::{MemResources, MonoShaper};
    use skelter_base::display::DisplayList;

    const SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="currentColor"/></svg>"#;
    const SLOTTED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor"><rect class="subject" width="8" height="16"/><rect x="8" width="8" height="16"/></svg>"#;

    #[test]
    fn keys() {
        for k in ["file.save", "debug.step-over", "check", "mcad.extrude+remove", "ecad.via-stitch", "a1.b2-c3"] {
            assert_eq!(validate_key(k), Ok(()), "{k}");
        }
        for k in ["", "File.save", "file..save", ".file", "file.", "file_save", "file.-x", "file.x-", "a+", "a+b+c", "a b", "+x"] {
            assert!(validate_key(k).is_err(), "{k}");
        }
    }

    #[test]
    fn namespaces() {
        let ext = Owner::Extension("acme".into());
        assert!(ext.check("ext.acme.gear").is_ok());
        assert!(ext.check("ext.acme.tools.gear+add").is_ok());
        assert!(ext.check("ext.acme").is_err());
        assert!(ext.check("ext.other.gear").is_err());
        assert!(ext.check("file.save").is_err());
        assert!(Owner::Host.check("ext.acme.gear").is_err());
        assert!(Owner::Host.check("file.save").is_ok());
        assert!(Owner::Extension("Bad.Id".into()).check("ext.Bad.Id.x").is_err());
    }

    #[test]
    fn register_get_unregister() {
        let reg = IconRegistry::new();
        let acme = Owner::Extension("acme".into());
        reg.register(Owner::Host, "file.save", Icon::svg(16, SQUARE).master(24, SQUARE)).unwrap();
        reg.register(acme.clone(), "ext.acme.gear", Icon::svg(16, SQUARE)).unwrap();
        reg.register(acme.clone(), "ext.acme.cog", Icon::glyph("codicons", "x")).unwrap();
        assert!(reg.contains("file.save"));
        assert_eq!(reg.get("file.save").unwrap().sizes(), [16, 24]);
        assert_eq!(reg.keys(), ["ext.acme.cog", "ext.acme.gear", "file.save"]);
        assert!(!reg.unregister(&acme, "file.save"), "not acme's");
        assert!(reg.unregister(&Owner::Host, "file.save"));
        assert_eq!(reg.unregister_owner(&acme), 2);
        assert!(reg.keys().is_empty());
    }

    #[test]
    fn resolve_by_physical_size() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "file.save", Icon::svg(16, SQUARE).master(24, SQUARE)).unwrap();
        // 16pt at 1.5x: 24 physical pixels, the 24 master
        assert_eq!(reg.resolve("file.save", 16.0, 1.5).map(|c| (c.master, c.fit)), Some((24, Fit::Exact)));
        // 16pt at 2x: the 16 master doubled
        assert_eq!(reg.resolve("file.save", 16.0, 2.0).map(|c| (c.master, c.fit)), Some((16, Fit::Double)));
        reg.register(Owner::Host, "glyph.icon", Icon::glyph("codicons", "x")).unwrap();
        assert_eq!(reg.resolve("glyph.icon", 16.0, 1.0), None);
        assert_eq!(reg.resolve("nothing", 16.0, 1.0), None);
    }

    #[test]
    fn z2_matches_icon_style_table() {
        // ICON-STYLE §5.3: a 16pt icon with the 16, 20 and 24 masters
        let m = [16, 20, 24];
        let case = |p| {
            let c = choose_master(&m, p).unwrap();
            (c.master, c.px, c.offset, c.fit)
        };
        assert_eq!(case(16), (16, 16, 0, Fit::Exact));
        assert_eq!(case(18), (16, 16, 1, Fit::Padded));
        assert_eq!(case(19), (20, 19, 0, Fit::Scaled));
        assert_eq!(case(20), (20, 20, 0, Fit::Exact));
        assert_eq!(case(21), (20, 20, 0, Fit::Padded));
        assert_eq!(case(22), (20, 20, 1, Fit::Padded));
        assert_eq!(case(24), (24, 24, 0, Fit::Exact));
        assert_eq!(case(26), (24, 24, 1, Fit::Padded));
        assert_eq!(case(27), (24, 27, 0, Fit::Scaled));
        assert_eq!(case(29), (16, 29, 0, Fit::Scaled));
        assert_eq!(case(32), (16, 32, 0, Fit::Double));
        assert_eq!(choose_master(&[], 16), None);
    }

    #[test]
    fn slot_rewrite_makes_inherited_paints_local() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" stroke="currentColor"><g class="accent"><path d="M1 1H2"/></g><path class="subject" fill="currentColor" d="M3 3H4"/></svg>"#;
        let out = String::from_utf8(slot_svg(svg).unwrap()).unwrap();
        assert!(out.contains(r#"<g stroke="currentColor" class="accent">"#), "{out}");
        // fill is inherited as `none`: untouched; stroke inherited currentColor: made local
        assert!(out.contains(r#"<path stroke="currentColor" class="subject" fill="currentColor""#), "{out}");
    }

    #[test]
    fn slot_style_sheet_has_only_colours() {
        let slots = Slots { subject: Some(Color::from_rgba8(0xCC, 0xA7, 0, 255)), accent: None, badge: None };
        assert_eq!(slot_style_sheet(Color::from_rgba8(255, 255, 255, 128), &slots), "svg{color:rgba(255,255,255,1.000)}.subject{color:rgba(204,167,0,1.000)}");
    }

    #[test]
    fn css_colours() {
        assert_eq!(parse_css_color("#CCA700"), Some(Color::from_rgba8(0xCC, 0xA7, 0x00, 255)));
        assert_eq!(parse_css_color("rgb(255, 0, 0)"), Some(Color::from_rgba8(255, 0, 0, 255)));
        assert_eq!(parse_css_color("black"), Some(Color::BLACK));
        assert_eq!(parse_css_color("nonsense"), None);
        assert_eq!(parse_css_color("red;}x{"), None);
        let s = Slots::from_css(Some("#f00"), Some("nonsense"), None);
        assert_eq!((s.subject, s.accent, s.badge), (Some(Color::from_rgba8(255, 0, 0, 255)), None, None));
        assert!(Slots::default().is_empty() && !s.is_empty());
    }

    #[test]
    fn rasterise_is_deterministic_coverage() {
        let a = rasterize(SQUARE, 16, Color::WHITE, &Slots::default()).unwrap();
        let b = rasterize(SQUARE, 16, Color::WHITE, &Slots::default()).unwrap();
        assert_eq!(a, b);
        assert_eq!((a.format, a.size), (ImageFormat::A8, DeviceSize { w: 16, h: 16 }));
        assert_eq!(a.pixels.len(), 256);
        assert!(a.pixels.iter().all(|&v| v == 255), "a full square covers every pixel");
        // a scaled master stays square and full
        let c = rasterize(SQUARE, 20, Color::WHITE, &Slots::default()).unwrap();
        assert_eq!(c.size, DeviceSize { w: 20, h: 20 });
        assert!(c.pixels.iter().all(|&v| v == 255));
    }

    #[test]
    fn rasterise_with_slots_is_premultiplied_rgba() {
        let slots = Slots { subject: Some(Color::from_rgba8(255, 0, 0, 255)), accent: None, badge: None };
        let img = rasterize(SLOTTED, 16, Color::from_rgba8(0, 0, 255, 255), &slots).unwrap();
        assert_eq!((img.format, img.size), (ImageFormat::Rgba8Premul, DeviceSize { w: 16, h: 16 }));
        let px = |x: usize, y: usize| &img.pixels[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
        assert_eq!(px(2, 8), [255, 0, 0, 255], "the subject in its slot colour");
        assert_eq!(px(12, 8), [0, 0, 255, 255], "the rest in `color`");
        // `color`'s alpha applies to the whole raster (premultiplied)
        let half = rasterize(SLOTTED, 16, Color::from_rgba8(0, 0, 255, 128), &slots).unwrap();
        let a = (128.0f32 / 255.0 * 255.0).round() as u8;
        assert_eq!(half.pixels[(8 * 16 + 2) * 4 + 3], a);
        assert!(rasterize(b"not svg", 16, Color::WHITE, &Slots::default()).is_err());
    }

    #[test]
    fn paints_a_tinted_mask_on_the_pixel_grid() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "file.save", Icon::svg(16, SQUARE).master(24, SQUARE)).unwrap();
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let red = Color::from_rgba8(255, 0, 0, 255);
        // 16pt at 1.5x, centred at (10.1, 10.1): 24 device pixels from round(15.15 - 12) = 3
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.5);
        let p = paint(&mut cx, &reg, "file.save", Rect::new(0.0, 0.0, 20.2, 20.2), &IconStyle::new(16.0, red)).unwrap();
        assert_eq!(p.choice.map(|c| (c.master, c.px)), Some((24, 24)));
        assert_eq!(p.rect, Rect::new(2.0, 2.0, 18.0, 18.0));
        let Prim::Icon(i) = &list.items[0] else { panic!("{:?}", list.items) };
        assert_eq!((i.rect, i.color), (p.rect, Some(red)));
        assert_eq!(res.images.len(), 1);
        let (id, data) = res.images.values().next().unwrap();
        assert_eq!((*id, data.format, data.size), (i.image, ImageFormat::A8, DeviceSize { w: 24, h: 24 }));
        // the same icon in another tint reuses the mask
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.5);
        paint(&mut cx, &reg, "file.save", Rect::new(0.0, 0.0, 20.2, 20.2), &IconStyle::new(16.0, Color::WHITE)).unwrap();
        assert_eq!(res.images.len(), 1);
    }

    #[test]
    fn paint_snaps_the_box_to_device_pixels() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "mcad.extrude", Icon::svg(16, SQUARE).master(20, SQUARE).master(24, SQUARE)).unwrap();
        let grey = Color::from_rgba8(0xCC, 0xCC, 0xCC, 255);
        // a rect at a fractional offset
        let rect = Rect::from_origin_size(Point::new(10.3, 7.7), Size::new(20.0, 20.0));
        for (scale, master) in [(1.0, 16), (1.25, 20), (1.5, 24)] {
            let mut list = DisplayList::new();
            let mut res = MemResources::default();
            let mut text = MonoShaper;
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, scale);
            let p = paint(&mut cx, &reg, "mcad.extrude", rect, &IconStyle::new(16.0, grey)).unwrap();
            let c = p.choice.unwrap();
            let on_grid = |v: f32| (v * scale - (v * scale).round()).abs() < 1e-3;
            assert!(on_grid(p.rect.min.x) && on_grid(p.rect.min.y), "Z1: the raster's origin is on a device pixel at {scale}: {:?}", p.rect);
            assert!((p.rect.width() * scale - c.px as f32).abs() < 1e-3, "painted 1:1 at {scale}");
            assert_eq!((c.master, p.tint), (master, Some(grey)));
            assert_eq!(res.images.values().next().unwrap().1.size, DeviceSize { w: c.px, h: c.px });
        }
    }

    #[test]
    fn slots_are_one_live_mask_per_colour() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "debug.warn", Icon::svg(16, SLOTTED)).unwrap();
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let mut style = IconStyle::new(16.0, Color::WHITE);
        let yellow = Color::from_rgba8(0xCC, 0xA7, 0, 255);
        style.slots.subject = Some(yellow);
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
        let p = paint(&mut cx, &reg, "debug.warn", Rect::new(0.0, 0.0, 16.0, 16.0), &style).unwrap();
        assert_eq!((p.tint, p.vector), (None, false));
        // `currentColor`'s mask, then the subject's: each tinted from its palette slot
        let tints: Vec<_> = list.items.iter().map(|i| if let Prim::Icon(i) = i { (i.color, i.slot.map(|s| s.index)) } else { panic!() }).collect();
        assert_eq!(tints, [(Some(Color::WHITE), Some(0)), (Some(yellow), Some(1))]);
        assert!(res.images.values().all(|(_, d)| d.format == ImageFormat::A8));
        let mask = |n: &str| res.images.iter().find(|(k, _)| k.ends_with(n)).map(|(_, (_, d))| d.pixels.clone()).unwrap();
        let (s0, s1) = (mask(";s0"), mask(";s1"));
        assert_eq!((s1[8 * 16 + 2], s1[8 * 16 + 12]), (255, 0), "the subject rect is the left half");
        assert_eq!((s0[8 * 16 + 2], s0[8 * 16 + 12]), (0, 255));
        // another colour set draws from the same masks
        style.slots.subject = Some(Color::from_rgba8(255, 0, 0, 255));
        style.color = Color::BLACK;
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
        paint(&mut cx, &reg, "debug.warn", Rect::new(0.0, 0.0, 16.0, 16.0), &style).unwrap();
        assert_eq!(res.images.len(), 2, "no new raster for new colours");
    }

    /// A stroked icon (24-unit art) with a classed accent ring.
    const STROKED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><circle class="accent" cx="12" cy="12" r="8" stroke-opacity="0.5"/></svg>"#;
    /// An icon the vector path cannot draw (a filter).
    const FILTERED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><filter id="b"><feGaussianBlur stdDeviation="1"/></filter><rect x="2" y="2" width="12" height="12" fill="currentColor" filter="url(#b)"/></svg>"#;

    #[test]
    fn vector_art_outlines_strokes_and_keeps_slots() {
        let art = VectorArt::parse(STROKED).unwrap();
        assert_eq!(art.size, Size::new(24.0, 24.0));
        let l: Vec<_> = art.layers.iter().map(|l| (l.kind, l.source, l.opacity)).collect();
        assert_eq!(l, [(LayerKind::Stroke, PaintSource::CurrentColor, 1.0), (LayerKind::Stroke, PaintSource::Slot(IconSlot::Accent), 0.5)]);
        // the line's outline: round caps reach a unit past both ends
        let b = art.layers[0].bounds;
        assert!((b.min.x - 4.0).abs() < 0.05 && (b.max.x - 20.0).abs() < 0.05 && (b.height() - 2.0).abs() < 0.05, "{b:?}");
        assert_eq!(VectorArt::parse(FILTERED).unwrap_err(), NotVector::Unsupported("filter".into()));
        assert!(matches!(VectorArt::parse(b"<svg").unwrap_err(), NotVector::Invalid(_)));
    }

    /// The prims an icon painted: `true` for a path, `false` for a raster.
    fn kinds(list: &DisplayList) -> Vec<bool> {
        list.items.iter().map(|p| matches!(p, Prim::Path(_))).collect()
    }

    #[test]
    fn vector_above_the_threshold_raster_below() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "reg-test.sizes", Icon::svg(16, STROKED).master(24, STROKED)).unwrap();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        // (points, scale) → physical pixels 16, 24, 48, 96, 200, and fractional scales
        for (pt, scale) in [(16.0, 1.0), (24.0, 1.0), (48.0, 1.0), (96.0, 1.0), (200.0, 1.0), (16.0, 1.25), (16.0, 1.5), (20.0, 1.25), (16.0, 2.0), (32.0, 1.5), (100.0, 1.25)] {
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, scale);
            let side = pt as f32;
            let p = paint(&mut cx, &reg, "reg-test.sizes", Rect::new(0.0, 0.0, side, side), &IconStyle::new(side, Color::WHITE)).unwrap();
            let px = p.choice.unwrap().px;
            assert_eq!(p.vector, px > VECTOR_THRESHOLD_PX, "{pt}pt at {scale}x is {px} px");
            assert_eq!(kinds(&list), if p.vector { vec![true, true] } else { vec![false] }, "{pt}pt at {scale}x");
            if p.vector {
                // the paths fill the box drawn: the art's 24 units over `px` device pixels
                let Prim::Path(pp) = &list.items[0] else { panic!() };
                assert!((pp.transform.a * scale - px as f32 / 24.0).abs() < 1e-4, "{:?}", pp.transform);
                assert_eq!(pp.slot.map(|s| s.index), Some(0));
            }
        }
        // the vector sizes all draw the same two paths
        assert_eq!(res.paths.len(), 2);
    }

    #[test]
    fn zoom_and_scale_changes_never_rasterise_a_vector_icon() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "reg-test.zoom", Icon::svg(24, STROKED)).unwrap();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        // 20 frames of a zoom the UI re-records (a zoom kind), then a scale change
        let frames = (0..20).map(|f| (32.0 + f as f32 * 8.0, 1.0)).chain([(32.0, 1.25), (32.0, 1.5), (32.0, 2.0)]);
        for (side, scale) in frames {
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, scale);
            assert!(paint(&mut cx, &reg, "reg-test.zoom", Rect::new(0.0, 0.0, side, side), &IconStyle::new(side, Color::WHITE)).unwrap().vector);
        }
        assert_eq!(res.images.len(), 0, "no raster");
        assert_eq!(res.paths.len(), 2, "the same two paths every frame");
    }

    #[test]
    fn a_theme_switch_over_100_slotted_icons_rasterises_nothing() {
        let reg = IconRegistry::new();
        for i in 0..100 {
            let svg = if i % 2 == 0 { SLOTTED } else { STROKED };
            reg.register(Owner::Host, &format!("reg-test.bar-{i}"), Icon::svg(16, svg).master(24, svg)).unwrap();
        }
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let theme = |color: Color, subject: Color, accent: Color| {
            let mut s = IconStyle::new(16.0, color);
            s.slots = Slots { subject: Some(subject), accent: Some(accent), badge: None };
            s
        };
        let dark = theme(Color::WHITE, Color::from_rgba8(0xCC, 0xA7, 0, 255), Color::from_rgba8(0x37, 0x94, 0xFF, 255));
        let light = theme(Color::BLACK, Color::from_rgba8(0xBF, 0x88, 0x03, 255), Color::from_rgba8(0x00, 0x5F, 0xB8, 255));
        // the toolbar at 1x (16 px: rasters) and at 2x (32 px: vectors)
        let mut draw = |res: &mut MemResources, style: &IconStyle, scale: f32| {
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, res, &mut text, scale);
            for i in 0..100 {
                paint(&mut cx, &reg, &format!("reg-test.bar-{i}"), Rect::new(i as f32 * 20.0, 0.0, i as f32 * 20.0 + 16.0, 16.0), style).unwrap();
            }
            list.items
                .iter()
                .map(|p| match p {
                    Prim::Icon(i) => i.color,
                    Prim::Path(p) => Some(p.paint.color()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let (d1, d2) = (draw(&mut res, &dark, 1.0), draw(&mut res, &dark, 2.0));
        let (images, paths) = (res.images.len(), res.paths.len());
        let (l1, l2) = (draw(&mut res, &light, 1.0), draw(&mut res, &light, 2.0));
        assert_eq!((res.images.len(), res.paths.len()), (images, paths), "a theme switch rasterises and uploads nothing");
        assert_eq!((d1.len(), d2.len()), (l1.len(), l2.len()));
        assert!(d1.iter().zip(&l1).all(|(a, b)| a != b) && d2.iter().zip(&l2).all(|(a, b)| a != b), "every colour changed");
        assert!(l1.contains(&Some(Color::BLACK)) && l2.contains(&Some(light.slots.accent.unwrap().with_alpha(0.5))));
        eprintln!("SKELTER-svg theme switch: 100 icons, {images} rasters and {paths} paths before and after");
    }

    #[test]
    fn a_multicolour_icon_keeps_its_colours_and_current_color_stays_live() {
        // a red half (a literal colour) and a `currentColor` half
        const TWO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="8" height="16" fill="#ff0000"/><rect x="8" width="8" height="16" fill="currentColor"/></svg>"##;
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "reg-test.two", Icon::multicolour(16, TWO)).unwrap();
        reg.register(Owner::Host, "reg-test.two-tinted", Icon::svg(16, TWO)).unwrap();
        let red = Color::from_rgba8(255, 0, 0, 255);
        let blue = Color::from_rgba8(0, 0, 255, 255);
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        for side in [16.0f32, 96.0] {
            let r = Rect::new(0.0, 0.0, side, side);
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
            let p = paint(&mut cx, &reg, "reg-test.two", r, &IconStyle::new(side, blue)).unwrap();
            assert_eq!((p.vector, p.tint), (side > 24.0, None));
            if p.vector {
                let paints: Vec<_> = list.items.iter().map(|i| if let Prim::Path(p) = i { (p.paint.color(), p.slot.map(|s| s.index)) } else { panic!() }).collect();
                assert_eq!(paints, [(red, None), (blue, Some(0))], "at {side}");
            } else {
                // the literal colours' RGBA raster, then `currentColor`'s mask tinted from slot 0
                let tints: Vec<_> = list.items.iter().map(|i| if let Prim::Icon(i) = i { (i.color, i.slot.map(|s| s.index)) } else { panic!() }).collect();
                assert_eq!(tints, [(None, None), (Some(blue), Some(0))]);
                let lit = res.images.iter().find(|(k, _)| k.contains("reg-test.two#") && k.ends_with(";lit")).map(|(_, (_, d))| d.clone()).unwrap();
                assert_eq!(lit.format, ImageFormat::Rgba8Premul);
                assert_eq!(&lit.pixels[(8 * 16 + 2) * 4..(8 * 16 + 2) * 4 + 4], &[255, 0, 0, 255], "red stays red");
                assert_eq!(lit.pixels[(8 * 16 + 12) * 4 + 3], 0, "the currentColor half is the mask's");
            }
            // `currentColor` follows `color` with no new raster
            let n = res.images.len();
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
            paint(&mut cx, &reg, "reg-test.two", r, &IconStyle::new(side, Color::WHITE)).unwrap();
            assert_eq!(res.images.len(), n);
            assert!(list.items.iter().any(|i| matches!(i, Prim::Icon(i) if i.color == Some(Color::WHITE)) || matches!(i, Prim::Path(p) if p.paint.color() == Color::WHITE)));
            // the same SVG registered tinted is monochrome
            let mut list = DisplayList::new();
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
            assert_eq!(paint(&mut cx, &reg, "reg-test.two-tinted", r, &IconStyle::new(side, blue)).unwrap().tint, Some(blue));
        }
    }

    #[test]
    fn re_registering_draws_a_new_raster() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "file.new", Icon::svg(16, SQUARE)).unwrap();
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let r = Rect::new(0.0, 0.0, 16.0, 16.0);
        paint(&mut PaintCx::new(&mut list, &mut res, &mut text, 1.0), &reg, "file.new", r, &IconStyle::new(16.0, Color::WHITE)).unwrap();
        reg.register(Owner::Host, "file.new", Icon::svg(16, SLOTTED)).unwrap();
        paint(&mut PaintCx::new(&mut list, &mut res, &mut text, 1.0), &reg, "file.new", r, &IconStyle::new(16.0, Color::WHITE)).unwrap();
        assert_eq!(res.images.len(), 2);
    }

    /// One test takes the diagnostics: the queue is the process's, and a second taker running in
    /// parallel would take the other's.
    #[test]
    fn refusals_missing_and_broken_icons_are_reported() {
        let reg = IconRegistry::new();
        assert!(matches!(reg.register(Owner::Host, "Bad", Icon::svg(16, SQUARE)), Err(IconError::BadKey { .. })));
        assert!(matches!(reg.register(Owner::Host, "ext.acme.gear", Icon::svg(16, SQUARE)), Err(IconError::Namespace { .. })));
        let empty = Icon::Svg { masters: BTreeMap::new(), hc: BTreeMap::new(), colors: IconColors::Tint };
        assert!(matches!(reg.register(Owner::Host, "reg-test.empty", empty), Err(IconError::BadIcon { .. })));
        assert!(matches!(reg.register(Owner::Host, "reg-test.zero", Icon::svg(0, SQUARE)), Err(IconError::BadIcon { .. })));
        assert!(reg.keys().is_empty());
        reg.register(Owner::Host, "reg-test.broken", Icon::svg(16, b"<svg".to_vec())).unwrap();
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let r = Rect::new(0.0, 0.0, 16.0, 16.0);
        let st = IconStyle::new(16.0, Color::WHITE);
        for _ in 0..2 {
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
            assert!(paint(&mut cx, &reg, "reg-test.nothing", r, &st).is_none());
            assert!(paint(&mut cx, &reg, "reg-test.broken", r, &st).is_none());
        }
        assert!(list.items.is_empty());
        // an icon with a filter draws from its raster above the threshold, reported once
        reg.register(Owner::Host, "reg-test.filtered", Icon::svg(16, FILTERED)).unwrap();
        let big = Rect::new(0.0, 0.0, 48.0, 48.0);
        for _ in 0..2 {
            let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
            let p = paint(&mut cx, &reg, "reg-test.filtered", big, &IconStyle::new(48.0, Color::WHITE)).unwrap();
            assert!(!p.vector && matches!(list.items.last(), Some(Prim::Icon(_))));
        }
        let d = take_diagnostics();
        assert!(d.iter().any(|e| matches!(e, IconError::BadKey { key, .. } if key == "Bad")));
        assert!(d.iter().any(|e| matches!(e, IconError::Namespace { key, .. } if key == "ext.acme.gear")));
        assert!(d.iter().any(|e| matches!(e, IconError::BadIcon { key, .. } if key == "reg-test.zero")));
        assert_eq!(d.iter().filter(|e| matches!(e, IconError::Missing { key } if key == "reg-test.nothing")).count(), 1, "once");
        assert_eq!(d.iter().filter(|e| matches!(e, IconError::Load { key, .. } if key == "reg-test.broken")).count(), 1, "once");
        assert_eq!(d.iter().filter(|e| matches!(e, IconError::Fallback { key, why } if key == "reg-test.filtered" && why == "filter")).count(), 1, "once, with why");
    }

    #[test]
    fn glyph_icons_need_a_registered_font() {
        let reg = IconRegistry::new();
        reg.register(Owner::Host, "reg-test.glyph", Icon::glyph("reg-test-font", "ab")).unwrap();
        let mut text = MonoShaper;
        assert_eq!(size(&mut text, &reg, "reg-test.glyph", 16.0), Some(Vec2::new(16.0, 16.0)), "unregistered font: a square");
        register_icon_font("reg-test-font");
        assert!(has_font("reg-test-font"));
        let adv = MonoShaper::advance(16.0);
        assert_eq!(size(&mut text, &reg, "reg-test.glyph", 16.0), Some(Vec2::new(2.0 * adv, 20.0)));
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
        let p = paint(&mut cx, &reg, "reg-test.glyph", Rect::new(0.0, 0.0, 40.0, 40.0), &IconStyle::new(16.0, Color::WHITE)).unwrap();
        assert_eq!(p.choice, None);
        assert!(matches!(list.items[0], Prim::Glyphs(_)));
        assert!(unregister_icon_font("reg-test-font"));
    }

    #[test]
    fn icon_font_names() {
        register_icon_names("reg-test-codicons", [("check".to_string(), "\u{eab2}".to_string())]);
        assert_eq!(icon_text("reg-test-codicons", "check", ""), Some("\u{eab2}".into()));
        assert_eq!(icon_text("reg-test-codicons", "nope", ""), None, "not in the table");
        assert_eq!(icon_text("reg-test-codicons", "nope", "x"), Some("x".into()), "the glyph");
        assert_eq!(icon_text("reg-test-ligatures", "home", ""), Some("home".into()), "no table: the name");
        unregister_icon_font("reg-test-codicons");
    }
}
