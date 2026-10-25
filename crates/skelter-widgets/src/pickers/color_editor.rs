//! `color-editor` (svelters-egui `widgets/color_editor.rs`; WIDGET-PARITY §2.20, the body of Qt's
//! `QColorDialog`) and the body `color-picker` shows in its popup ([`ColorBody`]).
//!
//! Layout, top to bottom: the saturation/value square, the hue strip, the alpha strip over a
//! checkerboard (unless `alpha` is `opaque`), each with a handle; the fields row (the hex field and,
//! per `fields`, R/G/B and H/S/V channel boxes); the palette (`swatches`) and the user's
//! `custom-colors` with an add cell.
//!
//! HSV is in sRGB; the body keeps its HSV while the user edits, so the hue of a grey survives a
//! round trip through bytes. Square, strips and channel boxes are tab stops: arrows step them
//! (Shift ×10), PageUp/PageDown by 10%, Home/End to the ends; dragging a channel box scrubs it
//! (2 points per unit). A drag reports [`ColorOut::Input`] per move and [`ColorOut::Commit`] at the
//! release; a key step, a swatch and a hex write are both at once.
//!
//! The hex field's typing needs TEXT-INPUT's single-line field; until then the body shows the hex
//! and takes whole-value writes through [`ColorBody::set_hex`] (the field's commit and AccessKit
//! SetValue both end there).

use super::color::{Hsv, HUE_MAX, parse_hex, rgba_color, to_hex};
use super::{PickerLook, Ptr};
use crate::table::shim::{self, Emit, HAlign, TextPainter, Value};
use skelter_base::color::{Color, Fill, GradientStop};
use skelter_base::display::{BorderStyle, DisplayList, Prim, Quad};
use skelter_base::event::{Modifiers, PointerEvent};
use skelter_base::geom::{Corners, Insets, Point, Rect, Size};
use skelter_base::key::Key;
use skelter_base::smallvec::SmallVec;

/// `fields` tokens.
pub const FIELDS: &[&str] = &["hex", "rgb", "hsv"];
/// `commit-mode` values of the colour kinds.
pub const COLOR_COMMIT_MODE: &[&str] = &["live", "confirm"];
/// At most this many custom colours (the add cell hides after).
pub const MAX_CUSTOM: usize = 16;

/// The parts' sizes (svelters-egui `SQUARE_UA`, `STRIP_UA`, `HANDLE_UA`, `CELL_UA`; fields are
/// `control-h` high).
pub const SQUARE: Size = Size::new(200.0, 140.0);
pub const STRIP_H: f32 = 12.0;
pub const HANDLE_W: f32 = 10.0;
pub const HANDLE_W_DRAGGING: f32 = 12.0;
pub const CELL: Size = Size::new(16.0, 16.0);
pub const CELL_GAP: f32 = 4.0;
pub const ROW_GAP: f32 = 6.0;
/// The checkerboard's square under alpha.
pub const CHECKER: f32 = 6.0;

/// A row of colour cells: the colours, their rects and the hit each cell reports.
type CellList<'a> = (&'a [String], &'a [Rect], fn(usize) -> Hit);

/// What the body is configured with (the props `color-editor` and `color-picker` share).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorCfg {
    /// The alpha strip shows (`alpha` other than `opaque`).
    pub alpha: bool,
    pub hex: bool,
    pub rgb: bool,
    pub hsv: bool,
    pub swatches: Vec<String>,
    pub custom: Vec<String>,
}

impl Default for ColorCfg {
    fn default() -> Self {
        ColorCfg { alpha: false, hex: true, rgb: true, hsv: false, swatches: Vec::new(), custom: Vec::new() }
    }
}

impl ColorCfg {
    /// `fields` as (hex, rgb, hsv).
    pub fn fields_of(v: &str) -> (bool, bool, bool) {
        let has = |t: &str| v.split([' ', ',']).any(|x| x == t);
        (has("hex"), has("rgb"), has("hsv"))
    }

    /// Whether the `alpha` prop's value shows the strip.
    pub fn alpha_of(v: &str) -> bool {
        matches!(v, "blend" | "blend-or-additive")
    }
}

/// A sub-control (a tab stop, or a drag target).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctl {
    Square,
    Hue,
    Alpha,
    /// R, G, B, H, S, V (0..6).
    Chan(u8),
    Hex,
}

/// The channel boxes: short label, accessible name, maximum.
pub const CHANNELS: [(&str, &str, f32); 6] = [("R", "Red", 255.0), ("G", "Green", 255.0), ("B", "Blue", 255.0), ("H", "Hue", 360.0), ("S", "Saturation", 100.0), ("V", "Value", 100.0)];

/// What the body did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorOut {
    /// The colour changed (a drag move, a step, a field, a swatch).
    Input(Hsv),
    /// A commit point: a drag released, a key step, a field set, a swatch picked.
    Commit,
    /// The add cell: append the current colour to `custom-colors`.
    AddCustom,
}

