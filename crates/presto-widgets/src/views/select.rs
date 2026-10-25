//! `select` (svelters-egui `widgets/select.rs`, the `combo-box` kind): a closed button showing the
//! picked option (or the hint), and a list of the options in its own overlay surface, at least as
//! wide as the button, flipped above it at the window's bottom edge and shrunk to the room there
//! (the list then scrolls by rows).
//!
//! Keys, closed and focused: Alt+Down, Alt+Up, F4, Enter and Space open the list; Up/Down/Home/End
//! pick the previous/next/first/last option on Windows and Linux and open the list on macOS (as
//! `NSPopUpButton`); typing picks by prefix (type-ahead). Open: Up/Down/Home/End/PageUp/PageDown move
//! the highlighted row, typing moves it by prefix, Enter or Space picks it, Escape, Tab, F4 and
//! Alt+Up close without a pick. The pointer: a press on the button opens or closes, a release on a
//! row picks, a press outside closes, the wheel scrolls an open list.
//!
//! A pick emits `changed` (when it changes `selected`) and then `commit`; opening and closing emit
//! `toggle`.

use super::draw;
use super::menu_model::{Typeahead, next_by, typeahead_find};
use super::placement::{AnchorParams, Placement, place};
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, Labels, Os, Palette, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::display::DisplayList;
use presto_base::event::{KeyEvent, KeyState, Modifiers, PointerPhase, ScrollDelta, WheelEvent};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;

/// The open list as laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListGeom {
    pub rect: Rect,
    pub row_h: f32,
    /// Rows that fit.
    pub visible: usize,
}

/// Padding inside the list's surface.
const LIST_PAD: f32 = 2.0;

static SELECT_SPEC: KindSpec = KindSpec {
    name: "select",
    props: &[
        prop("options", PropType::List, ChangeClass::Layout),
        prop("selected", PropType::I64, ChangeClass::Layout),
        prop("hint-text", PropType::Str, ChangeClass::Layout),
        prop("enabled", PropType::Bool, ChangeClass::Paint),
        prop("open", PropType::Bool, ChangeClass::Structure),
    ],
    signals: &[signal("changed", false, true), signal("commit", true, false), signal("toggle", true, false)],
    slots: &[],
    role: accesskit::Role::ComboBox,
};

/// A select's state.
#[derive(Clone, Debug)]
pub struct Select {
    pub options: Vec<String>,
    /// The picked option (`selected` out of range: none, the hint shows).
    pub selected: Option<usize>,
    pub hint: String,
    pub enabled: bool,
    pub open: bool,
    /// The list's current row while open.
    pub highlight: Option<usize>,
    /// The list's first shown row.
    pub first: usize,
    /// Where the list must stay (the window's content area, in the select's space).
    pub bounds: Rect,
    list: Option<ListGeom>,
    typeahead: Typeahead,
}

impl Default for Select {
    fn default() -> Self {
        Select {
            options: Vec::new(),
            selected: None,
            hint: String::new(),
            enabled: true,
            open: false,
            highlight: None,
            first: 0,
            bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
            list: None,
            typeahead: Typeahead::default(),
        }
    }
}

impl Select {
    pub fn list(&self) -> Option<ListGeom> {
        self.list
    }

    /// The button's natural size.
    pub fn measure(&self, labels: &mut dyn Labels, palette: &Palette) -> Size {
        let w = self.options.iter().chain(std::iter::once(&self.hint)).map(|o| labels.measure(o, palette.font_size, None).w).fold(0.0, f32::max);
        Size::new(w + 6.0 * 2.0 + 4.0 + 16.0, palette.font_size * 1.25 + 8.0)
    }

