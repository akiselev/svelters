//! `combo-box`, svelters' own select (OW3; ARCH-WIDGETS §6.1, WIDGET-PARITY §2.8): box, label,
//! chevron (R14; `content: icon(…)` per P2, closing VIS1 gap 8's filled triangle); a list of the
//! options in its own surface (`::part(popup)`, R7) at `Order::Foreground`, at least as wide as the
//! select (Anchored `match_anchor_width`, flipped above it at the bottom edge).
//!
//! Keys, closed and focused: Up/Down/Home/End pick the previous/next/first/last option, typing
//! picks by prefix (type-ahead, WG6 QW9), Enter, Space, F4 or Alt+Down open the list. Open (the
//! list holds the keyboard, focus goes back when it closes): Up/Down/Home/End/PageUp/PageDown move
//! the current row (`highlight`), typing moves it by prefix, Enter or Space picks it, Escape and
//! Tab close it. The pointer: a click opens, a row click picks, a press outside closes. The wheel
//! over the closed select changes the pick only as `wheel` says (`off` by default, as IDEs do).
//!
//! A pick dispatches `changed` (when it changes `selected`; EV1 filters can prevent it), then
//! `activate` and `commit` (§2.4: a pick is a commit). Writer lane: OW3.

use super::glyph::{self, Glyph};
use super::popup::{self, Grab, SURFACE};
use crate::interact::anchor::{self, AnchorParams, Placement};
use crate::interact::dismiss::{DismissParams, Outside};
use crate::interact::roving_list::{Orientation, RovingList, RovingListParams};
use crate::interact::typeahead::{self, Typeahead, TypeaheadParams};
use crate::interact::{Focusable, Hoverable, Pressable};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};
use egui::{Key, Modifiers, Order, Rect, Sense, vec2};

