//! `tab-view` and `tab`: a strip of tab captions over the selected page, drawn by skelter (svelters-egui
//! `views/tabs.rs` used egui's `ScrollArea` and selectable buttons; this is the own version, OW5c).
//!
//! Each child of a `tab-view` is a page; a `tab` names it with `title` (any other child with its
//! accessible name, else "Tab n": [`caption_of`]). Only the selected page is laid out and drawn
//! ([`TabView::page_rect`]). The strip:
//!
//! - **Selection:** a primary press on a caption selects it (as QTabBar and GTK notebooks do);
//!   `changed(selected)` reports a user pick, never a JS write. Focused, Left/Right/Home/End move the
//!   selection, Ctrl+Tab / Ctrl+Shift+Tab and Ctrl+PageDown / Ctrl+PageUp cycle it.
//! - **Closing:** a `closable` tab has a close button; a primary click on it (press and release on
//!   the button) or a middle click on the tab raises `close-requested` on the tab. JS removes it.
//! - **Overflow:** captions wider than the strip scroll horizontally (the wheel, and the scroll
//!   buttons at the strip's end, one caption per press, as Qt's tab bar does); a selection change
//!   scrolls the selected caption into view. The overflow button next to them opens a menu listing
//!   every tab (VS Code's "Show opened editors"), the shown one checked; picking a row selects it.
//!
//! [`TabView`] is the kind's native state (selection, hover, press, scroll); [`TabStrip`] is the
//! caption geometry, rebuilt when captions, look or width change.

pub mod kinds;

use crate::table::shim::{self, Emit, HAlign, TextPainter, TextStyle, Value};
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{DisplayList, Prim};
use skelter_base::event::{KeyEvent, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, ScrollDelta, WheelEvent};
use skelter_base::geom::{Corners, Point, Rect, RoundedRect, Size};
use skelter_base::key::Key;

/// A page's caption.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabCaption {
    pub title: String,
    pub closable: bool,
}

/// The caption of page `k`: a `tab`'s `title` (when it is a `tab`), else the child's accessible
/// name, else "Tab n".
pub fn caption_of(k: usize, tab_title: Option<&str>, closable: bool, accessible_name: &str) -> TabCaption {
    match tab_title {
        Some(t) if !t.is_empty() => TabCaption { title: t.to_string(), closable },
        Some(_) => TabCaption { title: format!("Tab {}", k + 1), closable },
        None if !accessible_name.is_empty() => TabCaption { title: accessible_name.to_string(), closable: false },
        None => TabCaption { title: format!("Tab {}", k + 1), closable: false },
    }
}

/// The strip's colours and metrics (from the computed style of the tab-view and its parts).
#[derive(Clone, Debug, PartialEq)]
pub struct TabLook {
    pub text: TextStyle,
    pub selected_fg: Color,
    pub bar_bg: Color,
    pub hover_bg: Color,
    pub pressed_bg: Color,
    pub selected_bg: Color,
    /// The selected caption's bottom line.
    pub indicator: Color,
    pub indicator_width: f32,
    /// The line under the strip.
    pub separator: Color,
    pub close_hover_bg: Color,
    pub focus: Color,
    pub disabled_fg: Color,
    pub pad_x: f32,
    pub pad_y: f32,
    /// Space between captions.
    pub gap: f32,
    /// The close button's square side.
    pub close_size: f32,
    pub radius: f32,
    pub min_tab_width: f32,
    pub max_tab_width: f32,
    /// The strip's least height (`tab-h`, VS Code's 35 pt tabs), the indicator included.
    pub min_height: f32,
    /// The overflow menu's surface and its current row.
    pub menu_bg: Color,
    pub menu_border: Color,
    pub menu_sel: Color,
    pub menu_sel_fg: Color,
    /// The current row's outline (VS Code's `menu.selectionBorder`: the high-contrast themes'
    /// active border, where the selection fill alone does not show).
    pub menu_sel_border: Option<Color>,
}

impl Default for TabLook {
    fn default() -> Self {
        let rgb = |r, g, b| Color::from_rgba8(r, g, b, 255);
        TabLook {
            text: TextStyle::default(),
            selected_fg: Color::BLACK,
            bar_bg: Color::TRANSPARENT,
            hover_bg: Color::from_rgba8(0, 0, 0, 20),
            pressed_bg: Color::from_rgba8(0, 0, 0, 36),
            selected_bg: Color::TRANSPARENT,
            indicator: rgb(0, 95, 184),
            indicator_width: 2.0,
            separator: rgb(212, 212, 212),
            close_hover_bg: Color::from_rgba8(0, 0, 0, 28),
            focus: rgb(0, 95, 184),
            disabled_fg: rgb(150, 150, 150),
            pad_x: 10.0,
            pad_y: 5.0,
            gap: 2.0,
            close_size: 16.0,
            radius: 4.0,
            min_tab_width: 40.0,
            max_tab_width: 240.0,
            min_height: 0.0,
            menu_bg: rgb(255, 255, 255),
            menu_border: rgb(204, 204, 204),
            menu_sel: rgb(0, 96, 192),
            menu_sel_fg: rgb(255, 255, 255),
            menu_sel_border: None,
        }
    }
}

/// One caption's geometry in strip coordinates (before scrolling).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabGeom {
    pub rect: Rect,
    /// The title's box (elided to it).
    pub label: Rect,
    pub close: Option<Rect>,
}