/// The rects of one layout (body-local points).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorGeom {
    pub square: Rect,
    pub hue: Rect,
    pub alpha: Option<Rect>,
    pub hex: Option<Rect>,
    pub chans: Vec<(u8, Rect)>,
    pub cells: Vec<Rect>,
    pub custom_cells: Vec<Rect>,
    /// The add cell; `None` once [`MAX_CUSTOM`] colours are stored.
    pub add: Option<Rect>,
}

impl ColorGeom {
    /// Lays the body out at `origin` in `width` (at least the square's width).
    pub fn new(look: &PickerLook, cfg: &ColorCfg, origin: Point, width: f32) -> ColorGeom {
        let w = width.max(SQUARE.w);
        let field = look.control_h;
        let mut y = origin.y;
        let mut take = |h: f32| {
            let r = Rect::new(origin.x, y, origin.x + w, y + h);
            y += h + ROW_GAP;
            r
        };
        let square = take(SQUARE.h);
        let hue = take(STRIP_H);
        let alpha = cfg.alpha.then(|| take(STRIP_H));
        let rows: Vec<[u8; 3]> = [cfg.rgb.then_some([0u8, 1, 2]), cfg.hsv.then_some([3u8, 4, 5])].into_iter().flatten().collect();
        let mut hex = None;
        let mut chans = Vec::new();
        if cfg.hex || !rows.is_empty() {
            let hex_w = if cfg.hex { (w * 0.36).max(64.0) } else { 0.0 };
            let lead = if cfg.hex { hex_w + CELL_GAP } else { 0.0 };
            let first = take(field);
            if cfg.hex {
                hex = Some(Rect::from_origin_size(first.min, Size::new(if rows.is_empty() { w } else { hex_w }, field)));
            }
            for (k, row) in rows.iter().enumerate() {
                let r = if k == 0 { first } else { take(field) };
                let x0 = origin.x + lead;
                let cw = (r.max.x - x0 - 2.0 * CELL_GAP) / 3.0;
                for (i, c) in row.iter().enumerate() {
                    let x = x0 + i as f32 * (cw + CELL_GAP);
                    chans.push((*c, Rect::new(x, r.min.y, x + cw, r.max.y)));
                }
            }
        }
        let per = (((w + CELL_GAP) / (CELL.w + CELL_GAP)).floor() as usize).max(1);
        let grid = |n: usize, y0: f32| -> Vec<Rect> {
            (0..n).map(|k| Rect::from_origin_size(Point::new(origin.x + (k % per) as f32 * (CELL.w + CELL_GAP), y0 + (k / per) as f32 * (CELL.h + CELL_GAP)), CELL)).collect()
        };
        let cells = grid(cfg.swatches.len(), y);
        let y2 = if cfg.swatches.is_empty() { y } else { y + cfg.swatches.len().div_ceil(per) as f32 * (CELL.h + CELL_GAP) - CELL_GAP + ROW_GAP };
        let with_add = cfg.custom.len() < MAX_CUSTOM;
        let mut custom_cells = grid(cfg.custom.len() + with_add as usize, y2);
        let add = if with_add { custom_cells.pop() } else { None };
        ColorGeom { square, hue, alpha, hex, chans, cells, custom_cells, add }
    }

    /// The body's size (content box).
    pub fn size(&self, origin: Point) -> Size {
        let bottom = [self.add.map(|r| r.max.y), self.custom_cells.last().map(|r| r.max.y), self.cells.last().map(|r| r.max.y), self.hex.map(|r| r.max.y), self.chans.last().map(|c| c.1.max.y), self.alpha.map(|r| r.max.y), Some(self.hue.max.y)]
            .into_iter()
            .flatten()
            .fold(0.0f32, f32::max);
        Size::new(self.square.width(), bottom - origin.y)
    }

    fn hit(&self, p: Point) -> Option<Hit> {
        if self.square.contains(p) {
            return Some(Hit::Ctl(Ctl::Square));
        }
        if self.hue.contains(p) {
            return Some(Hit::Ctl(Ctl::Hue));
        }
        if self.alpha.is_some_and(|r| r.contains(p)) {
            return Some(Hit::Ctl(Ctl::Alpha));
        }
        if self.hex.is_some_and(|r| r.contains(p)) {
            return Some(Hit::Ctl(Ctl::Hex));
        }
        if let Some((c, _)) = self.chans.iter().find(|(_, r)| r.contains(p)) {
            return Some(Hit::Ctl(Ctl::Chan(*c)));
        }
        if let Some(k) = self.cells.iter().position(|r| r.contains(p)) {
            return Some(Hit::Swatch(k));
        }
        if let Some(k) = self.custom_cells.iter().position(|r| r.contains(p)) {
            return Some(Hit::Custom(k));
        }
        self.add.filter(|r| r.contains(p)).map(|_| Hit::Add)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Ctl(Ctl),
    Swatch(usize),
    Custom(usize),
    Add,
}

/// The body's state between events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorBody {
    /// The colour being edited; none: from the value.
    hsv: Option<Hsv>,
    /// The value `hsv` was last synced with (a host write resets it).
    synced: Option<[u8; 4]>,
    dragging: Option<Ctl>,
    /// A channel scrub's start: (pointer x, channel value).
    scrub_from: Option<(f32, f32)>,
    hovered: Option<Hit>,
    pressed: Option<Hit>,
    /// The tab stop holding focus inside the body (the host's focus model sets it).
    pub focus: Option<Ctl>,
    /// The last hex write was not a colour (`:invalid` until the next edit).
    pub hex_invalid: bool,
}