    /// Lays the open list out under (or above) the button of `size`.
    pub fn prepare(&mut self, size: Size, labels: &mut dyn Labels, palette: &Palette) {
        if !self.open {
            self.list = None;
            return;
        }
        let row_h = palette.row_height;
        let w = self.options.iter().map(|o| labels.measure(o, palette.font_size, None).w).fold(0.0, f32::max) + 8.0 * 2.0 + 16.0;
        let placed = place(&AnchorParams {
            anchor: Rect::from_origin_size(Point::ZERO, size),
            size: Size::new(w, self.options.len() as f32 * row_h + LIST_PAD * 2.0),
            bounds: self.bounds,
            placement: Placement::BOTTOM_START,
            gap: 2.0,
            fit: true,
            match_anchor_width: true,
            ..AnchorParams::default()
        });
        let visible = (((placed.rect.height() - LIST_PAD * 2.0) / row_h).floor().max(1.0) as usize).min(self.options.len().max(1));
        self.list = Some(ListGeom { rect: placed.rect, row_h, visible });
        self.reveal();
    }

    /// Scrolls the list so the highlighted row shows.
    fn reveal(&mut self) {
        let Some(l) = self.list else { return };
        let max_first = self.options.len().saturating_sub(l.visible);
        if let Some(h) = self.highlight {
            if h < self.first {
                self.first = h;
            } else if h >= self.first + l.visible {
                self.first = h + 1 - l.visible;
            }
        }
        self.first = self.first.min(max_first);
    }

    /// The row at `p` in the open list.
    pub fn row_at(&self, p: Point) -> Option<usize> {
        let l = self.list?;
        if !l.rect.contains(p) {
            return None;
        }
        let i = ((p.y - l.rect.min.y - LIST_PAD) / l.row_h).floor();
        if i < 0.0 {
            return None;
        }
        let i = self.first + i as usize;
        (i < self.options.len() && i < self.first + l.visible).then_some(i)
    }

    fn set_open(&mut self, cx: &mut EventCx<'_>, open: bool) {
        if self.open == open {
            return;
        }
        self.open = open;
        self.typeahead.reset();
        if open {
            self.highlight = self.selected.or(if self.options.is_empty() { None } else { Some(0) });
            self.first = 0;
            cx.focus = true;
        } else {
            self.list = None;
        }
        cx.mark(ChangeClass::Structure);
        cx.emit("toggle", Value::Record(vec![("open".into(), Value::Bool(open))]));
    }

    /// Picks option `i`: `changed` when it differs, then `commit`.
    fn pick(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if i >= self.options.len() {
            return;
        }
        if self.selected != Some(i) {
            self.selected = Some(i);
            cx.mark(ChangeClass::Layout);
            cx.emit("changed", Value::Record(vec![("selected".into(), Value::I64(i as i64))]));
        }
        cx.emit("commit", Value::Record(vec![("selected".into(), Value::I64(i as i64))]));
    }

    fn key(&mut self, cx: &mut EventCx<'_>, ev: &KeyEvent) -> Handled {
        if ev.state != KeyState::Pressed {
            return Handled::No;
        }
        let m = ev.modifiers;
        let alt = m.contains(Modifiers::ALT);
        let plain = !alt && !m.contains(Modifiers::CTRL) && !m.contains(Modifiers::SUPER);
        let n = self.options.len();
        let typing = self.typeahead.typing(cx.now);
        if self.open {
            let page = self.list.map_or(1, |l| l.visible.max(2) - 1) as i32;
            let to = match ev.key {
                Some(Key::ArrowDown) if plain => next_by(n, self.highlight, 1, false, |_| true),
                Some(Key::ArrowUp) if plain => next_by(n, self.highlight, -1, false, |_| true),
                Some(Key::Home) if plain => next_by(n, None, 1, false, |_| true),
                Some(Key::End) if plain => next_by(n, None, -1, false, |_| true),
                Some(Key::PageDown) if plain => self.highlight.map(|h| (h + page as usize).min(n.saturating_sub(1))),
                Some(Key::PageUp) if plain => self.highlight.map(|h| h.saturating_sub(page as usize)),
                Some(Key::Space) if typing => return self.text(cx, " "),
                Some(Key::Enter | Key::Space) if plain => {
                    if let Some(h) = self.highlight {
                        self.pick(cx, h);
                    }
                    self.set_open(cx, false);
                    return Handled::Yes;
                }
                Some(Key::Escape | Key::Tab | Key::F4) => {
                    self.set_open(cx, false);
                    return if ev.key == Some(Key::Tab) { Handled::No } else { Handled::Yes };
                }
                Some(Key::ArrowUp) if alt => {
                    self.set_open(cx, false);
                    return Handled::Yes;
                }
                _ => {
                    return match ev.text.as_deref() {
                        Some(t) if !m.contains(Modifiers::CTRL) => self.text(cx, t),
                        _ => Handled::No,
                    };
                }
            };
            if to.is_some() && to != self.highlight {
                self.highlight = to;
                self.reveal();
                cx.mark(ChangeClass::Paint);
            }
            return Handled::Yes;
        }
        let macos = cx.policy.os == Os::MacOs;
        match ev.key {
            Some(Key::ArrowDown | Key::ArrowUp) if alt => self.set_open(cx, true),
            Some(Key::F4) if plain => self.set_open(cx, true),
            Some(Key::Space) if typing => return self.text(cx, " "),
            Some(Key::Enter | Key::Space) if plain => self.set_open(cx, true),
            Some(Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End) if plain && macos => self.set_open(cx, true),
            Some(k @ (Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End)) if plain => {
                let to = match k {
                    Key::ArrowDown => next_by(n, self.selected, 1, false, |_| true),
                    Key::ArrowUp => next_by(n, self.selected, -1, false, |_| true),
                    Key::Home => next_by(n, None, 1, false, |_| true),
                    _ => next_by(n, None, -1, false, |_| true),
                };
                if let Some(to) = to
                    && Some(to) != self.selected
                {
                    self.pick(cx, to);
                }
            }
            _ => {
                return match ev.text.as_deref() {
                    Some(t) if !m.contains(Modifiers::CTRL) && !alt => self.text(cx, t),
                    _ => Handled::No,
                };
            }
        }
        Handled::Yes
    }

