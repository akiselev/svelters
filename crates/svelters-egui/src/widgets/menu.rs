//! OW3: svelters' own menus (ARCH-WIDGETS §6.1 `menu-bar`, `menu`, `action`, `menu-separator`,
//! `context-menu`, `menu-button`; WIDGET-PARITY §2.14). The WG2a data model stays: the entries are
//! scene children of their root, out of flow, and `crate::menus` keeps their props, shortcuts and
//! the pick logic (check items, radio groups, `activate`). This module draws and drives them with
//! no egui menu, popup or button:
//!
//! - every open level is an `egui::Area` at `Order::Foreground` ([`popup::area`]), placed by
//!   [`Anchored`](crate::interact::Anchored) (`bottom-start` under a title or button, at the
//!   pointer for a context menu, `right-start` with flip for a submenu);
//! - rows are the entry nodes themselves, styled by CSS on `action`, `menu` and `menu-separator`
//!   (`:current`, `:hover`, `:checked`, `:open`, `:disabled`, `:focus-visible`) over the UA tables
//!   below, with paint parts `check`, `shortcut` and `chevron` (`content: icon(…)` per P2);
//! - keys: Up/Down/Home/End ([`Roving`], disabled rows reachable as R9 asks, separators skipped),
//!   Right/Left into and out of submenus (and across the bar's menus), Enter/Space pick or open,
//!   Escape closes one level, type-ahead ([`Typeahead`]); F10 (and a lone Alt where the policy
//!   says so) puts the bar in keyboard mode; Shift+F10 and the Menu key open a context menu on its
//!   focused owner (FM1 QA8);
//! - pointer: a submenu opens after the policy's delay on hover (at once on click), another row
//!   closes it after the same delay, and the pointer moving towards an open submenu (the "safe
//!   triangle") does not change the row; a press outside every level dismisses
//!   ([`Dismissable`](crate::interact::Dismissable));
//! - while open, the menu holds keyboard focus ([`Grab`]) and gives it back when it closes (EV3).
//!
//! Writer lane: OW3.

use super::glyph::{self, Glyph};
use super::popup::{self, Grab, SURFACE};
use crate::interact::anchor::{self, AnchorParams, Placement};
use crate::interact::dismiss::{DismissParams, Outside};
use crate::interact::roving::{Orientation, Roving, RovingParams};
use crate::interact::typeahead::{self, Typeahead, TypeaheadParams};
use crate::layout::Metrics;
use crate::menus::{self, MenuW};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::{Arena, NodeId, W};
use crate::walk::Out;
use egui::{Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Ui, Vec2, accesskit, vec2};
use svelters_abi::Value;

/// A menu's surface: [`popup::SURFACE`] (R7) at least 160 wide.
pub const MENU_UA: Ua = Ua(&[UaRule::base(SURFACE), UaRule::base(&[("min-width", Px(160.0))])]);

const CUR_DISABLED: StateBits = StateBits(StateBits::CURRENT.0 | StateBits::DISABLED.0);

/// A row (`action`, a `menu`'s submenu row): VS Code's 2em rows (22 + 2·2), 8px sides, radius 4;
/// the current row (keyboard or pointer) in `menu-sel` / `menu-sel-fg`; a disabled current row
/// only tinted (R9: reachable, not pickable).
pub const ITEM_UA: Ua = Ua(&[
    UaRule::base(&[
        ("padding-left", Px(8.0)),
        ("padding-right", Px(8.0)),
        ("padding-top", Px(2.0)),
        ("padding-bottom", Px(2.0)),
        ("line-height", Px(22.0)),
        ("border-radius", Px(4.0)),
        ("gap", Px(6.0)),
        ("font-size", Token("font-size")),
    ]),
    UaRule::when_not(StateBits::CURRENT, StateBits::DISABLED, &[("background-color", Token("menu-sel")), ("color", Token("menu-sel-fg"))]),
    UaRule::when(CUR_DISABLED, &[("background-color", Token("list-hover"))]),
]);