/// A channel's value of `h`: R/G/B 0..255, H 0..360, S/V 0..100.
pub fn channel(h: Hsv, c: u8) -> f32 {
    let [r, g, b, _] = h.to_srgba();
    match c {
        0 => r as f32,
        1 => g as f32,
        2 => b as f32,
        3 => (h.h * 360.0).round(),
        4 => (h.s * 100.0).round(),
        _ => (h.v * 100.0).round(),
    }
}

/// `h` with channel `c` set to `v` (clamped); an R/G/B write keeps a grey's hue.
pub fn with_channel(h: Hsv, c: u8, v: f32) -> Hsv {
    let v = v.clamp(0.0, CHANNELS[c.min(5) as usize].2);
    match c {
        0..=2 => {
            let mut rgba = h.to_srgba();
            rgba[c as usize] = v.round() as u8;
            let mut n = Hsv::from_srgba(rgba);
            if n.s == 0.0 {
                n.h = h.h;
            }
            n
        }
        3 => Hsv { h: (v / 360.0).min(HUE_MAX), ..h },
        4 => Hsv { s: v / 100.0, ..h },
        _ => Hsv { v: v / 100.0, ..h },
    }
}

impl ColorBody {
    /// The colour shown: the edit, else the value. A changed value (a host write, the first use)
    /// wins, keeping the edit's hue for a grey and its saturation for black.
    pub fn hsv(&mut self, value: [u8; 4]) -> Hsv {
        if self.synced != Some(value) {
            let mut h = Hsv::from_srgba(value);
            if let Some(old) = self.hsv
                && (h.s == 0.0 || h.v == 0.0)
            {
                h.h = old.h;
                if h.v == 0.0 {
                    h.s = old.s;
                }
            }
            self.hsv = Some(h);
            self.synced = Some(value);
        }
        self.hsv.unwrap_or_else(|| Hsv::from_srgba(value))
    }

    /// The colour shown, without syncing (for recording; call [`ColorBody::hsv`] first).
    pub fn shown(&self, value: [u8; 4]) -> Hsv {
        match (self.hsv, self.synced) {
            (Some(h), Some(s)) if s == value => h,
            _ => Hsv::from_srgba(value),
        }
    }

    /// Records that the owner wrote the body's edit `h` as the new value.
    pub fn edited(&mut self, h: Hsv) {
        self.hsv = Some(h);
        self.synced = Some(h.to_srgba());
    }

    pub fn dragging(&self) -> Option<Ctl> {
        self.dragging
    }

    /// The tab stops in order.
    pub fn stops(g: &ColorGeom) -> Vec<Ctl> {
        let mut v = vec![Ctl::Square, Ctl::Hue];
        if g.alpha.is_some() {
            v.push(Ctl::Alpha);
        }
        if g.hex.is_some() {
            v.push(Ctl::Hex);
        }
        v.extend(g.chans.iter().map(|c| Ctl::Chan(c.0)));
        v
    }