/// The closed select (VS Code's dropdown): `control-bg` fill, `control-border` boundary, radius
/// `input-radius`, 26px tall (18 + 2·3 + 2), 13px text, a pointer cursor (R10), a 1px `focus`
/// ring at −1 on keyboard focus (R8 for fields), the `focus` border while open, R9 when disabled.
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("control-bg")),
        ("color", Token("control-fg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(3.0)),
        ("padding-bottom", Px(3.0)),
        ("line-height", Px(18.0)),
        ("font-size", Token("font-size")),
        ("gap", Px(4.0)),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::OPEN, &[("border-color", Token("focus"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed"))]),
]);

/// `::part(chevron)`: `chevron-down`, 16 wide.
pub const CHEVRON_UA: Ua = Ua(&[UaRule::base(&[("font-size", Px(14.0)), ("width", Px(16.0))])]);
/// `::part(label)` while no option is picked (`hint-text`): the placeholder colour.
pub const HINT_UA: Ua = Ua(&[UaRule::base(&[("color", Token("placeholder"))])]);
/// `::part(popup)`: the list's surface (R7), 4px from the select, padding 2.
pub const POPUP_UA: Ua = Ua(&[UaRule::base(SURFACE), UaRule::base(&[("padding", Px(2.0)), ("margin", Px(2.0)), ("border-radius", Token("input-radius"))])]);
/// `::part(option)`: a 22px row (R6 flat), `list-sel` when current, `list-hover` under the pointer.
pub const OPTION_UA: Ua = Ua(&[
    UaRule::base(&[("padding-left", Px(6.0)), ("padding-right", Px(6.0)), ("line-height", Px(22.0)), ("border-radius", Px(2.0)), ("font-size", Token("font-size"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::CURRENT, &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg"))]),
]);

/// A drop-down list: shows the selected option, picks one from a list.
#[svelters::widget(namespace = "sv", replaces = "combo-box", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Select {
    /// The choices, in order.
    #[prop(measure)]
    options: Vec<String>,
    /// Index of the chosen option; out of range shows `hint-text`.
    #[prop(bind)]
    selected: i64,
    /// Shown while `selected` is out of range.
    #[prop(measure)]
    hint_text: String,
    /// The list is open; the user opening or closing it changes it, setting it opens or closes it
    /// (QComboBox `showPopup`/`hidePopup`).
    #[prop(bind)]
    open: bool,
    /// Rows shown before the list scrolls (QComboBox `maxVisibleItems`).
    #[prop(default = 30)]
    max_visible: i64,
    /// The wheel over the closed select changes the pick: `off` (default), `hover`, `focused`.
    #[prop(enumeration = "off|hover|focused", default = "off")]
    wheel: String,
    /// The user picked a different option; carries its index.
    #[event]
    changed: Event<i64>,
    /// The user picked an option, the same one or not (QComboBox `activated`); carries its index.
    #[event]
    activate: Event<i64>,
    /// The current row of the open list moved (keyboard or pointer; `highlighted`).
    #[event]
    highlight: Event<i64>,
    /// A pick finished the edit (§2.4); carries the index.
    #[event]
    commit: Event<i64>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    pressable: Pressable,
    /// The shown text (paint part).
    #[part(name = "label")]
    label_part: Part,
    /// The `chevron-down` glyph (paint part).
    #[part(name = "chevron")]
    chevron_part: Part,
    /// The list's surface (paint part, in its own layer).
    #[part(name = "popup")]
    popup_part: Part,
    /// A row of the list (paint part, repeated: `:current`, `:hover`, `:selected`).
    #[part(name = "option")]
    option_part: Part,
    roving: RovingList,
    typeahead: Typeahead,
    grab: Grab,
    /// The first row shown (the list scrolls past `max-visible`).
    first: usize,
    /// It was open last pass (open and close transitions).
    was_open: bool,
    /// The pass it opened in (its own press is not "outside").
    opened: u64,
    keyboard: bool,
}

impl Select {
    fn shown(&self) -> Option<&str> {
        usize::try_from(self.selected).ok().and_then(|k| self.options.get(k)).map(String::as_str)
    }

    fn all(&self) -> RovingListParams {
        RovingListParams { enabled: vec![true; self.options.len()], orientation: Orientation::Vertical, wrap: false }
    }

    /// Picks option `k`: `changed` (default: `selected = k`) when it differs, then `activate`
    /// and `commit`.
    fn pick(&mut self, cx: &mut WidgetCx<'_>, k: usize) {
        let k = k as i64;
        if k != self.selected && !cx.dispatch(&self.changed, k).prevented() {
            cx.set(&mut self.selected, k);
        }
        cx.dispatch(&self.activate, k);
        cx.dispatch(&self.commit, k);
    }

    fn set_open(&mut self, cx: &mut WidgetCx<'_>, on: bool, keyboard: bool) {
        cx.set(&mut self.open, on);
        if on {
            let cur = usize::try_from(self.selected).ok().filter(|k| *k < self.options.len());
            self.roving.set(cur.or(if keyboard { RovingList::edge(&self.all().enabled, true) } else { None }));
            self.keyboard = keyboard;
            self.first = 0;
        }
        self.typeahead.reset();
    }

    fn visible(&self) -> usize {
        (self.max_visible.max(1) as usize).min(self.options.len().max(1))
    }

    /// Scrolls so the current row is shown.
    fn reveal(&mut self) {
        let n = self.visible();
        if let Some(c) = self.roving.current() {
            if c < self.first {
                self.first = c;
            } else if c >= self.first + n {
                self.first = c + 1 - n;
            }
        }
        self.first = self.first.min(self.options.len().saturating_sub(n));
    }
}

impl Widget for Select {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA);
        // the widest option (QComboBox AdjustToContentsOnFirstShow), or the hint
        let w = self.options.iter().map(String::as_str).chain(std::iter::once(self.hint_text.as_str())).map(|t| paint::text_galley(cx.ctx, &s, t, None).size().x).fold(0.0, f32::max);
        let c = s.part(cx.part_css("chevron"), CHEVRON_UA);
        let cw = c.px("width").unwrap_or(16.0);
        vec2(w + s.px("gap").unwrap_or(4.0) + cw, s.px("line-height").unwrap_or(18.0)) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let ctx = ui.ctx().clone();
        let list_id = cx.id().with("ow3-list");
        self.grab.note_press(&ctx);
        // the host opened or closed it (`open`, `bind:open`)
        if self.open && !self.was_open {
            self.set_open(cx, true, false);
            self.opened = ctx.cumulative_pass_nr();
        }
        cx.set_state(StateBits::OPEN, self.open);
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        let enabled = cx.enabled();
        let n = self.options.len();
        let tp = TypeaheadParams::default();
        let now = ctx.input(|i| i.time);
        // closed: the keys of the focused select
        if enabled && !self.open && resp.has_focus() {
            let (keys, texts, alt_down) = ctx.input(|i| {
                let keys: Vec<Key> = i.events.iter().filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.any() => Some(*key),
                    _ => None,
                }).collect();
                let texts: Vec<String> = i.events.iter().filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.clone()),
                    _ => None,
                }).collect();
                (keys, texts, i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::ArrowDown, pressed: true, modifiers, .. } if modifiers.alt)))
            });
            let typing = self.typeahead.typing(&tp, now);
            for k in keys {
                let cur = usize::try_from(self.selected).ok().filter(|c| *c < n);
                let mut r = RovingList::default();
                r.set(cur);
                match k {
                    Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => {
                        if let Some(to) = r.key(&self.all(), k)
                            && Some(to) != cur
                        {
                            self.pick(cx, to);
                        }
                    }
                    Key::Space if typing => {}
                    Key::F4 => {
                        self.set_open(cx, true, true);
                        self.opened = ctx.cumulative_pass_nr();
                    }
                    _ => {}
                }
            }
            if alt_down {
                self.set_open(cx, true, true);
                self.opened = ctx.cumulative_pass_nr();
            }
            for t in texts {
                if let Some(q) = self.typeahead.push(&tp, &t, now) {
                    let cur = usize::try_from(self.selected).ok();
                    if let Some(to) = typeahead::find(self.options.iter().map(String::as_str), &[], &q, cur)
                        && Some(to) != cur
                    {
                        self.pick(cx, to);
                    }
                }
            }
        }
        // the wheel over the closed select (`wheel`)
        if enabled && !self.open && n > 0 {
            let on = match self.wheel.as_str() {
                "hover" => resp.hovered(),
                "focused" => resp.hovered() && resp.has_focus(),
                _ => false,
            };
            let dy = if on { ctx.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
            if dy != 0.0 {
                let cur = self.selected.clamp(0, n as i64 - 1);
                let to = (cur + if dy > 0.0 { -1 } else { 1 }).clamp(0, n as i64 - 1) as usize;
                if to as i64 != self.selected {
                    self.pick(cx, to);
                }
                ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        // a click, AccessKit `Click`, or Enter/Space on the focused select (egui reports them as clicks)
        let kb = ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space));
        let typing_space = ctx.input(|i| i.key_pressed(Key::Space)) && self.typeahead.typing(&tp, now);
        if enabled && fired.pressable.iter().any(|p| p.is_click()) && !typing_space {
            let open = !self.open;
            self.set_open(cx, open, kb);
            if open {
                self.opened = ctx.cumulative_pass_nr();
            }
        }
        if !enabled && self.open {
            self.set_open(cx, false, false);
        }
        // open: the list holds the keyboard
        if self.open {
            if !self.was_open || !self.grab.active {
                self.grab.take(&ctx, list_id);
            }
            // (not in the pass it opened: the key that opened it is not a pick)
            if self.was_open && ctx.memory(|m| m.has_focus(list_id)) {
                let (keys, texts) = ctx.input(|i| {
                    let keys: Vec<Key> = i.events.iter().filter_map(|e| match e {
                        egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.ctrl && !modifiers.command => Some(*key),
                        _ => None,
                    }).collect();
                    let texts: Vec<String> = i.events.iter().filter_map(|e| match e {
                        egui::Event::Text(t) => Some(t.clone()),
                        _ => None,
                    }).collect();
                    (keys, texts)
                });
                let before = self.roving.current();
                for k in keys {
                    let typing = self.typeahead.typing(&tp, now);
                    match k {
                        Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => {
                            self.roving.key(&self.all(), k);
                            self.keyboard = true;
                        }
                        Key::PageUp | Key::PageDown => {
                            let step = self.visible() as i64 * if k == Key::PageUp { -1 } else { 1 };
                            let c = self.roving.current().map_or(0, |c| c as i64);
                            self.roving.set(Some((c + step).clamp(0, n.saturating_sub(1) as i64) as usize).filter(|_| n > 0));
                            self.keyboard = true;
                        }
                        Key::Space if typing => {}
                        Key::Enter | Key::Space => {
                            if let Some(c) = self.roving.current() {
                                self.set_open(cx, false, false);
                                self.pick(cx, c);
                            }
                        }
                        Key::Escape | Key::Tab | Key::F4 => self.set_open(cx, false, false),
                        _ => {}
                    }
                    if !self.open {
                        break;
                    }
                }
                if self.open {
                    for t in texts {
                        if let Some(q) = self.typeahead.push(&tp, &t, now)
                            && let Some(to) = typeahead::find(self.options.iter().map(String::as_str), &[], &q, self.roving.current())
                        {
                            self.roving.set(Some(to));
                            self.keyboard = true;
                        }
                    }
                }
                if self.roving.current() != before
                    && let Some(c) = self.roving.current()
                {
                    cx.dispatch(&self.highlight, c as i64);
                }
                ctx.input_mut(|i| {
                    for k in [Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End, Key::PageUp, Key::PageDown, Key::Enter, Key::Space, Key::Escape, Key::Tab, Key::F4] {
                        i.consume_key(Modifiers::NONE, k);
                    }
                    i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
                });
            }
        }
        if self.open {
            self.reveal();
            self.list(ui, cx, &resp, list_id);
        }
        if !self.open && self.grab.active {
            self.grab.release(&ctx, list_id);
        }
        self.was_open = self.open;
        cx.set_state(StateBits::OPEN, self.open);
        cx.commit_state();
        // the closed select
        let host = cx.style(UA);
        if host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ctx.set_cursor_icon(c);
        }
        if host.opacity() < 1.0 {
            ui.multiply_opacity(host.opacity());
        }
        if ui.is_rect_visible(cx.rect()) {
            let g = cx.paint_box(ui, UA);
            let c = host.part(cx.part_css("chevron"), CHEVRON_UA);
            let cw = c.px("width").unwrap_or(16.0);
            let cr = Rect::from_min_max(egui::pos2(g.content.max.x - cw, g.content.min.y), g.content.max);
            let mut pc = cx.paint_cx(ui);
            glyph::paint(&mut pc.part("chevron"), &c, Glyph::ChevronDown, cr);
            let lr = Rect::from_min_max(g.content.min, egui::pos2(cr.min.x - host.px("gap").unwrap_or(4.0), g.content.max.y));
            let (text, l) = match self.shown() {
                Some(t) => (t.to_string(), host.part(cx.part_css("label"), Ua::NONE)),
                None => (self.hint_text.clone(), host.part(cx.part_css("label"), HINT_UA)),
            };
            let lg = paint::text_galley(&ctx, &l, &text, None);
            let clip = ui.painter().with_clip_rect(lr.intersect(ui.clip_rect()));
            let mut lpc = paint::PaintCx::for_node(&clip, pc.target.0);
            lpc.tag = "combo-box";
            paint::paint_text(&mut lpc.part("label"), &l, &lg, lr, Align::Start);
            cx.paint_outline(ui, UA);
        }
        let (enabled, value, open) = (cx.enabled(), self.shown().unwrap_or("").to_string(), self.open);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, enabled, &value));
        ctx.accesskit_node_builder(resp.id, |n| {
            n.set_value(value.clone());
            n.set_expanded(open);
            n.set_has_popup(egui::accesskit::HasPopup::Listbox);
        });
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::ComboBox);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: Key, m: Modifiers) -> bool {
        self.units_claims(key, m) || (!m.any() && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::F4 | Key::Enter | Key::Space)) || (self.open && key == Key::Escape)
    }
}

