//! The menu kinds (svelters-egui `widgets/menu.rs`, `kinds/menus.rs`): `menu-bar`, `context-menu`,
//! and the entries under them (`menu`, `menu-item`, `menu-separator`). The roots own the open
//! state ([`MenuNav`]) and the geometry: the bar's titles in flow, and one surface per open level,
//! recorded for the window's overlay stack (architecture §14) by [`Menus::record_levels`]:
//!
//! - level 0 goes `bottom-start` under its bar title, or at the pointer for a context menu; a
//!   submenu goes `right-start` beside its row with its first row level with it, flipped left at the
//!   window edge; every level is shrunk to the room it has (`fit`);
//! - the pointer: a press on a title opens or closes its menu, and moving over another title while a
//!   menu is open switches to it; a press on a submenu row opens it at once, hovering opens it after
//!   the platform's delay ([`Policy::submenu_delay`], 0 on macOS) and hovering another row closes it
//!   after the same delay, except while the pointer heads towards the open submenu (the safe
//!   triangle); an item picks on release; a press outside every level closes them all;
//! - the keys are [`MenuNav::key`]'s.
//!
//! Positions are in the root's local points: a `context-menu` shares its owner's space, and the
//! overlay layers the levels go into take the root's transform. `prepare` lays out with the labels
//! after input and before recording (architecture §4.2's `prepare` hook).
//!
//! macOS's own menu bar (`NSMenu`) is a platform concern, not drawn here.

use super::draw;
use super::menu_model::{EntryIx, EntryKind, KeyIn, MenuModel, MenuNav, MenuOut, Picked, shortcut_text};
use super::placement::{AnchorParams, Placement, place};
use super::shim::{
    self, Event, EventCx, Handled, Kind, KindSpec, Labels, Os, Palette, PropError, PropType, RecordCx, Value, prop,
    signal,
};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::display::DisplayList;
use presto_base::event::{KeyState, MouseButton, PointerEvent, PointerPhase};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;
use presto_base::time::Time;

/// The sizes menus are laid out with (VS Code's: 26 pt rows, 4 pt surface padding, 160 pt minimum).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuMetrics {
    pub row_h: f32,
    pub sep_h: f32,
    pub pad: f32,
    pub row_pad_x: f32,
    pub check_w: f32,
    pub chevron_w: f32,
    pub shortcut_gap: f32,
    pub min_w: f32,
    pub bar_pad: f32,
    pub title_pad_x: f32,
    pub font_size: f32,
    pub radius: f32,
}

impl MenuMetrics {
    pub fn from_palette(p: &Palette) -> MenuMetrics {
        MenuMetrics {
            row_h: p.row_height + 4.0,
            sep_h: 9.0,
            pad: 4.0,
            row_pad_x: 8.0,
            check_w: 22.0,
            chevron_w: 24.0,
            shortcut_gap: 24.0,
            min_w: 160.0,
            bar_pad: 4.0,
            title_pad_x: 8.0,
            font_size: p.font_size,
            radius: p.radius,
        }
    }
}

/// One open level as laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelGeom {
    pub container: EntryIx,
    pub rect: Rect,
    /// Every visible child (separators and labels too) with its row rect.
    pub rows: Vec<(EntryIx, Rect)>,
    /// Some row has check or radio state (the check column is drawn).
    pub checks: bool,
}

/// What is under a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuHit {
    Title(EntryIx),
    /// A row of level `k`.
    Row(usize, EntryIx),
    /// A level's padding or a separator.
    Surface(usize),
}

/// The open state and geometry shared by `menu-bar` and `context-menu`.
#[derive(Clone, Debug, Default)]
pub struct Menus {
    pub model: MenuModel,
    pub nav: MenuNav,
    bar: bool,
    pub enabled: bool,
    /// Where the levels must stay (the window's content area, in the root's space).
    pub bounds: Rect,
    /// A context menu's point.
    point: Option<Point>,
    titles: Vec<(EntryIx, Rect)>,
    levels: Vec<LevelGeom>,
    metrics: Option<MenuMetrics>,
    macos: bool,
    /// The row under the pointer and since when (submenu delays).
    hover: Option<(usize, EntryIx, Time)>,
    hover_title: Option<EntryIx>,
    last_pointer: Option<Point>,
    /// Where the press that opened the menu was: a release there is not a pick.
    press_origin: Option<Point>,
}

impl Menus {
    fn new(bar: bool) -> Menus {
        Menus {
            bar,
            enabled: true,
            bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
            ..Menus::default()
        }
    }

    /// The open levels as last laid out.
    pub fn levels(&self) -> &[LevelGeom] {
        &self.levels
    }
    /// The bar's titles as last laid out.
    pub fn titles(&self) -> &[(EntryIx, Rect)] {
        &self.titles
    }

    /// Lays out the bar's titles and the open levels (after input, before recording).
    pub fn prepare(&mut self, labels: &mut dyn Labels, palette: &Palette, os: Os) {
        let mt = MenuMetrics::from_palette(palette);
        self.metrics = Some(mt);
        self.macos = os == Os::MacOs;
        self.titles.clear();
        if self.bar {
            let mut x = mt.bar_pad;
            for t in self.model.rows(MenuModel::ROOT) {
                let w = labels.measure(&self.model.get(t).label, mt.font_size, None).w + mt.title_pad_x * 2.0;
                self.titles.push((t, Rect::new(x, mt.bar_pad, x + w, mt.bar_pad + mt.row_h - 4.0)));
                x += w;
            }
        }
        self.levels.clear();
        for k in 0..self.nav.levels.len() {
            let container = self.nav.levels[k].container;
            let (anchor, placement, cross) = if k == 0 {
                match (self.bar, self.point) {
                    (true, _) => match self.titles.iter().find(|(t, _)| *t == container) {
                        Some((_, r)) => (*r, Placement::BOTTOM_START, 0.0),
                        None => break,
                    },
                    (false, Some(p)) => (Rect::from_origin_size(p, Size::ZERO), Placement::BOTTOM_START, 0.0),
                    (false, None) => (Rect::ZERO, Placement::BOTTOM_START, 0.0),
                }
            } else {
                match self.levels[k - 1].rows.iter().find(|(r, _)| *r == container) {
                    Some((_, r)) => (*r, Placement::RIGHT_START, -mt.pad),
                    None => break,
                }
            };
            let g = self.layout_level(labels, &mt, container, anchor, placement, cross);
            self.levels.push(g);
        }
    }