/// `::part(check)`: the check column (a check for checked check and radio items).
pub const CHECK_UA: Ua = Ua(&[UaRule::base(&[("font-size", Px(14.0)), ("width", Px(16.0))])]);
/// `::part(shortcut)`: the accelerator, right-aligned, muted unless the row is current.
pub const SHORTCUT_UA: Ua = Ua(&[UaRule::base(&[("margin-left", Px(24.0))]), UaRule::when_not(StateBits::NONE, StateBits::CURRENT, &[("color", Token("muted"))])]);
/// `::part(chevron)`: a submenu row's `chevron-right` (R14), a menu-button's `chevron-down`.
pub const CHEVRON_UA: Ua = Ua(&[UaRule::base(&[("font-size", Px(14.0)), ("width", Px(16.0)), ("margin-left", Px(8.0))])]);
/// `menu-separator`: a 1px `menu-border-color` line with 4px above and below.
pub const SEPARATOR_UA: Ua = Ua(&[UaRule::base(&[("height", Px(1.0)), ("margin-top", Px(4.0)), ("margin-bottom", Px(4.0)), ("background-color", Token("menu-border-color"))])]);
/// `menu-bar`: 4px around its titles (a 30px bar, VS Code's title-bar menus).
pub const BAR_UA: Ua = Ua(&[UaRule::base(&[("padding", Px(4.0)), ("gap", Px(0.0))])]);
/// A bar title (the `menu` node in the bar): 8px sides, radius 4, `toolbar-hover` when hovered,
/// open or current.
pub const TITLE_UA: Ua = Ua(&[
    UaRule::base(&[("padding-left", Px(8.0)), ("padding-right", Px(8.0)), ("line-height", Px(22.0)), ("border-radius", Px(4.0)), ("font-size", Token("font-size"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::OPEN, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::CURRENT, &[("background-color", Token("toolbar-hover"))]),
]);

// ---- rows ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Action,
    Sub,
    Sep,
}

#[derive(Clone, Copy, Debug)]
struct Row {
    i: usize,
    kind: Kind,
    enabled: bool,
}

fn collect(arena: &Arena, p: usize, flatten: bool, v: &mut Vec<Row>) {
    for k in 0..arena.children[p].len() {
        let c = arena.children[p][k].index() as usize;
        let cm = arena.common(c);
        if !cm.visible {
            continue;
        }
        let enabled = cm.enabled;
        match arena.w(c) {
            W::Menu(MenuW::Menu { .. }) if flatten => collect(arena, c, false, v),
            W::Menu(MenuW::Menu { .. }) => v.push(Row { i: c, kind: Kind::Sub, enabled }),
            W::Menu(MenuW::Action { .. }) => v.push(Row { i: c, kind: Kind::Action, enabled }),
            W::Menu(MenuW::Separator) => v.push(Row { i: c, kind: Kind::Sep, enabled }),
            _ => {}
        }
    }
}

/// The rows of `container` (`flatten`: its `menu` children inline, a menu-button's menu), with
/// leading, trailing and doubled separators collapsed (QMenu `separatorsCollapsible`).
fn rows(arena: &Arena, container: usize, flatten: bool) -> Vec<Row> {
    let mut v = Vec::new();
    collect(arena, container, flatten, &mut v);
    let mut out: Vec<Row> = Vec::with_capacity(v.len());
    for r in v {
        if r.kind == Kind::Sep && out.last().is_none_or(|l| l.kind == Kind::Sep) {
            continue;
        }
        out.push(r);
    }
    while out.last().is_some_and(|l| l.kind == Kind::Sep) {
        out.pop();
    }
    out
}

fn label(arena: &Arena, i: usize) -> String {
    match arena.w(i) {
        W::Menu(m) => m.label().to_string(),
        _ => String::new(),
    }
}

/// (checkable or radio, checked, radio, shortcut text).
fn action(arena: &Arena, i: usize, ctx: &Context) -> (bool, bool, bool, Option<String>) {
    match arena.w(i) {
        W::Menu(MenuW::Action { checkable, checked, group, shortcut, .. }) => {
            let sc = menus::parse_shortcut(shortcut).ok().flatten().map(|s| ctx.format_shortcut(&s));
            (*checkable || !group.is_empty(), *checked, !group.is_empty(), sc)
        }
        _ => (false, false, false, None),
    }
}

fn roving(rows: &[Row]) -> RovingParams {
    RovingParams { enabled: rows.iter().map(|r| r.kind != Kind::Sep).collect(), orientation: Orientation::Vertical, wrap: true }
}

// ---- state ----

/// One open level: the container whose rows it shows.
#[derive(Clone)]
struct Level {
    menu: NodeId,
    flatten: bool,
    /// The submenu row that opened it (levels after the first).
    opener: Option<NodeId>,
    roving: Roving,
    /// Where it was drawn (last pass).
    rect: Rect,
}

/// A menu root's open state, in the context's temp data under the root's id.
#[derive(Clone, Default)]
struct State {
    levels: Vec<Level>,
    grab: Grab,
    /// The keyboard drives it (rows show `:focus-visible`; opening a level selects its first row).
    keyboard: bool,
    /// The bar's current title (keyboard mode or the open one) and whether the bar has the
    /// keyboard with no menu open (F10).
    bar: Roving,
    bar_keys: bool,
    /// Alt went down with nothing else pressed (a lone Alt tap focuses the bar).
    alt: bool,
    /// The row under the pointer and since when (submenu open and close delays).
    hover: Option<(NodeId, f64)>,
    typeahead: Typeahead,
    /// A context menu's anchor.
    anchor: Option<Rect>,
    /// The pass it opened in.
    opened: u64,
}

/// One root's pass.
struct Menu<'a> {
    arena: &'a mut Arena,
    out: &'a mut Vec<Out>,
    ctx: Context,
    key: Id,
    focus: Id,
    st: State,
    delay: f64,
    now: f64,
    dark: bool,
}

impl<'a> Menu<'a> {
    fn load(arena: &'a mut Arena, root: usize, ui: &Ui, out: &'a mut Vec<Out>) -> Menu<'a> {
        let ctx = ui.ctx().clone();
        let key = arena.core[root].egui_id.with("ow3-menu");
        let mut st = ctx.data(|d| d.get_temp::<State>(key)).unwrap_or_default();
        st.grab.note_press(&ctx);
        let delay = crate::native::policy_of(&ctx).submenu_delay_ms as f64 / 1000.0;
        let now = ctx.input(|i| i.time);
        let dark = popup::dark(&ctx);
        Menu { arena, out, ctx, key, focus: key.with("focus"), st, delay, now, dark }
    }

    fn save(self) {
        let (key, st) = (self.key, self.st);
        self.ctx.data_mut(|d| d.insert_temp(key, st));
    }

    fn is_open(&self) -> bool {
        !self.st.levels.is_empty()
    }

    fn idx(&self, id: NodeId) -> Option<usize> {
        self.arena.idx(id)
    }

    fn level_rows(&self, k: usize) -> Vec<Row> {
        let l = &self.st.levels[k];
        self.idx(l.menu).map_or_else(Vec::new, |m| rows(self.arena, m, l.flatten))
    }

    /// Opens `menu` as the only level (`keyboard`: its first row current).
    fn open(&mut self, menu: usize, flatten: bool, keyboard: bool) {
        self.clear_bits();
        let Some(id) = self.arena.slots.id_at(menu) else { return };
        self.st.levels = vec![Level { menu: id, flatten, opener: None, roving: Roving::default(), rect: Rect::NOTHING }];
        self.st.keyboard = keyboard;
        self.st.hover = None;
        self.st.typeahead.reset();
        if keyboard {
            self.first(0);
        }
        self.st.grab.take(&self.ctx, self.focus);
        self.st.opened = self.ctx.cumulative_pass_nr();
        self.ctx.request_repaint();
    }

    /// Opens submenu row `row` above level `k` (closing deeper levels).
    fn push(&mut self, k: usize, row: usize, keyboard: bool) {
        self.truncate(k + 1);
        let Some(id) = self.arena.slots.id_at(row) else { return };
        self.st.levels.push(Level { menu: id, flatten: false, opener: Some(id), roving: Roving::default(), rect: Rect::NOTHING });
        if keyboard {
            self.st.keyboard = true;
            self.first(k + 1);
        }
        self.ctx.request_repaint();
    }

    fn first(&mut self, k: usize) {
        let rows = self.level_rows(k);
        let to = Roving::edge(&roving(&rows).enabled, true);
        self.st.levels[k].roving.set(to);
    }

    /// Keeps levels `..n`; clears the state bits of the rows of the closed ones.
    fn truncate(&mut self, n: usize) {
        while self.st.levels.len() > n {
            let k = self.st.levels.len() - 1;
            for r in self.level_rows(k) {
                self.set_bits(r.i, StateBits::NONE);
            }
            self.st.levels.pop();
        }
    }

    fn clear_bits(&mut self) {
        self.truncate(0);
    }

    fn close_all(&mut self) {
        self.clear_bits();
        self.st.keyboard = false;
        self.st.hover = None;
        self.st.typeahead.reset();
        if !self.st.bar_keys {
            self.st.grab.release(&self.ctx, self.focus);
        }
        self.ctx.request_repaint();
    }

    fn set_bits(&mut self, i: usize, bits: StateBits) {
        if self.arena.core[i].state != bits {
            crate::style::set_own_state(self.arena, i, bits);
        }
    }

    /// The user picked action `i`: the menus close (focus back), then `crate::menus::activate`.
    fn pick(&mut self, i: usize) {
        self.st.bar_keys = false;
        self.close_all();
        menus::activate(self.arena, i, &self.ctx, self.out);
    }

    fn style(&self, i: usize, ua: Ua, state: StateBits) -> (Option<std::sync::Arc<svelters_style::ComputedStyle>>, Ua, StateBits) {
        let css = self.arena.slots.id_at(i).and_then(|id| popup::css(self.arena, id));
        (css, ua, state)
    }

    // ---- keys ----

    /// The keys of this pass for the deepest level (the menu holds focus).
    fn keys(&mut self, bar: Option<&[Row]>) {
        // (not in the pass it opened: the key that opened it is not a pick)
        if !self.is_open() || self.st.opened == self.ctx.cumulative_pass_nr() || !self.ctx.memory(|m| m.has_focus(self.focus)) {
            return;
        }
        let (keys, texts) = self.ctx.input(|i| {
            let keys: Vec<Key> = i.events.iter().filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.ctrl && !modifiers.alt && !modifiers.command && !modifiers.mac_cmd => Some(*key),
                _ => None,
            }).collect();
            let texts: Vec<String> = i.events.iter().filter_map(|e| match e {
                egui::Event::Text(t) => Some(t.clone()),
                _ => None,
            }).collect();
            (keys, texts)
        });
        let tp = TypeaheadParams::default();
        let mut used = Vec::new();
        for key in keys {
            if !self.is_open() {
                break;
            }
            let d = self.st.levels.len() - 1;
            let rows = self.level_rows(d);
            let cur = self.st.levels[d].roving.current().and_then(|c| rows.get(c).copied());
            let handled = match key {
                Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => {
                    self.st.levels[d].roving.key(&roving(&rows), key);
                    self.truncate(d + 1);
                    true
                }
                Key::ArrowRight => {
                    match (cur, bar) {
                        (Some(r), _) if r.kind == Kind::Sub && r.enabled => self.push(d, r.i, true),
                        (_, Some(titles)) => self.bar_step(titles, 1),
                        _ => {}
                    }
                    true
                }
                Key::ArrowLeft => {
                    match bar {
                        _ if d > 0 => self.truncate(d),
                        Some(titles) => self.bar_step(titles, -1),
                        None => {}
                    }
                    true
                }
                Key::Space if self.st.typeahead.typing(&tp, self.now) => false,
                Key::Enter | Key::Space => {
                    match cur {
                        Some(r) if r.kind == Kind::Sub && r.enabled => self.push(d, r.i, true),
                        Some(r) if r.kind == Kind::Action && r.enabled => self.pick(r.i),
                        _ => {}
                    }
                    true
                }
                Key::Escape => {
                    if d > 0 {
                        self.truncate(d);
                    } else {
                        self.st.bar_keys = false;
                        self.close_all();
                    }
                    true
                }
                Key::Tab => true,
                _ => false,
            };
            if handled {
                self.st.keyboard = true;
                used.push(key);
            }
        }
        if self.is_open() {
            let d = self.st.levels.len() - 1;
            for t in texts {
                let Some(q) = self.st.typeahead.push(&tp, &t, self.now) else { continue };
                let rows = self.level_rows(d);
                let labels: Vec<String> = rows.iter().map(|r| if r.kind == Kind::Sep { String::new() } else { label(self.arena, r.i) }).collect();
                let enabled = roving(&rows).enabled;
                if let Some(to) = typeahead::find(labels.iter().map(String::as_str), &enabled, &q, self.st.levels[d].roving.current()) {
                    self.st.levels[d].roving.set(Some(to));
                    self.truncate(d + 1);
                    self.st.keyboard = true;
                }
            }
        }
        self.ctx.input_mut(|i| {
            for k in &used {
                i.consume_key(Modifiers::NONE, *k);
                i.consume_key(Modifiers::SHIFT, *k);
            }
            i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
        });
    }

    /// The bar's next (`step` 1) or previous title with a menu, opened from the keyboard.
    fn bar_step(&mut self, titles: &[Row], step: i32) {
        let enabled: Vec<bool> = titles.iter().map(|t| t.kind == Kind::Sub && t.enabled).collect();
        let to = Roving::next(&enabled, self.st.bar.current(), step, true);
        if let Some(to) = to {
            self.st.bar.set(Some(to));
            if self.is_open() {
                self.open(titles[to].i, false, true);
            }
        }
    }

    // ---- painting ----

    /// Paints every open level, level 0 next to `anchor` (`placement`, `gap`: `None` for the
    /// surface's margin), and drives the pointer. Returns the rects of the levels.
    fn paint(&mut self, anchor: Rect, placement: Placement, gap: Option<f32>) -> Vec<Rect> {
        let mut rects = Vec::new();
        let mut anchor = (anchor, placement, gap, 0.0);
        let pointer = self.ctx.input(|i| (i.pointer.latest_pos(), i.pointer.delta() != Vec2::ZERO || i.pointer.any_pressed()));
        let mut under: Option<(usize, Row)> = None;
        let mut k = 0;
        while k < self.st.levels.len() {
            let (r, next, hovered) = self.level(k, anchor, pointer);
            rects.push(r);
            if let Some(h) = hovered {
                under = Some((k, h));
            }
            match next {
                Some(row) if k + 1 < self.st.levels.len() => {
                    // the submenu goes beside its row, its first row level with it
                    let s = StyleView::ua_only(MENU_UA, StateBits::NONE, self.dark);
                    let [_, _, pt, _] = s.sides("padding");
                    let [_, _, bt, _] = s.sides("border-width");
                    anchor = (row, Placement::RIGHT_START, Some(0.0), -(pt + bt));
                }
                _ => {
                    self.truncate(k + 1);
                }
            }
            k += 1;
        }
        self.hover_delays(under, pointer.0);
        rects
    }

    /// Opens and closes submenus after the hover delay.
    fn hover_delays(&mut self, under: Option<(usize, Row)>, pos: Option<Pos2>) {
        let Some((k, row)) = under else {
            self.st.hover = None;
            return;
        };
        let _ = pos;
        let id = self.arena.slots.id_at(row.i);
        let since = match self.st.hover {
            Some((h, t)) if Some(h) == id => t,
            _ => {
                self.st.hover = id.map(|h| (h, self.now));
                self.now
            }
        };
        let open_here = self.st.levels.get(k + 1).and_then(|l| l.opener);
        let wants = row.kind == Kind::Sub && row.enabled;
        let done = if wants { open_here == id } else { open_here.is_none() };
        if done {
            return;
        }
        let left = since + self.delay - self.now;
        if left > 0.0 {
            self.ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
            return;
        }
        if wants {
            self.push(k, row.i, false);
        } else {
            self.truncate(k + 1);
        }
    }

    /// Paints level `k` next to `anchor`; returns its rect, the rect of the row whose submenu is
    /// open (the next level's anchor) and the row under the pointer.
    #[allow(clippy::type_complexity)]
    fn level(&mut self, k: usize, (anchor, placement, gap, cross): (Rect, Placement, Option<f32>, f32), (ptr, moved): (Option<Pos2>, bool)) -> (Rect, Option<Rect>, Option<Row>) {
        let ctx = self.ctx.clone();
        let rows = self.level_rows(k);
        let Some(host) = self.idx(self.st.levels[k].menu) else { return (Rect::NOTHING, None, None) };
        // the surface: a context menu's own node, else the `::part(popup)` of its menu or button
        let host_id = self.arena.slots.id_at(host).expect("live node");
        let surface_css = if matches!(self.arena.w(host), W::Menu(MenuW::Context { .. })) { popup::css(self.arena, host_id) } else { popup::part_css(self.arena, host_id, "popup") };
        let surf = StyleView::new(surface_css.as_deref(), MENU_UA, StateBits::OPEN, self.dark);
        let [pl, pr, pt, pb] = surf.sides("padding");
        let [bl, br, bt, bb] = surf.sides("border-width");
        // measure the rows
        let checks = rows.iter().any(|r| action(self.arena, r.i, &ctx).0);
        let mut heights = Vec::with_capacity(rows.len());
        let mut width: f32 = 0.0;
        for r in &rows {
            let (css, ua, _) = self.style(r.i, if r.kind == Kind::Sep { SEPARATOR_UA } else { ITEM_UA }, StateBits::NONE);
            let v = StyleView::new(css.as_deref(), ua, StateBits::NONE, self.dark);
            if r.kind == Kind::Sep {
                let [ml, mr, mt, mb] = v.sides("margin");
                heights.push(v.px("height").unwrap_or(1.0) + mt + mb);
                width = width.max(ml + mr);
                continue;
            }
            let [rl, rr, rt, rb] = v.sides("padding");
            let lh = v.px("line-height").unwrap_or(v.font_size() * 1.4);
            heights.push(lh + rt + rb);
            let g = paint::text_galley(&ctx, &v, &label(self.arena, r.i), None);
            let mut w = rl + rr + g.size().x;
            if checks {
                let c = v.part(None, CHECK_UA);
                w += c.px("width").unwrap_or(16.0) + v.px("gap").unwrap_or(6.0);
            }
            if let (_, _, _, Some(sc)) = action(self.arena, r.i, &ctx) {
                let s = v.part(None, SHORTCUT_UA);
                w += s.sides("margin")[0] + paint::text_galley(&ctx, &s, &sc, None).size().x;
            }
            if r.kind == Kind::Sub {
                let c = v.part(None, CHEVRON_UA);
                w += c.sides("margin")[0] + c.px("width").unwrap_or(16.0);
            }
            width = width.max(w);
        }
        let inner = vec2(width.max(surf.px("min-width").unwrap_or(0.0) - pl - pr - bl - br), heights.iter().sum());
        let size = inner + vec2(pl + pr + bl + br, pt + pb + bt + bb);
        let gap = gap.unwrap_or_else(|| popup::gap(&surf, placement.side));
        let placed = anchor::place(&AnchorParams { anchor, size, bounds: anchor::bounds(&ctx), placement, gap, cross, ..AnchorParams::default() });
        let frame = placed.rect;
        self.st.levels[k].rect = frame;
        // the row rects
        let mut y = frame.top() + bt + pt;
        let x = (frame.left() + bl + pl, frame.right() - br - pr);
        let rects: Vec<Rect> = heights.iter().map(|h| {
            let r = Rect::from_min_max(egui::pos2(x.0, y), egui::pos2(x.1, y + h));
            y += h;
            r
        }).collect();
        let deepest = k + 1 == self.st.levels.len();
        let open_sub = self.st.levels.get(k + 1).and_then(|l| l.opener);
        let next_rect = self.st.levels.get(k + 1).and_then(|l| (l.rect != Rect::NOTHING).then_some(l.rect));
        let level_id = self.key.with(("level", k));
        let mut next = None;
        let mut under = None;
        let focus = self.focus;
        let keyboard = self.st.keyboard;
        let mut active = None;
        popup::area(&ctx, level_id, Order::Foreground, frame, |ui| {
            let mut pc = paint::PaintCx::for_node(ui.painter(), host_id.to_raw());
            pc.tag = "menu";
            paint::paint_box(&mut pc.part("popup"), &surf, frame);
            // the level's AccessKit node (a `Menu`); the deepest one also holds the keyboard
            let lid = if deepest { focus } else { level_id.with("surface") };
            let lr = if deepest && self.st.grab.active { self.st.grab.keep(ui, focus, frame); ui.interact(frame, lid, Sense::focusable_noninteractive()) } else { ui.interact(frame, lid, Sense::hover()) };
            let _ = lr;
            let mut current = self.st.levels[k].roving.current();
            // rows: interact first (the pointer may move the current row), then paint
            let mut resps = Vec::with_capacity(rows.len());
            for (n, (r, rect)) in rows.iter().zip(&rects).enumerate() {
                if r.kind == Kind::Sep {
                    resps.push(None);
                    continue;
                }
                let resp = ui.interact(*rect, self.arena.core[r.i].egui_id, Sense::click());
                if resp.hovered() {
                    under = Some(*r);
                    // the safe triangle: moving from the open submenu's row towards it keeps it
                    let aiming = matches!((open_sub, next_rect, ptr), (Some(_), Some(sub), Some(p)) if current.is_some_and(|c| c != n) && towards(p, ctx.input(|i| i.pointer.delta()), sub));
                    if (moved || current.is_none()) && !aiming {
                        current = Some(n);
                        self.st.keyboard = false;
                    } else if aiming {
                        under = None;
                    }
                }
                resps.push(Some(resp));
            }
            self.st.levels[k].roving.set(current);
            for (n, (r, rect)) in rows.iter().zip(&rects).enumerate() {
                let node_id = self.arena.slots.id_at(r.i);
                if r.kind == Kind::Sep {
                    let (css, ua, _) = self.style(r.i, SEPARATOR_UA, StateBits::NONE);
                    let v = StyleView::new(css.as_deref(), ua, StateBits::NONE, self.dark);
                    let [ml, mr, mt, _] = v.sides("margin");
                    let line = Rect::from_min_size(egui::pos2(rect.left() + ml, rect.top() + mt), vec2(rect.width() - ml - mr, v.px("height").unwrap_or(1.0)));
                    let mut pc = paint::PaintCx::for_node(ui.painter(), node_id.map_or(0, |i| i.to_raw()));
                    pc.tag = "menu-separator";
                    paint::paint_box(&mut pc, &v, line);
                    continue;
                }
                let (checkable, checked, radio, sc) = action(self.arena, r.i, &ctx);
                let is_cur = current == Some(n);
                let resp = resps[n].clone().expect("a row's response");
                let mut bits = StateBits::NONE;
                bits.set(StateBits::DISABLED, !r.enabled);
                bits.set(StateBits::CURRENT, is_cur);
                bits.set(StateBits::HOVERED, resp.hovered());
                bits.set(StateBits::CHECKED, checked);
                bits.set(StateBits::OPEN, r.kind == Kind::Sub && open_sub == node_id);
                bits.set(StateBits::FOCUS_VISIBLE, is_cur && keyboard);
                self.set_bits(r.i, bits);
                let (css, ua, _) = self.style(r.i, ITEM_UA, bits);
                let v = StyleView::new(css.as_deref(), ua, bits, self.dark);
                let mut pc = paint::PaintCx::for_node(ui.painter(), node_id.map_or(0, |i| i.to_raw()));
                pc.tag = if r.kind == Kind::Sub { "menu" } else { "action" };
                let g = paint::box_geom(&v, *rect);
                paint::paint_box(&mut pc, &v, *rect);
                let mut content = g.content;
                if checks {
                    let c = v.part(None, CHECK_UA);
                    let w = c.px("width").unwrap_or(16.0);
                    let cr = Rect::from_min_size(content.min, vec2(w, content.height()));
                    if checked {
                        glyph::paint(&mut pc.part("check"), &c, if radio && c.keyword("content").is_some() { Glyph::Dot } else { Glyph::Check }, cr);
                    }
                    content.min.x += w + v.px("gap").unwrap_or(6.0);
                }
                if r.kind == Kind::Sub {
                    let c = v.part(None, CHEVRON_UA);
                    let w = c.px("width").unwrap_or(16.0);
                    let cr = Rect::from_min_max(egui::pos2(content.max.x - w, content.min.y), content.max);
                    glyph::paint(&mut pc.part("chevron"), &c, Glyph::ChevronRight, cr);
                    content.max.x -= w + c.sides("margin")[0];
                }
                if let Some(sc) = &sc {
                    let s = v.part(None, SHORTCUT_UA);
                    let sg = paint::text_galley(&ctx, &s, sc, None);
                    paint::paint_text(&mut pc.part("shortcut"), &s, &sg, content, Align::End);
                    content.max.x -= sg.size().x + s.sides("margin")[0];
                }
                let text = label(self.arena, r.i);
                let lg = paint::text_galley(&ctx, &v, &text, None);
                paint::paint_text(&mut pc.part("label"), &v, &lg, content, Align::Start);
                // AccessKit: menu item roles, toggled, expanded, the accelerator
                let role = match (checkable, radio) {
                    (true, true) => egui::Role::MenuItemRadio,
                    (true, false) => egui::Role::MenuItemCheckBox,
                    _ => egui::Role::MenuItem,
                };
                let enabled = r.enabled;
                resp.widget_info(|| egui::WidgetInfo::labeled(role, enabled, &text));
                let expanded = (r.kind == Kind::Sub).then_some(open_sub == node_id);
                ctx.accesskit_node_builder(resp.id, |nb| {
                    if checkable {
                        nb.set_toggled(if checked { accesskit::Toggled::True } else { accesskit::Toggled::False });
                    }
                    if let Some(e) = expanded {
                        nb.set_expanded(e);
                        nb.set_has_popup(accesskit::HasPopup::Menu);
                    }
                    if let Some(sc) = &sc {
                        nb.set_keyboard_shortcut(sc.clone());
                    }
                });
                if is_cur {
                    active = Some(resp.id);
                }
                if r.kind == Kind::Sub && open_sub == node_id {
                    next = Some(*rect);
                }
                let tip = &self.arena.common(r.i).tooltip;
                if !tip.is_empty() {
                    let _ = super::tooltip::attach(ui, &resp, tip, true);
                }
                if resp.clicked() && r.enabled {
                    match r.kind {
                        Kind::Action => self.pick(r.i),
                        Kind::Sub if open_sub != node_id => {
                            self.push(k, r.i, false);
                            next = Some(*rect);
                        }
                        _ => {}
                    }
                    if !self.is_open() {
                        break;
                    }
                }
            }
            ctx.accesskit_node_builder(lid, |nb| {
                nb.set_role(accesskit::Role::Menu);
                if let Some(a) = active {
                    nb.set_active_descendant(a.accesskit_id());
                }
            });
        });
        (frame, next, under)
    }
}