    /// The keys a focused sub-control takes.
    pub fn claims(&self, key: Key) -> bool {
        self.focus.is_some_and(|c| c != Ctl::Hex) && matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown)
    }

    fn from_point(h: &mut Hsv, ctl: Ctl, r: Rect, p: Point) {
        let fx = ((p.x - r.min.x) / r.width()).clamp(0.0, 1.0);
        let fy = ((p.y - r.min.y) / r.height()).clamp(0.0, 1.0);
        match ctl {
            Ctl::Square => {
                h.s = fx;
                h.v = 1.0 - fy;
            }
            Ctl::Hue => h.h = fx.min(HUE_MAX),
            Ctl::Alpha => h.a = fx,
            Ctl::Chan(_) | Ctl::Hex => {}
        }
    }

    fn ctl_rect(g: &ColorGeom, ctl: Ctl) -> Option<Rect> {
        match ctl {
            Ctl::Square => Some(g.square),
            Ctl::Hue => Some(g.hue),
            Ctl::Alpha => g.alpha,
            Ctl::Hex => g.hex,
            Ctl::Chan(c) => g.chans.iter().find(|x| x.0 == c).map(|x| x.1),
        }
    }

    /// A pointer event in body-local points laid out as `g`. `shift`: channel scrubs step by 10.
    pub fn pointer(&mut self, ev: &PointerEvent, g: &ColorGeom, cfg: &ColorCfg, value: [u8; 4], enabled: bool) -> Vec<ColorOut> {
        let mut out = Vec::new();
        let Some(p) = Ptr::of(ev) else { return out };
        let mut h = self.hsv(value);
        let before = h;
        let mul = if ev.modifiers.contains(Modifiers::SHIFT) { 10.0 } else { 1.0 };
        match p {
            Ptr::Move(p) => {
                self.hovered = g.hit(p);
                match self.dragging {
                    Some(Ctl::Chan(c)) => {
                        if let Some((x0, v0)) = self.scrub_from {
                            h = with_channel(h, c, v0 + ((p.x - x0) / 2.0).round() * mul);
                        }
                    }
                    Some(ctl) => {
                        if let Some(r) = ColorBody::ctl_rect(g, ctl) {
                            ColorBody::from_point(&mut h, ctl, r, p);
                        }
                    }
                    None => {}
                }
            }
            Ptr::Leave => self.hovered = None,
            Ptr::Down(p) if enabled => {
                let hit = g.hit(p);
                self.pressed = hit;
                if let Some(Hit::Ctl(ctl)) = hit {
                    self.focus = Some(ctl);
                    match ctl {
                        Ctl::Square | Ctl::Hue | Ctl::Alpha => {
                            self.dragging = Some(ctl);
                            if let Some(r) = ColorBody::ctl_rect(g, ctl) {
                                ColorBody::from_point(&mut h, ctl, r, p);
                            }
                        }
                        Ctl::Chan(c) => {
                            self.dragging = Some(ctl);
                            self.scrub_from = Some((p.x, channel(h, c)));
                        }
                        Ctl::Hex => {}
                    }
                }
            }
            Ptr::Down(_) => {}
            Ptr::Up(p) => {
                let pressed = self.pressed.take();
                if self.dragging.take().is_some() {
                    self.scrub_from = None;
                    if h != before {
                        out.push(ColorOut::Input(h));
                    }
                    out.push(ColorOut::Commit);
                    self.hsv = Some(h);
                    return out;
                }
                if !enabled || pressed != g.hit(p) {
                    return out;
                }
                let pick = |list: &[String], k: usize| parse_hex(list.get(k)?).ok();
                let picked = match pressed {
                    Some(Hit::Swatch(k)) => pick(&cfg.swatches, k),
                    Some(Hit::Custom(k)) => pick(&cfg.custom, k),
                    Some(Hit::Add) => {
                        out.push(ColorOut::AddCustom);
                        None
                    }
                    _ => None,
                };
                if let Some(mut rgba) = picked {
                    if !cfg.alpha {
                        rgba[3] = 255;
                    }
                    h = Hsv::from_srgba(rgba);
                    self.hsv = Some(h);
                    out.push(ColorOut::Input(h));
                    out.push(ColorOut::Commit);
                }
                return out;
            }
        }
        if h != before {
            self.hsv = Some(h);
            out.push(ColorOut::Input(h));
        }
        out
    }

    /// A key on the focused sub-control: a step is an input and a commit.
    pub fn key(&mut self, key: Key, m: Modifiers, value: [u8; 4], enabled: bool) -> Vec<ColorOut> {
        let Some(ctl) = self.focus.filter(|_| enabled && self.claims(key)) else { return Vec::new() };
        let mut h = self.hsv(value);
        let mul = if m.contains(Modifiers::SHIFT) { 10.0 } else { 1.0 };
        match ctl {
            Ctl::Chan(c) => {
                let v = channel(h, c);
                let max = CHANNELS[c as usize].2;
                let n = match key {
                    Key::ArrowUp | Key::ArrowRight => v + mul,
                    Key::ArrowDown | Key::ArrowLeft => v - mul,
                    Key::PageUp => v + 10.0,
                    Key::PageDown => v - 10.0,
                    Key::Home => 0.0,
                    _ => max,
                };
                h = with_channel(h, c, n);
            }
            _ => {
                let step = 0.01 * mul;
                let (dx, dy) = match key {
                    Key::ArrowLeft => (-step, 0.0),
                    Key::ArrowRight => (step, 0.0),
                    Key::ArrowUp => (0.0, step),
                    Key::ArrowDown => (0.0, -step),
                    Key::PageUp => (0.1, 0.1),
                    Key::PageDown => (-0.1, -0.1),
                    Key::Home => (-1.0, 0.0),
                    _ => (1.0, 0.0),
                };
                match ctl {
                    Ctl::Square => {
                        h.s = (h.s + dx).clamp(0.0, 1.0);
                        h.v = (h.v + dy).clamp(0.0, 1.0);
                    }
                    Ctl::Hue => h.h = (h.h + dx + dy).clamp(0.0, HUE_MAX),
                    Ctl::Alpha => h.a = (h.a + dx + dy).clamp(0.0, 1.0),
                    Ctl::Chan(_) | Ctl::Hex => {}
                }
            }
        }
        self.hsv = Some(h);
        vec![ColorOut::Input(h), ColorOut::Commit]
    }

    /// A whole hex value written to the hex field (its commit, or AccessKit SetValue): a colour is
    /// an input and a commit; anything else marks the field invalid.
    pub fn set_hex(&mut self, s: &str, cfg: &ColorCfg) -> Vec<ColorOut> {
        match parse_hex(s) {
            Ok(mut c) => {
                if !cfg.alpha {
                    c[3] = 255;
                }
                let h = Hsv::from_srgba(c);
                self.hsv = Some(h);
                self.hex_invalid = false;
                vec![ColorOut::Input(h), ColorOut::Commit]
            }
            Err(_) => {
                self.hex_invalid = true;
                Vec::new()
            }
        }
    }

    /// The hex the field shows for `h` (opaque without the alpha strip).
    pub fn hex_of(h: Hsv, cfg: &ColorCfg) -> String {
        to_hex(if cfg.alpha { h.to_srgba() } else { h.opaque().to_srgba() })
    }

    /// The accessible value of a slider sub-control (svelters-egui's: 0..100 and a text).
    pub fn slider_value(h: Hsv, ctl: Ctl) -> (f64, String) {
        match ctl {
            Ctl::Square => ((h.s * 100.0).round() as f64, format!("saturation {}%, value {}%", (h.s * 100.0).round(), (h.v * 100.0).round())),
            Ctl::Hue => ((h.h * 100.0).round() as f64, format!("{}°", (h.h * 360.0).round())),
            _ => ((h.a * 100.0).round() as f64, format!("{}%", (h.a * 100.0).round())),
        }
    }

    /// Records the body at `g` for colour `value`. `focus_visible`: the focused sub-control shows
    /// its ring.
    #[allow(clippy::too_many_arguments)]
    pub fn record(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, g: &ColorGeom, cfg: &ColorCfg, value: [u8; 4], enabled: bool, focus_visible: bool) {
        let h = self.shown(value);
        let ring = |c: Ctl| focus_visible && self.focus == Some(c) && self.dragging.is_none();
        record_square(list, look, g.square, h);
        record_handle_ring(list, look, g.square, h, self.dragging == Some(Ctl::Square), ring(Ctl::Square));
        record_hue(list, look, g.hue);
        record_strip_handle(list, look, g.hue, h.h, self.dragging == Some(Ctl::Hue), ring(Ctl::Hue));
        if let Some(a) = g.alpha {
            record_alpha(list, look, a, h);
            record_strip_handle(list, look, a, h.a, self.dragging == Some(Ctl::Alpha), ring(Ctl::Alpha));
        }
        let field_fg = if enabled { look.control_fg } else { look.disabled_fg };
        if let Some(r) = g.hex {
            let border = if self.hex_invalid { look.invalid_border } else { look.control_border };
            shim::frame(list, r, look.control_bg, border, look.stroke, look.radius);
            text.draw(list, &ColorBody::hex_of(h, cfg), &look.text(field_fg), r.inset(Insets { top: 0.0, right: 4.0, bottom: 0.0, left: 4.0 }), HAlign::Start, true);
            if ring(Ctl::Hex) {
                shim::frame(list, r.inflate(-1.0), Color::TRANSPARENT, look.focus, 1.0, look.radius);
            }
        }
        for (c, r) in &g.chans {
            let border = if self.dragging == Some(Ctl::Chan(*c)) { look.focus } else { look.control_border };
            shim::frame(list, *r, look.control_bg, border, look.stroke, look.radius);
            let inner = r.inset(Insets { top: 0.0, right: 4.0, bottom: 0.0, left: 4.0 });
            text.draw(list, CHANNELS[*c as usize].0, &look.text(look.muted), inner, HAlign::Start, false);
            text.draw(list, &format!("{}", channel(h, *c)), &look.text(field_fg), inner, HAlign::End, false);
            if ring(Ctl::Chan(*c)) {
                shim::frame(list, r.inflate(-1.0), Color::TRANSPARENT, look.focus, 1.0, look.radius);
            }
        }
        let cur = h.to_srgba();
        let lists: [CellList<'_>; 2] = [(&cfg.swatches, &g.cells, Hit::Swatch), (&cfg.custom, &g.custom_cells, Hit::Custom)];
        for (colors, rects, hit) in lists {
            for (k, (c, r)) in colors.iter().zip(rects.iter()).enumerate() {
                let Ok(rgba) = parse_hex(c) else { continue };
                record_cell(list, look, *r, Some(rgba), rgba == cur, self.hovered == Some(hit(k)));
            }
        }
        if let Some(r) = g.add {
            record_cell(list, look, r, None, false, self.hovered == Some(Hit::Add));
        }
    }
}