    fn layout_level(
        &self,
        labels: &mut dyn Labels,
        mt: &MenuMetrics,
        container: EntryIx,
        anchor: Rect,
        placement: Placement,
        cross: f32,
    ) -> LevelGeom {
        let rows = self.model.rows(container);
        let checks = rows.iter().any(|&r| self.model.get(r).check != super::menu_model::Check::None);
        let subs = rows.iter().any(|&r| self.model.get(r).kind == EntryKind::Menu);
        let mut label_w: f32 = 0.0;
        let mut short_w: f32 = 0.0;
        let mut h = mt.pad * 2.0;
        for &r in &rows {
            let e = self.model.get(r);
            match e.kind {
                EntryKind::Separator => h += mt.sep_h,
                _ => {
                    h += mt.row_h;
                    label_w = label_w.max(labels.measure(&e.label, mt.font_size, None).w);
                    let s = shortcut_text(&e.shortcut, self.macos);
                    if !s.is_empty() {
                        short_w = short_w.max(labels.measure(&s, mt.font_size, None).w);
                    }
                }
            }
        }
        let w = mt.pad * 2.0
            + mt.row_pad_x * 2.0
            + if checks { mt.check_w } else { 0.0 }
            + label_w
            + if short_w > 0.0 { mt.shortcut_gap + short_w } else { 0.0 }
            + if subs { mt.chevron_w } else { 0.0 };
        let placed = place(&AnchorParams {
            anchor,
            size: Size::new(w.max(mt.min_w), h),
            bounds: self.bounds,
            placement,
            gap: if placement == Placement::RIGHT_START { 0.0 } else { 2.0 },
            cross,
            fit: true,
            ..AnchorParams::default()
        });
        let rect = placed.rect;
        let mut y = rect.min.y + mt.pad;
        let mut out = Vec::with_capacity(rows.len());
        for &r in &rows {
            let rh = if self.model.get(r).kind == EntryKind::Separator { mt.sep_h } else { mt.row_h };
            out.push((r, Rect::new(rect.min.x + mt.pad, y, rect.max.x - mt.pad, y + rh)));
            y += rh;
        }
        LevelGeom { container, rect, rows: out, checks }
    }

    /// What is at `p` (innermost level first).
    pub fn hit(&self, p: Point) -> Option<MenuHit> {
        for (k, l) in self.levels.iter().enumerate().rev() {
            if !l.rect.contains(p) {
                continue;
            }
            for &(r, rr) in &l.rows {
                if rr.contains(p) && self.model.get(r).is_row() {
                    return Some(MenuHit::Row(k, r));
                }
            }
            return Some(MenuHit::Surface(k));
        }
        self.titles.iter().find(|(_, r)| r.contains(p)).map(|&(t, _)| MenuHit::Title(t))
    }

    /// Opens the context menu at `p` (the owner's `contextmenu`, the JS `popup()` call).
    pub fn open_at(&mut self, p: Point, keyboard: bool, out: &mut Vec<MenuOut>) {
        self.point = Some(p);
        self.press_origin = (!keyboard).then_some(p);
        self.nav.open_root(&self.model, keyboard, out);
    }

    pub fn close(&mut self, out: &mut Vec<MenuOut>) {
        self.nav.close_all(out);
        self.hover = None;
    }

    /// Pointer input; positions in the root's space.
    pub fn pointer(&mut self, cx: &mut EventCx<'_>, ev: &PointerEvent, out: &mut Vec<MenuOut>) -> Handled {
        let p = ev.pos;
        let delta = self.last_pointer.map_or(Vec2::ZERO, |l| p - l);
        self.last_pointer = Some(p);
        let hit = self.hit(p);
        match ev.phase {
            PointerPhase::Down => {
                let primary = matches!(ev.button, Some(MouseButton::Primary | MouseButton::Secondary));
                match hit {
                    Some(MenuHit::Title(t)) if primary && self.model.enabled(t) => {
                        if self.nav.levels.first().is_some_and(|l| l.container == t) {
                            self.close(out);
                        } else {
                            self.nav.open_title(&self.model, t, false, out);
                            self.press_origin = Some(p);
                        }
                        cx.capture = Some(true);
                        Handled::Yes
                    }
                    Some(MenuHit::Row(k, r)) => {
                        self.press_origin = None;
                        if self.model.get(r).kind == EntryKind::Menu {
                            self.nav.open_sub(&self.model, k, r, false, out);
                        }
                        Handled::Yes
                    }
                    Some(MenuHit::Surface(_)) => Handled::Yes,
                    _ if self.nav.is_open() || self.nav.bar_keys => {
                        // a press outside dismisses and goes on to what is under it
                        self.close(out);
                        Handled::No
                    }
                    _ => Handled::No,
                }
            }
            PointerPhase::Up => {
                let moved = self.press_origin.is_none_or(|o| (p - o).length() >= cx.policy.drag_threshold);
                self.press_origin = None;
                match hit {
                    Some(MenuHit::Row(_, r)) if self.model.get(r).kind == EntryKind::Item => {
                        if moved {
                            self.nav.pick(&mut self.model, r, out);
                        }
                        Handled::Yes
                    }
                    Some(_) => Handled::Yes,
                    None => Handled::No,
                }
            }
            PointerPhase::Move => {
                self.hover_title = match hit {
                    Some(MenuHit::Title(t)) => Some(t),
                    _ => None,
                };
                match hit {
                    // switching between the bar's menus while one is open
                    Some(MenuHit::Title(t)) => {
                        if self.nav.is_open() && self.model.enabled(t) {
                            self.nav.open_title(&self.model, t, false, out);
                        }
                        self.hover = None;
                    }
                    Some(MenuHit::Row(k, r)) => {
                        let heading = self.nav.open_below(k).filter(|&s| s != r).and_then(|_| self.levels.get(k + 1)).is_some_and(|sub| towards(p, delta, sub.rect));
                        if !heading {
                            self.nav.keyboard = false;
                            self.nav.set_current(k, Some(r), out);
                            if self.hover.is_none_or(|(hk, hr, _)| hk != k || hr != r) {
                                self.hover = Some((k, r, cx.now));
                            }
                            self.hover_delay(cx, out);
                        }
                    }
                    _ => self.hover = None,
                }
                if hit.is_some() { Handled::Yes } else { Handled::No }
            }
            PointerPhase::Leave | PointerPhase::Cancel => {
                self.hover = None;
                self.hover_title = None;
                Handled::No
            }
            PointerPhase::Enter => Handled::No,
        }
    }