    /// Type-ahead: moves the highlight (open) or picks (closed).
    fn text(&mut self, cx: &mut EventCx<'_>, t: &str) -> Handled {
        let Some(q) = self.typeahead.push(t, cx.now) else { return Handled::No };
        let labels: Vec<&str> = self.options.iter().map(String::as_str).collect();
        let from = if self.open { self.highlight } else { self.selected };
        if let Some(to) = typeahead_find(&labels, |_| true, &q, from) {
            if self.open {
                self.highlight = Some(to);
                self.reveal();
                cx.mark(ChangeClass::Paint);
            } else if Some(to) != self.selected {
                self.pick(cx, to);
            }
        }
        Handled::Yes
    }

    fn wheel(&mut self, cx: &mut EventCx<'_>, ev: &WheelEvent) -> Handled {
        let Some(l) = self.list.filter(|l| l.rect.contains(ev.pos)) else { return Handled::No };
        let rows = match ev.delta {
            ScrollDelta::Lines(v) => -v.y * 3.0,
            ScrollDelta::Pixels(v) => -v.y / l.row_h,
            ScrollDelta::Pages(v) => -v.y * l.visible as f32,
        }
        .round() as i64;
        let max_first = self.options.len().saturating_sub(l.visible) as i64;
        let first = (self.first as i64 + rows).clamp(0, max_first) as usize;
        if first != self.first {
            self.first = first;
            cx.mark(ChangeClass::Paint);
        }
        Handled::Yes
    }

    /// Records the open list (for the overlay stack); `None` while closed.
    pub fn record_list(&self, palette: &Palette, labels: &mut dyn Labels) -> Option<DisplayList> {
        let l = self.list?;
        let mut list = DisplayList::new();
        draw::shadow(&mut list, l.rect, palette.radius, palette.shadow, 8.0, Vec2::new(0.0, 2.0));
        draw::boxed(&mut list, l.rect, palette.radius, palette.surface_raised, 1.0, palette.border);
        for (k, i) in (self.first..self.options.len()).take(l.visible).enumerate() {
            let y = l.rect.min.y + LIST_PAD + k as f32 * l.row_h;
            let row = Rect::new(l.rect.min.x + LIST_PAD, y, l.rect.max.x - LIST_PAD, y + l.row_h);
            let current = self.highlight == Some(i);
            if current {
                draw::fill(&mut list, row, palette.radius, palette.accent);
            }
            let fg = if current { palette.on_accent } else { palette.text };
            let origin = Point::new(row.min.x + 8.0, y + (l.row_h - palette.font_size * 1.25) / 2.0);
            labels.record(&mut list, &self.options[i], origin, palette.font_size, fg, Some(row.width() - 16.0 - 16.0));
            if self.selected == Some(i) {
                let c = Point::new(row.max.x - 12.0, (row.min.y + row.max.y) / 2.0);
                draw::line(&mut list, Point::new(c.x - 4.0, c.y), Point::new(c.x - 1.0, c.y + 3.0), 1.5, fg);
                draw::line(&mut list, Point::new(c.x - 1.0, c.y + 3.0), Point::new(c.x + 5.0, c.y - 4.0), 1.5, fg);
            }
        }
        Some(list)
    }
}

