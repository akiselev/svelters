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
//! `toggle`. `selected` and `open` are native-owned and bindable. `changed` is not cancelable: the
//! pick is applied before listeners run.
//!
//! Look (CSS over the UA tables below): the button is a field (`input-bg`, `input-border-color`,
//! `:open` takes the focus colour, `:placeholder-shown` while the hint shows); the list is
//! `::part(list)` (the menu surface's tokens) with `::part(option)` rows (`:selected` is the
//! highlighted row, `:checked` the picked one).

use super::common::{emit, platform};
use super::menu::{check_mark, stroke};
use super::menu_model::{Typeahead, next_by, typeahead_find};
use super::placement::{AnchorParams, Placement, place};
use crate::controls::common::host_view;
use crate::interact::{Platform, StateBits};
use crate::kind_props;
use crate::paint::{self, Align, Elide, PaintCx, StyleView, Ua, UaRule, UaValue::{Keyword, Number, Px, Token}};
use crate::shim::core::{
    A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, Resources,
    SignalSpec, Styles, Value, ValueType,
};
use crate::shim::text::TextShaper;
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::display::DisplayList;
use presto_base::event::{KeyEvent, KeyState, Modifiers, PointerPhase, ScrollDelta, WheelEvent};
use presto_base::geom::{Point, Rect, Size};
use presto_base::key::Key;

/// The closed button: a field.
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("input-bg")),
        ("color", Token("input-fg")),
        ("border-color", Token("input-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(4.0)),
        ("padding-bottom", Px(4.0)),
        ("cursor", Keyword("pointer")),
    ]),
    UaRule::when(StateBits::PLACEHOLDER_SHOWN, &[("color", Token("placeholder"))]),
    UaRule::when(StateBits::OPEN, &[("border-color", Token("input-border-color-focus"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("border-color", Token("input-border-color-focus"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed"))]),
]);

/// `::part(list)`: the open list's surface (the menu's tokens).
pub const LIST_UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("popup-shadow")),
    ("padding", Px(2.0)),
])]);

/// `::part(option)`: a row; `:selected` is the highlighted one.
pub const OPTION_UA: Ua = Ua(&[
    UaRule::base(&[("height", Token("row-h")), ("padding-left", Px(8.0)), ("padding-right", Px(8.0)), ("border-radius", Token("radius")), ("color", Token("fg"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("menu-sel")), ("color", Token("menu-sel-fg"))]),
]);

/// Room the button keeps for its chevron.
const CHEVRON_W: f32 = 20.0;
/// Room a row keeps for the picked option's check mark.
const CHECK_W: f32 = 16.0;

kind_props! {
    /// The select's props.
    pub struct SelectProps;
    /// The options' labels (a list of strings).
    options "options": Vec<Value> = None ; List, Layout;
    /// The picked option's index (`-1` or out of range: none, the hint shows); set natively by a pick.
    selected "selected": i64 = I64(-1) ; I64, Layout, bindable;
    /// Shown while nothing is picked.
    hint_text "hint-text": String = Str("") ; Str, Layout;
    enabled "enabled": bool = Bool(true) ; Bool, Paint;
    /// The list shows; set natively when it opens or closes.
    open "open": bool = Bool(false) ; Bool, Structure, bindable;
}

pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("selected", ValueType::I64)], "A pick changed `selected`."),
    SignalSpec::new("commit", &[("selected", ValueType::I64)], "An option was picked (changed or not)."),
    SignalSpec::new("toggle", &[("open", ValueType::Bool)], "The list opened or closed."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "select",
    props: SelectProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: false,
    role: accesskit::Role::ComboBox,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: true,
};

/// The open list as laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListGeom {
    pub rect: Rect,
    pub row_h: f32,
    /// The surface's padding.
    pub pad: f32,
    /// Rows that fit.
    pub visible: usize,
}

/// A select's state.
#[derive(Clone, Debug)]
pub struct Select {
    pub props: SelectProps,
    pub options: Vec<String>,
    /// The picked option (`selected` out of range: none, the hint shows).
    pub selected: Option<usize>,
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
            props: SelectProps::default(),
            options: Vec::new(),
            selected: None,
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
    pub fn is_open(&self) -> bool {
        self.props.open
    }