    /// Opens or closes a submenu once the pointer has rested on a row for the platform's delay.
    fn hover_delay(&mut self, cx: &mut EventCx<'_>, out: &mut Vec<MenuOut>) {
        let Some((k, r, since)) = self.hover else { return };
        let wants = self.model.get(r).kind == EntryKind::Menu && self.model.enabled(r);
        let open_here = self.nav.open_below(k);
        let done = if wants { open_here == Some(r) } else { open_here.is_none() };
        if done || k >= self.nav.levels.len() {
            return;
        }
        let due = since + cx.policy.submenu_delay;
        if cx.now < due {
            cx.wake_at(due);
            return;
        }
        if wants {
            self.nav.open_sub(&self.model, k, r, false, out);
        } else {
            self.nav.truncate(k + 1, out);
        }
    }

    /// A key; `true` when used.
    pub fn key(&mut self, cx: &EventCx<'_>, ev: &presto_base::event::KeyEvent, out: &mut Vec<MenuOut>) -> bool {
        let k = KeyIn { key: ev.key, text: ev.text.as_deref(), state: ev.state, modifiers: ev.modifiers, now: cx.now };
        // Shift+F10 and the Menu key open a context menu from the keyboard
        if !self.bar
            && !self.nav.is_open()
            && ev.state == KeyState::Pressed
            && (ev.key == Some(Key::Menu)
                || (ev.key == Some(Key::F10) && ev.modifiers == presto_base::event::Modifiers::SHIFT))
        {
            self.open_at(Point::ZERO, true, out);
            return true;
        }
        self.nav.key(&mut self.model, self.bar, cx.policy.alt_focuses_menu_bar, k, out)
    }

    /// Records the bar's titles (in flow).
    pub fn record_bar(&self, list: &mut DisplayList, palette: &Palette, labels: &mut dyn Labels) {
        let Some(mt) = self.metrics else { return };
        let open = self.nav.levels.first().map(|l| l.container);
        let mnemonics = self.nav.bar_keys || self.nav.keyboard;
        for &(t, r) in &self.titles {
            let e = self.model.get(t);
            let current = open == Some(t) || (self.nav.bar_keys && self.nav.bar_current == Some(t)) || self.hover_title == Some(t);
            if current && self.enabled {
                draw::fill(list, r, mt.radius, palette.hover);
            }
            let color = if self.enabled && e.enabled { palette.text } else { palette.text_muted };
            let origin = Point::new(r.min.x + mt.title_pad_x, r.min.y + (r.height() - mt.font_size * 1.25) / 2.0);
            labels.record(list, &e.label, origin, mt.font_size, color, None);
            if mnemonics {
                underline(list, labels, &e.label, e.mnemonic_at, origin, mt.font_size, color);
            }
        }
    }

    /// Records the open levels, outermost first (each goes into its own overlay layer).
    pub fn record_levels(&self, palette: &Palette, labels: &mut dyn Labels) -> Vec<DisplayList> {
        let Some(mt) = self.metrics else { return Vec::new() };
        let mut lists = Vec::with_capacity(self.levels.len());
        for (k, l) in self.levels.iter().enumerate() {
            let mut list = DisplayList::new();
            draw::shadow(&mut list, l.rect, mt.radius, palette.shadow, 8.0, Vec2::new(0.0, 2.0));
            draw::boxed(&mut list, l.rect, mt.radius, palette.surface_raised, 1.0, palette.border);
            let current = self.nav.levels.get(k).and_then(|n| n.current);
            let open_sub = self.nav.open_below(k);
            for &(r, rr) in &l.rows {
                self.record_row(&mut list, labels, palette, &mt, l, r, rr, current == Some(r) || open_sub == Some(r));
            }
            lists.push(list);
        }
        lists
    }