impl Kind for Select {
    fn spec() -> &'static KindSpec {
        &SELECT_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "options" => {
                let l = value.as_list().ok_or(PropError::Invalid("list of strings"))?;
                self.options = l.iter().map(|v| v.as_str().map(str::to_string).ok_or(PropError::Invalid("list of strings"))).collect::<Result<_, _>>()?;
                self.selected = self.selected.filter(|&s| s < self.options.len());
                self.highlight = self.highlight.filter(|&h| h < self.options.len());
                Ok(ChangeClass::Layout)
            }
            "selected" => {
                let i = value.as_i64().ok_or(PropError::Invalid("integer"))?;
                self.selected = (i >= 0 && (i as usize) < self.options.len()).then_some(i as usize);
                Ok(ChangeClass::Layout)
            }
            "hint-text" => {
                self.hint = value.as_str().ok_or(PropError::Invalid("string"))?.to_string();
                Ok(ChangeClass::Layout)
            }
            "enabled" => {
                self.enabled = value.as_bool().ok_or(PropError::Invalid("bool"))?;
                if !self.enabled {
                    self.open = false;
                    self.list = None;
                }
                Ok(ChangeClass::Paint)
            }
            "open" => {
                let open = value.as_bool().ok_or(PropError::Invalid("bool"))?;
                if open != self.open {
                    self.open = open;
                    self.highlight = if open { self.selected } else { None };
                    self.first = 0;
                    if !open {
                        self.list = None;
                    }
                }
                Ok(ChangeClass::Structure)
            }
            _ => Err(PropError::Unknown),
        }
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let p = cx.palette;
        let r = Rect::from_origin_size(Point::ZERO, cx.size);
        draw::boxed(cx.list, r, p.radius, p.surface, 1.0, if self.open { p.accent } else { p.border });
        let (text, color) = match self.selected.and_then(|s| self.options.get(s)) {
            Some(o) => (o.as_str(), if self.enabled { p.text } else { p.text_muted }),
            None => (self.hint.as_str(), p.text_muted),
        };
        let origin = Point::new(6.0, (cx.size.h - p.font_size * 1.25) / 2.0);
        cx.labels.record(cx.list, text, origin, p.font_size, color, Some((cx.size.w - 6.0 * 2.0 - 20.0).max(0.0)));
        let c = Point::new(cx.size.w - 6.0 - 8.0, cx.size.h / 2.0);
        draw::line(cx.list, Point::new(c.x - 4.0, c.y - 2.0), Point::new(c.x, c.y + 2.0), 1.25, color);
        draw::line(cx.list, Point::new(c.x, c.y + 2.0), Point::new(c.x + 4.0, c.y - 2.0), 1.25, color);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        if !self.enabled {
            return Handled::No;
        }
        match ev {
            Event::Key(k) => self.key(cx, k),
            Event::Wheel(w) => self.wheel(cx, w),
            Event::Focus(false) => {
                self.set_open(cx, false);
                Handled::No
            }
            Event::Pointer(p) => {
                let on_button = Rect::from_origin_size(Point::ZERO, cx.size).contains(p.pos);
                let row = self.row_at(p.pos);
                let in_list = self.list.is_some_and(|l| l.rect.contains(p.pos));
                match p.phase {
                    PointerPhase::Down if on_button => {
                        let open = !self.open;
                        self.set_open(cx, open);
                        cx.focus = true;
                        Handled::Yes
                    }
                    PointerPhase::Down if in_list => Handled::Yes,
                    PointerPhase::Down if self.open => {
                        self.set_open(cx, false);
                        Handled::No
                    }
                    PointerPhase::Up => match row {
                        Some(i) => {
                            self.pick(cx, i);
                            self.set_open(cx, false);
                            Handled::Yes
                        }
                        None if in_list || on_button => Handled::Yes,
                        None => Handled::No,
                    },
                    PointerPhase::Move => {
                        if let Some(i) = row
                            && self.highlight != Some(i)
                        {
                            self.highlight = Some(i);
                            cx.mark(ChangeClass::Paint);
                        }
                        if row.is_some() || on_button { Handled::Yes } else { Handled::No }
                    }
                    _ => Handled::No,
                }
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        if let Some(o) = self.selected.and_then(|s| self.options.get(s)) {
            node.set_value(o.as_str());
        }
        node.set_expanded(self.open);
        node.set_has_popup(accesskit::HasPopup::Listbox);
        if !self.enabled {
            node.set_disabled();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::shim::{FixedLabels, Policy};
    use super::*;
    use presto_base::event::{KeyLocation, MouseButton, PointerEvent, PointerType};
    use presto_base::time::Time;

    fn sel() -> Select {
        let mut s = Select::default();
        s.set_prop("options", &Value::List(["Apple", "Banana", "Blueberry", "Cherry"].map(|o| Value::Str(o.into())).to_vec())).unwrap();
        s.set_prop("selected", &Value::I64(0)).unwrap();
        s.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        s
    }

    fn key_with(k: Option<Key>, text: Option<&str>, modifiers: Modifiers) -> Event {
        Event::Key(KeyEvent {
            key: k,
            physical: 0,
            text: text.map(str::to_string),
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers,
        })
    }

    fn key(k: Key) -> Event {
        key_with(Some(k), None, Modifiers::default())
    }

    const SIZE: Size = Size::new(120.0, 26.0);

    fn send(s: &mut Select, policy: &Policy, ms: u64, ev: Event) -> (Handled, Vec<super::super::shim::Emitted>) {
        let mut cx = EventCx::new(Time(ms * 1_000_000), SIZE, policy);
        let h = s.event(&mut cx, &ev);
        s.prepare(SIZE, &mut FixedLabels, &Palette::default());
        (h, cx.emitted)
    }

    fn changed(sig: &[super::super::shim::Emitted]) -> Option<i64> {
        sig.iter().find(|e| e.signal == "changed").and_then(|e| e.detail.field("selected")).and_then(Value::as_i64)
    }

    #[test]
    fn arrows_pick_while_closed_on_windows_and_linux() {
        for os in [Os::Windows, Os::Linux] {
            let policy = Policy::for_os(os);
            let mut s = sel();
            let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
            assert_eq!((s.selected, changed(&sig), s.open), (Some(1), Some(1), false));
            send(&mut s, &policy, 0, key(Key::End));
            assert_eq!(s.selected, Some(3));
            let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
            assert_eq!((s.selected, changed(&sig)), (Some(3), None), "no wrap, no change");
            send(&mut s, &policy, 0, key(Key::Home));
            assert_eq!(s.selected, Some(0));
        }
    }

    #[test]
    fn arrows_open_on_macos() {
        let policy = Policy::for_os(Os::MacOs);
        let mut s = sel();
        let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
        assert!(s.open);
        assert_eq!(s.selected, Some(0));
        assert_eq!(changed(&sig), None);
        assert!(sig.iter().any(|e| e.signal == "toggle"));
    }

    #[test]
    fn open_keys_move_the_highlight_and_pick() {
        let policy = Policy::for_os(Os::Windows);
        let mut s = sel();
        send(&mut s, &policy, 0, key_with(Some(Key::ArrowDown), None, Modifiers::ALT));
        assert!(s.open);
        assert_eq!(s.highlight, Some(0));
        send(&mut s, &policy, 0, key(Key::ArrowDown));
        send(&mut s, &policy, 0, key(Key::ArrowDown));
        assert_eq!((s.highlight, s.selected), (Some(2), Some(0)), "moving the highlight picks nothing");
        let (_, sig) = send(&mut s, &policy, 0, key(Key::Enter));
        assert_eq!((s.selected, changed(&sig), s.open), (Some(2), Some(2), false));
        assert!(sig.iter().any(|e| e.signal == "commit"));
        // F4 opens, Escape closes without a pick
        send(&mut s, &policy, 0, key(Key::F4));
        assert!(s.open);
        send(&mut s, &policy, 0, key(Key::ArrowUp));
        send(&mut s, &policy, 0, key(Key::Escape));
        assert_eq!((s.selected, s.open), (Some(2), false));
    }

    #[test]
    fn typeahead_picks_closed_and_highlights_open() {
        let policy = Policy::for_os(Os::Linux);
        let mut s = sel();
        let (_, sig) = send(&mut s, &policy, 0, key_with(Some(Key::B), Some("b"), Modifiers::default()));
        assert_eq!((s.selected, changed(&sig)), (Some(1), Some(1)));
        send(&mut s, &policy, 200, key_with(Some(Key::L), Some("l"), Modifiers::default()));
        assert_eq!(s.selected, Some(2), "\"bl\" is Blueberry");
        send(&mut s, &policy, 3000, key(Key::Enter));
        assert!(s.open);
        send(&mut s, &policy, 3100, key_with(Some(Key::C), Some("c"), Modifiers::default()));
        assert_eq!((s.highlight, s.selected), (Some(3), Some(2)));
    }

    #[test]
    fn the_list_is_as_wide_as_the_button_and_flips_at_the_bottom() {
        let policy = Policy::for_os(Os::Linux);
        let mut s = sel();
        send(&mut s, &policy, 0, key(Key::F4));
        let l = s.list().unwrap();
        assert!(l.rect.width() >= SIZE.w);
        assert_eq!(l.rect.min.y, SIZE.h + 2.0);
        assert_eq!(l.visible, 4);
        // near the bottom: above the button
        let mut s = sel();
        s.bounds = Rect::new(0.0, -200.0, 400.0, 40.0);
        send(&mut s, &policy, 0, key(Key::F4));
        assert_eq!(s.list().unwrap().rect.max.y, -2.0);
    }

    #[test]
    fn pointer_opens_picks_and_dismisses() {
        let policy = Policy::for_os(Os::Windows);
        let mut s = sel();
        let ptr = |phase, pos| {
            Event::Pointer(PointerEvent {
                phase,
                pointer: 0,
                kind: PointerType::Mouse,
                button: Some(MouseButton::Primary),
                buttons: 0,
                pos,
                modifiers: Modifiers::default(),
                pressure: 0.0,
                tilt: Vec2::ZERO,
                history: Default::default(),
            })
        };
        send(&mut s, &policy, 0, ptr(PointerPhase::Down, Point::new(10.0, 10.0)));
        assert!(s.open);
        let l = s.list().unwrap();
        let row3 = Point::new(l.rect.min.x + 10.0, l.rect.min.y + LIST_PAD + 3.5 * l.row_h);
        assert_eq!(s.row_at(row3), Some(3));
        let (_, sig) = send(&mut s, &policy, 10, ptr(PointerPhase::Up, row3));
        assert_eq!((s.selected, changed(&sig), s.open), (Some(3), Some(3), false));
        send(&mut s, &policy, 20, ptr(PointerPhase::Down, Point::new(10.0, 10.0)));
        let (h, _) = send(&mut s, &policy, 30, ptr(PointerPhase::Down, Point::new(390.0, 290.0)));
        assert!(!s.open);
        assert_eq!(h, Handled::No);
    }

    #[test]
    fn props_validate() {
        let mut s = sel();
        assert_eq!(s.set_prop("selected", &Value::I64(9)), Ok(ChangeClass::Layout));
        assert_eq!(s.selected, None, "out of range shows the hint");
        assert!(s.set_prop("options", &Value::List(vec![Value::I64(1)])).is_err());
        assert_eq!(s.set_prop("nope", &Value::Null), Err(PropError::Unknown));
        let mut node = accesskit::Node::new(accesskit::Role::ComboBox);
        s.set_prop("selected", &Value::I64(1)).unwrap();
        s.a11y(&mut node);
        assert_eq!(node.value(), Some("Banana"));
        assert_eq!(node.is_expanded(), Some(false));
    }
}
