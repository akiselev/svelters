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
//!   the platform's delay ([`Policy::submenu_delay_ms`](crate::interact::Policy), 0 on macOS) and
//!   hovering another row closes it after the same delay, except while the pointer heads towards the
//!   open submenu (the safe triangle); an item picks on release; a press outside every level closes
//!   them all;
//! - the keys are [`MenuNav::key`]'s.
//!
//! Look (CSS over the UA tables below, from the theme's tokens): the bar's `::part(title)` (`:hover`
//! and `:open` while its menu shows), each level's `::part(menu)` surface (`menu-bg`,
//! `menu-border-color`, `menu-radius`, `popup-shadow`), its `::part(item)` rows (`:selected` is the
//! current row: `menu-sel`/`menu-sel-fg`; `:disabled`, `:checked`), `::part(separator)`,
//! `::part(shortcut)` (`muted`) and `::part(group-label)`. Row heights, paddings and the surface's
//! minimum width come from the same views.
//!
//! Positions are in the root's local points: a `context-menu` shares its owner's space, and the
//! overlay layers the levels go into take the root's transform. `prepare` lays out with the labels
//! after input and before recording (architecture §4.2's `prepare` hook).
//!
//! macOS's own menu bar (`NSMenu`) is a platform concern, not drawn here.