/// The captions laid out in a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabStrip {
    pub tabs: Vec<TabGeom>,
    /// The strip's height (the bar).
    pub height: f32,
    /// The captions' total width.
    pub content_width: f32,
}

impl TabStrip {
    /// Lays `captions` out left to right: each is `pad_x + title (+ gap + close) + pad_x` wide,
    /// within `min_tab_width..=max_tab_width` (a longer title is elided).
    pub fn layout(captions: &[TabCaption], look: &TabLook, text: &mut dyn TextPainter) -> TabStrip {
        let line = text.measure("Ag", &look.text).h;
        let height = ((line + 2.0 * look.pad_y).max(look.close_size + 2.0 * look.pad_y) + look.indicator_width).max(look.min_height);
        let mut x = 0.0;
        let mut tabs = Vec::with_capacity(captions.len());
        for c in captions {
            let tw = text.measure(&c.title, &look.text).w;
            let close_w = if c.closable { look.gap + look.close_size } else { 0.0 };
            let w = (look.pad_x * 2.0 + tw + close_w).clamp(look.min_tab_width, look.max_tab_width.max(look.min_tab_width));
            let rect = Rect::new(x, 0.0, x + w, height);
            let label_max = rect.max.x - look.pad_x - close_w;
            let label = Rect::new(rect.min.x + look.pad_x, 0.0, label_max.max(rect.min.x + look.pad_x), height - look.indicator_width);
            let close = c.closable.then(|| {
                let cx = rect.max.x - look.pad_x - look.close_size;
                let cy = (height - look.indicator_width - look.close_size) / 2.0;
                Rect::new(cx, cy, cx + look.close_size, cy + look.close_size)
            });
            tabs.push(TabGeom { rect, label, close });
            x += w + look.gap;
        }
        let content_width = if tabs.is_empty() { 0.0 } else { x - look.gap };
        TabStrip { tabs, height, content_width }
    }
}

/// The buttons at the end of an overflowing strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctl {
    /// Scrolls one caption back.
    Left,
    /// Scrolls one caption on.
    Right,
    /// Opens the menu of every tab.
    Menu,
}

/// What the pointer is over in the strip (or its overflow menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabHit {
    Tab(usize),
    Close(usize),
    Ctl(Ctl),
    /// A row of the open overflow menu.
    Item(usize),
}

impl TabHit {
    fn tab(self) -> Option<usize> {
        match self {
            TabHit::Tab(k) | TabHit::Close(k) => Some(k),
            _ => None,
        }
    }
}

/// The overflow menu's row metrics (from the last relayout).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MenuGeom {
    row_h: f32,
    width: f32,
    pad_x: f32,
}

/// The overflow menu's inner padding above the first and below the last row.
const MENU_PAD: f32 = 4.0;
/// Room for the check mark before a row's title.
const MENU_CHECK: f32 = 18.0;
/// The gap between the strip and the menu.
const MENU_GAP: f32 = 2.0;

/// A `tab-view`'s native state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabView {
    /// The `selected` prop (unclamped: [`TabView::shown`] clamps it to the pages).
    pub selected: i64,
    pub enabled: bool,
    /// Keyboard focus is on the strip, and whether it came from the keyboard (`:focus-visible`).
    pub focused: bool,
    pub focus_visible: bool,
    strip: TabStrip,
    width: f32,
    scroll: f32,
    hover: Option<TabHit>,
    pressed: Option<(TabHit, MouseButton)>,
    /// The overflow menu is open, and its current row (the pointer's or the arrow keys').
    menu_open: bool,
    menu_cur: Option<usize>,
    menu: MenuGeom,
    /// The page last scrolled into view: a new selection (a JS write before the strip was laid
    /// out, too) is revealed at the next relayout; a wheel scroll away from it stays.
    revealed: Option<usize>,
    /// The strip's direction is `rtl`: the geometry is laid out left to right and mirrored in the
    /// strip where it meets the pointer, the paint and AccessKit ([`TabView::flip`]); captions run
    /// from the right, the scroll and overflow buttons sit at the left (swapped), the overflow
    /// menu opens under them, and the arrow keys swap.
    pub rtl: bool,
}

impl TabView {
    pub fn new() -> TabView {
        TabView { enabled: true, ..TabView::default() }
    }

    /// The shown page for `n` pages: `selected` clamped, `None` without pages.
    pub fn shown(&self, n: usize) -> Option<usize> {
        (n > 0).then(|| self.selected.clamp(0, n as i64 - 1) as usize)
    }

    pub fn strip(&self) -> &TabStrip {
        &self.strip
    }

    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// The strip's height, for layout.
    pub fn bar_height(&self) -> f32 {
        self.strip.height
    }

    /// The natural size: the shown page's plus the bar, at least as wide as `min_width`.
    pub fn natural(&self, page: Size, min_width: f32) -> Size {
        Size::new(page.w.max(min_width), page.h + self.strip.height)
    }

    /// Where the shown page goes in a tab-view of `size`.
    pub fn page_rect(&self, size: Size) -> Rect {
        Rect::new(0.0, self.strip.height, size.w, size.h.max(self.strip.height))
    }

    /// A JS write of `selected`: no `changed`, but the caption scrolls into view.
    pub fn set_selected(&mut self, selected: i64) {
        self.selected = selected;
        if let Some(k) = self.shown(self.strip.tabs.len()) {
            self.reveal(k);
        }
    }