/// Whether the pointer at `p`, moving by `delta`, heads into `sub` (the triangle from where it
/// was to the submenu's near edge).
fn towards(p: Pos2, delta: Vec2, sub: Rect) -> bool {
    if delta == Vec2::ZERO {
        return false;
    }
    let from = p - delta;
    let edge_x = if sub.center().x >= from.x { sub.left() } else { sub.right() };
    let (a, b) = (egui::pos2(edge_x, sub.top()), egui::pos2(edge_x, sub.bottom()));
    in_triangle(p, from, a, b) && (p.x - from.x).abs() > 0.0
}

fn in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let s = |p1: Pos2, p2: Pos2, p3: Pos2| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

// ---- measure ----

/// The size of an own `menu-bar` (one title row high, no width of its own) or `menu-button` (a
/// button with a label and a chevron, R14).
pub(crate) fn measure(m: &Metrics, w: &MenuW, css: Option<&svelters_style::ComputedStyle>) -> Vec2 {
    let dark = popup::dark(&m.ctx);
    match w {
        MenuW::Bar { .. } => {
            let s = StyleView::new(css, BAR_UA, StateBits::NONE, dark);
            let t = StyleView::ua_only(TITLE_UA, StateBits::NONE, dark);
            let [_, _, pt, pb] = s.sides("padding");
            let [_, _, bt, bb] = s.sides("border-width");
            vec2(0.0, t.px("line-height").unwrap_or(22.0) + pt + pb + bt + bb)
        }
        MenuW::Button { label, .. } => {
            let s = StyleView::new(css, super::button::UA, StateBits::NONE, dark);
            let c = s.part(None, CHEVRON_UA);
            let mut x = c.px("width").unwrap_or(16.0);
            if !label.is_empty() {
                x += paint::text_galley(&m.ctx, &s, label, None).size().x + s.px("gap").unwrap_or(4.0);
            }
            vec2(x, s.px("line-height").unwrap_or(16.0)) + paint::box_extra(&s)
        }
        _ => Vec2::ZERO,
    }
}

