//! ICR1: the icon registry and how icons are drawn (plans/milestones/ICR1-icon-registry.md,
//! plans/research/ICON-STYLE.md §5–§6).
//!
//! An [`IconRegistry`] maps a **key** (`file.save`, `debug.step-over`, `mcad.extrude+remove`) to an
//! [`Icon`]: an SVG with one drawing per optical size (12, 14, 16, 20, 24, 32+), or a glyph of an
//! icon font (SVP6). Rust registers icons (the host or the app; [`IconRegistry::register_set`] takes
//! an IC1 `icons.py build` output through AS1's `Assets`); Svelte looks them up by key
//! (`<icon name="file.save">`, `content: icon(check)`, `::part(icon)`).
//!
//! Keys are lowercase dotted segments of kebab-case (`[a-z0-9]+(-[a-z0-9]+)*`), with an optional
//! `+<badge>` suffix for IC1's composed ids. The `ext.` prefix belongs to extensions: an extension
//! registers only under `ext.<its id>.`, and nothing else registers there. That is a namespace rule
//! with a register-time diagnostic, not a security check.
//!
//! Drawing ([`paint`]) follows ICON-STYLE: the box snaps to whole physical pixels (Z1), the master
//! is chosen by the physical size (Z2, [`choose_master`]), and the colours are:
//! - no slot set: the SVG is rasterised with `currentColor` white and tinted by `color` (a glyph
//!   tints the same way);
//! - a slot set (`--icon-subject`, `--icon-accent`, `--icon-badge`): it is rasterised with `color`
//!   as the root colour and the slots injected as a usvg style sheet on the classes `subject`,
//!   `accent` and `badge` (only `color` declarations), and painted untinted, so a warning yellow is
//!   not darkened by the text colour. One raster is cached per distinct colour set.
//!
//! usvg resolves `currentColor` at the element that *declares* the paint, so a classed group whose
//! stroke is inherited from the root would keep the root's colour. Before rasterising with slots,
//! [`slot_svg`] gives each classed element an explicit `fill`/`stroke="currentColor"` where it
//! inherits one, which makes its own (slot) `color` apply (ICON-STYLE §6's open question).

use egui::load::SizeHint;
use egui::{Color32, ColorImage, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, RwLock};
use svelters_modules::{AssetRef, Assets};

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
    /// `register_set`: the manifest could not be read.
    Set { what: String, why: String },
    /// Drawing: no icon is registered under the key (reported once per key).
    Missing { key: String },
    /// Drawing: the SVG did not load or rasterise (reported once per key and master).
    Load { key: String, why: String },
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::BadKey { key, why } => write!(f, "icon key {key:?}: {why}"),
            IconError::Namespace { key, owner, why } => write!(f, "icon key {key:?} refused for {owner:?}: {why}"),
            IconError::BadIcon { key, why } => write!(f, "icon {key:?}: {why}"),
            IconError::Set { what, why } => write!(f, "icon set {what}: {why}"),
            IconError::Missing { key } => write!(f, "no icon is registered under {key:?}"),
            IconError::Load { key, why } => write!(f, "icon {key:?} did not load: {why}"),
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
    /// An AS1 asset (an extension's or the app's file); re-read when its version changes.
    Asset(Arc<Assets>, AssetRef),
}

impl fmt::Debug for IconData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconData::Static(b) => write!(f, "Static({} bytes)", b.len()),
            IconData::Shared(b) => write!(f, "Shared({} bytes)", b.len()),
            IconData::Asset(_, r) => write!(f, "Asset({r})"),
        }
    }
}

impl IconData {
    fn version(&self) -> u64 {
        match self {
            IconData::Asset(a, r) => a.version(r),
            _ => 0,
        }
    }