    #[allow(clippy::too_many_arguments)]
    fn record_row(
        &self,
        list: &mut DisplayList,
        labels: &mut dyn Labels,
        palette: &Palette,
        mt: &MenuMetrics,
        l: &LevelGeom,
        r: EntryIx,
        rr: Rect,
        current: bool,
    ) {
        let e = self.model.get(r);
        match e.kind {
            EntryKind::Separator => {
                let y = (rr.min.y + rr.max.y) / 2.0;
                draw::line(list, Point::new(rr.min.x, y), Point::new(rr.max.x, y), 1.0, palette.border);
                return;
            }
            EntryKind::GroupLabel => {
                let origin = Point::new(rr.min.x + mt.row_pad_x, rr.min.y + (rr.height() - mt.font_size * 1.25) / 2.0);
                labels.record(list, &e.label, origin, mt.font_size, palette.text_muted, Some(rr.width() - mt.row_pad_x * 2.0));
                return;
            }
            _ => {}
        }
        let enabled = self.model.enabled(r);
        let (fg, muted) = if current && enabled {
            draw::fill(list, rr, mt.radius, palette.accent);
            (palette.on_accent, palette.on_accent)
        } else {
            if current {
                // R9: a disabled current row is only tinted
                draw::fill(list, rr, mt.radius, palette.hover);
            }
            (if enabled { palette.text } else { palette.text_muted }, palette.text_muted)
        };
        let mut x = rr.min.x + mt.row_pad_x;
        let ty = rr.min.y + (rr.height() - mt.font_size * 1.25) / 2.0;
        if l.checks {
            if e.checked {
                let c = Point::new(x + mt.check_w / 2.0 - 3.0, (rr.min.y + rr.max.y) / 2.0);
                match e.check {
                    super::menu_model::Check::Radio(_) => draw::fill(list, Rect::new(c.x - 3.0, c.y - 3.0, c.x + 3.0, c.y + 3.0), 3.0, fg),
                    _ => check_mark(list, c, fg),
                }
            }
            x += mt.check_w;
        }
        let right = rr.max.x - mt.row_pad_x - if e.kind == EntryKind::Menu { mt.chevron_w } else { 0.0 };
        let origin = Point::new(x, ty);
        labels.record(list, &e.label, origin, mt.font_size, fg, Some((right - x).max(0.0)));
        if self.nav.keyboard {
            underline(list, labels, &e.label, e.mnemonic_at, origin, mt.font_size, fg);
        }
        let s = shortcut_text(&e.shortcut, self.macos);
        if !s.is_empty() {
            let w = labels.measure(&s, mt.font_size, None).w;
            labels.record(list, &s, Point::new(right - w, ty), mt.font_size, if current && enabled { fg } else { muted }, None);
        }
        if e.kind == EntryKind::Menu {
            let cx = rr.max.x - mt.row_pad_x - mt.chevron_w / 2.0 + 4.0;
            let cy = (rr.min.y + rr.max.y) / 2.0;
            draw::line(list, Point::new(cx - 2.0, cy - 4.0), Point::new(cx + 2.0, cy), 1.25, fg);
            draw::line(list, Point::new(cx + 2.0, cy), Point::new(cx - 2.0, cy + 4.0), 1.25, fg);
        }
    }