    /// The button's state over the node's: `:open`, `:disabled`, `:placeholder-shown`.
    fn state(&self, base: StateBits) -> StateBits {
        let mut s = base;
        s.set(StateBits::OPEN, self.props.open);
        s.set(StateBits::DISABLED, !self.props.enabled);
        s.set(StateBits::PLACEHOLDER_SHOWN, self.selected.is_none());
        s
    }

    /// The text the button shows.
    fn shown(&self) -> &str {
        self.selected.and_then(|s| self.options.get(s)).map_or(self.props.hint_text.as_str(), String::as_str)
    }

    /// The button's natural size under `style`.
    pub fn measure(&self, text: &mut dyn TextShaper, style: &Styles) -> Size {
        let host = host_view(style, UA);
        let w = self.options.iter().chain(std::iter::once(&self.props.hint_text)).map(|o| paint::measure_text(text, &host, o, None).w).fold(0.0, f32::max);
        let line = paint::measure_text(text, &host, "Xg", None).h;
        let extra = paint::box_extra(&host);
        Size::new(w + CHEVRON_W + extra.x, line + extra.y)
    }

    /// Lays the open list out under (or above) the button of `size`.
    pub fn prepare(&mut self, size: Size, text: &mut dyn TextShaper, style: &Styles) {
        if !self.props.open {
            self.list = None;
            return;
        }
        let host = host_view(style, UA);
        let list = host.part(style.part_css("list"), LIST_UA);
        let option = list.part(style.part_css("option"), OPTION_UA);
        let [pad, ..] = list.sides("padding");
        let row_h = option.px("height").unwrap_or(22.0);
        let row_extra = paint::box_extra(&option).x;
        let w = self.options.iter().map(|o| paint::measure_text(text, &option, o, None).w).fold(0.0, f32::max) + row_extra + CHECK_W + pad * 2.0;
        let placed = place(&AnchorParams {
            anchor: Rect::from_origin_size(Point::ZERO, size),
            size: Size::new(w, self.options.len() as f32 * row_h + pad * 2.0),
            bounds: self.bounds,
            placement: Placement::BOTTOM_START,
            gap: 2.0,
            fit: true,
            match_anchor_width: true,
            ..AnchorParams::default()
        });
        let visible = (((placed.rect.height() - pad * 2.0) / row_h).floor().max(1.0) as usize).min(self.options.len().max(1));
        self.list = Some(ListGeom { rect: placed.rect, row_h, pad, visible });
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
        let i = ((p.y - l.rect.min.y - l.pad) / l.row_h).floor();
        if i < 0.0 {
            return None;
        }
        let i = self.first + i as usize;
        (i < self.options.len() && i < self.first + l.visible).then_some(i)
    }

    fn set_open(&mut self, cx: &mut EventCx<'_>, open: bool) {
        if self.props.open == open {
            return;
        }
        self.props.open = open;
        self.typeahead.reset();
        if open {
            self.highlight = self.selected.or(if self.options.is_empty() { None } else { Some(0) });
            self.first = 0;
            cx.focus_request = Some(true);
        } else {
            self.list = None;
        }
        cx.prop_changed(P::open.id());
        emit(cx, &SPEC, "toggle", vec![Value::Bool(open)]);
    }