fn gradient(rect: Rect, angle_deg: f32, colors: &[Color], radius: f32) -> Prim {
    let n = (colors.len().max(2) - 1) as f32;
    let stops: SmallVec<[GradientStop; 4]> = colors.iter().enumerate().map(|(i, c)| GradientStop { offset: i as f32 / n, color: *c }).collect();
    Prim::Quad(Quad { rect, radii: Corners::all(radius), fill: Fill::Linear { angle_deg, stops }, border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid })
}

/// The saturation/value square: white → the hue (left to right) under transparent → black (top to
/// bottom), then its border.
pub fn record_square(list: &mut DisplayList, look: &PickerLook, r: Rect, h: Hsv) {
    let hue = Hsv { h: h.h, s: 1.0, v: 1.0, a: 1.0 }.color();
    list.push(gradient(r, 90.0, &[Color::WHITE, hue], look.radius), r);
    list.push(gradient(r, 180.0, &[Color::BLACK.with_alpha(0.0), Color::BLACK], look.radius), r);
    shim::frame(list, r, Color::TRANSPARENT, look.control_border, look.stroke, look.radius);
}

/// The hue strip: the six primaries and red again, left to right.
pub fn record_hue(list: &mut DisplayList, look: &PickerLook, r: Rect) {
    let stops: Vec<Color> = (0..=6).map(|k| Hsv { h: k as f32 / 6.0, s: 1.0, v: 1.0, a: 1.0 }.color()).collect();
    list.push(gradient(r, 90.0, &stops, look.radius), r);
    shim::frame(list, r, Color::TRANSPARENT, look.control_border, look.stroke, look.radius);
}