    /// Turns navigation results into signals and work marks.
    fn report(&self, cx: &mut EventCx<'_>, out: Vec<MenuOut>) {
        for o in out {
            match o {
                MenuOut::Opened(c) => {
                    cx.mark(ChangeClass::Structure);
                    cx.emit("open", id_record("menu", &self.model.get(c).id, None));
                }
                MenuOut::Closed(c) => {
                    cx.mark(ChangeClass::Structure);
                    cx.emit("close", id_record("menu", &self.model.get(c).id, None));
                }
                MenuOut::Current(r) => {
                    cx.mark(ChangeClass::Paint);
                    cx.emit("hover", id_record("item", &self.model.get(r).id, None));
                }
                MenuOut::Picked(Picked { activated, toggled }) => {
                    cx.mark(ChangeClass::Paint);
                    for (id, c) in toggled {
                        cx.emit("toggled", id_record("item", &id, Some(c)));
                    }
                    if let Some((id, c)) = activated {
                        cx.emit("activate", id_record("item", &id, Some(c)));
                    }
                }
                MenuOut::BarKeys(_) => cx.mark(ChangeClass::Paint),
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        if !self.enabled {
            return Handled::No;
        }
        let mut out = Vec::new();
        let h = match ev {
            Event::Pointer(p) => self.pointer(cx, p, &mut out),
            Event::Key(k) => {
                if self.key(cx, k, &mut out) {
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            Event::Timer => {
                self.hover_delay(cx, &mut out);
                Handled::No
            }
            Event::Focus(false) => {
                if self.nav.is_open() || self.nav.bar_keys {
                    self.close(&mut out);
                }
                Handled::No
            }
            _ => Handled::No,
        };
        if self.nav.is_open() {
            // the open menu holds the keyboard
            cx.focus = true;
        }
        self.report(cx, out);
        h
    }
}

fn id_record(key: &str, id: &str, checked: Option<bool>) -> Value {
    let mut fs = vec![(key.to_string(), Value::Str(id.to_string()))];
    if let Some(c) = checked {
        fs.push(("checked".to_string(), Value::Bool(c)));
    }
    Value::Record(fs)
}

/// Underlines the mnemonic letter of `label` (at byte `at`) recorded at `origin`.
fn underline(list: &mut DisplayList, labels: &mut dyn Labels, label: &str, at: Option<usize>, origin: Point, size: f32, color: Color) {
    let Some(at) = at else { return };
    let Some(ch) = label[at..].chars().next() else { return };
    let x0 = origin.x + labels.measure(&label[..at], size, None).w;
    let x1 = origin.x + labels.measure(&label[..at + ch.len_utf8()], size, None).w;
    let y = origin.y + size * 1.15;
    draw::line(list, Point::new(x0, y), Point::new(x1, y), 1.0, color);
}

/// A check mark centred near `c`.
fn check_mark(list: &mut DisplayList, c: Point, color: Color) {
    draw::line(list, Point::new(c.x - 4.0, c.y), Point::new(c.x - 1.0, c.y + 3.0), 1.5, color);
    draw::line(list, Point::new(c.x - 1.0, c.y + 3.0), Point::new(c.x + 5.0, c.y - 4.0), 1.5, color);
}

/// Whether the pointer at `p`, moving by `delta`, heads into `sub` (the triangle from where it was
/// to the submenu's near edge).
pub fn towards(p: Point, delta: Vec2, sub: Rect) -> bool {
    if delta == Vec2::ZERO || delta.x == 0.0 {
        return false;
    }
    let from = p - delta;
    let edge_x = if (sub.min.x + sub.max.x) / 2.0 >= from.x { sub.min.x } else { sub.max.x };
    in_triangle(p, from, Point::new(edge_x, sub.min.y), Point::new(edge_x, sub.max.y))
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let s = |p1: Point, p2: Point, p3: Point| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

// ---- the kinds ----

const ROOT_SIGNALS: &[shim::SignalSpec] = &[
    signal("activate", false, false),
    signal("toggled", true, false),
    signal("hover", true, false),
    signal("open", true, false),
    signal("close", true, false),
];

static MENU_BAR_SPEC: KindSpec = KindSpec {
    name: "menu-bar",
    props: &[prop("enabled", PropType::Bool, ChangeClass::Paint)],
    signals: ROOT_SIGNALS,
    slots: &[""],
    role: accesskit::Role::MenuBar,
};

static CONTEXT_MENU_SPEC: KindSpec = KindSpec {
    name: "context-menu",
    props: &[prop("enabled", PropType::Bool, ChangeClass::Paint), prop("open", PropType::Bool, ChangeClass::Structure)],
    signals: ROOT_SIGNALS,
    slots: &[""],
    role: accesskit::Role::Menu,
};

/// `menu-bar`: a row of titles, each opening its menu below it.
#[derive(Clone, Debug)]
pub struct MenuBar {
    pub menus: Menus,
}

impl Default for MenuBar {
    fn default() -> Self {
        MenuBar { menus: Menus::new(true) }
    }
}

impl MenuBar {
    /// The bar's height for `palette`.
    pub fn height(palette: &Palette) -> f32 {
        let mt = MenuMetrics::from_palette(palette);
        mt.row_h - 4.0 + mt.bar_pad * 2.0
    }
}

impl Kind for MenuBar {
    fn spec() -> &'static KindSpec {
        &MENU_BAR_SPEC
    }
    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "enabled" => {
                self.menus.enabled = value.as_bool().ok_or(PropError::Invalid("bool"))?;
                if !self.menus.enabled {
                    self.menus.close(&mut Vec::new());
                }
                Ok(ChangeClass::Paint)
            }
            _ => Err(PropError::Unknown),
        }
    }
    fn record(&self, cx: &mut RecordCx<'_>) {
        self.menus.record_bar(cx.list, cx.palette, cx.labels);
    }
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.menus.event(cx, ev)
    }
    fn a11y(&self, node: &mut accesskit::Node) {
        if !self.menus.enabled {
            node.set_disabled();
        }
    }
}

/// `context-menu`: a menu at a point of its owner, opened by the owner's context click, Shift+F10,
/// the Menu key or `open = true`.
#[derive(Clone, Debug)]
pub struct ContextMenu {
    pub menus: Menus,
}

impl Default for ContextMenu {
    fn default() -> Self {
        ContextMenu { menus: Menus::new(false) }
    }
}

impl ContextMenu {
    /// The owner's context click at `p` (owner-local points).
    pub fn context_click(&mut self, cx: &mut EventCx<'_>, p: Point) {
        let mut out = Vec::new();
        self.menus.open_at(p, false, &mut out);
        cx.focus = true;
        self.menus.report(cx, out);
    }
}

impl Kind for ContextMenu {
    fn spec() -> &'static KindSpec {
        &CONTEXT_MENU_SPEC
    }
    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "enabled" => {
                self.menus.enabled = value.as_bool().ok_or(PropError::Invalid("bool"))?;
                Ok(ChangeClass::Paint)
            }
            "open" => {
                let open = value.as_bool().ok_or(PropError::Invalid("bool"))?;
                let mut out = Vec::new();
                if open && !self.menus.nav.is_open() {
                    let p = self.menus.point.unwrap_or(Point::ZERO);
                    self.menus.open_at(p, false, &mut out);
                } else if !open {
                    self.menus.close(&mut out);
                }
                Ok(ChangeClass::Structure)
            }
            _ => Err(PropError::Unknown),
        }
    }
    /// Out of flow: its levels are recorded into the overlay stack ([`Menus::record_levels`]).
    fn record(&self, _cx: &mut RecordCx<'_>) {}
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.menus.event(cx, ev)
    }
    fn a11y(&self, node: &mut accesskit::Node) {
        if !self.menus.enabled {
            node.set_disabled();
        }
    }
}

// ---- the entries ----

static MENU_SPEC: KindSpec = KindSpec {
    name: "menu",
    props: &[
        prop("label", PropType::Str, ChangeClass::Layout),
        prop("enabled", PropType::Bool, ChangeClass::Paint),
        prop("visible", PropType::Bool, ChangeClass::Structure),
    ],
    signals: &[],
    slots: &[""],
    role: accesskit::Role::MenuItem,
};