// ---- the roots ----

/// Paints an own in-flow root (`menu-bar`, `menu-button`) at its rect.
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    let resp = match arena.w(i) {
        W::Menu(MenuW::Bar { .. }) => bar(arena, i, ui, out),
        W::Menu(MenuW::Button { .. }) => button(arena, i, ui, out),
        _ => return ui.response(),
    };
    let h = arena.slots.id_at(i).expect("live node").to_raw();
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        resp = super::tooltip::attach(ui, &resp, &tip, arena.own.contains("tooltip"));
    }
    if arena.common(i).invalid {
        crate::leaf::invalid_outline(ui, resp.rect);
    }
    arena.core[i].drawn = resp.rect;
    resp
}

/// `menu-bar`: its titles in a row; a click opens a title's menu, hovering another title while
/// one is open switches to it, F10 (or a lone Alt, per policy) gives it the keyboard.
fn bar(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    let rect = ui.max_rect();
    let enabled = arena.common(i).enabled && ui.is_enabled();
    if enabled {
        menus::shortcuts(arena, i, ui, out);
    }
    let titles = rows(arena, i, false).into_iter().filter(|r| r.kind != Kind::Sep).collect::<Vec<_>>();
    let mut m = Menu::load(arena, i, ui, out);
    let ctx = m.ctx.clone();
    let id = m.arena.core[i].egui_id;
    let bar_css = m.arena.slots.id_at(i).and_then(|n| popup::css(m.arena, n));
    let bs = StyleView::new(bar_css.as_deref(), BAR_UA, StateBits::NONE, m.dark);
    let mut pc = paint::PaintCx::for_node(ui.painter(), m.arena.slots.id_at(i).map_or(0, |n| n.to_raw()));
    pc.tag = "menu-bar";
    let g = paint::paint_box(&mut pc, &bs, rect);
    let resp = ui.interact(rect, id, Sense::hover());
    ctx.accesskit_node_builder(resp.id, |n| n.set_role(accesskit::Role::MenuBar));
    // keyboard mode: F10, a lone Alt
    let policy = crate::native::policy_of(&ctx);
    let (f10, alt) = ctx.input(|inp| {
        let f10 = inp.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::F10, pressed: true, modifiers, .. } if !modifiers.any()));
        let others = inp.events.iter().any(|e| matches!(e, egui::Event::Key { .. } | egui::Event::PointerButton { .. } | egui::Event::Text(_)));
        (f10, (inp.modifiers.alt, others))
    });
    let mut tap = false;
    if policy.alt_focuses_menu_bar {
        match alt {
            (true, false) if !m.st.alt => m.st.alt = !m.st.bar_keys && !m.is_open(),
            (true, true) => m.st.alt = false,
            (false, others) => {
                tap = m.st.alt && !others;
                m.st.alt = false;
            }
            _ => {}
        }
    }
    let first = Roving::edge(&titles.iter().map(|t| t.enabled).collect::<Vec<_>>(), true);
    if enabled && (f10 || tap) && !titles.is_empty() {
        if m.is_open() || m.st.bar_keys {
            m.st.bar_keys = false;
            m.close_all();
            m.st.grab.release(&ctx, m.focus);
        } else {
            m.st.bar_keys = true;
            m.st.bar.set(first);
            m.st.grab.take(&ctx, m.focus);
        }
        ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::F10));
    }
    // the bar's own keys while it has the keyboard and no menu is open
    if m.st.bar_keys && !m.is_open() && ctx.memory(|mm| mm.has_focus(m.focus)) {
        let p = RovingParams { enabled: titles.iter().map(|t| t.enabled).collect(), orientation: Orientation::Horizontal, wrap: true };
        let keys: Vec<Key> = ctx.input(|inp| inp.events.iter().filter_map(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.any() => Some(*key),
            _ => None,
        }).collect());
        for k in keys {
            let used = match k {
                Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => m.st.bar.key(&p, k).is_some(),
                Key::ArrowDown | Key::Enter | Key::Space => {
                    if let Some(t) = m.st.bar.current().and_then(|c| titles.get(c).copied()) {
                        match t.kind {
                            Kind::Sub => m.open(t.i, false, true),
                            _ => {
                                m.st.bar_keys = false;
                                m.st.grab.release(&ctx, m.focus);
                                menus::activate(m.arena, t.i, &ctx, m.out);
                            }
                        }
                    }
                    true
                }
                Key::Escape | Key::Tab => {
                    m.st.bar_keys = false;
                    m.st.grab.release(&ctx, m.focus);
                    true
                }
                _ => false,
            };
            if used {
                ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, k));
            }
        }
    }
    if m.st.bar_keys && !ctx.memory(|mm| mm.has_focus(m.focus)) && !m.is_open() {
        m.st.bar_keys = false; // focus went elsewhere (a click on a field)
        m.st.grab.active = false;
    }
    m.keys(Some(&titles));
    // the titles
    let ts = StyleView::ua_only(TITLE_UA, StateBits::NONE, m.dark);
    let mut x = g.content.left();
    let open_menu = m.st.levels.first().map(|l| l.menu);
    let mut anchor = None;
    for (n, t) in titles.iter().enumerate() {
        let node = m.arena.slots.id_at(t.i);
        let (css, ua, _) = m.style(t.i, TITLE_UA, StateBits::NONE);
        let v = StyleView::new(css.as_deref(), ua, StateBits::NONE, m.dark);
        let text = label(m.arena, t.i);
        let gal = paint::text_galley(&ctx, &v, &text, None);
        let w = gal.size().x + paint::box_extra(&v).x;
        let r = Rect::from_min_size(egui::pos2(x, g.content.top()), vec2(w, g.content.height().max(ts.px("line-height").unwrap_or(22.0))));
        x += w + bs.px("gap").unwrap_or(0.0);
        let tr = ui.interact(r, m.arena.core[t.i].egui_id, if enabled && t.enabled { Sense::click() } else { Sense::hover() });
        let is_open = open_menu.is_some() && open_menu == node;
        if enabled && t.enabled {
            let switch = tr.hovered() && m.is_open() && !is_open && t.kind == Kind::Sub;
            if tr.clicked() && is_open {
                m.st.bar_keys = false;
                m.close_all();
            } else if (tr.clicked() || switch) && t.kind == Kind::Sub {
                let kb = m.st.keyboard && switch;
                m.st.bar.set(Some(n));
                m.open(t.i, false, kb);
            } else if tr.clicked() {
                m.close_all();
                menus::activate(m.arena, t.i, &ctx, m.out);
            }
        }
        let is_open = m.st.levels.first().is_some_and(|l| Some(l.menu) == node);
        let mut bits = StateBits::NONE;
        bits.set(StateBits::HOVERED, tr.hovered());
        bits.set(StateBits::OPEN, is_open);
        bits.set(StateBits::DISABLED, !t.enabled || !enabled);
        bits.set(StateBits::CURRENT, (m.st.bar_keys || m.is_open()) && m.st.bar.current() == Some(n));
        bits.set(StateBits::FOCUS_VISIBLE, m.st.bar_keys && m.st.bar.current() == Some(n));
        m.set_bits(t.i, bits);
        let v = StyleView::new(css.as_deref(), ua, bits, m.dark);
        let mut tpc = paint::PaintCx::for_node(ui.painter(), node.map_or(0, |n| n.to_raw()));
        tpc.tag = "menu";
        let tg = paint::paint_box(&mut tpc, &v, r);
        let gal = paint::text_galley(&ctx, &v, &text, None);
        paint::paint_text(&mut tpc.part("label"), &v, &gal, tg.content, Align::Center);
        tr.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, t.enabled && enabled, &text));
        if t.kind == Kind::Sub {
            ctx.accesskit_node_builder(tr.id, |nb| {
                nb.set_expanded(is_open);
                nb.set_has_popup(accesskit::HasPopup::Menu);
            });
        }
        if is_open {
            anchor = Some(r);
        }
    }
    if m.st.bar_keys && !m.is_open() {
        m.st.grab.keep(ui, m.focus, rect); // the bar holds the keyboard (F10) with no menu open
    }
    if let Some(a) = anchor {
        let rects = m.paint(a, Placement::BOTTOM_START, Some(0.0));
        let mut inside = rects;
        inside.push(rect);
        dismiss(&mut m, inside);
    } else if m.is_open() {
        m.close_all(); // its title went away
    }
    m.save();
    resp
}