    /// Rebuilds the caption geometry for `captions` in a strip `width` wide; hover and press on
    /// captions that no longer exist are dropped.
    pub fn relayout(&mut self, captions: &[TabCaption], look: &TabLook, text: &mut dyn TextPainter, width: f32) {
        self.strip = TabStrip::layout(captions, look, text);
        self.width = width;
        let n = captions.len();
        let gone = |h: TabHit| h.tab().is_some_and(|t| t >= n) || matches!(h, TabHit::Item(k) if k >= n);
        if self.hover.is_some_and(gone) {
            self.hover = None;
        }
        if self.pressed.is_some_and(|(h, _)| gone(h)) {
            self.pressed = None;
        }
        let line = text.measure("Ag", &look.text).h;
        let widest = captions.iter().map(|c| text.measure(&c.title, &look.text).w).fold(0.0f32, f32::max);
        self.menu = MenuGeom { row_h: (line + 2.0 * look.pad_y).ceil(), width: (widest + MENU_CHECK + 3.0 * look.pad_x).clamp(120.0, 360.0), pad_x: look.pad_x };
        if !self.overflows() {
            self.menu_open = false;
        }
        if self.menu_cur.is_some_and(|k| k >= n) {
            self.menu_cur = None;
        }
        self.clamp_scroll();
        if let Some(k) = self.shown(n).filter(|&k| self.revealed != Some(k)) {
            self.reveal(k);
        }
    }

    /// The captions are wider than the strip: it shows its scroll and overflow buttons.
    pub fn overflows(&self) -> bool {
        self.strip.content_width > self.width + 0.5
    }

    /// The width of one button at the strip's end.
    fn ctl_w(&self) -> f32 {
        (self.strip.height * 0.9).round()
    }

    /// The width the captions show in (the strip less its buttons when it overflows).
    pub fn viewport(&self) -> f32 {
        if self.overflows() { (self.width - 3.0 * self.ctl_w()).max(0.0) } else { self.width }
    }

    /// A button at the strip's end (tab-view coordinates); `None` while the captions fit.
    pub fn ctl_rect(&self, c: Ctl) -> Option<Rect> {
        if !self.overflows() {
            return None;
        }
        let w = self.ctl_w();
        let k = match c {
            Ctl::Left => 0.0,
            Ctl::Right => 1.0,
            Ctl::Menu => 2.0,
        };
        let x = self.viewport() + k * w;
        Some(Rect::new(x, 0.0, x + w, self.strip.height))
    }

    /// The open overflow menu's surface (tab-view coordinates): under the overflow button, its
    /// right edge on the strip's.
    pub fn menu_rect(&self) -> Option<Rect> {
        if !self.menu_open {
            return None;
        }
        let h = 2.0 * MENU_PAD + self.strip.tabs.len() as f32 * self.menu.row_h;
        let y = self.strip.height + MENU_GAP;
        Some(Rect::new(self.width - self.menu.width, y, self.width, y + h))
    }

    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Row `k` of the open menu.
    fn menu_row(&self, k: usize) -> Option<Rect> {
        let m = self.menu_rect()?;
        let y = m.min.y + MENU_PAD + k as f32 * self.menu.row_h;
        Some(Rect::new(m.min.x + MENU_PAD, y, m.max.x - MENU_PAD, y + self.menu.row_h))
    }

    /// Opens or closes the overflow menu (its current row: the shown tab).
    pub fn set_menu(&mut self, open: bool) {
        self.menu_open = open && self.overflows();
        self.menu_cur = if self.menu_open { self.shown(self.strip.tabs.len()) } else { None };
    }

    /// One caption on (`forward`) or back: the next caption cut at that edge comes fully in.
    pub fn scroll_step(&mut self, forward: bool) -> bool {
        let before = self.scroll;
        let view = self.viewport();
        if forward {
            if let Some(t) = self.strip.tabs.iter().find(|t| t.rect.max.x > self.scroll + view + 0.5) {
                self.scroll = t.rect.max.x - view;
            }
        } else if let Some(t) = self.strip.tabs.iter().rev().find(|t| t.rect.min.x < self.scroll - 0.5) {
            self.scroll = t.rect.min.x;
        }
        self.clamp_scroll();
        self.scroll != before
    }