impl Select {
    /// The open list: its surface below (or above) the select, the visible rows, the pointer.
    fn list(&mut self, ui: &egui::Ui, cx: &mut WidgetCx<'_>, host: &egui::Response, list_id: egui::Id) {
        let ctx = ui.ctx().clone();
        let n = self.options.len();
        let vis = self.visible().min(n);
        let (frame, rh) = {
            let hs = cx.style(UA);
            let surf = hs.part(cx.part_css("popup"), POPUP_UA);
            let row0 = StyleView::new(cx.part_css("option"), OPTION_UA, StateBits::NONE, hs.is_dark());
            let rh = row0.px("line-height").unwrap_or(22.0) + row0.sides("padding")[2] + row0.sides("padding")[3];
            let w = self.options.iter().map(|t| paint::text_galley(&ctx, &row0, t, None).size().x).fold(0.0, f32::max) + paint::box_extra(&row0).x;
            let size = vec2(w, rh * vis as f32) + paint::box_extra(&surf);
            let placement = Placement::BOTTOM_START;
            let placed = anchor::place(&AnchorParams { anchor: host.rect, size, bounds: anchor::bounds(&ctx), placement, gap: popup::gap(&surf, placement.side), match_anchor_width: true, ..AnchorParams::default() });
            (placed.rect, rh)
        };
        // a press outside the list and the select closes it (not the press that opened it)
        if self.opened != ctx.cumulative_pass_nr() {
            let dp = DismissParams { inside: vec![frame, host.rect], on_outside: Outside::Close, escape: false };
            if popup::dismissed(&ctx, &dp).is_some() {
                self.set_open(cx, false, false);
                return;
            }
        }
        let node = cx.paint_cx(ui).target.0;
        let hs = cx.style(UA);
        let surf = hs.part(cx.part_css("popup"), POPUP_UA);
        let row_css = cx.part_css("option");
        let geom = paint::box_geom(&surf, frame);
        let (moved, scroll) = ctx.input(|i| (i.pointer.delta() != egui::Vec2::ZERO, i.smooth_scroll_delta.y));
        let mut picked = None;
        let before = self.roving.current();
        let selected = usize::try_from(self.selected).ok();
        let keyboard = self.keyboard;
        let mut current = self.roving.current();
        let mut first = self.first;
        let mut hovered_list = false;
        popup::area(&ctx, list_id.with("area"), Order::Foreground, frame, |lui| {
            let mut pc = paint::PaintCx::for_node(lui.painter(), node);
            pc.tag = "combo-box";
            paint::paint_box(&mut pc.part("popup"), &surf, frame);
            self.grab.keep(lui, list_id, frame);
            hovered_list = lui.rect_contains_pointer(frame);
            if hovered_list && scroll != 0.0 && n > vis {
                let step = if scroll > 0.0 { -1i64 } else { 1 };
                first = (first as i64 + step).clamp(0, (n - vis) as i64) as usize;
            }
            let mut active = None;
            for (row, k) in (first..(first + vis).min(n)).enumerate() {
                let r = Rect::from_min_size(egui::pos2(geom.content.left(), geom.content.top() + row as f32 * rh), vec2(geom.content.width(), rh));
                let resp = lui.interact(r, list_id.with(("option", k)), Sense::click());
                if resp.hovered() && (moved || current.is_none()) {
                    current = Some(k);
                }
                let mut bits = StateBits::NONE;
                bits.set(StateBits::CURRENT, current == Some(k));
                bits.set(StateBits::HOVERED, resp.hovered());
                bits.set(StateBits::SELECTED, selected == Some(k));
                bits.set(StateBits::FOCUS_VISIBLE, current == Some(k) && keyboard);
                let v = StyleView::new(row_css, OPTION_UA, bits, hs.is_dark());
                let g = paint::paint_box(&mut pc.part("option"), &v, r);
                let text = &self.options[k];
                let gal = paint::text_galley(lui.ctx(), &v, text, None);
                paint::paint_text(&mut pc.part("option"), &v, &gal, g.content, Align::Start);
                let sel = selected == Some(k);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, sel, text));
                if current == Some(k) {
                    active = Some(resp.id);
                }
                if resp.clicked() {
                    picked = Some(k);
                }
            }
            lui.ctx().accesskit_node_builder(list_id, |nb| {
                nb.set_role(egui::accesskit::Role::ListBox);
                if let Some(a) = active {
                    nb.set_active_descendant(a.accesskit_id());
                }
            });
        });
        if hovered_list && scroll != 0.0 {
            ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        }
        self.first = first;
        self.roving.set(current);
        if current != before
            && let Some(c) = current
        {
            self.keyboard = false;
            cx.dispatch(&self.highlight, c as i64);
        }
        if let Some(k) = picked {
            self.set_open(cx, false, false);
            self.pick(cx, k);
        }
    }
}