/// Closes every level on Escape-less dismissal: a press outside the levels and `inside`.
fn dismiss(m: &mut Menu<'_>, inside: Vec<Rect>) {
    if !m.is_open() {
        return;
    }
    if m.st.opened == m.ctx.cumulative_pass_nr() {
        return; // the press that opened it (a secondary click at the pointer) is not outside
    }
    let p = DismissParams { inside, on_outside: Outside::Close, escape: false };
    if popup::dismissed(&m.ctx, &p).is_some() {
        m.st.bar_keys = false;
        m.close_all();
    }
}

/// `menu-button`: a button (the own button's look) with a `chevron` part; a click, Enter, Space or
/// Down opens its menu below it; `active` mirrors the open state both ways.
fn button(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    let rect = ui.max_rect();
    let W::Menu(MenuW::Button { label: text, active, cmd, .. }) = arena.w(i) else { return ui.response() };
    let (text, active, cmd) = (text.clone(), *active, *cmd);
    if let W::Menu(MenuW::Button { cmd, .. }) = arena.w_mut(i) {
        *cmd = None;
    }
    let enabled = arena.common(i).enabled && ui.is_enabled();
    if enabled {
        menus::shortcuts(arena, i, ui, out);
    }
    let mut m = Menu::load(arena, i, ui, out);
    let ctx = m.ctx.clone();
    let id = m.arena.core[i].egui_id;
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let policy = crate::native::policy_of(&ctx);
    if enabled && resp.is_pointer_button_down_on() && policy.focus_on_click && !m.is_open() {
        resp.request_focus();
    }
    let kb_open = enabled && resp.has_focus() && ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::ArrowDown) || inp.consume_key(Modifiers::ALT, Key::ArrowDown));
    match cmd {
        Some(true) if !m.is_open() => m.open(i, true, false),
        Some(false) => m.close_all(),
        _ => {}
    }
    if enabled && (resp.clicked() || kb_open) {
        if m.is_open() {
            m.close_all();
        } else {
            let kb = kb_open || ctx.input(|inp| inp.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter | Key::Space, pressed: true, .. })));
            m.open(i, true, kb);
        }
    }
    m.keys(None);
    let rects = if m.is_open() { m.paint(rect, Placement::BOTTOM_START, None) } else { Vec::new() };
    let mut inside = rects;
    inside.push(rect);
    dismiss(&mut m, inside);
    // the button: the own button's look, a label and the chevron (R14)
    let open = m.is_open();
    let mut bits = m.arena.core[i].state;
    bits.set(StateBits::HOVERED, resp.hovered() || (!enabled && resp.contains_pointer()));
    bits.set(StateBits::PRESSED, enabled && resp.is_pointer_button_down_on());
    bits.set(StateBits::OPEN, open);
    bits.set(StateBits::DISABLED, !enabled);
    bits.set(StateBits::FOCUSED, resp.has_focus());
    bits.set(StateBits::FOCUS_VISIBLE, resp.has_focus() && m.st.keyboard);
    m.set_bits(i, bits);
    let node = m.arena.slots.id_at(i);
    let (css, ua, _) = m.style(i, super::button::UA, bits);
    let v = StyleView::new(css.as_deref(), ua, bits, m.dark);
    if v.opacity() < 1.0 {
        ui.multiply_opacity(v.opacity());
    }
    if ui.is_rect_visible(rect) {
        let mut pc = paint::PaintCx::for_node(ui.painter(), node.map_or(0, |n| n.to_raw()));
        pc.tag = "menu-button";
        let g = paint::paint_box(&mut pc, &v, rect);
        let c = v.part(None, CHEVRON_UA);
        let cw = c.px("width").unwrap_or(16.0);
        let cr = Rect::from_min_max(egui::pos2(g.content.max.x - cw, g.content.min.y), g.content.max);
        glyph::paint(&mut pc.part("chevron"), &c, Glyph::ChevronDown, cr);
        let lr = Rect::from_min_max(g.content.min, egui::pos2(cr.min.x, g.content.max.y));
        let lg = paint::text_galley(&ctx, &v, &text, None);
        paint::paint_text(&mut pc.part("label"), &v, &lg, lr, Align::Center);
        paint::paint_outline(&mut pc, &v, rect);
    }
    if v.state().contains(StateBits::HOVERED)
        && let Some(cur) = v.cursor()
    {
        ctx.set_cursor_icon(cur);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &text));
    ctx.accesskit_node_builder(resp.id, |nb| {
        nb.set_expanded(open);
        nb.set_has_popup(accesskit::HasPopup::Menu);
    });
    let arena = &mut *m.arena;
    let out = &mut *m.out;
    if open != active {
        if let W::Menu(MenuW::Button { active, .. }) = arena.w_mut(i) {
            *active = open;
        }
        menus::announce(arena, i, Some(crate::kinds::P::Active), &[("changed", vec![Value::Bool(open)])], out);
    }
    m.save();
    resp
}