static MENU_ITEM_SPEC: KindSpec = KindSpec {
    name: "menu-item",
    props: &[
        prop("label", PropType::Str, ChangeClass::Layout),
        prop("shortcut", PropType::Str, ChangeClass::Layout),
        prop("enabled", PropType::Bool, ChangeClass::Paint),
        prop("visible", PropType::Bool, ChangeClass::Structure),
        prop("checkable", PropType::Bool, ChangeClass::Layout),
        prop("checked", PropType::Bool, ChangeClass::Paint),
        prop("group", PropType::Str, ChangeClass::Layout),
    ],
    signals: &[signal("activate", false, false), signal("toggled", true, false), signal("hover", true, false)],
    slots: &[],
    role: accesskit::Role::MenuItem,
};

static MENU_SEPARATOR_SPEC: KindSpec = KindSpec {
    name: "menu-separator",
    props: &[prop("visible", PropType::Bool, ChangeClass::Structure)],
    signals: &[],
    slots: &[],
    role: accesskit::Role::GenericContainer,
};

/// The props of one menu entry; the root builds its [`MenuModel`] from its entry children (the
/// tree walk is `presto-core`'s; [`MenuModel::add`] takes these entries).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub entry: super::menu_model::Entry,
    group: String,
    checkable: bool,
}

impl MenuEntry {
    pub fn menu() -> MenuEntry {
        MenuEntry { entry: super::menu_model::Entry::menu("", ""), group: String::new(), checkable: false }
    }
    pub fn item() -> MenuEntry {
        MenuEntry { entry: super::menu_model::Entry::item("", ""), group: String::new(), checkable: false }
    }
    pub fn separator() -> MenuEntry {
        MenuEntry { entry: super::menu_model::Entry::separator(), group: String::new(), checkable: false }
    }

    fn sync_check(&mut self) {
        use super::menu_model::Check;
        self.entry.check = if !self.group.is_empty() {
            Check::Radio(self.group.clone())
        } else if self.checkable {
            Check::Check
        } else {
            Check::None
        };
    }

    fn spec_for(&self) -> &'static KindSpec {
        match self.entry.kind {
            EntryKind::Menu => &MENU_SPEC,
            EntryKind::Separator => &MENU_SEPARATOR_SPEC,
            _ => &MENU_ITEM_SPEC,
        }
    }

    /// Applies a prop write (`menu`, `menu-item` and `menu-separator` share it).
    pub fn set(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        let spec = self.spec_for().prop(name).ok_or(PropError::Unknown)?;
        let b = || value.as_bool().ok_or(PropError::Invalid("bool"));
        let s = || value.as_str().map(str::to_string).ok_or(PropError::Invalid("string"));
        match name {
            "label" => self.entry.set_label(&s()?),
            "shortcut" => self.entry.shortcut = s()?,
            "enabled" => self.entry.enabled = b()?,
            "visible" => self.entry.visible = b()?,
            "checked" => self.entry.checked = b()?,
            "checkable" => {
                self.checkable = b()?;
                self.sync_check();
            }
            "group" => {
                self.group = s()?;
                self.sync_check();
            }
            _ => return Err(PropError::Unknown),
        }
        Ok(spec.change)
    }

    /// The entry's AccessKit properties.
    pub fn node(&self, node: &mut accesskit::Node) {
        use super::menu_model::Check;
        let e = &self.entry;
        node.set_role(match (&e.kind, &e.check) {
            (EntryKind::Separator, _) => accesskit::Role::GenericContainer,
            (_, Check::Check) => accesskit::Role::MenuItemCheckBox,
            (_, Check::Radio(_)) => accesskit::Role::MenuItemRadio,
            _ => accesskit::Role::MenuItem,
        });
        if e.kind == EntryKind::Separator {
            return;
        }
        node.set_label(e.label.as_str());
        if !e.enabled {
            node.set_disabled();
        }
        if e.check != Check::None {
            node.set_toggled(accesskit::Toggled::from(e.checked));
        }
        if e.kind == EntryKind::Menu {
            node.set_has_popup(accesskit::HasPopup::Menu);
        }
        if !e.shortcut.is_empty() {
            node.set_keyboard_shortcut(shortcut_text(&e.shortcut, false).as_str());
        }
    }
}

/// `menu`: a bar title or a submenu row.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuKind(pub MenuEntry);
/// `menu-item`: an action, check or radio row.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItemKind(pub MenuEntry);
/// `menu-separator`.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuSeparatorKind(pub MenuEntry);

impl Default for MenuKind {
    fn default() -> Self {
        MenuKind(MenuEntry::menu())
    }
}
impl Default for MenuItemKind {
    fn default() -> Self {
        MenuItemKind(MenuEntry::item())
    }
}
impl Default for MenuSeparatorKind {
    fn default() -> Self {
        MenuSeparatorKind(MenuEntry::separator())
    }
}

macro_rules! entry_kind {
    ($t:ty, $spec:expr) => {
        impl Kind for $t {
            fn spec() -> &'static KindSpec {
                &$spec
            }
            fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
                self.0.set(name, value)
            }
            /// Drawn by its root's levels.
            fn record(&self, _cx: &mut RecordCx<'_>) {}
            fn a11y(&self, node: &mut accesskit::Node) {
                self.0.node(node)
            }
        }
    };
}
entry_kind!(MenuKind, MENU_SPEC);
entry_kind!(MenuItemKind, MENU_ITEM_SPEC);
entry_kind!(MenuSeparatorKind, MENU_SEPARATOR_SPEC);