/// A checkerboard of [`CHECKER`] squares under alpha (light grey on white).
pub fn record_checker(list: &mut DisplayList, r: Rect) {
    shim::fill(list, r, Color::WHITE, 0.0);
    let grey = Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff);
    let (cols, rows) = ((r.width() / CHECKER).ceil() as usize, (r.height() / CHECKER).ceil() as usize);
    for y in 0..rows {
        for x in (y % 2..cols).step_by(2) {
            let c = Rect::new(r.min.x + x as f32 * CHECKER, r.min.y + y as f32 * CHECKER, r.min.x + (x + 1) as f32 * CHECKER, r.min.y + (y + 1) as f32 * CHECKER).intersect(&r);
            shim::fill(list, c, grey, 0.0);
        }
    }
}

/// The alpha strip: the colour from transparent to opaque over a checkerboard.
pub fn record_alpha(list: &mut DisplayList, look: &PickerLook, r: Rect, h: Hsv) {
    list.push(Prim::PushClip(skelter_base::geom::RoundedRect { rect: r, radii: Corners::all(look.radius) }), r);
    record_checker(list, r);
    list.push(Prim::PopClip, r);
    let c = h.opaque().color();
    list.push(gradient(r, 90.0, &[c.with_alpha(0.0), c], look.radius), r);
    shim::frame(list, r, Color::TRANSPARENT, look.control_border, look.stroke, look.radius);
}

/// The square's handle: a white ring at (saturation, 1 − value).
pub fn record_handle_ring(list: &mut DisplayList, look: &PickerLook, r: Rect, h: Hsv, dragging: bool, ring: bool) {
    let w = if dragging { HANDLE_W_DRAGGING } else { HANDLE_W };
    let c = Point::new(r.min.x + h.s.clamp(0.0, 1.0) * r.width(), r.min.y + (1.0 - h.v.clamp(0.0, 1.0)) * r.height());
    let hr = Rect::new(c.x - w / 2.0, c.y - w / 2.0, c.x + w / 2.0, c.y + w / 2.0);
    shim::frame(list, hr, Color::TRANSPARENT, Color::WHITE, 2.0, w / 2.0);
    if ring {
        shim::frame(list, hr.inflate(3.0), Color::TRANSPARENT, look.focus, 2.0, w / 2.0 + 3.0);
    }
}

/// A strip's handle: a white bar at `t` (0..1) along it.
pub fn record_strip_handle(list: &mut DisplayList, look: &PickerLook, r: Rect, t: f32, dragging: bool, ring: bool) {
    let w = if dragging { HANDLE_W_DRAGGING } else { HANDLE_W } * 0.6;
    let x = r.min.x + t.clamp(0.0, 1.0) * r.width();
    let hr = Rect::new(x - w / 2.0, r.min.y - 1.0, x + w / 2.0, r.max.y + 1.0);
    shim::frame(list, hr, Color::TRANSPARENT, Color::WHITE, 2.0, 2.0);
    if ring {
        shim::frame(list, hr.inflate(3.0), Color::TRANSPARENT, look.focus, 2.0, 5.0);
    }
}