    fn max_scroll(&self) -> f32 {
        (self.strip.content_width - self.viewport()).max(0.0)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Scrolls caption `k` fully into view (its left edge wins when it is wider than the strip).
    fn reveal(&mut self, k: usize) {
        let Some(t) = self.strip.tabs.get(k) else { return };
        self.revealed = Some(k);
        let view = self.viewport();
        if t.rect.max.x - self.scroll > view {
            self.scroll = t.rect.max.x - view;
        }
        if t.rect.min.x - self.scroll < 0.0 {
            self.scroll = t.rect.min.x;
        }
        self.clamp_scroll();
    }

    /// `r` mirrored in the strip in `rtl` (the logical, left-to-right geometry → tab-view
    /// coordinates, and back).
    pub fn flip(&self, r: Rect) -> Rect {
        if self.rtl { Rect::new(self.width - r.max.x, r.min.y, self.width - r.min.x, r.max.y) } else { r }
    }

    fn flip_pt(&self, p: Point) -> Point {
        if self.rtl { Point::new(self.width - p.x, p.y) } else { p }
    }

    fn flip_align(&self, a: HAlign) -> HAlign {
        match (a, self.rtl) {
            (HAlign::Start, true) => HAlign::End,
            (HAlign::End, true) => HAlign::Start,
            (a, _) => a,
        }
    }

    /// What is under `p` (tab-view coordinates).
    pub fn hit(&self, p: Point) -> Option<TabHit> {
        let p = self.flip_pt(p);
        if self.menu_rect().is_some_and(|m| m.contains(p)) {
            return (0..self.strip.tabs.len()).find(|&k| self.menu_row(k).is_some_and(|r| r.contains(p))).map(TabHit::Item);
        }
        if p.y < 0.0 || p.y >= self.strip.height || p.x < 0.0 || p.x >= self.width {
            return None;
        }
        if let Some(c) = [Ctl::Left, Ctl::Right, Ctl::Menu].into_iter().find(|&c| self.ctl_rect(c).is_some_and(|r| r.contains(p))) {
            return Some(TabHit::Ctl(c));
        }
        if p.x >= self.viewport() {
            return None;
        }
        let q = Point::new(p.x + self.scroll, p.y);
        let k = self.strip.tabs.iter().position(|t| t.rect.contains(q))?;
        Some(match self.strip.tabs[k].close {
            Some(c) if c.contains(q) => TabHit::Close(k),
            _ => TabHit::Tab(k),
        })
    }

    /// The cursor over `p`.
    pub fn cursor(&self, p: Point) -> Option<CursorIcon> {
        self.hit(p).map(|_| CursorIcon::Default)
    }

    fn pick(&mut self, k: usize, out: &mut Vec<Emit>) {
        if self.shown(self.strip.tabs.len()) != Some(k) {
            self.selected = k as i64;
            out.push(Emit::new("changed", vec![Value::I64(k as i64)]));
        }
        self.reveal(k);
    }

    /// A pointer event in tab-view coordinates. Returns the tab-view's events (`changed`) and the
    /// pages whose `close-requested` fires, and whether the strip needs a repaint.
    pub fn pointer(&mut self, ev: &PointerEvent) -> TabOutput {
        let mut o = TabOutput::default();
        let hit = if self.enabled { self.hit(ev.pos) } else { None };
        match ev.phase {
            PointerPhase::Move | PointerPhase::Enter => {
                o.repaint = hit != self.hover;
                self.hover = hit;
                if let Some(TabHit::Item(k)) = hit {
                    o.repaint |= self.menu_cur != Some(k);
                    self.menu_cur = Some(k);
                }
            }
            PointerPhase::Leave => {
                o.repaint = self.hover.is_some();
                self.hover = None;
            }
            PointerPhase::Cancel => {
                o.repaint = self.pressed.is_some();
                self.pressed = None;
            }
            PointerPhase::Down => {
                let (Some(h), Some(b)) = (hit, ev.button) else { return o };
                self.pressed = Some((h, b));
                o.repaint = true;
                if b == MouseButton::Primary {
                    match h {
                        TabHit::Tab(k) => {
                            self.pick(k, &mut o.events);
                            o.take_focus = true;
                        }
                        TabHit::Ctl(Ctl::Left) => {
                            self.scroll_step(false);
                        }
                        TabHit::Ctl(Ctl::Right) => {
                            self.scroll_step(true);
                        }
                        TabHit::Ctl(Ctl::Menu) => {
                            self.set_menu(!self.menu_open);
                            o.take_focus = true;
                        }
                        TabHit::Item(k) => {
                            self.set_menu(false);
                            self.pick(k, &mut o.events);
                            o.take_focus = true;
                        }
                        TabHit::Close(_) => {}
                    }
                }
            }
            PointerPhase::Up => {
                let Some((h, b)) = self.pressed.take() else { return o };
                o.repaint = true;
                if Some(b) != ev.button {
                    return o;
                }
                match (h, b) {
                    (TabHit::Close(k), MouseButton::Primary) if hit == Some(TabHit::Close(k)) => o.close.push(k),
                    (TabHit::Tab(t) | TabHit::Close(t), MouseButton::Middle) if hit.and_then(TabHit::tab) == Some(t) && self.closable(t) => o.close.push(t),
                    _ => {}
                }
            }
        }
        o
    }

    fn closable(&self, k: usize) -> bool {
        self.strip.tabs.get(k).is_some_and(|t| t.close.is_some())
    }

    /// The wheel over the strip scrolls it horizontally when the captions overflow. Returns whether
    /// it scrolled (the event is consumed).
    pub fn wheel(&mut self, ev: &WheelEvent) -> bool {
        if self.max_scroll() <= 0.0 {
            return false;
        }
        let d = match ev.delta {
            ScrollDelta::Lines(v) => {
                let l = if v.x != 0.0 { v.x } else { v.y };
                l * 40.0
            }
            ScrollDelta::Pixels(v) => {
                if v.x != 0.0 {
                    v.x
                } else {
                    v.y
                }
            }
            ScrollDelta::Pages(v) => (if v.x != 0.0 { v.x } else { v.y }) * self.width,
        };
        let before = self.scroll;
        let horizontal = match ev.delta {
            ScrollDelta::Lines(v) | ScrollDelta::Pixels(v) | ScrollDelta::Pages(v) => v.x != 0.0,
        };
        // `rtl`: the content runs leftward, so a horizontal wheel moves it the other way
        let d = if self.rtl && horizontal { -d } else { d };
        self.scroll -= d;
        self.clamp_scroll();
        self.scroll != before
    }

    /// A key on the focused strip: arrows, Home and End move the selection; Ctrl+Tab and
    /// Ctrl+PageDown (Shift / PageUp backwards) cycle it. Returns the output, `handled` when the
    /// key was taken.
    pub fn key(&mut self, ev: &KeyEvent) -> TabOutput {
        let mut o = TabOutput::default();
        let n = self.strip.tabs.len();
        if ev.state != KeyState::Pressed || !self.enabled || n == 0 {
            return o;
        }
        let cur = self.shown(n).unwrap_or(0);
        if self.menu_open {
            // the overflow menu has the keys: Up/Down move its row, Enter or Space picks, Escape closes
            let row = self.menu_cur.unwrap_or(cur);
            match ev.key {
                Some(Key::ArrowDown) => self.menu_cur = Some((row + 1).min(n - 1)),
                Some(Key::ArrowUp) => self.menu_cur = Some(row.saturating_sub(1)),
                Some(Key::Home) => self.menu_cur = Some(0),
                Some(Key::End) => self.menu_cur = Some(n - 1),
                Some(Key::Enter) | Some(Key::Space) => {
                    self.set_menu(false);
                    self.pick(row, &mut o.events);
                }
                Some(Key::Escape) => self.set_menu(false),
                _ => return o,
            }
            o.handled = true;
            o.repaint = true;
            return o;
        }
        let ctrl = ev.modifiers.contains(Modifiers::CTRL);
        let shift = ev.modifiers.contains(Modifiers::SHIFT);
        let cycle = |d: i64| ((cur as i64 + d).rem_euclid(n as i64)) as usize;
        let to = match ev.key {
            Some(Key::Tab) if ctrl => Some(cycle(if shift { -1 } else { 1 })),
            Some(Key::PageDown) if ctrl => Some(cycle(1)),
            Some(Key::PageUp) if ctrl => Some(cycle(-1)),
            Some(Key::ArrowRight) if self.focused => Some(if self.rtl { cur.saturating_sub(1) } else { (cur + 1).min(n - 1) }),
            Some(Key::ArrowLeft) if self.focused => Some(if self.rtl { (cur + 1).min(n - 1) } else { cur.saturating_sub(1) }),
            Some(Key::Home) if self.focused => Some(0),
            Some(Key::End) if self.focused => Some(n - 1),
            _ => None,
        };
        if let Some(k) = to {
            o.handled = true;
            o.repaint = true;
            self.focus_visible = self.focused;
            self.pick(k, &mut o.events);
        }
        o
    }

    /// Records the strip: the bar, each visible caption (hover, press and selection fills, the
    /// title, the close button), the selected caption's line, the line under the strip and the
    /// focus ring. Pages record themselves.
    pub fn record(&self, captions: &[TabCaption], look: &TabLook, text: &mut dyn TextPainter, list: &mut DisplayList) {
        let h = self.strip.height;
        let full = Rect::new(0.0, 0.0, self.width, h);
        shim::fill(list, full, look.bar_bg, 0.0);
        shim::line(list, Point::new(0.0, h - 0.5), Point::new(self.width, h - 0.5), 1.0, look.separator);
        self.record_ctls(look, list);
        let bar = self.flip(Rect::new(0.0, 0.0, self.viewport(), h));
        list.push(Prim::PushClip(RoundedRect { rect: bar, radii: Corners::ZERO }), bar);
        let shown = self.shown(captions.len());
        for (k, (c, g)) in captions.iter().zip(&self.strip.tabs).enumerate() {
            let dx = skelter_base::geom::Vec2::new(-self.scroll, 0.0);
            let r = g.rect.translate(dx);
            if r.max.x <= 0.0 || r.min.x >= self.viewport() {
                continue;
            }
            let on = shown == Some(k);
            let hovered = self.hover.and_then(TabHit::tab) == Some(k);
            let pressed = self.pressed.is_some_and(|(h, b)| h == TabHit::Tab(k) && b == MouseButton::Primary);
            let r = self.flip(r);
            let body = Rect::new(r.min.x, r.min.y, r.max.x, r.max.y - look.indicator_width);
            let bg = if pressed {
                look.pressed_bg
            } else if on {
                look.selected_bg
            } else if hovered && self.enabled {
                look.hover_bg
            } else {
                Color::TRANSPARENT
            };
            shim::fill(list, body, bg, look.radius);
            let fg = if !self.enabled {
                look.disabled_fg
            } else if on {
                look.selected_fg
            } else {
                look.text.color
            };
            text.draw(list, &c.title, &look.text.with_color(fg), self.flip(g.label.translate(dx)), self.flip_align(HAlign::Start), true);
            if let Some(cr) = g.close {
                let cr = self.flip(cr.translate(dx));
                if self.hover == Some(TabHit::Close(k)) && self.enabled {
                    shim::fill(list, cr, look.close_hover_bg, look.radius);
                }
                // the × glyph: two strokes over the middle half of the button
                let m = cr.width() * 0.3;
                let (a, b) = (Point::new(cr.min.x + m, cr.min.y + m), Point::new(cr.max.x - m, cr.max.y - m));
                shim::line(list, a, b, 1.2, fg);
                shim::line(list, Point::new(a.x, b.y), Point::new(b.x, a.y), 1.2, fg);
            }
            if on {
                let y = r.max.y - look.indicator_width / 2.0;
                // a disabled strip's marker dims with its captions
                let marker = if self.enabled { look.indicator } else { look.disabled_fg };
                shim::line(list, Point::new(r.min.x, y), Point::new(r.max.x, y), look.indicator_width, marker);
                if self.focused && self.focus_visible {
                    let ring = body.inflate(-1.0);
                    shim::frame(list, ring, Color::TRANSPARENT, look.focus, 1.0, look.radius);
                }
            }
        }
        list.push(Prim::PopClip, Rect::ZERO);
    }

    /// The scroll and overflow buttons of an overflowing strip: chevrons, dimmed where there is
    /// nothing more to scroll to, a hover fill, the overflow button pressed while its menu is open.
    fn record_ctls(&self, look: &TabLook, list: &mut DisplayList) {
        for c in [Ctl::Left, Ctl::Right, Ctl::Menu] {
            let Some(r) = self.ctl_rect(c) else { continue };
            let r = self.flip(r);
            let body = Rect::new(r.min.x + 1.0, r.min.y + 2.0, r.max.x - 1.0, r.max.y - look.indicator_width - 2.0);
            let live = self.enabled
                && match c {
                    Ctl::Left => self.scroll > 0.5,
                    Ctl::Right => self.scroll < self.max_scroll() - 0.5,
                    Ctl::Menu => true,
                };
            if c == Ctl::Menu && self.menu_open {
                shim::fill(list, body, look.pressed_bg, look.radius);
            } else if live && self.hover == Some(TabHit::Ctl(c)) {
                shim::fill(list, body, look.close_hover_bg, look.radius);
            }
            let fg = if live { look.text.color } else { look.disabled_fg };
            let (cx, cy) = ((body.min.x + body.max.x) / 2.0, (body.min.y + body.max.y) / 2.0);
            let s = 3.5;
            // `rtl` swaps the scroll buttons' chevrons with their places (back points right)
            let c_shape = match (c, self.rtl) {
                (Ctl::Left, true) => Ctl::Right,
                (Ctl::Right, true) => Ctl::Left,
                (c, _) => c,
            };
            let pts = match c_shape {
                Ctl::Left => [Point::new(cx + s * 0.5, cy - s), Point::new(cx - s * 0.5, cy), Point::new(cx + s * 0.5, cy + s)],
                Ctl::Right => [Point::new(cx - s * 0.5, cy - s), Point::new(cx + s * 0.5, cy), Point::new(cx - s * 0.5, cy + s)],
                Ctl::Menu => [Point::new(cx - s, cy - s * 0.5), Point::new(cx, cy + s * 0.5), Point::new(cx + s, cy - s * 0.5)],
            };
            shim::line(list, pts[0], pts[1], 1.3, fg);
            shim::line(list, pts[1], pts[2], 1.3, fg);
        }
    }

    /// Records the open overflow menu (tab-view coordinates): its surface, a row per tab (the
    /// current row filled, the shown tab checked). Nothing while it is closed.
    pub fn record_menu(&self, captions: &[TabCaption], look: &TabLook, text: &mut dyn TextPainter, list: &mut DisplayList) {
        let Some(m) = self.menu_rect() else { return };
        let m = self.flip(m);
        shim::frame(list, m, look.menu_bg, look.menu_border, 1.0, look.radius);
        let shown = self.shown(captions.len());
        for (k, c) in captions.iter().enumerate() {
            let Some(lr) = self.menu_row(k) else { continue };
            let r = self.flip(lr);
            let cur = self.menu_cur == Some(k);
            if cur {
                match look.menu_sel_border {
                    Some(b) => shim::frame(list, r, look.menu_sel, b, 1.0, look.radius),
                    None => shim::fill(list, r, look.menu_sel, look.radius),
                }
            }
            let fg = if cur { look.menu_sel_fg } else { look.text.color };
            if shown == Some(k) {
                // a check mark in the row's first column
                // the mark's place mirrors, its shape does not (10 pt wide)
                let x = self.flip(Rect::new(lr.min.x + self.menu.pad_x * 0.6, 0.0, lr.min.x + self.menu.pad_x * 0.6 + 10.0, 0.0)).min.x;
                let y = (r.min.y + r.max.y) / 2.0;
                shim::line(list, Point::new(x, y), Point::new(x + 3.5, y + 3.5), 1.3, fg);
                shim::line(list, Point::new(x + 3.5, y + 3.5), Point::new(x + 10.0, y - 4.0), 1.3, fg);
            }
            let label = self.flip(Rect::new(lr.min.x + self.menu.pad_x * 0.6 + MENU_CHECK, lr.min.y, lr.max.x - self.menu.pad_x, lr.max.y));
            text.draw(list, &c.title, &look.text.with_color(fg), label, self.flip_align(HAlign::Start), true);
        }
    }

    /// The captions' AccessKit nodes (role `Tab`, selected state, bounds in tab-view coordinates);
    /// the tab-view itself is a `TabList`.
    pub fn a11y(&self, captions: &[TabCaption]) -> Vec<accesskit::Node> {
        let shown = self.shown(captions.len());
        captions
            .iter()
            .zip(&self.strip.tabs)
            .enumerate()
            .map(|(k, (c, g))| {
                let mut n = accesskit::Node::new(accesskit::Role::Tab);
                n.set_label(c.title.clone());
                n.set_selected(shown == Some(k));
                n.set_position_in_set(k + 1);
                n.set_size_of_set(captions.len());
                let r = self.flip(g.rect.translate(skelter_base::geom::Vec2::new(-self.scroll, 0.0)));
                n.set_bounds(accesskit::Rect { x0: r.min.x as f64, y0: r.min.y as f64, x1: r.max.x as f64, y1: r.max.y as f64 });
                n.add_action(accesskit::Action::Click);
                if !self.enabled {
                    n.set_disabled();
                }
                n
            })
            .collect()
    }
}

/// What an input did to a [`TabView`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabOutput {
    /// The tab-view's events (`changed`).
    pub events: Vec<Emit>,
    /// Pages whose tab raises `close-requested`.
    pub close: Vec<usize>,
    /// The tab-view takes keyboard focus (a press on a caption).
    pub take_focus: bool,
    pub handled: bool,
    pub repaint: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::shim::testing::FixedText;
    use skelter_base::event::{KeyLocation, PointerType};
    use skelter_base::geom::Vec2;