#[cfg(test)]
mod tests {
    use super::super::menu_model::Entry;
    use super::super::shim::{FixedLabels, Policy};
    use super::*;
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers, PointerType};
    use presto_base::time::Span;

    fn ms(n: u64) -> Time {
        Time(n * 1_000_000)
    }

    fn bar() -> MenuBar {
        let mut b = MenuBar::default();
        let m = &mut b.menus.model;
        let file = m.add(MenuModel::ROOT, Entry::menu("file", "&File"));
        m.add(file, Entry::item("new", "&New").shortcut("Primary+N"));
        m.add(file, Entry::separator());
        let recent = m.add(file, Entry::menu("recent", "&Recent"));
        m.add(recent, Entry::item("a", "alpha"));
        m.add(recent, Entry::item("b", "beta"));
        m.add(file, Entry::item("save", "&Save"));
        let edit = m.add(MenuModel::ROOT, Entry::menu("edit", "&Edit"));
        m.add(edit, Entry::item("undo", "&Undo"));
        b.menus.bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        b
    }

    fn prepare(b: &mut Menus) {
        b.prepare(&mut FixedLabels, &Palette::default(), Os::Linux);
    }

    fn ptr(phase: PointerPhase, p: Point) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: 0,
            pos: p,
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    fn key(k: Key) -> Event {
        Event::Key(KeyEvent {
            key: Some(k),
            physical: 0,
            text: None,
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers: Modifiers::default(),
        })
    }

    fn center(r: Rect) -> Point {
        Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
    }

    fn row(b: &Menus, k: usize, id: &str) -> Rect {
        let ix = b.model.find(id).unwrap();
        b.levels()[k].rows.iter().find(|(r, _)| *r == ix).unwrap().1
    }

    fn send(b: &mut MenuBar, policy: &Policy, now: Time, ev: Event) -> (Handled, Vec<shim::Emitted>, Option<Time>) {
        let mut cx = EventCx::new(now, Size::new(800.0, 30.0), policy);
        let h = b.event(&mut cx, &ev);
        prepare(&mut b.menus);
        (h, cx.emitted, cx.wake)
    }

    #[test]
    fn levels_are_placed_below_titles_and_beside_rows() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy::for_os(Os::Linux);
        let file_title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(file_title)));
        assert!(b.menus.nav.is_open());
        let l0 = b.menus.levels()[0].rect;
        assert_eq!(l0.min, Point::new(file_title.min.x, file_title.max.y + 2.0));
        assert!(l0.width() >= 160.0);
        let rec = row(&b.menus, 0, "recent");
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Down, center(rec)));
        let l1 = b.menus.levels()[1].rect;
        assert_eq!(l1.min.x, rec.max.x, "right-start beside its row");
        assert_eq!(l1.min.y, rec.min.y - 4.0, "its first row level with the opener");
    }

    #[test]
    fn hovering_switches_bar_menus_while_one_is_open() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy::for_os(Os::Windows);
        let [(file, f), (edit, e)] = [b.menus.titles()[0], b.menus.titles()[1]];
        // hovering a title with nothing open opens nothing
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Move, center(e)));
        assert!(!b.menus.nav.is_open());
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(f)));
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Up, center(f)));
        assert_eq!(b.menus.nav.levels[0].container, file, "a release on the title is not a pick and keeps it open");
        let (_, sig, _) = send(&mut b, &policy, ms(5), ptr(PointerPhase::Move, center(e)));
        assert_eq!(b.menus.nav.levels[0].container, edit);
        assert!(sig.iter().any(|s| s.signal == "close") && sig.iter().any(|s| s.signal == "open"));
        // pressing the open title closes it
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Down, center(e)));
        assert!(!b.menus.nav.is_open());
    }

    #[test]
    fn submenus_open_after_the_delay_and_the_safe_triangle_holds() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy { submenu_delay: Span::from_millis(225), ..Policy::for_os(Os::Linux) };
        let f = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(f)));
        let rec = row(&b.menus, 0, "recent");
        let (_, _, wake) = send(&mut b, &policy, ms(100), ptr(PointerPhase::Move, center(rec)));
        assert_eq!(b.menus.nav.levels.len(), 1, "not before the delay");
        assert_eq!(wake, Some(ms(325)));
        send(&mut b, &policy, ms(325), Event::Timer);
        assert_eq!(b.menus.nav.levels.len(), 2, "opened by the timer");
        // heading right-down towards the submenu over Save does not change the row
        let sub = b.menus.levels()[1].rect;
        let save = row(&b.menus, 0, "save");
        let start = Point::new(sub.min.x - 6.0, center(rec).y);
        send(&mut b, &policy, ms(400), ptr(PointerPhase::Move, start));
        let p = Point::new(sub.min.x - 2.0, save.min.y + 1.0);
        if b.menus.hit(p) == Some(MenuHit::Row(0, b.menus.model.find("save").unwrap())) {
            send(&mut b, &policy, ms(410), ptr(PointerPhase::Move, p));
            assert_eq!(b.menus.nav.levels[0].current, b.menus.model.find("recent"));
        }
        // resting on Save: the submenu closes after the delay
        send(&mut b, &policy, ms(500), ptr(PointerPhase::Move, Point::new(center(save).x - 40.0, center(save).y)));
        send(&mut b, &policy, ms(510), ptr(PointerPhase::Move, Point::new(center(save).x - 41.0, center(save).y)));
        assert_eq!(b.menus.nav.levels.len(), 2);
        send(&mut b, &policy, ms(800), Event::Timer);
        assert_eq!(b.menus.nav.levels.len(), 1);
    }

    #[test]
    fn macos_opens_submenus_at_once() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy::for_os(Os::MacOs);
        let title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(title)));
        let rec = row(&b.menus, 0, "recent");
        send(&mut b, &policy, ms(1), ptr(PointerPhase::Move, center(rec)));
        assert_eq!(b.menus.nav.levels.len(), 2);
    }

    #[test]
    fn a_release_on_an_item_picks_and_a_press_outside_dismisses() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy::for_os(Os::Windows);
        let title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(title)));
        let save = row(&b.menus, 0, "save");
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Move, center(save)));
        let (_, sig, _) = send(&mut b, &policy, ms(20), ptr(PointerPhase::Up, center(save)));
        assert!(!b.menus.nav.is_open());
        let act = sig.iter().find(|s| s.signal == "activate").unwrap();
        assert_eq!(act.detail.field("item"), Some(&Value::Str("save".into())));
        let title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(30), ptr(PointerPhase::Down, center(title)));
        let (h, _, _) = send(&mut b, &policy, ms(40), ptr(PointerPhase::Down, Point::new(700.0, 500.0)));
        assert!(!b.menus.nav.is_open());
        assert_eq!(h, Handled::No, "the press goes on to what is under it");
    }

    #[test]
    fn keys_reach_the_bar_menus() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = Policy::for_os(Os::Linux);
        send(&mut b, &policy, ms(0), key(Key::F10));
        assert!(b.menus.nav.bar_keys);
        send(&mut b, &policy, ms(0), key(Key::ArrowDown));
        assert_eq!(b.menus.nav.current(), b.menus.model.find("new"));
        send(&mut b, &policy, ms(0), key(Key::ArrowDown));
        assert_eq!(b.menus.nav.current(), b.menus.model.find("recent"), "the separator is skipped");
        send(&mut b, &policy, ms(0), key(Key::ArrowRight));
        assert_eq!(b.menus.nav.current(), b.menus.model.find("a"));
        assert_eq!(b.menus.levels().len(), 2);
        send(&mut b, &policy, ms(0), key(Key::Escape));
        send(&mut b, &policy, ms(0), key(Key::ArrowRight));
        assert_eq!(b.menus.nav.current(), b.menus.model.find("a"), "Right on a submenu row opens it");
        send(&mut b, &policy, ms(0), key(Key::ArrowRight));
        assert_eq!(b.menus.nav.levels[0].container, b.menus.model.find("edit").unwrap(), "Right on a plain row: next menu");
        let (_, sig, _) = send(&mut b, &policy, ms(0), key(Key::Enter));
        assert!(sig.iter().any(|s| s.signal == "activate" && s.detail.field("item") == Some(&Value::Str("undo".into()))));
    }

    #[test]
    fn context_menu_opens_at_the_point_and_flips_at_the_edge() {
        let mut c = ContextMenu::default();
        c.menus.model.add(MenuModel::ROOT, Entry::item("cut", "Cu&t").shortcut("Primary+X"));
        c.menus.model.add(MenuModel::ROOT, Entry::item("copy", "&Copy"));
        c.menus.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let policy = Policy::for_os(Os::Windows);
        let mut cx = EventCx::new(ms(0), Size::new(400.0, 300.0), &policy);
        c.context_click(&mut cx, Point::new(390.0, 290.0));
        assert!(cx.emitted.iter().any(|s| s.signal == "open"));
        prepare(&mut c.menus);
        let r = c.menus.levels()[0].rect;
        assert_eq!(r.max.y, 290.0 - 2.0, "flipped above the point");
        assert_eq!(r.max.x, 400.0, "shifted inside");
        // the release of the opening click right there is not a pick
        let mut cx = EventCx::new(ms(5), Size::new(400.0, 300.0), &policy);
        c.event(&mut cx, &ptr(PointerPhase::Up, Point::new(390.0, 290.0)));
        assert!(c.menus.nav.is_open());
        // Shift+F10 opens it from the keyboard with its first row current
        let mut c2 = ContextMenu::default();
        c2.menus.model.add(MenuModel::ROOT, Entry::item("x", "x"));
        let mut cx = EventCx::new(ms(0), Size::new(10.0, 10.0), &policy);
        let ev = Event::Key(KeyEvent {
            key: Some(Key::F10),
            physical: 0,
            text: None,
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers: Modifiers::SHIFT,
        });
        assert_eq!(c2.event(&mut cx, &ev), Handled::Yes);
        assert_eq!(c2.menus.nav.current(), c2.menus.model.find("x"));
    }

    #[test]
    fn entries_take_props_and_describe_themselves() {
        let mut it = MenuItemKind::default();
        assert_eq!(it.set_prop("label", &Value::Str("&Wrap".into())), Ok(ChangeClass::Layout));
        assert_eq!(it.set_prop("checkable", &Value::Bool(true)), Ok(ChangeClass::Layout));
        assert_eq!(it.set_prop("checked", &Value::Bool(true)), Ok(ChangeClass::Paint));
        assert_eq!(it.set_prop("nope", &Value::Bool(true)), Err(PropError::Unknown));
        assert_eq!(it.0.entry.label, "Wrap");
        assert_eq!(it.0.entry.mnemonic, Some('w'));
        let mut node = accesskit::Node::new(accesskit::Role::Unknown);
        it.a11y(&mut node);
        assert_eq!(node.role(), accesskit::Role::MenuItemCheckBox);
        assert_eq!(node.toggled(), Some(accesskit::Toggled::True));
        let mut sep = MenuSeparatorKind::default();
        assert_eq!(sep.set_prop("label", &Value::Str("x".into())), Err(PropError::Unknown));
        assert_eq!(MenuItemKind::spec().signal("activate").map(|s| s.passive), Some(false));
    }

    #[test]
    fn towards_is_the_triangle_to_the_near_edge() {
        let sub = Rect::new(200.0, 0.0, 300.0, 200.0);
        assert!(towards(Point::new(150.0, 105.0), Vec2::new(10.0, 5.0), sub));
        assert!(!towards(Point::new(150.0, 105.0), Vec2::new(-10.0, 5.0), sub), "moving away");
        assert!(!towards(Point::new(150.0, 105.0), Vec2::ZERO, sub));
    }
}