use super::common::{emit, ms, platform};
use super::menu_model::{Check, Entry, EntryIx, EntryKind, KeyIn, MenuModel, MenuNav, MenuOut, Picked, shortcut_text};
use super::placement::{AnchorParams, Placement, place};
use crate::controls::common::host_view;
use crate::interact::{Platform, StateBits};
use crate::paint::{self, Align, Elide, PaintCx, StyleView, Ua, UaRule, UaValue::{Px, Token}};
use crate::shim::core::{
    A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, Resources,
    SignalSpec, StyleCx, Styles, Value, ValueType,
};
use crate::shim::text::TextShaper;
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::display::{DisplayList, Line, LineCap, Prim};
use presto_base::event::{KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;
use presto_base::time::Time;

/// `menu-bar` itself: 4 pt around its titles.
pub const BAR_UA: Ua = Ua(&[UaRule::base(&[("padding", Px(4.0))])]);

/// `::part(title)`: a bar title (VS Code's menubar: 22 pt, 8 pt sides, the toolbar hover fill).
pub const TITLE_UA: Ua = Ua(&[
    UaRule::base(&[("height", Px(22.0)), ("padding-left", Px(8.0)), ("padding-right", Px(8.0)), ("border-radius", Token("radius"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::OPEN, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// `::part(menu)`: one open level's surface (VS Code's: 4 pt padding, 160 pt minimum width).
pub const SURFACE_UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("popup-shadow")),
    ("padding", Px(4.0)),
    ("min-width", Px(160.0)),
])]);

/// `::part(item)`: a row (26 pt, 8 pt sides). `:selected` is the current row; a disabled current row
/// is only tinted (R9).
pub const ITEM_UA: Ua = Ua(&[
    UaRule::base(&[("height", Px(26.0)), ("padding-left", Px(8.0)), ("padding-right", Px(8.0)), ("border-radius", Token("radius")), ("color", Token("fg"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("menu-sel")), ("color", Token("menu-sel-fg"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
    UaRule::when(StateBits(StateBits::SELECTED.0 | StateBits::DISABLED.0), &[("background-color", Token("list-hover")), ("color", Token("disabled-fg"))]),
]);

/// `::part(separator)`: a 9 pt row with a hairline in its middle.
pub const SEPARATOR_UA: Ua = Ua(&[UaRule::base(&[("height", Px(9.0)), ("background-color", Token("menu-border-color"))])]);

/// `::part(shortcut)`: the accelerator text, muted but for the current row.
pub const SHORTCUT_UA: Ua = Ua(&[
    UaRule::base(&[("color", Token("muted"))]),
    UaRule::when(StateBits::SELECTED, &[("color", Token("menu-sel-fg"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// `::part(group-label)`: a section heading row.
pub const GROUP_LABEL_UA: Ua = Ua(&[UaRule::base(&[("color", Token("muted"))])]);

/// The columns rows keep for check marks and submenu chevrons, and the gap before a shortcut.
const CHECK_W: f32 = 22.0;
const CHEVRON_W: f32 = 24.0;
const SHORTCUT_GAP: f32 = 24.0;

/// The sizes menus are laid out with, read from the parts' views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuMetrics {
    pub row_h: f32,
    pub sep_h: f32,
    /// The surface's padding (one value: its left).
    pub pad: f32,
    pub row_pad_x: f32,
    pub min_w: f32,
    pub bar_pad: f32,
    pub title_h: f32,
    pub title_pad_x: f32,
}

/// The views of a root and its parts, in a state (rows and titles override it per row).
struct Views<'a> {
    host: &'a StyleView<'a>,
    title: StyleView<'a>,
    surface: &'a StyleView<'a>,
    item: StyleView<'a>,
    separator: StyleView<'a>,
    group: StyleView<'a>,
}

/// Builds [`Views`] of `style` for a root with `ua`, then runs `f` with them.
fn with_views<R>(style: &Styles, ua: Ua, f: impl FnOnce(&Views) -> R) -> R {
    let host = host_view(style, ua);
    let surface = host.part(style.part_css("menu"), SURFACE_UA);
    let v = Views {
        title: host.part(style.part_css("title"), TITLE_UA),
        item: surface.part(style.part_css("item"), ITEM_UA),
        separator: surface.part(style.part_css("separator"), SEPARATOR_UA),
        group: surface.part(style.part_css("group-label"), GROUP_LABEL_UA),
        surface: &surface,
        host: &host,
    };
    f(&v)
}

impl MenuMetrics {
    fn of(v: &Views) -> MenuMetrics {
        let [_, _, bar_top, _] = v.host.sides("padding");
        let [pad, ..] = v.surface.sides("padding");
        let [row_pad_x, ..] = v.item.sides("padding");
        let [title_pad_x, ..] = v.title.sides("padding");
        MenuMetrics {
            row_h: v.item.px("height").unwrap_or(26.0),
            sep_h: v.separator.px("height").unwrap_or(9.0),
            pad,
            row_pad_x,
            min_w: v.surface.px("min-width").unwrap_or(0.0),
            bar_pad: bar_top,
            title_h: v.title.px("height").unwrap_or(22.0),
            title_pad_x,
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
    /// Shortcuts are shown the macOS way (from the last event's policy).
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
            macos: cfg!(target_os = "macos"),
            ..Menus::default()
        }
    }

    fn ua(&self) -> Ua {
        if self.bar { BAR_UA } else { Ua::NONE }
    }

    /// The open levels as last laid out.
    pub fn levels(&self) -> &[LevelGeom] {
        &self.levels
    }
    /// The bar's titles as last laid out.
    pub fn titles(&self) -> &[(EntryIx, Rect)] {
        &self.titles
    }

    /// The bar's natural size: its titles in a row inside its padding.
    pub fn bar_size(&self, text: &mut dyn TextShaper, style: &Styles) -> Size {
        with_views(style, self.ua(), |v| {
            let mt = MenuMetrics::of(v);
            let w: f32 = self.model.rows(MenuModel::ROOT).iter().map(|&t| paint::measure_text(text, &v.title, &self.model.get(t).label, None).w + mt.title_pad_x * 2.0).sum();
            Size::new(w + mt.bar_pad * 2.0, mt.title_h + mt.bar_pad * 2.0)
        })
    }

    /// Lays out the bar's titles and the open levels (after input, before recording).
    pub fn prepare(&mut self, text: &mut dyn TextShaper, style: &Styles) {
        with_views(style, self.ua(), |v| self.prepare_with(text, v));
    }

    fn prepare_with(&mut self, text: &mut dyn TextShaper, v: &Views) {
        let mt = MenuMetrics::of(v);
        self.metrics = Some(mt);
        self.titles.clear();
        if self.bar {
            let mut x = mt.bar_pad;
            for t in self.model.rows(MenuModel::ROOT) {
                let w = paint::measure_text(text, &v.title, &self.model.get(t).label, None).w + mt.title_pad_x * 2.0;
                self.titles.push((t, Rect::new(x, mt.bar_pad, x + w, mt.bar_pad + mt.title_h)));
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
            let g = self.layout_level(text, v, &mt, container, anchor, placement, cross);
            self.levels.push(g);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_level(
        &self,
        text: &mut dyn TextShaper,
        v: &Views,
        mt: &MenuMetrics,
        container: EntryIx,
        anchor: Rect,
        placement: Placement,
        cross: f32,
    ) -> LevelGeom {
        let rows = self.model.rows(container);
        let checks = rows.iter().any(|&r| self.model.get(r).check != Check::None);
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
                    label_w = label_w.max(paint::measure_text(text, &v.item, &e.label, None).w);
                    let s = shortcut_text(&e.shortcut, self.macos);
                    if !s.is_empty() {
                        short_w = short_w.max(paint::measure_text(text, &v.item, &s, None).w);
                    }
                }
            }
        }
        let w = mt.pad * 2.0
            + mt.row_pad_x * 2.0
            + if checks { CHECK_W } else { 0.0 }
            + label_w
            + if short_w > 0.0 { SHORTCUT_GAP + short_w } else { 0.0 }
            + if subs { CHEVRON_W } else { 0.0 };
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
                cx.repaint = true;
                if hit.is_some() { Handled::Yes } else { Handled::No }
            }
            PointerPhase::Leave | PointerPhase::Cancel => {
                self.hover_left();
                Handled::No
            }
            PointerPhase::Enter => Handled::No,
        }
    }

    fn hover_left(&mut self) {
        self.hover = None;
        self.hover_title = None;
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
        let due = since + ms(cx.policy.submenu_delay_ms);
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
        if !self.bar && !self.nav.is_open() && ev.state == KeyState::Pressed && (ev.key == Some(Key::Menu) || (ev.key == Some(Key::F10) && ev.modifiers == Modifiers::SHIFT)) {
            self.open_at(Point::ZERO, true, out);
            return true;
        }
        self.nav.key(&mut self.model, self.bar, cx.policy.alt_focuses_menu_bar, k, out)
    }

    /// Records the bar's titles (in flow) under the node's style.
    pub fn record_bar(&self, pc: &mut PaintCx, style: &Styles) {
        if self.metrics.is_none() {
            return;
        }
        with_views(style, self.ua(), |v| {
            let open = self.nav.levels.first().map(|l| l.container);
            let mnemonics = self.nav.bar_keys || self.nav.keyboard;
            for &(t, r) in &self.titles {
                let e = self.model.get(t);
                let mut st = StateBits::NONE;
                st.set(StateBits::HOVERED, self.hover_title == Some(t) || (self.nav.bar_keys && self.nav.bar_current == Some(t)));
                st.set(StateBits::OPEN, open == Some(t));
                st.set(StateBits::DISABLED, !self.enabled || !e.enabled);
                let tv = v.title.in_state(st);
                let g = paint::paint_box(pc, &tv, r);
                let layout = paint::text_layout(pc.text, &tv, &e.label, None);
                let at = paint::paint_text(pc, &tv, &layout, g.content, Align::Start);
                if mnemonics {
                    underline(pc, &tv, &e.label, e.mnemonic_at, at);
                }
            }
        });
    }

    /// Records the open levels, outermost first, each into its own list for its overlay layer.
    pub fn record_levels(&self, text: &mut dyn TextShaper, res: &mut dyn Resources, style: &Styles, scale: f32) -> Vec<DisplayList> {
        let Some(mt) = self.metrics else { return Vec::new() };
        with_views(style, self.ua(), |v| {
            let mut lists = Vec::with_capacity(self.levels.len());
            for (k, l) in self.levels.iter().enumerate() {
                let mut list = DisplayList::new();
                let mut pc = PaintCx::new(&mut list, &mut *res, &mut *text, scale);
                paint::paint_box(&mut pc, v.surface, l.rect);
                let current = self.nav.levels.get(k).and_then(|n| n.current);
                let open_sub = self.nav.open_below(k);
                for &(r, rr) in &l.rows {
                    self.record_row(&mut pc, v, &mt, l, r, rr, current == Some(r) || open_sub == Some(r));
                }
                lists.push(list);
            }
            lists
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_row(&self, pc: &mut PaintCx, v: &Views, mt: &MenuMetrics, l: &LevelGeom, r: EntryIx, rr: Rect, current: bool) {
        let e = self.model.get(r);
        match e.kind {
            EntryKind::Separator => {
                let y = (rr.min.y + rr.max.y) / 2.0;
                if let Some(c) = v.separator.color("background-color") {
                    stroke(pc, Point::new(rr.min.x, y), Point::new(rr.max.x, y), 1.0, c);
                }
                return;
            }
            EntryKind::GroupLabel => {
                let content = Rect::new(rr.min.x + mt.row_pad_x, rr.min.y, rr.max.x - mt.row_pad_x, rr.max.y);
                let shown = paint::elide(pc.text, &v.group, &e.label, Elide::End, content.width());
                let layout = paint::text_layout(pc.text, &v.group, &shown, None);
                paint::paint_text(pc, &v.group, &layout, content, Align::Start);
                return;
            }
            _ => {}
        }
        let mut st = StateBits::NONE;
        st.set(StateBits::SELECTED, current);
        st.set(StateBits::DISABLED, !self.model.enabled(r));
        st.set(StateBits::CHECKED, e.checked);
        let iv = v.item.in_state(st);
        let g = paint::paint_box(pc, &iv, rr);
        let fg = iv.color("color").unwrap_or(Color::BLACK);
        let mut x = g.content.min.x;
        let cy = (rr.min.y + rr.max.y) / 2.0;
        if l.checks {
            if e.checked {
                let c = Point::new(x + CHECK_W / 2.0 - 3.0, cy);
                match e.check {
                    Check::Radio(_) => dot(pc, c, 3.0, fg),
                    _ => check_mark(pc, c, fg),
                }
            }
            x += CHECK_W;
        }
        let right = g.content.max.x - if e.kind == EntryKind::Menu { CHEVRON_W } else { 0.0 };
        let shown = paint::elide(pc.text, &iv, &e.label, Elide::End, (right - x).max(0.0));
        let layout = paint::text_layout(pc.text, &iv, &shown, None);
        let at = paint::paint_text(pc, &iv, &layout, Rect::new(x, rr.min.y, right, rr.max.y), Align::Start);
        if self.nav.keyboard && shown == e.label {
            underline(pc, &iv, &e.label, e.mnemonic_at, at);
        }
        let s = shortcut_text(&e.shortcut, self.macos);
        if !s.is_empty() {
            let sv = iv.part(None, SHORTCUT_UA);
            let layout = paint::text_layout(pc.text, &sv, &s, None);
            paint::paint_text(pc, &sv, &layout, Rect::new(x, rr.min.y, right, rr.max.y), Align::End);
        }
        if e.kind == EntryKind::Menu {
            let cx = g.content.max.x - CHEVRON_W / 2.0 + 4.0;
            stroke(pc, Point::new(cx - 2.0, cy - 4.0), Point::new(cx + 2.0, cy), 1.25, fg);
            stroke(pc, Point::new(cx + 2.0, cy), Point::new(cx - 2.0, cy + 4.0), 1.25, fg);
        }
    }

    /// Turns navigation results into signals and repaints; `open` is the root's bindable `open` prop
    /// (a context menu's), told when the menu opens or closes natively.
    fn report(&self, cx: &mut EventCx<'_>, spec: &KindSpec, open: Option<PropId>, out: Vec<MenuOut>) {
        for o in out {
            cx.repaint = true;
            match o {
                MenuOut::Opened(c) => {
                    if self.nav.levels.len() == 1
                        && let Some(p) = open
                    {
                        cx.prop_changed(p);
                    }
                    emit(cx, spec, "open", vec![Value::str(&self.model.get(c).id)]);
                }
                MenuOut::Closed(c) => {
                    if !self.nav.is_open()
                        && let Some(p) = open
                    {
                        cx.prop_changed(p);
                    }
                    emit(cx, spec, "close", vec![Value::str(&self.model.get(c).id)]);
                }
                MenuOut::Current(r) => emit(cx, spec, "hover", vec![Value::str(&self.model.get(r).id)]),
                MenuOut::Picked(Picked { activated, toggled }) => {
                    for (id, c) in toggled {
                        emit(cx, spec, "toggled", vec![Value::str(&id), Value::Bool(c)]);
                    }
                    if let Some((id, c)) = activated {
                        emit(cx, spec, "activate", vec![Value::str(&id), Value::Bool(c)]);
                    }
                }
                MenuOut::BarKeys(_) => {}
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event, spec: &KindSpec, open: Option<PropId>) -> Handled {
        self.macos = platform(cx.policy) == Platform::Macos;
        if !self.enabled {
            return Handled::No;
        }
        let mut out = Vec::new();
        let h = match ev {
            Event::Pointer(p) => self.pointer(cx, p, &mut out),
            Event::Hover(false) => {
                self.hover_left();
                cx.repaint = true;
                Handled::No
            }
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
            Event::Focus { gained: false, .. } => {
                if self.nav.is_open() || self.nav.bar_keys {
                    self.close(&mut out);
                }
                Handled::No
            }
            // a press outside the root and its levels dismisses (light dismiss) and goes on
            Event::OutsidePress(_) => {
                if self.wants_outside() {
                    self.close(&mut out);
                }
                Handled::No
            }
            _ => Handled::No,
        };
        if self.nav.is_open() {
            // the open menu holds the keyboard
            cx.focus_request = Some(true);
        }
        self.report(cx, spec, open, out);
        h
    }

    /// A menu is open or the bar is highlighted: a press outside dismisses it ([`Kind::light_dismiss`]).
    pub fn wants_outside(&self) -> bool {
        self.enabled && (self.nav.is_open() || self.nav.bar_keys)
    }

    /// The keys the root takes while it has focus.
    fn claims(&self, key: Key, m: Modifiers) -> bool {
        if self.nav.is_open() || self.nav.bar_keys {
            return matches!(key, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::Enter | Key::Space | Key::Escape);
        }
        key == Key::F10 || (!self.bar && key == Key::Menu) || (self.bar && m.contains(Modifiers::ALT))
    }
}

/// A straight line of `width` in `color`.
pub(crate) fn stroke(pc: &mut PaintCx, from: Point, to: Point, width: f32, color: Color) {
    let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(width);
    pc.push(Prim::Line(Line { from, to, width, color, cap: LineCap::Butt, dash: None, wavy: false }), ink);
}

/// A filled dot of radius `r` at `c` (a radio item's mark).
fn dot(pc: &mut PaintCx, c: Point, r: f32, color: Color) {
    let rect = Rect::new(c.x - r, c.y - r, c.x + r, c.y + r);
    pc.push(
        Prim::Quad(presto_base::display::Quad {
            rect,
            radii: presto_base::geom::Corners::all(r),
            fill: presto_base::color::Fill::Solid(color),
            border_widths: presto_base::geom::Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        rect,
    );
}

/// A check mark centred near `c`.
pub(crate) fn check_mark(pc: &mut PaintCx, c: Point, color: Color) {
    stroke(pc, Point::new(c.x - 4.0, c.y), Point::new(c.x - 1.0, c.y + 3.0), 1.5, color);
    stroke(pc, Point::new(c.x - 1.0, c.y + 3.0), Point::new(c.x + 5.0, c.y - 4.0), 1.5, color);
}

/// Underlines the mnemonic letter of `label` (at byte `at`) painted in `text_rect` under `s`.
fn underline(pc: &mut PaintCx, s: &StyleView, label: &str, at: Option<usize>, text_rect: Rect) {
    let Some(at) = at else { return };
    let Some(ch) = label[at..].chars().next() else { return };
    let x0 = text_rect.min.x + paint::measure_text(pc.text, s, &label[..at], None).w;
    let x1 = text_rect.min.x + paint::measure_text(pc.text, s, &label[..at + ch.len_utf8()], None).w;
    let y = text_rect.max.y - 1.0;
    let color = s.color("color").unwrap_or(Color::BLACK);
    stroke(pc, Point::new(x0, y), Point::new(x1, y), 1.0, color);
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

// ---- the roots ----

/// The roots' signals. `activate` is not cancelable: the pick (check and radio state) is applied
/// before listeners run.
pub const ROOT_SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("activate", &[("item", ValueType::Str), ("checked", ValueType::Bool)], "An item was picked (its id and check state after the pick)."),
    SignalSpec::new("toggled", &[("item", ValueType::Str), ("checked", ValueType::Bool)], "A check or radio item changed state (each item of a radio group that did)."),
    SignalSpec::new("hover", &[("item", ValueType::Str)], "The current row changed (pointer or keys)."),
    SignalSpec::new("open", &[("menu", ValueType::Str)], "A menu level opened."),
    SignalSpec::new("close", &[("menu", ValueType::Str)], "A menu level closed."),
];

pub mod bar_props {
    use crate::kind_props;
    kind_props! {
        /// `menu-bar`'s props.
        pub struct BarProps;
        enabled "enabled": bool = Bool(true) ; Bool, Paint;
        /// A native menu bar shows these menus (the runner sets it from `PlatformCx::set_menu_bar`'s
        /// result): this one is not laid out and takes no input.
        native_bar "native-bar": bool = Bool(false) ; Bool, Layout;
    }
}

pub mod context_props {
    use crate::kind_props;
    kind_props! {
        /// `context-menu`'s props.
        pub struct ContextProps;
        enabled "enabled": bool = Bool(true) ; Bool, Paint;
        /// Shown; set natively when it opens or closes (`bind:open`).
        open "open": bool = Bool(false) ; Bool, Structure, bindable;
    }
}

pub static MENU_BAR_SPEC: KindSpec = KindSpec {
    tag: "menu-bar",
    props: bar_props::BarProps::SPECS,
    signals: ROOT_SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::MenuBar,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

pub static CONTEXT_MENU_SPEC: KindSpec = KindSpec {
    tag: "context-menu",
    props: context_props::ContextProps::SPECS,
    signals: ROOT_SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Menu,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// `menu-bar`: a row of titles, each opening its menu below it.
#[derive(Clone, Debug)]
pub struct MenuBar {
    pub props: bar_props::BarProps,
    pub menus: Menus,
}

impl Default for MenuBar {
    fn default() -> Self {
        MenuBar { props: Default::default(), menus: Menus::new(true) }
    }
}

impl MenuBar {
    /// The native menu bar picked item `id` (the runner's `AppHandler::menu_activated`): picks it as a
    /// click would (check and radio state, then `toggled` and `activate`). The runner sends the bar
    /// again afterwards (`to_menu_desc`) so the native checkmarks follow.
    pub fn menu_activated(&mut self, cx: &mut EventCx<'_>, id: &str) -> Handled {
        let Some(ix) = self.menus.model.find(id) else { return Handled::No };
        let Some(p) = self.menus.model.pick(ix) else { return Handled::No };
        self.menus.report(cx, &MENU_BAR_SPEC, None, vec![MenuOut::Picked(p)]);
        Handled::Yes
    }
}

impl Kind for MenuBar {
    fn parts() -> &'static [&'static str] {
        &["menu", "title", "item", "separator", "group-label"]
    }
    fn spec() -> &'static KindSpec {
        &MENU_BAR_SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &MENU_BAR_SPEC
    }
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        if self.props.native_bar {
            s.display = taffy::Display::None;
        }
        s
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)?;
        self.menus.enabled = self.props.enabled;
        if !self.menus.enabled {
            self.menus.close(&mut Vec::new());
        }
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }
    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.menus.bar_size(cx.text, &cx.style);
        taffy::Size { width: s.w, height: s.h }
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.menus.prepare(cx.text, &cx.style);
    }
    fn record(&self, cx: &mut RecordCx) {
        let style = Styles { computed: cx.style.computed, parts: cx.style.parts, children: cx.style.children, state: cx.style.state, dark: cx.style.dark };
        self.menus.record_bar(&mut PaintCx::of(cx), &style);
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if self.props.native_bar {
            return Handled::No;
        }
        self.menus.event(cx, ev, &MENU_BAR_SPEC, None)
    }
    fn light_dismiss(&self) -> bool {
        !self.props.native_bar && self.menus.wants_outside()
    }
    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::MenuBar);
        if !self.menus.enabled {
            cx.node.set_disabled();
        }
    }
    fn claims_key(&self, key: Key, m: Modifiers) -> bool {
        !self.props.native_bar && self.menus.claims(key, m)
    }
}

/// `context-menu`: a menu at a point of its owner, opened by the owner's context click, Shift+F10,
/// the Menu key or `open = true`.
#[derive(Clone, Debug)]
pub struct ContextMenu {
    pub props: context_props::ContextProps,
    pub menus: Menus,
}

impl Default for ContextMenu {
    fn default() -> Self {
        ContextMenu { props: Default::default(), menus: Menus::new(false) }
    }
}

impl ContextMenu {
    /// The owner's context click at `p` (owner-local points).
    pub fn context_click(&mut self, cx: &mut EventCx<'_>, p: Point) {
        let mut out = Vec::new();
        self.menus.open_at(p, false, &mut out);
        cx.focus_request = Some(true);
        self.sync_open();
        self.menus.report(cx, &CONTEXT_MENU_SPEC, Some(context_props::P::open.id()), out);
    }

    fn sync_open(&mut self) {
        self.props.open = self.menus.nav.is_open();
    }
}

impl Kind for ContextMenu {
    fn parts() -> &'static [&'static str] {
        &["menu", "title", "item", "separator", "group-label"]
    }
    fn spec() -> &'static KindSpec {
        &CONTEXT_MENU_SPEC
    }

    fn prop_state(&self) -> StateBits {
        if self.props.open { StateBits::OPEN } else { StateBits::NONE }
    }
    fn prop_states() -> StateBits {
        StateBits::OPEN
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &CONTEXT_MENU_SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)?;
        self.menus.enabled = self.props.enabled;
        let mut out = Vec::new();
        if self.props.open && !self.menus.nav.is_open() && self.menus.enabled {
            let p = self.menus.point.unwrap_or(Point::ZERO);
            self.menus.open_at(p, false, &mut out);
        } else if !self.props.open || !self.menus.enabled {
            self.menus.close(&mut out);
        }
        self.sync_open();
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.menus.prepare(cx.text, &cx.style);
    }
    /// Out of flow: its levels are recorded into the overlay stack ([`Menus::record_levels`]).
    fn record(&self, _cx: &mut RecordCx) {}
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let h = self.menus.event(cx, ev, &CONTEXT_MENU_SPEC, Some(context_props::P::open.id()));
        self.sync_open();
        h
    }
    fn light_dismiss(&self) -> bool {
        self.menus.wants_outside()
    }
    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Menu);
        if !self.menus.enabled {
            cx.node.set_disabled();
        }
    }
    fn claims_key(&self, key: Key, m: Modifiers) -> bool {
        self.menus.claims(key, m)
    }
}

// ---- the entries ----

pub mod menu_props {
    use crate::kind_props;
    kind_props! {
        /// `menu`'s props.
        pub struct MenuProps;
        /// The caption; `&` marks the mnemonic (`&File`).
        label "label": String = Str("") ; Str, Layout;
        enabled "enabled": bool = Bool(true) ; Bool, Paint;
        visible "visible": bool = Bool(true) ; Bool, Structure;
    }
}

pub mod item_props {
    use crate::kind_props;
    kind_props! {
        /// `menu-item`'s props.
        pub struct ItemProps;
        /// The caption; `&` marks the mnemonic.
        label "label": String = Str("") ; Str, Layout;
        /// The accelerator as authored (`Primary+S`), shown the platform's way.
        shortcut "shortcut": String = Str("") ; Str, Layout;
        enabled "enabled": bool = Bool(true) ; Bool, Paint;
        visible "visible": bool = Bool(true) ; Bool, Structure;
        /// A check item.
        checkable "checkable": bool = Bool(false) ; Bool, Layout;
        /// Set natively when picked (`bind:checked`).
        checked "checked": bool = Bool(false) ; Bool, Paint, bindable;
        /// A radio item of this group (non-empty).
        group "group": String = Str("") ; Str, Layout;
    }
}

pub mod separator_props {
    use crate::kind_props;
    kind_props! {
        /// `menu-separator`'s props.
        pub struct SeparatorProps;
        visible "visible": bool = Bool(true) ; Bool, Structure;
    }
}

pub static MENU_SPEC: KindSpec = KindSpec {
    tag: "menu",
    props: menu_props::MenuProps::SPECS,
    signals: &[],
    slots: &[],
    children: true,
    role: accesskit::Role::MenuItem,
    name_from: NameFrom::Prop("label"),
    text_prop: None,
    focusable: false,
};

pub static MENU_ITEM_SPEC: KindSpec = KindSpec {
    tag: "menu-item",
    props: item_props::ItemProps::SPECS,
    signals: &[],
    slots: &[],
    children: false,
    role: accesskit::Role::MenuItem,
    name_from: NameFrom::Prop("label"),
    text_prop: Some("label"),
    focusable: false,
};

pub static MENU_SEPARATOR_SPEC: KindSpec = KindSpec {
    tag: "menu-separator",
    props: separator_props::SeparatorProps::SPECS,
    signals: &[],
    slots: &[],
    children: false,
    role: accesskit::Role::GenericContainer,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// The entry's AccessKit properties (`menu`, `menu-item` and `menu-separator` share them).
pub fn entry_node(e: &Entry, node: &mut accesskit::Node) {
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

/// `menu`: a bar title or a submenu row. The root builds its [`MenuModel`] from its entry children
/// (the tree walk is presto-core's; [`MenuModel::add`] takes their [`Entry`]s).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuKind {
    pub props: menu_props::MenuProps,
    pub entry: Entry,
}

/// `menu-item`: an action, check or radio row.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItemKind {
    pub props: item_props::ItemProps,
    pub entry: Entry,
}

/// `menu-separator`.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuSeparatorKind {
    pub props: separator_props::SeparatorProps,
    pub entry: Entry,
}

impl Default for MenuKind {
    fn default() -> Self {
        MenuKind { props: Default::default(), entry: Entry::menu("", "") }
    }
}
impl Default for MenuItemKind {
    fn default() -> Self {
        MenuItemKind { props: Default::default(), entry: Entry::item("", "") }
    }
}
impl Default for MenuSeparatorKind {
    fn default() -> Self {
        MenuSeparatorKind { props: Default::default(), entry: Entry::separator() }
    }
}

impl MenuKind {
    fn sync(&mut self) {
        self.entry.set_label(&self.props.label);
        self.entry.enabled = self.props.enabled;
        self.entry.visible = self.props.visible;
    }
}

impl MenuItemKind {
    fn sync(&mut self) {
        let p = &self.props;
        self.entry.set_label(&p.label);
        self.entry.shortcut = p.shortcut.clone();
        self.entry.enabled = p.enabled;
        self.entry.visible = p.visible;
        self.entry.checked = p.checked;
        self.entry.check = if !p.group.is_empty() {
            Check::Radio(p.group.clone())
        } else if p.checkable {
            Check::Check
        } else {
            Check::None
        };
    }
}

impl MenuSeparatorKind {
    fn sync(&mut self) {
        self.entry.visible = self.props.visible;
    }
}

macro_rules! entry_kind {
    ($t:ty, $spec:expr) => {
        impl Kind for $t {
            fn spec() -> &'static KindSpec {
                &$spec
            }
            fn kind_spec(&self) -> &'static KindSpec {
                &$spec
            }
            fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
                self.props.set(prop, v)?;
                self.sync();
                Ok(())
            }
            fn get_prop(&self, prop: PropId) -> Option<Value> {
                self.props.get(prop)
            }
            /// Drawn by its root's levels.
            fn record(&self, _cx: &mut RecordCx) {}
            fn a11y(&self, cx: &mut A11yCx) {
                entry_node(&self.entry, cx.node)
            }
        }
    };
}
entry_kind!(MenuKind, MENU_SPEC);
entry_kind!(MenuItemKind, MENU_ITEM_SPEC);
entry_kind!(MenuSeparatorKind, MENU_SEPARATOR_SPEC);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Native, Policy};
    use crate::shim::testing::{MemResources, MonoShaper};
    use presto_base::change::ChangeClass;
    use presto_base::event::{KeyEvent, KeyLocation, PointerType};

    fn ms(n: u64) -> Time {
        Time(n * 1_000_000)
    }

    fn policy(os: Platform) -> Policy {
        *Native::for_os(os).policy()
    }

    fn styles() -> Styles<'static> {
        Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true }
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
        b.prepare(&mut MonoShaper, &styles());
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
        Event::Key(KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() })
    }

    fn center(r: Rect) -> Point {
        Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
    }

    fn row(b: &Menus, k: usize, id: &str) -> Rect {
        let ix = b.model.find(id).unwrap();
        b.levels()[k].rows.iter().find(|(r, _)| *r == ix).unwrap().1
    }

    type Signals = Vec<(&'static str, Vec<Value>)>;

    /// Sends `ev` to `k` at `now`; returns what it handled, the signals by name and its wake-up.
    fn send_to<K: Kind>(k: &mut K, policy: &Policy, now: Time, size: Size, ev: Event) -> (Handled, Signals, Option<Time>) {
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(now, size, &mut state, policy);
        let h = k.event(&mut cx, &ev);
        let spec = k.kind_spec();
        let sig = cx.emitted.iter().map(|e| (spec.signals[e.signal.0 as usize].name, e.args.clone())).collect();
        (h, sig, cx.wake)
    }

    fn send(b: &mut MenuBar, policy: &Policy, now: Time, ev: Event) -> (Handled, Signals, Option<Time>) {
        let out = send_to(b, policy, now, Size::new(800.0, 30.0), ev);
        prepare(&mut b.menus);
        out
    }

    #[test]
    fn levels_are_placed_below_titles_and_beside_rows() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = policy(Platform::Linux);
        let file_title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(file_title)));
        assert!(b.menus.nav.is_open());
        let l0 = b.menus.levels()[0].rect;
        assert_eq!(l0.min, Point::new(file_title.min.x, file_title.max.y + 2.0));
        assert!(l0.width() >= 160.0, "the surface's min-width");
        let rec = row(&b.menus, 0, "recent");
        assert_eq!(rec.height(), 26.0, "the item's height");
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Down, center(rec)));
        let l1 = b.menus.levels()[1].rect;
        assert_eq!(l1.min.x, rec.max.x, "right-start beside its row");
        assert_eq!(l1.min.y, rec.min.y - 4.0, "its first row level with the opener");
    }

    #[test]
    fn hovering_switches_bar_menus_while_one_is_open() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = policy(Platform::Windows);
        let [(file, f), (edit, e)] = [b.menus.titles()[0], b.menus.titles()[1]];
        // hovering a title with nothing open opens nothing
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Move, center(e)));
        assert!(!b.menus.nav.is_open());
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(f)));
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Up, center(f)));
        assert_eq!(b.menus.nav.levels[0].container, file, "a release on the title is not a pick and keeps it open");
        let (_, sig, _) = send(&mut b, &policy, ms(5), ptr(PointerPhase::Move, center(e)));
        assert_eq!(b.menus.nav.levels[0].container, edit);
        assert!(sig.iter().any(|s| s.0 == "close") && sig.iter().any(|s| s.0 == "open"));
        // pressing the open title closes it
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Down, center(e)));
        assert!(!b.menus.nav.is_open());
    }

    #[test]
    fn submenus_open_after_the_delay_and_the_safe_triangle_holds() {
        let mut b = bar();
        prepare(&mut b.menus);
        let mut policy = policy(Platform::Linux);
        policy.submenu_delay_ms = 225;
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
        let policy = policy(Platform::Macos);
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
        let policy = policy(Platform::Windows);
        let title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(title)));
        let save = row(&b.menus, 0, "save");
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Move, center(save)));
        let (_, sig, _) = send(&mut b, &policy, ms(20), ptr(PointerPhase::Up, center(save)));
        assert!(!b.menus.nav.is_open());
        let act = sig.iter().find(|s| s.0 == "activate").unwrap();
        assert_eq!(act.1[0], Value::str("save"));
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
        let policy = policy(Platform::Linux);
        assert!(b.claims_key(Key::F10, Modifiers::default()));
        send(&mut b, &policy, ms(0), key(Key::F10));
        assert!(b.menus.nav.bar_keys);
        assert!(b.claims_key(Key::ArrowDown, Modifiers::default()));
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
        assert!(sig.iter().any(|s| s.0 == "activate" && s.1[0] == Value::str("undo")));
    }

    #[test]
    fn an_outside_press_light_dismisses_the_open_menus() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = policy(Platform::Windows);
        assert!(!b.light_dismiss(), "nothing open: no outside presses wanted");
        let title = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(title)));
        send(&mut b, &policy, ms(5), ptr(PointerPhase::Up, center(title)));
        assert!(b.light_dismiss());
        let (h, sig, _) = send(&mut b, &policy, ms(10), Event::OutsidePress(Point::new(-50.0, -50.0)));
        assert_eq!(h, Handled::No, "the press goes on to what is under it");
        assert!(!b.menus.nav.is_open());
        assert!(sig.iter().any(|s| s.0 == "close"));
        assert!(!b.light_dismiss());
        // a context menu: closing writes `open` back
        let mut c = ContextMenu::default();
        c.menus.model.add(MenuModel::ROOT, Entry::item("cut", "Cu&t"));
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(ms(0), Size::new(400.0, 300.0), &mut state, &policy);
        c.context_click(&mut cx, Point::new(10.0, 10.0));
        assert!(c.light_dismiss());
        let mut cx = EventCx::new(ms(1), Size::new(400.0, 300.0), &mut state, &policy);
        c.event(&mut cx, &Event::OutsidePress(Point::new(-5.0, 0.0)));
        assert!(!c.props.open);
        assert_eq!(cx.changed, vec![context_props::P::open.id()]);
    }

    #[test]
    fn context_menu_opens_at_the_point_and_flips_at_the_edge() {
        let mut c = ContextMenu::default();
        c.menus.model.add(MenuModel::ROOT, Entry::item("cut", "Cu&t").shortcut("Primary+X"));
        c.menus.model.add(MenuModel::ROOT, Entry::item("copy", "&Copy"));
        c.menus.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let policy = policy(Platform::Windows);
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(ms(0), Size::new(400.0, 300.0), &mut state, &policy);
        c.context_click(&mut cx, Point::new(390.0, 290.0));
        let open = CONTEXT_MENU_SPEC.signal("open").unwrap();
        assert!(cx.emitted.iter().any(|s| s.signal == open));
        assert_eq!(cx.changed, vec![context_props::P::open.id()], "`bind:open` is told");
        assert!(c.props.open);
        prepare(&mut c.menus);
        let r = c.menus.levels()[0].rect;
        assert_eq!(r.max.y, 290.0 - 2.0, "flipped above the point");
        assert_eq!(r.max.x, 400.0, "shifted inside");
        // the release of the opening click right there is not a pick
        send_to(&mut c, &policy, ms(5), Size::new(400.0, 300.0), ptr(PointerPhase::Up, Point::new(390.0, 290.0)));
        assert!(c.menus.nav.is_open());
        // Shift+F10 opens it from the keyboard with its first row current
        let mut c2 = ContextMenu::default();
        c2.menus.model.add(MenuModel::ROOT, Entry::item("x", "x"));
        let ev = Event::Key(KeyEvent { key: Some(Key::F10), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::SHIFT });
        let (h, _, _) = send_to(&mut c2, &policy, ms(0), Size::new(10.0, 10.0), ev);
        assert_eq!(h, Handled::Yes);
        assert_eq!(c2.menus.nav.current(), c2.menus.model.find("x"));
    }

    #[test]
    fn levels_record_surface_rows_and_shortcuts() {
        let mut b = bar();
        prepare(&mut b.menus);
        let policy = policy(Platform::Linux);
        let f = b.menus.titles()[0].1;
        send(&mut b, &policy, ms(0), ptr(PointerPhase::Down, center(f)));
        let new = center(row(&b.menus, 0, "new"));
        send(&mut b, &policy, ms(10), ptr(PointerPhase::Move, new));
        let mut res = MemResources::default();
        let lists = b.menus.record_levels(&mut MonoShaper, &mut res, &styles(), 1.0);
        assert_eq!(lists.len(), 1);
        let kinds = testkit::kinds(&lists[0]);
        // the popup shadow and the surface, then the current row's fill
        assert_eq!(&kinds[..3], ["shadow", "quad", "quad"]);
        assert!(kinds.contains(&"line"), "the separator");
        assert!(kinds.iter().filter(|k| **k == "glyphs").count() >= 4, "labels and the shortcut");
        // the bar's open title has its fill
        let (bar_list, _) = testkit::record(&b, Size::new(800.0, 30.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&bar_list)[0], "quad");
    }

    #[test]
    fn entries_take_props_and_describe_themselves() {
        let mut it = MenuItemKind::default();
        let spec = MenuItemKind::spec();
        let label = spec.prop("label").unwrap();
        assert_eq!(spec.prop_spec(label).unwrap().change, ChangeClass::Layout);
        assert_eq!(spec.prop_spec(spec.prop("checked").unwrap()).unwrap().change, ChangeClass::Paint);
        assert!(spec.prop_spec(spec.prop("checked").unwrap()).unwrap().bindable);
        it.set_prop(label, &Value::str("&Wrap")).unwrap();
        it.set_prop(spec.prop("checkable").unwrap(), &Value::Bool(true)).unwrap();
        it.set_prop(spec.prop("checked").unwrap(), &Value::Bool(true)).unwrap();
        assert!(it.set_prop(label, &Value::Bool(true)).is_err(), "type-checked");
        assert!(spec.prop("nope").is_none());
        assert_eq!(it.entry.label, "Wrap");
        assert_eq!(it.entry.mnemonic, Some('w'));
        let node = testkit::a11y(&it, StateBits::NONE, Size::new(100.0, 26.0));
        assert_eq!(node.role(), accesskit::Role::MenuItemCheckBox);
        assert_eq!(node.toggled(), Some(accesskit::Toggled::True));
        assert!(MenuSeparatorKind::spec().prop("label").is_none());
        let activate = &ROOT_SIGNALS[MENU_BAR_SPEC.signal("activate").unwrap().0 as usize];
        assert!(!activate.cancelable, "the pick is applied before listeners run");
    }

    #[test]
    fn towards_is_the_triangle_to_the_near_edge() {
        let sub = Rect::new(200.0, 0.0, 300.0, 200.0);
        assert!(towards(Point::new(150.0, 105.0), Vec2::new(10.0, 5.0), sub));
        assert!(!towards(Point::new(150.0, 105.0), Vec2::new(-10.0, 5.0), sub), "moving away");
        assert!(!towards(Point::new(150.0, 105.0), Vec2::ZERO, sub));
    }
}
