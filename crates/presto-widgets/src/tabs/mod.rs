//! `tab-view` and `tab`: a strip of tab captions over the selected page, drawn by presto (svelters-egui
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
//! - **Overflow:** captions wider than the strip scroll horizontally with the wheel; a selection
//!   change scrolls the selected caption into view.
//!
//! [`TabView`] is the kind's native state (selection, hover, press, scroll); [`TabStrip`] is the
//! caption geometry, rebuilt when captions, look or width change.

pub mod kinds;

use crate::table::shim::{self, Emit, HAlign, TextPainter, TextStyle, Value};
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::cursor::CursorIcon;
use presto_base::display::{DisplayList, Prim};
use presto_base::event::{KeyEvent, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, ScrollDelta, WheelEvent};
use presto_base::geom::{Corners, Point, Rect, RoundedRect, Size};
use presto_base::key::Key;

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
        let height = (line + 2.0 * look.pad_y).max(look.close_size + 2.0 * look.pad_y) + look.indicator_width;
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

/// What the pointer is over in the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabHit {
    Tab(usize),
    Close(usize),
}

impl TabHit {
    fn tab(self) -> usize {
        match self {
            TabHit::Tab(k) | TabHit::Close(k) => k,
        }
    }
}

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
        if self.hover.is_some_and(|h| h.tab() >= n) {
            self.hover = None;
        }
        if self.pressed.is_some_and(|(h, _)| h.tab() >= n) {
            self.pressed = None;
        }
        self.clamp_scroll();
    }

    fn max_scroll(&self) -> f32 {
        (self.strip.content_width - self.width).max(0.0)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Scrolls caption `k` fully into view (its left edge wins when it is wider than the strip).
    fn reveal(&mut self, k: usize) {
        let Some(t) = self.strip.tabs.get(k) else { return };
        if t.rect.max.x - self.scroll > self.width {
            self.scroll = t.rect.max.x - self.width;
        }
        if t.rect.min.x - self.scroll < 0.0 {
            self.scroll = t.rect.min.x;
        }
        self.clamp_scroll();
    }

    /// What is under `p` (tab-view coordinates).
    pub fn hit(&self, p: Point) -> Option<TabHit> {
        if p.y < 0.0 || p.y >= self.strip.height || p.x < 0.0 || p.x >= self.width {
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
                if b == MouseButton::Primary
                    && let TabHit::Tab(k) = h
                {
                    self.pick(k, &mut o.events);
                    o.take_focus = true;
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
                    (_, MouseButton::Middle) if hit.map(TabHit::tab) == Some(h.tab()) && self.closable(h.tab()) => o.close.push(h.tab()),
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
        let ctrl = ev.modifiers.contains(Modifiers::CTRL);
        let shift = ev.modifiers.contains(Modifiers::SHIFT);
        let cycle = |d: i64| ((cur as i64 + d).rem_euclid(n as i64)) as usize;
        let to = match ev.key {
            Some(Key::Tab) if ctrl => Some(cycle(if shift { -1 } else { 1 })),
            Some(Key::PageDown) if ctrl => Some(cycle(1)),
            Some(Key::PageUp) if ctrl => Some(cycle(-1)),
            Some(Key::ArrowRight) if self.focused => Some((cur + 1).min(n - 1)),
            Some(Key::ArrowLeft) if self.focused => Some(cur.saturating_sub(1)),
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
        let bar = Rect::new(0.0, 0.0, self.width, h);
        shim::fill(list, bar, look.bar_bg, 0.0);
        shim::line(list, Point::new(0.0, h - 0.5), Point::new(self.width, h - 0.5), 1.0, look.separator);
        list.push(Prim::PushClip(RoundedRect { rect: bar, radii: Corners::ZERO }), bar);
        let shown = self.shown(captions.len());
        for (k, (c, g)) in captions.iter().zip(&self.strip.tabs).enumerate() {
            let dx = presto_base::geom::Vec2::new(-self.scroll, 0.0);
            let r = g.rect.translate(dx);
            if r.max.x <= 0.0 || r.min.x >= self.width {
                continue;
            }
            let on = shown == Some(k);
            let hovered = self.hover.map(TabHit::tab) == Some(k);
            let pressed = self.pressed.is_some_and(|(h, b)| h == TabHit::Tab(k) && b == MouseButton::Primary);
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
            text.draw(list, &c.title, &look.text.with_color(fg), g.label.translate(dx), HAlign::Start, true);
            if let Some(cr) = g.close {
                let cr = cr.translate(dx);
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
                shim::line(list, Point::new(r.min.x, y), Point::new(r.max.x, y), look.indicator_width, look.indicator);
                if self.focused && self.focus_visible {
                    let ring = body.inflate(-1.0);
                    shim::frame(list, ring, Color::TRANSPARENT, look.focus, 1.0, look.radius);
                }
            }
        }
        list.push(Prim::PopClip, Rect::ZERO);
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
                let r = g.rect;
                n.set_bounds(accesskit::Rect { x0: (r.min.x - self.scroll) as f64, y0: r.min.y as f64, x1: (r.max.x - self.scroll) as f64, y1: r.max.y as f64 });
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
    use presto_base::event::{KeyLocation, PointerType};
    use presto_base::geom::Vec2;

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
        assert!((last.max.x - v.scroll() - 150.0).abs() < 1e-3, "the last caption ends at the strip's edge");
        let wheel = WheelEvent { pos: Point::new(10.0, 5.0), delta: ScrollDelta::Lines(Vec2::new(0.0, 1.0)), phase: None, momentum: None, modifiers: Modifiers::default() };
        let before = v.scroll();
        assert!(v.wheel(&wheel));
        assert!(v.scroll() < before);
        let fits = view(&caps(2, false), 600.0);
        let mut fits = fits;
        assert!(!fits.wheel(&wheel), "nothing to scroll");
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