/// `context-menu` node `i` attached to an owner drawn at `owner`: a secondary click on the owner
/// (the innermost owner wins) opens it at the pointer; Shift+F10 or the Menu key on the focused
/// owner and `popup()` open it below the owner.
pub(crate) fn context(arena: &mut Arena, i: usize, owner: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let W::Menu(MenuW::Context { cmd, .. }) = arena.w(i) else { return };
    let cmd = *cmd;
    if let W::Menu(MenuW::Context { cmd, .. }) = arena.w_mut(i) {
        *cmd = None;
    }
    let enabled = arena.common(i).enabled && ui.is_enabled();
    if enabled {
        menus::shortcuts(arena, i, ui, out);
    }
    let mut m = Menu::load(arena, i, ui, out);
    let ctx = m.ctx.clone();
    if enabled
        && owner.is_positive()
        && ui.input(|inp| inp.pointer.secondary_clicked())
        && ui.rect_contains_pointer(owner)
        && menus::claim(&ctx, owner.area())
        && let Some(pos) = ctx.pointer_interact_pos()
    {
        m.st.anchor = Some(popup::point(pos));
        m.open(i, false, false);
    }
    // FM1 QA8: the context-menu keys on the focused owner
    // (the focused widget drawn in the owner: its response id need not be the node id)
    let focused_owner = owner.is_positive() && ctx.memory(|mm| mm.focused()).and_then(|f| ctx.read_response(f)).is_some_and(|r| r.layer_id == ui.layer_id() && owner.contains(r.rect.center()));
    if enabled && focused_owner && !m.is_open() && ctx.input(|inp| inp.key_pressed(Key::Menu) || inp.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::F10, pressed: true, modifiers, .. } if modifiers.shift_only()))) && menus::claim(&ctx, owner.area()) {
        ctx.input_mut(|inp| inp.consume_key(Modifiers::SHIFT, Key::F10) || inp.consume_key(Modifiers::NONE, Key::Menu));
        m.st.anchor = Some(popup::point(owner.left_bottom()));
        m.open(i, false, true);
    }
    match cmd {
        Some(true) => {
            m.st.anchor = Some(popup::point(owner.left_bottom()));
            m.open(i, false, false);
        }
        Some(false) => m.close_all(),
        None => {}
    }
    m.keys(None);
    if m.is_open() {
        let a = m.st.anchor.unwrap_or(popup::point(owner.left_bottom()));
        let rects = m.paint(a, Placement::BOTTOM_START, Some(0.0));
        dismiss(&mut m, rects);
    }
    m.save();
}