/// A palette cell (`rgba`), or the add cell (`None`: a plus).
pub fn record_cell(list: &mut DisplayList, look: &PickerLook, r: Rect, rgba: Option<[u8; 4]>, selected: bool, hovered: bool) {
    let (border, width) = if selected {
        (look.fg, 2.0)
    } else if hovered {
        (look.focus, look.stroke)
    } else {
        (look.control_border, look.stroke)
    };
    match rgba {
        Some(c) => {
            if c[3] < 255 {
                list.push(Prim::PushClip(skelter_base::geom::RoundedRect { rect: r, radii: Corners::all(3.0) }), r);
                record_checker(list, r);
                list.push(Prim::PopClip, r);
            }
            shim::frame(list, r, rgba_color(c), border, width, 3.0);
        }
        None => {
            shim::frame(list, r, Color::TRANSPARENT, border, width, 3.0);
            let (cx, cy, s) = ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0, r.width().min(r.height()) * 0.3);
            shim::line(list, Point::new(cx - s, cy), Point::new(cx + s, cy), 1.0, look.icon);
            shim::line(list, Point::new(cx, cy - s), Point::new(cx, cy + s), 1.0, look.icon);
        }
    }
}

/// The `color-editor` kind: an inline colour editor with `bind:value` and `bind:custom-colors`.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorEditor {
    /// The colour, normalized hex.
    pub value: String,
    pub cfg: ColorCfg,
    pub body: ColorBody,
}

impl Default for ColorEditor {
    fn default() -> Self {
        ColorEditor { value: "#000000".into(), cfg: ColorCfg::default(), body: ColorBody::default() }
    }
}

impl ColorEditor {
    pub fn rgba(&self) -> [u8; 4] {
        super::color::rgba_or_black(&self.value)
    }

    pub fn geom(&self, look: &PickerLook) -> ColorGeom {
        ColorGeom::new(look, &self.cfg, Point::ZERO, SQUARE.w)
    }

    pub fn size(&self, look: &PickerLook) -> Size {
        self.geom(look).size(Point::ZERO)
    }

    /// Turns the body's outcomes into the kind's props and signals: an input sets `value` and
    /// raises `changed(value)`; a commit raises `commit(value)`; the add cell appends the colour to
    /// `custom-colors` (raising its bind change as `custom-colors-changed(list)`).
    pub fn apply(&mut self, outs: Vec<ColorOut>) -> Vec<Emit> {
        let mut emits = Vec::new();
        for o in outs {
            match o {
                ColorOut::Input(h) => {
                    let hex = ColorBody::hex_of(h, &self.cfg);
                    self.body.edited(h);
                    if hex != self.value {
                        self.value = hex.clone();
                        emits.push(Emit::new("changed", vec![Value::Str(hex)]));
                    }
                }
                ColorOut::Commit => emits.push(Emit::new("commit", vec![Value::Str(self.value.clone())])),
                ColorOut::AddCustom => {
                    if self.cfg.custom.len() < MAX_CUSTOM {
                        self.cfg.custom.push(self.value.clone());
                        emits.push(Emit::new("custom-colors-changed", vec![Value::List(self.cfg.custom.iter().cloned().map(Value::Str).collect())]));
                    }
                }
            }
        }
        emits
    }

    pub fn pointer(&mut self, ev: &PointerEvent, look: &PickerLook, enabled: bool) -> Vec<Emit> {
        let g = self.geom(look);
        let v = self.rgba();
        let outs = self.body.pointer(ev, &g, &self.cfg, v, enabled);
        self.apply(outs)
    }

    pub fn key(&mut self, key: Key, m: Modifiers, enabled: bool) -> Vec<Emit> {
        let v = self.rgba();
        let outs = self.body.key(key, m, v, enabled);
        self.apply(outs)
    }