    fn bytes(&self) -> Result<Arc<[u8]>, String> {
        match self {
            IconData::Static(b) => Ok(Arc::from(*b)),
            IconData::Shared(b) => Ok(b.clone()),
            IconData::Asset(a, r) => a.bytes(r).map(|b| Arc::from(&*b)).map_err(|e| e.to_string()),
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
    Svg { masters: BTreeMap<u32, IconData>, hc: BTreeMap<u32, IconData> },
    /// A glyph of a registered icon font (`egui::FontFamily::Name(family)`), sized by the box.
    Glyph { family: String, text: String },
}

impl Icon {
    /// An SVG icon with its `size` master.
    pub fn svg(size: u32, data: impl Into<IconData>) -> Icon {
        Icon::Svg { masters: BTreeMap::from([(size, data.into())]), hc: BTreeMap::new() }
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

/// Keys to icons. One is global ([`IconRegistry::global`]); a context can have its own
/// ([`set_registry`]).
#[derive(Default)]
pub struct IconRegistry {
    entries: RwLock<HashMap<String, Entry>>,
    generation: AtomicU64,
}

static GLOBAL: LazyLock<Arc<IconRegistry>> = LazyLock::new(|| Arc::new(IconRegistry::default()));

impl IconRegistry {
    pub fn new() -> Arc<IconRegistry> {
        Arc::new(IconRegistry::default())
    }

    /// The process's registry: what every context uses unless [`set_registry`] gave it another.
    pub fn global() -> Arc<IconRegistry> {
        GLOBAL.clone()
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

    /// The master that draws `key` in a box of `box_pt` points at `ppp` pixels per point
    /// (P = round(box_pt × ppp), rule Z2). `None` for an unknown key or a glyph icon.
    pub fn resolve(&self, key: &str, box_pt: f32, ppp: f32) -> Option<Choice> {
        let (icon, _) = self.entry(key)?;
        choose_master(&icon.sizes(), physical(box_pt, ppp))
    }

    /// Registers an IC1 build (`tools/icons/icons.py build <set> <out>`) that `assets` serves at
    /// `asset://<root>/<dir>/`: `manifest.json` lists the ids and their sizes, the masters are
    /// `<size>/<id>.svg` and the High Contrast copies `hc/<size>/<id>.svg` (when present). Each id
    /// is registered as for [`register`](Self::register) (an extension's set must use
    /// `ext.<id>.` ids); returns how many registered, refusals are reported.
    pub fn register_set(&self, owner: Owner, assets: &Arc<Assets>, root: &str, dir: &str) -> Result<usize, IconError> {
        let at = |p: &str| -> Result<AssetRef, IconError> {
            let path = if dir.is_empty() { p.to_string() } else { format!("{}/{p}", dir.trim_end_matches('/')) };
            AssetRef::new(root, &path).map_err(|e| IconError::Set { what: format!("{root}/{dir}"), why: e.to_string() })
        };
        let set_err = |why: String| {
            let e = IconError::Set { what: format!("asset://{root}/{dir}"), why };
            report(e.clone());
            e
        };
        let manifest = at("manifest.json")?;
        let bytes = assets.bytes(&manifest).map_err(|e| set_err(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| set_err(format!("manifest.json: {e}")))?;
        let icons = json.get("icons").and_then(|v| v.as_object()).ok_or_else(|| set_err("manifest.json has no `icons` object".into()))?;
        let mut n = 0;
        for (id, info) in icons {
            let sizes: Vec<u32> = info.get("sizes").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect()).unwrap_or_default();
            let mut icon = Icon::Svg { masters: BTreeMap::new(), hc: BTreeMap::new() };
            for s in sizes {
                let Ok(r) = at(&format!("{s}/{id}.svg")) else { continue };
                icon = icon.master(s, IconData::Asset(assets.clone(), r));
                if let Ok(h) = at(&format!("hc/{s}/{id}.svg"))
                    && assets.bytes(&h).is_ok()
                {
                    icon = icon.hc_master(s, IconData::Asset(assets.clone(), h));
                }
            }
            if self.register(owner.clone(), id, icon).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }
}

/// `round(pt × ppp)`, at least 1.
fn physical(pt: f32, ppp: f32) -> u32 {
    (pt * ppp).round().max(1.0) as u32
}

/// The box side of an `icon` with no `size`: 16 points, the default chrome size (ICON-STYLE §5.1).
pub const DEFAULT_SIZE: f32 = 16.0;

/// Whether an `icon`'s `family`/`name` draws from `ctx`'s registry: the default family (`icons`)
/// and a registered key.
pub fn is_registry_icon(ctx: &Context, family: &str, name: &str) -> bool {
    family == crate::paint::IconKey::REGISTRY_FAMILY && !name.is_empty() && registry(ctx).contains(name)
}

fn registry_id() -> Id {
    Id::unique("svelters-egui::icons::registry")
}

/// Gives `ctx` its own registry instead of the global one (tests, an embed that keeps its icons
/// apart).
pub fn set_registry(ctx: &Context, registry: Arc<IconRegistry>) {
    ctx.data_mut(|d| d.insert_temp(registry_id(), registry));
}

/// The registry `ctx` draws from: its own ([`set_registry`]) or the global one.
pub fn registry(ctx: &Context) -> Arc<IconRegistry> {
    ctx.data(|d| d.get_temp::<Arc<IconRegistry>>(registry_id())).unwrap_or_else(IconRegistry::global)
}

// ---- colours ----

/// The named colour slots (ICON-STYLE §6); `None` keeps the part on `currentColor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Slots {
    pub subject: Option<Color32>,
    pub accent: Option<Color32>,
    pub badge: Option<Color32>,
}

impl Slots {
    pub fn is_empty(&self) -> bool {
        self.subject.is_none() && self.accent.is_none() && self.badge.is_none()
    }

    /// From CSS custom property values (`--icon-subject` etc., as computed text); a value that is
    /// not a colour is ignored.
    pub fn from_css(subject: Option<&str>, accent: Option<&str>, badge: Option<&str>) -> Slots {
        Slots { subject: subject.and_then(parse_css_color), accent: accent.and_then(parse_css_color), badge: badge.and_then(parse_css_color) }
    }
}

/// A CSS colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, a named colour) as svgtypes
/// reads it.
pub fn parse_css_color(s: &str) -> Option<Color32> {
    svgtypes_color::parse(s.trim())
}

/// CSS colours through usvg's own parser (svgtypes, no direct dependency): a one-pixel SVG with
/// the colour as `color`, read back.
mod svgtypes_color {
    pub struct Color(pub egui::Color32);

    impl std::str::FromStr for Color {
        type Err = ();
        fn from_str(s: &str) -> Result<Color, ()> {
            if s.is_empty() || s.contains(['"', '<', '>', ';', '{', '}']) {
                return Err(());
            }
            let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" color="{s}"><rect width="1" height="1" fill="currentColor"/></svg>"#);
            let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).map_err(|_| ())?;
            let resvg::usvg::Node::Path(p) = tree.root().children().first().ok_or(())? else { return Err(()) };
            let f = p.fill().ok_or(())?;
            let resvg::usvg::Paint::Color(c) = f.paint() else { return Err(()) };
            // an unparsable `color` falls back to black in usvg: accept black only when asked for
            let a = (f.opacity().get() * 255.0).round() as u8;
            let out = egui::Color32::from_rgba_unmultiplied(c.red, c.green, c.blue, a);
            if out == egui::Color32::BLACK && !is_black(s) {
                return Err(());
            }
            Ok(Color(out))
        }
    }

    fn is_black(s: &str) -> bool {
        let s = s.to_ascii_lowercase().replace(' ', "");
        matches!(s.as_str(), "black" | "#000" | "#000000" | "#000f" | "#000000ff" | "rgb(0,0,0)" | "rgba(0,0,0,1)")
    }

    pub(super) fn parse(s: &str) -> Option<egui::Color32> {
        s.parse::<Color>().ok().map(|c| c.0)
    }
}

/// How an icon is drawn: its box side in points, the tint (`color`), the slots and the theme's
/// contrast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconStyle {
    pub size: f32,
    pub color: Color32,
    pub slots: Slots,
    /// Use the High Contrast masters when the icon has them.
    pub high_contrast: bool,
}

impl IconStyle {
    pub fn new(size: f32, color: Color32) -> IconStyle {
        IconStyle { size, color, slots: Slots::default(), high_contrast: false }
    }
}

fn css_rgba(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("rgba({r},{g},{b},{:.3})", a as f32 / 255.0)
}

/// The usvg style sheet for `color` and `slots`: `svg{color:…}` plus one `color` declaration per
/// set slot on its class. Nothing but `color` ever reaches an icon (ICON-STYLE §6, L3/L4).
pub fn slot_style_sheet(color: Color32, slots: &Slots) -> String {
    let mut s = format!("svg{{color:{}}}", css_rgba(color.to_opaque()));
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

/// Rasterises one master at `px` square pixels: white `currentColor` (to be tinted) when `slots`
/// is empty, else `color` and the slots, untinted.
pub fn rasterize(svg: &[u8], px: u32, color: Color32, slots: &Slots) -> Result<ColorImage, String> {
    let hint = SizeHint::Size { width: px.max(1), height: px.max(1), maintain_aspect_ratio: true };
    if slots.is_empty() {
        crate::assets::rasterize_styled(svg, hint, crate::assets::SVG_STYLE)
    } else {
        crate::assets::rasterize_styled(&slot_svg(svg)?, hint, &slot_style_sheet(color, slots))
    }
}

// ---- drawing ----

/// (key, generation, master, data version, hc, px, colours when slots are set)
type CacheKey = (String, u64, u32, u64, bool, u32, Option<[Option<Color32>; 4]>);

#[derive(Default)]
struct Cache {
    /// Each raster with the use counter at its last draw.
    textures: Mutex<HashMap<CacheKey, (TextureHandle, u64)>>,
    uses: AtomicU64,
}

/// The rasters kept per key (zoom levels × colour sets × hc); the least recently drawn beyond this
/// are dropped, so the rasters of an earlier zoom level go once the new ones are drawn.
pub const RASTERS_PER_KEY: usize = 6;

impl Cache {
    fn get(&self, ck: &CacheKey) -> Option<TextureHandle> {
        let n = self.uses.fetch_add(1, Ordering::Relaxed);
        lock(&self.textures).get_mut(ck).map(|(t, used)| {
            *used = n;
            t.clone()
        })
    }

    fn insert(&self, ck: CacheKey, t: TextureHandle) {
        let n = self.uses.fetch_add(1, Ordering::Relaxed);
        let mut cache = lock(&self.textures);
        // an older generation or file version of the same key is dead
        cache.retain(|k, _| !(k.0 == ck.0 && (k.1 != ck.1 || k.3 != ck.3)));
        let mut same: Vec<(u64, CacheKey)> = cache.iter().filter(|(k, _)| k.0 == ck.0).map(|(k, (_, u))| (*u, k.clone())).collect();
        if same.len() >= RASTERS_PER_KEY {
            same.sort_by_key(|(u, _)| *u);
            for (_, k) in same.iter().take(same.len() + 1 - RASTERS_PER_KEY) {
                cache.remove(k);
            }
        }
        cache.insert(ck, (t, n));
    }

    fn len_of(&self, key: &str) -> usize {
        lock(&self.textures).keys().filter(|k| k.0 == key).count()
    }
}

fn cache(ctx: &Context) -> Arc<Cache> {
    let id = Id::unique("svelters-egui::icons::cache");
    if let Some(c) = ctx.data(|d| d.get_temp::<Arc<Cache>>(id)) {
        return c;
    }
    let c = Arc::new(Cache::default());
    ctx.data_mut(|d| d.insert_temp(id, c.clone()));
    c
}

/// How many rasters of `key` `ctx` keeps (tests: eviction by last use, [`RASTERS_PER_KEY`]).
pub fn cached_rasters(ctx: &Context, key: &str) -> usize {
    cache(ctx).len_of(key)
}

/// What [`paint`] drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Painted {
    /// The drawn rect in points (on the physical pixel grid).
    pub rect: Rect,
    /// The master and fit, for an SVG icon.
    pub choice: Option<Choice>,
    /// The tint the texture or glyph was painted with (white when slots were set).
    pub tint: Color32,
}

/// The size of `key`'s box at `style.size` points: a square for SVG icons, the glyph's size for a
/// glyph icon. `None` for an unknown key.
pub fn size(ctx: &Context, registry: &IconRegistry, key: &str, size_pt: f32) -> Option<Vec2> {
    match registry.get(key)? {
        Icon::Svg { .. } => Some(Vec2::splat(size_pt)),
        Icon::Glyph { family, text } => {
            let fam = egui::FontFamily::Name(family.as_str().into());
            if !crate::fonts::has_family(ctx, &fam) {
                return Some(Vec2::splat(size_pt));
            }
            Some(ctx.fonts_mut(|f| f.layout_no_wrap(text, egui::FontId::new(size_pt, fam), Color32::WHITE).size()))
        }
    }
}

/// Paints `key` from `registry` centred in `rect` (rules Z1 and Z2). `None` when the key is not
/// registered (reported once) or the icon cannot be drawn (reported once; nothing is painted).
pub fn paint(painter: &Painter, registry: &IconRegistry, key: &str, rect: Rect, style: &IconStyle) -> Option<Painted> {
    let Some((icon, generation)) = registry.entry(key) else {
        report(IconError::Missing { key: key.to_string() });
        return None;
    };
    let ctx = painter.ctx();
    let ppp = painter.pixels_per_point();
    match icon {
        Icon::Glyph { family, text } => {
            let fam = egui::FontFamily::Name(family.as_str().into());
            if !crate::fonts::has_family(ctx, &fam) {
                report(IconError::Load { key: key.to_string(), why: format!("font family {family:?} is not bound") });
                return None;
            }
            let g = ctx.fonts_mut(|f| f.layout_no_wrap(text, egui::FontId::new(style.size, fam), style.color));
            let pos = snap_pos(rect.center() - g.size() / 2.0, ppp);
            let r = Rect::from_min_size(pos, g.size());
            painter.galley(pos, g, style.color);
            Some(Painted { rect: r, choice: None, tint: style.color })
        }
        Icon::Svg { masters, hc } => {
            // Z1: the box on whole physical pixels, centred in `rect`
            let p = physical(style.size, ppp);
            let origin = ((rect.center().to_vec2() * ppp) - Vec2::splat(p as f32 / 2.0)).round();
            let sizes: Vec<u32> = masters.keys().copied().collect();
            let choice = choose_master(&sizes, p)?;
            let use_hc = style.high_contrast && hc.contains_key(&choice.master);
            let data = if use_hc { &hc[&choice.master] } else { &masters[&choice.master] };
            let colours = (!style.slots.is_empty()).then_some([Some(style.color.to_opaque()), style.slots.subject, style.slots.accent, style.slots.badge]);
            let ck: CacheKey = (key.to_string(), generation, choice.master, data.version(), use_hc, choice.px, colours);
            let c = cache(ctx);
            let tex = c.get(&ck);
            let tex = match tex {
                Some(t) => t,
                None => {
                    let img = data.bytes().and_then(|b| rasterize(&b, choice.px, style.color, &style.slots));
                    let img = match img {
                        Ok(i) => i,
                        Err(why) => {
                            report(IconError::Load { key: key.to_string(), why });
                            return None;
                        }
                    };
                    let t = ctx.load_texture(format!("icon:{key}@{}", choice.px), img, TextureOptions::LINEAR);
                    c.insert(ck, t.clone());
                    t
                }
            };
            let [tw, th] = tex.size();
            let min = (origin + Vec2::splat(choice.offset as f32)) / ppp;
            let r = Rect::from_min_size(Pos2::new(min.x, min.y), Vec2::new(tw as f32, th as f32) / ppp);
            let tint = if style.slots.is_empty() { style.color } else { Color32::from_white_alpha(style.color.a()) };
            painter.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
            Some(Painted { rect: r, choice: Some(choice), tint })
        }
    }
}

fn snap_pos(p: Pos2, ppp: f32) -> Pos2 {
    Pos2::new((p.x * ppp).round() / ppp, (p.y * ppp).round() / ppp)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn css_colours() {
        assert_eq!(parse_css_color("#CCA700"), Some(Color32::from_rgb(0xCC, 0xA7, 0x00)));
        assert_eq!(parse_css_color("rgb(255, 0, 0)"), Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(parse_css_color("black"), Some(Color32::BLACK));
        assert_eq!(parse_css_color("nonsense"), None);
        assert_eq!(parse_css_color("red;}x{"), None);
    }
}