    fn caps(n: usize, closable: bool) -> Vec<TabCaption> {
        (0..n).map(|k| TabCaption { title: format!("Page {k}"), closable }).collect()
    }

    fn view(captions: &[TabCaption], width: f32) -> TabView {
        let mut v = TabView::new();
        v.relayout(captions, &TabLook::default(), &mut FixedText::default(), width);
        v
    }

    fn ptr(phase: PointerPhase, button: Option<MouseButton>, p: Point) -> PointerEvent {
        PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons: 0, pos: p, modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: Default::default() }
    }

    fn key(k: Key, m: Modifiers) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

    fn centre(v: &TabView, k: usize) -> Point {
        let r = v.strip().tabs[k].rect;
        Point::new((r.min.x + r.max.x) / 2.0 - v.scroll(), r.height() / 2.0)
    }

    fn close_centre(v: &TabView, k: usize) -> Point {
        let r = v.strip().tabs[k].close.unwrap();
        Point::new((r.min.x + r.max.x) / 2.0 - v.scroll(), (r.min.y + r.max.y) / 2.0)
    }

    #[test]
    fn rtl_mirrors_the_strip_its_buttons_and_the_arrows() {
        let c = caps(10, false);
        let mut v = view(&c, 200.0);
        v.rtl = true;
        assert!(v.overflows());
        // caption 0 is at the right: a press at the mirrored centre picks it
        v.set_selected(1);
        let p = centre(&v, 0);
        let o = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), Point::new(200.0 - p.x, p.y)));
        assert_eq!(v.shown(10), Some(0), "{o:?}");
        // the buttons sit at the left edge
        let m = v.flip(v.ctl_rect(Ctl::Menu).unwrap());
        assert_eq!(m.min.x, 0.0);
        let mid = Point::new((m.min.x + m.max.x) / 2.0, m.height() / 2.0);
        assert_eq!(v.hit(mid), Some(TabHit::Ctl(Ctl::Menu)));
        // Left moves toward the end
        v.focused = true;
        v.key(&key(Key::ArrowLeft, Modifiers::default()));
        assert_eq!(v.shown(10), Some(1));
        v.key(&key(Key::ArrowRight, Modifiers::default()));
        assert_eq!(v.shown(10), Some(0));
    }

    #[test]
    fn a_press_selects_once_and_js_writes_do_not_report() {
        let c = caps(3, false);
        let mut v = view(&c, 600.0);
        let o = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), centre(&v, 2)));
        assert_eq!(o.events, vec![Emit::new("changed", vec![Value::I64(2)])]);
        assert!(o.take_focus);
        assert_eq!(v.shown(3), Some(2));
        v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), centre(&v, 2)));
        let again = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), centre(&v, 2)));
        assert!(again.events.is_empty(), "the selected tab again: no change");
        v.set_selected(0);
        assert_eq!(v.shown(3), Some(0));
        v.set_selected(99);
        assert_eq!(v.shown(3), Some(2), "clamped to the pages");
        assert_eq!(v.shown(0), None);
    }

    #[test]
    fn close_button_and_middle_click_request_close() {
        let c = caps(2, true);
        let mut v = view(&c, 600.0);
        let p = close_centre(&v, 1);
        assert_eq!(v.hit(p), Some(TabHit::Close(1)));
        let down = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), p));
        assert!(down.events.is_empty(), "a press on the close button does not select");
        let up = v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), p));
        assert_eq!(up.close, vec![1]);
        // released elsewhere: no close
        v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), p));
        assert!(v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), centre(&v, 0))).close.is_empty());
        // middle click on the caption
        v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Middle), centre(&v, 0)));
        assert_eq!(v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Middle), centre(&v, 0))).close, vec![0]);
        // not closable: middle click does nothing
        let mut plain = view(&caps(2, false), 600.0);
        plain.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Middle), centre(&plain, 0)));
        assert!(plain.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Middle), centre(&plain, 0))).close.is_empty());
    }

    #[test]
    fn keys_move_and_cycle_the_selection() {
        let c = caps(3, false);
        let mut v = view(&c, 600.0);
        assert!(!v.key(&key(Key::ArrowRight, Modifiers::default())).handled, "arrows need focus");
        v.focused = true;
        assert_eq!(v.key(&key(Key::ArrowRight, Modifiers::default())).events, vec![Emit::new("changed", vec![Value::I64(1)])]);
        assert!(v.focus_visible);
        v.key(&key(Key::End, Modifiers::default()));
        assert_eq!(v.shown(3), Some(2));
        assert!(v.key(&key(Key::ArrowRight, Modifiers::default())).events.is_empty(), "no wrap at the end");
        v.key(&key(Key::Tab, Modifiers::CTRL));
        assert_eq!(v.shown(3), Some(0), "Ctrl+Tab wraps");
        v.key(&key(Key::Tab, Modifiers::CTRL.union(Modifiers::SHIFT)));
        assert_eq!(v.shown(3), Some(2));
        v.key(&key(Key::PageUp, Modifiers::CTRL));
        assert_eq!(v.shown(3), Some(1));
    }

    #[test]
    fn overflowing_captions_scroll_and_reveal_the_selection() {
        let c = caps(10, false);
        let mut v = view(&c, 150.0);
        assert!(v.strip().content_width > 150.0);
        v.focused = true;
        v.key(&key(Key::End, Modifiers::default()));
        let last = v.strip().tabs[9].rect;
        assert!((last.max.x - v.scroll() - v.viewport()).abs() < 1e-3, "the last caption ends at the scroll buttons");
        let wheel = WheelEvent { pos: Point::new(10.0, 5.0), delta: ScrollDelta::Lines(Vec2::new(0.0, 1.0)), phase: None, momentum: None, modifiers: Modifiers::default() };
        let before = v.scroll();
        assert!(v.wheel(&wheel));
        assert!(v.scroll() < before);
        let fits = view(&caps(2, false), 600.0);
        let mut fits = fits;
        assert!(!fits.wheel(&wheel), "nothing to scroll");
    }

    #[test]
    fn an_overflowing_strip_has_scroll_buttons_and_a_menu_of_every_tab() {
        let c = caps(10, false);
        let mut v = view(&c, 200.0);
        assert!(v.overflows() && v.viewport() < 200.0);
        let (l, r, m) = (v.ctl_rect(Ctl::Left).unwrap(), v.ctl_rect(Ctl::Right).unwrap(), v.ctl_rect(Ctl::Menu).unwrap());
        assert_eq!((l.min.x, m.max.x), (v.viewport(), 200.0), "at the strip's end, in order");
        let mid = |r: Rect| Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
        assert_eq!(v.hit(mid(r)), Some(TabHit::Ctl(Ctl::Right)));
        // the right button brings the first cut caption fully in; the left one goes back
        v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), mid(r)));
        let first_cut = v.strip().tabs.iter().find(|t| t.rect.max.x > v.viewport()).unwrap().rect;
        assert!((v.scroll() - (first_cut.max.x - v.viewport())).abs() < 1e-3);
        v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), mid(r)));
        v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), mid(l)));
        assert_eq!(v.scroll(), 0.0, "back to the first caption");
        v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), mid(l)));
        // the overflow button opens the menu under it, right-aligned to the strip
        let o = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), mid(m)));
        assert!(v.menu_open() && o.take_focus);
        v.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), mid(m)));
        let menu = v.menu_rect().unwrap();
        assert_eq!(menu.max.x, 200.0);
        assert!(menu.min.y > v.strip().height);
        // a click on the last row selects page 9, closes the menu and reveals the caption
        let row = Point::new(menu.min.x + 20.0, menu.max.y - MENU_PAD - 2.0);
        assert_eq!(v.hit(row), Some(TabHit::Item(9)));
        let o = v.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), row));
        assert_eq!(o.events, vec![Emit::new("changed", vec![Value::I64(9)])]);
        assert!(!v.menu_open());
        let last = v.strip().tabs[9].rect;
        assert!((last.max.x - v.scroll() - v.viewport()).abs() < 1e-3, "the picked caption in view");
        // keys while it is open: Up moves the row, Enter picks, Escape closes
        v.focused = true;
        v.set_menu(true);
        assert!(v.key(&key(Key::ArrowUp, Modifiers::default())).handled);
        assert_eq!(v.key(&key(Key::Enter, Modifiers::default())).events, vec![Emit::new("changed", vec![Value::I64(8)])]);
        v.set_menu(true);
        v.key(&key(Key::Escape, Modifiers::default()));
        assert!(!v.menu_open());
        // a selection written before the strip was laid out is revealed at its first layout
        let mut w = TabView::new();
        w.set_selected(9);
        w.relayout(&c, &TabLook::default(), &mut FixedText::default(), 200.0);
        assert!((w.strip().tabs[9].rect.max.x - w.scroll() - w.viewport()).abs() < 1e-3);
        // captions that fit: no buttons, no menu
        let mut fits = view(&caps(2, false), 600.0);
        assert!(fits.ctl_rect(Ctl::Menu).is_none());
        fits.set_menu(true);
        assert!(!fits.menu_open());
    }

    #[test]
    fn record_draws_titles_the_indicator_and_close_glyphs() {
        let c = caps(2, true);
        let v = view(&c, 600.0);
        let mut text = FixedText::default();
        let mut list = DisplayList::new();
        v.record(&c, &TabLook::default(), &mut text, &mut list);
        assert_eq!(text.texts(), vec!["Page 0", "Page 1"]);
        let lines = list.items.iter().filter(|p| matches!(p, Prim::Line(_))).count();
        // separator + 2 × (two close strokes) + the selected indicator
        assert_eq!(lines, 1 + 4 + 1);
        assert!(matches!(list.items.first(), Some(Prim::Line(_)) | Some(Prim::Quad(_))));
        assert!(matches!(list.items.last(), Some(Prim::PopClip)));
        let nodes = v.a11y(&c);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].role(), accesskit::Role::Tab);
        assert_eq!(nodes[0].is_selected(), Some(true));
        assert_eq!(nodes[1].is_selected(), Some(false));
    }

    #[test]
    fn captions_fall_back_to_names() {
        assert_eq!(caption_of(0, Some("Pcb"), true, ""), TabCaption { title: "Pcb".into(), closable: true });
        assert_eq!(caption_of(1, Some(""), false, "x").title, "Tab 2");
        assert_eq!(caption_of(2, None, true, "Schematic"), TabCaption { title: "Schematic".into(), closable: false });
        assert_eq!(caption_of(3, None, false, "").title, "Tab 4");
    }
}