    pub fn record(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, enabled: bool, focus_visible: bool) {
        self.body.record(list, text, look, &self.geom(look), &self.cfg, self.rgba(), enabled, focus_visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pickers::test_util::ptr;
    use crate::table::shim::testing::FixedText;
    use skelter_base::event::PointerPhase;

    #[test]
    fn a_square_drag_sets_saturation_and_value_and_commits_at_release() {
        let look = PickerLook::default();
        let mut e = ColorEditor { value: "#ff0000".into(), ..ColorEditor::default() };
        let sq = e.geom(&look).square;
        // the square's centre: s = 0.5, v = 0.5
        let (cx, cy) = (sq.min.x + sq.width() / 2.0, sq.min.y + sq.height() / 2.0);
        let out = e.pointer(&ptr(PointerPhase::Down, cx, cy), &look, true);
        assert_eq!(out.len(), 1);
        assert_eq!(e.value, "#804040");
        assert_eq!(e.body.dragging(), Some(Ctl::Square));
        // drag to the top-right corner: full saturation and value
        e.pointer(&ptr(PointerPhase::Move, sq.max.x + 20.0, sq.min.y - 20.0), &look, true);
        assert_eq!(e.value, "#ff0000");
        let out = e.pointer(&ptr(PointerPhase::Up, sq.max.x + 20.0, sq.min.y - 20.0), &look, true);
        assert_eq!(out, vec![Emit::new("commit", vec![Value::Str("#ff0000".into())])]);
        assert_eq!(e.body.dragging(), None);
        // bottom-left: black, the hue kept
        e.pointer(&ptr(PointerPhase::Down, sq.min.x, sq.max.y - 0.01), &look, true);
        e.pointer(&ptr(PointerPhase::Up, sq.min.x, sq.max.y - 0.01), &look, true);
        assert_eq!(e.value, "#000000");
        assert_eq!(e.body.hsv(e.rgba()).h, 0.0);
    }

    #[test]
    fn the_hue_survives_a_grey() {
        let mut b = ColorBody::default();
        let h = b.hsv([0, 0, 255, 255]);
        assert!((h.h - 2.0 / 3.0).abs() < 1e-6);
        let h = b.hsv([128, 128, 128, 255]);
        assert!((h.h - 2.0 / 3.0).abs() < 1e-6, "grey keeps the previous hue");
    }

    #[test]
    fn keys_step_the_focused_control() {
        let mut e = ColorEditor { value: "#808080".into(), cfg: ColorCfg { rgb: true, ..ColorCfg::default() }, ..ColorEditor::default() };
        e.body.focus = Some(Ctl::Chan(0));
        let out = e.key(Key::ArrowUp, Modifiers::SHIFT, true);
        assert_eq!(e.value, "#8a8080");
        assert_eq!(out.last(), Some(&Emit::new("commit", vec![Value::Str("#8a8080".into())])));
        e.key(Key::End, Modifiers::default(), true);
        assert_eq!(e.value, "#ff8080");
        e.body.focus = Some(Ctl::Hex);
        assert!(e.key(Key::ArrowUp, Modifiers::default(), true).is_empty(), "the hex field takes no steps");
    }

    #[test]
    fn hex_writes_apply_or_mark_invalid() {
        let mut e = ColorEditor::default();
        let outs = e.body.set_hex("#12345678", &e.cfg);
        e.apply(outs);
        assert_eq!(e.value, "#123456", "opaque without the alpha strip");
        assert!(e.body.set_hex("#xyz", &e.cfg).is_empty());
        assert!(e.body.hex_invalid);
        e.cfg.alpha = true;
        let outs = e.body.set_hex("#12345678", &e.cfg);
        e.apply(outs);
        assert_eq!(e.value, "#12345678");
    }

    #[test]
    fn swatches_and_the_add_cell() {
        let look = PickerLook::default();
        let mut e = ColorEditor { cfg: ColorCfg { swatches: vec!["#00ff00".into()], ..ColorCfg::default() }, ..ColorEditor::default() };
        let g = e.geom(&look);
        let c = g.cells[0];
        e.pointer(&ptr(PointerPhase::Down, c.min.x + 1.0, c.min.y + 1.0), &look, true);
        let out = e.pointer(&ptr(PointerPhase::Up, c.min.x + 1.0, c.min.y + 1.0), &look, true);
        assert_eq!(out, vec![Emit::new("changed", vec![Value::Str("#00ff00".into())]), Emit::new("commit", vec![Value::Str("#00ff00".into())])]);
        let a = g.add.unwrap();
        e.pointer(&ptr(PointerPhase::Down, a.min.x + 1.0, a.min.y + 1.0), &look, true);
        e.pointer(&ptr(PointerPhase::Up, a.min.x + 1.0, a.min.y + 1.0), &look, true);
        assert_eq!(e.cfg.custom, vec!["#00ff00".to_string()]);
    }

    #[test]
    fn channels_round_trip() {
        let h = Hsv::from_srgba([10, 200, 30, 255]);
        assert_eq!(channel(h, 1), 200.0);
        let h2 = with_channel(h, 0, 300.0);
        assert_eq!(h2.to_srgba()[0], 255, "clamped");
        let h3 = with_channel(h, 3, 120.0);
        assert!((h3.h - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn records_square_strips_and_fields() {
        let look = PickerLook::default();
        let e = ColorEditor { value: "#ff000080".into(), cfg: ColorCfg { alpha: true, ..ColorCfg::default() }, ..ColorEditor::default() };
        let mut list = DisplayList::new();
        let mut t = FixedText::default();
        e.record(&mut list, &mut t, &look, true, false);
        let gradients = list.items.iter().filter(|p| matches!(p, Prim::Quad(Quad { fill: Fill::Linear { .. }, .. }))).count();
        assert_eq!(gradients, 4, "two for the square, one hue, one alpha");
        assert_eq!(t.texts(), vec!["#ff000080", "R", "255", "G", "0", "B", "0"]);
    }
}