    /// Picks option `i`: `changed` when it differs, then `commit`.
    fn pick(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if i >= self.options.len() {
            return;
        }
        if self.selected != Some(i) {
            self.selected = Some(i);
            self.props.selected = i as i64;
            cx.prop_changed(P::selected.id());
            emit(cx, &SPEC, "changed", vec![Value::I64(i as i64)]);
        }
        emit(cx, &SPEC, "commit", vec![Value::I64(i as i64)]);
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
        if self.props.open {
            let page = self.list.map_or(1, |l| l.visible.max(2) - 1);
            let to = match ev.key {
                Some(Key::ArrowDown) if plain => next_by(n, self.highlight, 1, false, |_| true),
                Some(Key::ArrowUp) if plain => next_by(n, self.highlight, -1, false, |_| true),
                Some(Key::Home) if plain => next_by(n, None, 1, false, |_| true),
                Some(Key::End) if plain => next_by(n, None, -1, false, |_| true),
                Some(Key::PageDown) if plain => self.highlight.map(|h| (h + page).min(n.saturating_sub(1))),
                Some(Key::PageUp) if plain => self.highlight.map(|h| h.saturating_sub(page)),
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
                cx.repaint = true;
            }
            return Handled::Yes;
        }
        let macos = platform(cx.policy) == Platform::Macos;
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
        let from = if self.props.open { self.highlight } else { self.selected };
        if let Some(to) = typeahead_find(&labels, |_| true, &q, from) {
            if self.props.open {
                self.highlight = Some(to);
                self.reveal();
                cx.repaint = true;
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
            cx.repaint = true;
        }
        Handled::Yes
    }

    /// Records the open list (for the overlay stack) under the node's style; `None` while closed.
    pub fn record_list(&self, text: &mut dyn TextShaper, res: &mut dyn Resources, style: &Styles, scale: f32) -> Option<DisplayList> {
        let l = self.list?;
        let host = host_view(style, UA);
        let list_v = host.part(style.part_css("list"), LIST_UA);
        let option = list_v.part(style.part_css("option"), OPTION_UA);
        let mut list = DisplayList::new();
        let mut pc = PaintCx::new(&mut list, res, text, scale);
        paint::paint_box(&mut pc, &list_v, l.rect);
        for (k, i) in (self.first..self.options.len()).take(l.visible).enumerate() {
            let y = l.rect.min.y + l.pad + k as f32 * l.row_h;
            let row = Rect::new(l.rect.min.x + l.pad, y, l.rect.max.x - l.pad, y + l.row_h);
            let mut st = StateBits::NONE;
            st.set(StateBits::SELECTED, self.highlight == Some(i));
            st.set(StateBits::CHECKED, self.selected == Some(i));
            let ov = option.in_state(st);
            let g = paint::paint_box(&mut pc, &ov, row);
            let content = Rect::new(g.content.min.x, row.min.y, g.content.max.x - CHECK_W, row.max.y);
            label(&mut pc, &ov, &self.options[i], content);
            if self.selected == Some(i) {
                let fg = ov.color("color").unwrap_or(Color::BLACK);
                check_mark(&mut pc, Point::new(g.content.max.x - CHECK_W / 2.0, (row.min.y + row.max.y) / 2.0), fg);
            }
        }
        Some(list)
    }
}

/// `text` elided to `content` and painted at its start, centred vertically.
fn label(pc: &mut PaintCx, s: &StyleView, text: &str, content: Rect) {
    let shown = paint::elide(pc.text, s, text, Elide::End, content.width().max(0.0));
    let layout = paint::text_layout(pc.text, s, &shown, None);
    paint::paint_text(pc, s, &layout, content, Align::Start);
}

impl Kind for Select {
    fn parts() -> &'static [&'static str] {
        &["list", "option"]
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    /// `:open` and `:placeholder-shown` follow the props, so CSS on the node sees them.
    /// The list is open: a press outside it and the button closes it.
    fn light_dismiss(&self) -> bool {
        self.props.open && self.props.enabled
    }

    fn prop_state(&self) -> StateBits {
        let mut s = StateBits::NONE;
        s.set(StateBits::OPEN, self.props.open);
        s.set(StateBits::PLACEHOLDER_SHOWN, self.selected.is_none());
        s
    }
    fn prop_states() -> StateBits {
        StateBits(StateBits::OPEN.0 | StateBits::PLACEHOLDER_SHOWN.0)
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        if prop == P::options.id() {
            let strings = matches!(v, Value::List(l) if l.iter().all(|o| matches!(o, Value::Str(_))));
            if !strings {
                return Err(PropError::Type { prop: "options", expected: ValueType::List });
            }
        }
        self.props.set(prop, v)?;
        if prop == P::options.id() {
            self.options = self.props.options.iter().filter_map(|o| o.as_str().map(str::to_owned)).collect();
            self.highlight = self.highlight.filter(|&h| h < self.options.len());
        }
        let s = self.props.selected;
        self.selected = (s >= 0 && (s as usize) < self.options.len()).then_some(s as usize);
        if prop == P::enabled.id() && !self.props.enabled {
            self.props.open = false;
            self.list = None;
        }
        if prop == P::open.id() {
            self.highlight = if self.props.open { self.selected } else { None };
            self.first = 0;
            if !self.props.open {
                self.list = None;
            }
        }
        Ok(())
    }

    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.measure(cx.text, &cx.style);
        taffy::Size { width: s.w, height: s.h }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.prepare(cx.size, cx.text, &cx.style);
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_view(&cx.style, UA).in_state(self.state(cx.style.state));
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        let g = paint::paint_box(&mut pc, &host, rect);
        let content = Rect::new(g.content.min.x, g.content.min.y, (g.content.max.x - CHEVRON_W).max(g.content.min.x), g.content.max.y);
        label(&mut pc, &host, self.shown(), content);
        let color = host.color("color").unwrap_or(Color::BLACK);
        let c = Point::new(g.content.max.x - CHEVRON_W / 2.0, (rect.min.y + rect.max.y) / 2.0);
        stroke(&mut pc, Point::new(c.x - 4.0, c.y - 2.0), Point::new(c.x, c.y + 2.0), 1.25, color);
        stroke(&mut pc, Point::new(c.x, c.y + 2.0), Point::new(c.x + 4.0, c.y - 2.0), 1.25, color);
        paint::paint_outline(&mut pc, &host, rect);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::OPEN, self.props.open);
        cx.state.set(StateBits::DISABLED, !self.props.enabled);
        cx.enabled = self.props.enabled;
        if !self.props.enabled {
            return Handled::No;
        }
        let h = match ev {
            Event::Key(k) => self.key(cx, k),
            Event::Wheel(w) => self.wheel(cx, w),
            Event::Focus { gained: false, .. } => {
                self.set_open(cx, false);
                Handled::No
            }
            // light dismiss: a press outside the button and its list closes the list
            Event::OutsidePress(p) => {
                if self.props.open && !self.list.is_some_and(|l| l.rect.contains(*p)) {
                    self.set_open(cx, false);
                }
                Handled::No
            }
            Event::Pointer(p) => {
                let on_button = Rect::from_origin_size(Point::ZERO, cx.size).contains(p.pos);
                let row = self.row_at(p.pos);
                let in_list = self.list.is_some_and(|l| l.rect.contains(p.pos));
                match p.phase {
                    PointerPhase::Down if on_button => {
                        let open = !self.props.open;
                        self.set_open(cx, open);
                        cx.focus_request = Some(true);
                        Handled::Yes
                    }
                    PointerPhase::Down if in_list => Handled::Yes,
                    PointerPhase::Down if self.props.open => {
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
                            cx.repaint = true;
                        }
                        if row.is_some() || on_button { Handled::Yes } else { Handled::No }
                    }
                    _ => Handled::No,
                }
            }
            _ => Handled::No,
        };
        cx.state.set(StateBits::OPEN, self.props.open);
        h
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::ComboBox);
        if let Some(o) = self.selected.and_then(|s| self.options.get(s)) {
            cx.node.set_value(o.as_str());
        }
        cx.node.set_expanded(self.props.open);
        cx.node.set_has_popup(accesskit::HasPopup::Listbox);
        if !self.props.enabled {
            cx.node.set_disabled();
        }
    }

    fn claims_key(&self, key: Key, m: Modifiers) -> bool {
        let alt = m.contains(Modifiers::ALT);
        if self.props.open {
            return matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown | Key::Enter | Key::Space | Key::Escape | Key::F4);
        }
        matches!(key, Key::Enter | Key::Space | Key::F4) || (matches!(key, Key::ArrowUp | Key::ArrowDown) && (alt || !m.contains(Modifiers::CTRL))) || (!alt && matches!(key, Key::Home | Key::End))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Native, Policy};
    use crate::shim::testing::{MemResources, MonoShaper};
    use presto_base::event::{KeyLocation, MouseButton, PointerEvent, PointerType};
    use presto_base::geom::Vec2;
    use presto_base::time::Time;

    fn policy(os: Platform) -> Policy {
        *Native::for_os(os).policy()
    }

    fn styles() -> Styles<'static> {
        Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true }
    }

    fn set(s: &mut Select, name: &str, v: Value) -> Result<(), PropError> {
        s.set_prop(SPEC.prop(name).unwrap(), &v)
    }

    fn sel() -> Select {
        let mut s = Select::default();
        set(&mut s, "options", Value::List(["Apple", "Banana", "Blueberry", "Cherry"].map(Value::str).to_vec())).unwrap();
        set(&mut s, "selected", Value::I64(0)).unwrap();
        s.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        s
    }

    fn key_with(k: Option<Key>, text: Option<&str>, modifiers: Modifiers) -> Event {
        Event::Key(KeyEvent { key: k, physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers })
    }

    fn key(k: Key) -> Event {
        key_with(Some(k), None, Modifiers::default())
    }

    const SIZE: Size = Size::new(120.0, 26.0);

    type Signals = Vec<(&'static str, Vec<Value>)>;

    fn send(s: &mut Select, policy: &Policy, ms: u64, ev: Event) -> (Handled, Signals) {
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(Time(ms * 1_000_000), SIZE, &mut state, policy);
        let h = s.event(&mut cx, &ev);
        let sig = cx.emitted.iter().map(|e| (SPEC.signals[e.signal.0 as usize].name, e.args.clone())).collect();
        s.prepare(SIZE, &mut MonoShaper, &styles());
        (h, sig)
    }

    fn changed(sig: &Signals) -> Option<i64> {
        sig.iter().find(|e| e.0 == "changed").and_then(|e| e.1[0].as_i64())
    }

    #[test]
    fn arrows_pick_while_closed_on_windows_and_linux() {
        for os in [Platform::Windows, Platform::Linux] {
            let policy = policy(os);
            let mut s = sel();
            let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
            assert_eq!((s.selected, changed(&sig), s.is_open()), (Some(1), Some(1), false));
            assert_eq!(s.get_prop(P::selected.id()), Some(Value::I64(1)), "`bind:selected` reads the pick");
            send(&mut s, &policy, 0, key(Key::End));
            assert_eq!(s.selected, Some(3));
            let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
            assert_eq!((s.selected, changed(&sig)), (Some(3), None), "no wrap, no change");
            send(&mut s, &policy, 0, key(Key::Home));
            assert_eq!(s.selected, Some(0));
        }
    }

    #[test]
    fn an_outside_press_closes_the_list_but_a_press_in_it_does_not() {
        let policy = policy(Platform::Windows);
        let mut s = sel();
        assert!(!s.light_dismiss());
        send(&mut s, &policy, 0, key(Key::F4));
        assert!(s.is_open() && s.light_dismiss());
        let in_list = s.list().expect("laid out").rect.min + presto_base::geom::Vec2::new(4.0, 4.0);
        send(&mut s, &policy, 1, Event::OutsidePress(in_list));
        assert!(s.is_open(), "the list is drawn outside the button but is part of the select");
        let (h, sig) = send(&mut s, &policy, 2, Event::OutsidePress(Point::new(-40.0, -40.0)));
        assert_eq!(h, Handled::No);
        assert!(!s.is_open());
        assert!(sig.iter().any(|e| e.0 == "toggle" && e.1 == vec![Value::Bool(false)]));
        assert_eq!(s.selected, Some(0), "nothing picked");
    }

    #[test]
    fn arrows_open_on_macos() {
        let policy = policy(Platform::Macos);
        let mut s = sel();
        let (_, sig) = send(&mut s, &policy, 0, key(Key::ArrowDown));
        assert!(s.is_open());
        assert_eq!(s.selected, Some(0));
        assert_eq!(changed(&sig), None);
        assert!(sig.iter().any(|e| e.0 == "toggle"));
    }

    #[test]
    fn open_keys_move_the_highlight_and_pick() {
        let policy = policy(Platform::Windows);
        let mut s = sel();
        send(&mut s, &policy, 0, key_with(Some(Key::ArrowDown), None, Modifiers::ALT));
        assert!(s.is_open());
        assert_eq!(s.highlight, Some(0));
        send(&mut s, &policy, 0, key(Key::ArrowDown));
        send(&mut s, &policy, 0, key(Key::ArrowDown));
        assert_eq!((s.highlight, s.selected), (Some(2), Some(0)), "moving the highlight picks nothing");
        let (_, sig) = send(&mut s, &policy, 0, key(Key::Enter));
        assert_eq!((s.selected, changed(&sig), s.is_open()), (Some(2), Some(2), false));
        assert!(sig.iter().any(|e| e.0 == "commit"));
        // F4 opens, Escape closes without a pick
        send(&mut s, &policy, 0, key(Key::F4));
        assert!(s.is_open());
        assert!(s.claims_key(Key::Escape, Modifiers::default()));
        send(&mut s, &policy, 0, key(Key::ArrowUp));
        send(&mut s, &policy, 0, key(Key::Escape));
        assert_eq!((s.selected, s.is_open()), (Some(2), false));
    }

    #[test]
    fn typeahead_picks_closed_and_highlights_open() {
        let policy = policy(Platform::Linux);
        let mut s = sel();
        let (_, sig) = send(&mut s, &policy, 0, key_with(Some(Key::B), Some("b"), Modifiers::default()));
        assert_eq!((s.selected, changed(&sig)), (Some(1), Some(1)));
        send(&mut s, &policy, 200, key_with(Some(Key::L), Some("l"), Modifiers::default()));
        assert_eq!(s.selected, Some(2), "\"bl\" is Blueberry");
        send(&mut s, &policy, 3000, key(Key::Enter));
        assert!(s.is_open());
        send(&mut s, &policy, 3100, key_with(Some(Key::C), Some("c"), Modifiers::default()));
        assert_eq!((s.highlight, s.selected), (Some(3), Some(2)));
    }

    #[test]
    fn the_list_is_as_wide_as_the_button_and_flips_at_the_bottom() {
        let policy = policy(Platform::Linux);
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
        let policy = policy(Platform::Windows);
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
        assert!(s.is_open());
        let l = s.list().unwrap();
        let row3 = Point::new(l.rect.min.x + 10.0, l.rect.min.y + l.pad + 3.5 * l.row_h);
        assert_eq!(s.row_at(row3), Some(3));
        let (_, sig) = send(&mut s, &policy, 10, ptr(PointerPhase::Up, row3));
        assert_eq!((s.selected, changed(&sig), s.is_open()), (Some(3), Some(3), false));
        send(&mut s, &policy, 20, ptr(PointerPhase::Down, Point::new(10.0, 10.0)));
        let (h, _) = send(&mut s, &policy, 30, ptr(PointerPhase::Down, Point::new(390.0, 290.0)));
        assert!(!s.is_open());
        assert_eq!(h, Handled::No);
    }

    #[test]
    fn records_the_field_and_the_list() {
        let mut s = sel();
        let (list, _) = testkit::record(&s, SIZE, StateBits::NONE);
        let kinds = testkit::kinds(&list);
        assert_eq!(kinds[0], "quad", "the field");
        assert!(kinds.contains(&"glyphs") && kinds.contains(&"line"), "the value and the chevron");
        assert!(s.record_list(&mut MonoShaper, &mut MemResources::default(), &styles(), 1.0).is_none(), "closed");
        set(&mut s, "open", Value::Bool(true)).unwrap();
        s.prepare(SIZE, &mut MonoShaper, &styles());
        let l = s.record_list(&mut MonoShaper, &mut MemResources::default(), &styles(), 1.0).unwrap();
        let kinds = testkit::kinds(&l);
        assert_eq!(kinds.iter().filter(|k| **k == "glyphs").count(), 4, "every option");
    }

    #[test]
    fn props_validate() {
        let mut s = sel();
        set(&mut s, "selected", Value::I64(9)).unwrap();
        assert_eq!(s.selected, None, "out of range shows the hint");
        assert!(set(&mut s, "options", Value::List(vec![Value::I64(1)])).is_err());
        assert!(SPEC.prop("nope").is_none());
        assert!(SPEC.prop_spec(P::selected.id()).unwrap().bindable);
        set(&mut s, "selected", Value::I64(1)).unwrap();
        let node = testkit::a11y(&s, StateBits::NONE, SIZE);
        assert_eq!(node.value(), Some("Banana"));
        assert_eq!(node.is_expanded(), Some(false));
    }
}
