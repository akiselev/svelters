//! `Scrollable` (OW2, ARCH-WIDGETS §1.2, §3.3): an offset over content larger than its viewport,
//! moved by the wheel and trackpad (with per-platform momentum), the scroll bars' thumbs and
//! tracks, the keyboard, AccessKit and the host. Used by `scroll-area` and the standalone
//! `scrollbar`; OW5 moves `virtual-list`, `table` and `tab-view` onto it.
//!
//! The host gives the geometry before each run ([`Scrollable::set_geometry`]: content and viewport
//! size, the bar tracks it painted) and reads back what to do: [`Scrollable::offset`], the wheel
//! delta it took ([`Scrollable::consumed`], subtracted from egui's `smooth_scroll_delta` so an outer
//! scrollable gets only the rest: scroll chaining, WG3s pin 9) and the keys it used.
//!
//! **Order.** A scrollable runs its unit *after* its children were drawn (post-order), so the
//! innermost scrollable under the pointer sees the wheel first and an outer one gets what is left,
//! as in browsers and egui. The offset it computes is drawn in the next frame (it asks for one).
//!
//! Platform rows ([`ScrollPolicy`]): momentum (`Os`: the OS sends it as wheel events; `Own`: decay
//! 0.95 per 16 ms from the release velocity, stopping below 0.5 pt per frame; `None`), what a
//! track press does (page or jump), how long overlay bars stay before they fade.

use super::{InputCx, Intents, Unit};
use crate::native::{BarMode, Kinetic, ScrollPolicy, TrackClick};
use crate::provider::StateBits;
use egui::{Key, Modifiers, Rect, Vec2, accesskit, vec2};

// R15 (coordinator, 2026-10-22): provider states a `Scrollable` sets on its host when the content
// overflows in that direction (scrolled away from that edge), matched by the theme as
// `:state(sv.overflow-top)` and so on (the edge shadows). No new built-in bits.
crate::state_bits! {
    pub mod sv {
        OVERFLOW_TOP = "overflow-top",
        OVERFLOW_BOTTOM = "overflow-bottom",
        OVERFLOW_LEFT = "overflow-left",
        OVERFLOW_RIGHT = "overflow-right",
    }
}

/// The four overflow states, as one mask.
pub fn overflow_bits() -> StateBits {
    sv::OVERFLOW_TOP.bits() | sv::OVERFLOW_BOTTOM.bits() | sv::OVERFLOW_LEFT.bits() | sv::OVERFLOW_RIGHT.bits()
}

/// What a host configures on its [`Scrollable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollParams {
    /// The axes that scroll (x, y).
    pub axes: [bool; 2],
    /// At an edge, the wheel goes on to the scrollable around it (`overscroll-behavior: auto`);
    /// false keeps it here (`contain`, `none`).
    pub chain: bool,
    /// PageUp/PageDown, Home/End are handled (keyboard focus is in the host or a child that did
    /// not claim them).
    pub page_keys: bool,
    /// The arrow keys are handled (the host itself is focused).
    pub arrow_keys: bool,
}

impl Default for ScrollParams {
    fn default() -> Self {
        ScrollParams { axes: [false, true], chain: true, page_keys: false, arrow_keys: false }
    }
}

/// What moved the offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollBy {
    Wheel,
    /// Momentum after a trackpad flick (`Kinetic::Own`).
    Kinetic,
    Thumb,
    Track,
    Key,
    A11y,
}

/// What [`Scrollable`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scroll {
    /// The offset changed this pass.
    Scrolled { offset: Vec2, by: ScrollBy },
    /// A thumb drag started or ended (`drag-started`, `drag-stopped`; `:dragging` on the thumb).
    ThumbDrag { axis: usize, active: bool },
}

/// One bar's geometry along its axis, as the host laid it out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarGeom {
    /// The track (the whole bar).
    pub track: Rect,
    /// The thumb inside it.
    pub thumb: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    axis: usize,
    /// Where the pointer grabbed the thumb, from the thumb's start along the axis.
    grab: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TrackHold {
    axis: usize,
    /// +1: towards the end.
    dir: f32,
    next: f64,
}

/// The scroll state of one scrollable.
#[derive(Debug, Default)]
pub struct Scrollable {
    offset: Vec2,
    content: Vec2,
    viewport: Vec2,
    bars: [Option<BarGeom>; 2],
    /// Points per second (content coordinates), the trackpad's recent speed or the flick's.
    vel: Vec2,
    flick: bool,
    last_time: f64,
    /// The last time it scrolled or was hovered (overlay bars fade from it).
    last_activity: f64,
    hovered: bool,
    drag: Option<Drag>,
    hold: Option<TrackHold>,
    consumed: Vec2,
    keys_used: Vec<(Key, Modifiers)>,
    a11y_used: bool,
}

/// Seconds before a held track press repeats, and between repeats.
const TRACK_REPEAT: (f64, f64) = (0.3, 0.05);
/// Points AccessKit's ScrollUp/Down/Left/Right move (egui's distance).
pub const A11Y_STEP: f32 = 100.0;

impl Scrollable {
    /// The offset (content points scrolled out at the top left).
    pub fn offset(&self) -> Vec2 {
        self.offset
    }
    /// The largest offset on each axis.
    pub fn max_offset(&self) -> Vec2 {
        (self.content - self.viewport).max(Vec2::ZERO)
    }
    pub fn content(&self) -> Vec2 {
        self.content
    }
    pub fn viewport(&self) -> Vec2 {
        self.viewport
    }
    /// Whether the content overflows on axis `d`.
    pub fn overflows(&self, d: usize) -> bool {
        self.content[d] > self.viewport[d] + 0.5
    }
    /// Scrolled away from the start (top or left) of axis `d`: the edge shadow there (R15).
    pub fn away_from_start(&self, d: usize) -> bool {
        self.offset[d] > 0.5
    }
    /// Scrolled away from the end of axis `d`.
    pub fn away_from_end(&self, d: usize) -> bool {
        self.offset[d] < self.max_offset()[d] - 0.5
    }
    /// The `sv.overflow-*` states for this offset and geometry (set them on the host).
    pub fn overflow_state(&self) -> StateBits {
        let mut b = StateBits::NONE;
        b.set(sv::OVERFLOW_TOP.bits(), self.away_from_start(1));
        b.set(sv::OVERFLOW_BOTTOM.bits(), self.away_from_end(1));
        b.set(sv::OVERFLOW_LEFT.bits(), self.away_from_start(0));
        b.set(sv::OVERFLOW_RIGHT.bits(), self.away_from_end(0));
        b
    }

    /// The thumb of axis `d` is being dragged.
    pub fn dragging(&self, d: usize) -> bool {
        self.drag.is_some_and(|g| g.axis == d)
    }
    /// The wheel delta this run took (subtract it from egui's `smooth_scroll_delta`).
    pub fn consumed(&self) -> Vec2 {
        self.consumed
    }
    /// The keys this run used (consume them from egui's input).
    pub fn keys_used(&self) -> &[(Key, Modifiers)] {
        &self.keys_used
    }
    /// An AccessKit scroll action on the host was handled this run.
    pub fn a11y_used(&self) -> bool {
        self.a11y_used
    }

    /// Sets the offset from the host (a prop write), clamped once the geometry is known.
    pub fn set_offset(&mut self, o: Vec2) {
        self.offset = o.max(Vec2::ZERO);
        if self.content != Vec2::ZERO {
            self.offset = self.offset.min(self.max_offset());
        }
        self.flick = false;
    }

    /// The geometry of this frame: content and viewport size, the bars as painted. Clamps the
    /// offset to the new content.
    pub fn set_geometry(&mut self, content: Vec2, viewport: Vec2, bars: [Option<BarGeom>; 2]) {
        self.content = content;
        self.viewport = viewport;
        self.bars = bars;
        self.offset = self.offset.max(Vec2::ZERO).min(self.max_offset());
    }

    /// The bars' geometry for `viewport` (screen), `width` thick, `min_thumb` the shortest thumb,
    /// for the axes in `show` (each bar leaves room for the other one's corner when both show).
    pub fn bar_geometry(&self, viewport: Rect, width: f32, min_thumb: f32, show: [bool; 2]) -> [Option<BarGeom>; 2] {
        let mut out = [None, None];
        for d in 0..2 {
            if !show[d] {
                continue;
            }
            let o = 1 - d;
            let corner = if show[o] { width } else { 0.0 };
            let track = if d == 1 {
                Rect::from_min_max(egui::pos2(viewport.max.x - width, viewport.min.y), egui::pos2(viewport.max.x, viewport.max.y - corner))
            } else {
                Rect::from_min_max(egui::pos2(viewport.min.x, viewport.max.y - width), egui::pos2(viewport.max.x - corner, viewport.max.y))
            };
            let len = track.size()[d];
            let content = self.content[d].max(1.0);
            let thumb_len = (len * (self.viewport[d] / content).min(1.0)).max(min_thumb.min(len));
            let max = self.max_offset()[d];
            let t = if max > 0.0 { (self.offset[d] / max).clamp(0.0, 1.0) } else { 0.0 };
            let start = track.min[d] + (len - thumb_len) * t;
            let mut thumb = track;
            thumb.min[d] = start;
            thumb.max[d] = start + thumb_len;
            out[d] = Some(BarGeom { track, thumb });
        }
        out
    }

    /// Overlay bars' opacity at `now`: 1 while hovered, dragged or just scrolled, then fading.
    pub fn bar_alpha(&self, now: f64, p: &ScrollPolicy) -> f32 {
        if self.hovered || self.drag.is_some() || self.hold.is_some() {
            return 1.0;
        }
        let t = (now - self.last_activity) as f32;
        if t <= p.bar_hold {
            1.0
        } else if p.bar_fade > 0.0 && t < p.bar_hold + p.bar_fade {
            1.0 - (t - p.bar_hold) / p.bar_fade
        } else {
            0.0
        }
    }

    /// Seconds until the overlay bars' look changes again without input (fade frames).
    pub fn bar_repaint(&self, now: f64, p: &ScrollPolicy) -> Option<f32> {
        if self.hovered || self.drag.is_some() {
            return None;
        }
        let t = (now - self.last_activity) as f32;
        if t < p.bar_hold {
            Some(p.bar_hold - t)
        } else if t < p.bar_hold + p.bar_fade {
            Some(1.0 / 60.0)
        } else {
            None
        }
    }

    /// Marks activity now (a scroll from outside, a reveal): overlay bars show and fade.
    pub fn touch(&mut self, now: f64) {
        self.last_activity = now;
    }

    /// Scrolls by `delta` (content points; positive: towards the end) on the enabled axes;
    /// returns what it could not take (at an edge), per axis.
    pub fn scroll_by(&mut self, p: &ScrollParams, delta: Vec2) -> Vec2 {
        let max = self.max_offset();
        let mut rest = Vec2::ZERO;
        for d in 0..2 {
            if delta[d] == 0.0 {
                continue;
            }
            if !p.axes[d] {
                rest[d] = delta[d];
                continue;
            }
            let new = (self.offset[d] + delta[d]).clamp(0.0, max[d]);
            rest[d] = delta[d] - (new - self.offset[d]);
            self.offset[d] = new;
        }
        rest
    }

    /// The offset change that brings `target` (screen) into `view` (screen): `center` centres it,
    /// otherwise the least scrolling (nothing when it is already inside; its start when larger).
    pub fn reveal_delta(view: Rect, target: Rect, center: bool, margin: f32) -> Vec2 {
        let target = target.expand(margin);
        let mut d = Vec2::ZERO;
        for a in 0..2 {
            let (v0, v1, t0, t1) = (view.min[a], view.max[a], target.min[a], target.max[a]);
            d[a] = if center {
                (t0 + t1) / 2.0 - (v0 + v1) / 2.0
            } else if t0 < v0 || t1 - t0 > v1 - v0 {
                t0 - v0
            } else if t1 > v1 {
                t1 - v1
            } else {
                0.0
            };
        }
        d
    }

    fn page(&self, d: usize, p: &ScrollPolicy) -> f32 {
        (self.viewport[d] - p.line).max(self.viewport[d] * 0.5).max(1.0)
    }

    fn thumb_track_len(&self, d: usize) -> Option<(f32, f32)> {
        let b = self.bars[d]?;
        Some((b.track.size()[d], b.thumb.size()[d]))
    }

    /// Offset on axis `d` that puts the thumb's start at `pos` (screen).
    fn offset_for_thumb(&self, d: usize, pos: f32) -> Option<f32> {
        let b = self.bars[d]?;
        let (len, thumb) = self.thumb_track_len(d)?;
        let room = len - thumb;
        let max = self.max_offset()[d];
        if room <= 0.0 {
            return Some(0.0);
        }
        Some(((pos - b.track.min[d]) / room).clamp(0.0, 1.0) * max)
    }

    fn input_wheel(&mut self, p: &ScrollParams, cx: &InputCx<'_>, out: &mut Intents<Scroll>) {
        let pol = &cx.policy.scroll;
        let dt = (cx.now - self.last_time).clamp(0.0, 0.1) as f32;
        // the trackpad's phase: momentum (Kinetic::Own) starts when the fingers lift
        let mut trackpad_end = false;
        let mut trackpad_move = false;
        for e in &cx.input.events {
            if let egui::Event::MouseWheel { phase, source: egui::MouseWheelSource::Trackpad, .. } = e {
                match phase {
                    egui::TouchPhase::End => trackpad_end = true,
                    egui::TouchPhase::Start | egui::TouchPhase::Move => trackpad_move = true,
                    egui::TouchPhase::Cancel => {}
                }
            }
        }
        let wheel = cx.input.smooth_scroll_delta;
        if cx.resp.contains_pointer && wheel != Vec2::ZERO {
            // the wheel moves the content: a positive delta scrolls towards the start
            let want = -wheel;
            let before = self.offset;
            let rest = self.scroll_by(p, want);
            for d in 0..2 {
                if want[d] == 0.0 || !p.axes[d] {
                    continue;
                }
                let moved = self.offset[d] != before[d];
                // the whole delta stays here when it moved (browsers do not split a wheel step) or
                // when it must not chain; at an edge it goes on to the scrollable around
                if moved || !p.chain || rest[d] == 0.0 {
                    self.consumed[d] = wheel[d];
                }
            }
            if trackpad_move && dt > 0.0 {
                let v = (self.offset - before) / dt;
                self.vel = self.vel * 0.5 + v * 0.5;
            }
            self.flick = false;
            if self.offset != before {
                self.last_activity = cx.now;
                out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Wheel });
            }
        } else if trackpad_move {
            self.flick = false;
        }
        if trackpad_end && pol.kinetic == Kinetic::Own && cx.resp.contains_pointer && self.vel.length() > 30.0 {
            self.flick = true;
        }
        if self.flick {
            if cx.input.pointer.any_pressed() || dt == 0.0 {
                self.flick = cx.input.pointer.any_pressed().then_some(false).unwrap_or(self.flick);
            }
            if self.flick && dt > 0.0 {
                let before = self.offset;
                let step = self.vel * dt;
                self.scroll_by(p, step);
                self.vel *= 0.95f32.powf(dt / 0.016);
                if self.offset == before || self.vel.length() * 0.016 < 0.5 {
                    self.flick = false;
                    self.vel = Vec2::ZERO;
                }
                if self.offset != before {
                    self.last_activity = cx.now;
                    out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Kinetic });
                }
            }
        } else if !trackpad_move {
            self.vel = Vec2::ZERO;
        }
    }

    fn input_bars(&mut self, cx: &InputCx<'_>, out: &mut Intents<Scroll>) {
        let pol = cx.policy.scroll;
        let ptr = cx.input.pointer.interact_pos();
        let primary_down = cx.input.pointer.primary_down();
        // a thumb drag in progress follows the pointer until release
        if let Some(g) = self.drag {
            match (primary_down, ptr) {
                (true, Some(pos)) => {
                    if let Some(o) = self.offset_for_thumb(g.axis, pos[g.axis] - g.grab)
                        && o != self.offset[g.axis]
                    {
                        self.offset[g.axis] = o;
                        self.last_activity = cx.now;
                        out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Thumb });
                    }
                }
                _ => {
                    self.drag = None;
                    out.push(Scroll::ThumbDrag { axis: g.axis, active: false });
                }
            }
            return;
        }
        if !primary_down {
            self.hold = None;
        }
        for d in 0..2 {
            let Some(b) = self.bars[d] else { continue };
            let thumb_name = if d == 0 { "thumb-x" } else { "thumb-y" };
            let bar_name = if d == 0 { "scrollbar-x" } else { "scrollbar-y" };
            let pressed_now = cx.input.pointer.primary_pressed();
            let on_thumb = cx.parts.get(thumb_name).is_some_and(|r| r.down_on) || (pressed_now && ptr.is_some_and(|p| b.thumb.contains(p)));
            let on_track = cx.parts.get(bar_name).is_some_and(|r| r.down_on);
            let Some(pos) = ptr else { continue };
            if pressed_now && on_thumb && b.thumb.contains(pos) {
                self.drag = Some(Drag { axis: d, grab: pos[d] - b.thumb.min[d] });
                self.last_activity = cx.now;
                out.push(Scroll::ThumbDrag { axis: d, active: true });
                return;
            }
            if pressed_now && on_track && b.track.contains(pos) {
                let mut mode = pol.track_click;
                if pol.shift_track_click_inverts && cx.input.modifiers.shift_only() {
                    mode = if mode == TrackClick::Page { TrackClick::Jump } else { TrackClick::Page };
                }
                match mode {
                    TrackClick::Jump => {
                        let half = b.thumb.size()[d] / 2.0;
                        if let Some(o) = self.offset_for_thumb(d, pos[d] - half) {
                            self.offset[d] = o;
                            out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Track });
                        }
                        self.drag = Some(Drag { axis: d, grab: half });
                        out.push(Scroll::ThumbDrag { axis: d, active: true });
                    }
                    TrackClick::Page => {
                        let dir = if pos[d] < b.thumb.min[d] { -1.0 } else { 1.0 };
                        self.page_step(d, dir, &pol, cx, out);
                        self.hold = Some(TrackHold { axis: d, dir, next: cx.now + TRACK_REPEAT.0 });
                    }
                }
                self.last_activity = cx.now;
                return;
            }
        }
        // a held track press repeats until the thumb reaches the pointer
        if let (Some(h), Some(pos)) = (self.hold, ptr)
            && primary_down
            && cx.now >= h.next
            && let Some(b) = self.bars[h.axis]
        {
            let before_thumb = if h.dir < 0.0 { pos[h.axis] < b.thumb.min[h.axis] } else { pos[h.axis] > b.thumb.max[h.axis] };
            if before_thumb {
                self.page_step(h.axis, h.dir, &pol, cx, out);
                self.hold = Some(TrackHold { next: cx.now + TRACK_REPEAT.1, ..h });
            } else {
                self.hold = None;
            }
        }
    }

    fn page_step(&mut self, d: usize, dir: f32, pol: &ScrollPolicy, cx: &InputCx<'_>, out: &mut Intents<Scroll>) {
        let before = self.offset[d];
        let max = self.max_offset()[d];
        self.offset[d] = (before + dir * self.page(d, pol)).clamp(0.0, max);
        if self.offset[d] != before {
            self.last_activity = cx.now;
            out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Track });
        }
    }

    fn input_keys(&mut self, p: &ScrollParams, cx: &InputCx<'_>, out: &mut Intents<Scroll>) {
        if !p.page_keys && !p.arrow_keys {
            return;
        }
        let pol = &cx.policy.scroll;
        // the vertical axis when it scrolls, else the horizontal one
        let main = if p.axes[1] { 1 } else { 0 };
        let before = self.offset;
        let max = self.max_offset();
        for e in &cx.input.events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
            let m = *modifiers;
            if m.alt || (m.shift && !matches!(key, Key::Space)) {
                continue;
            }
            let step: Option<(usize, f32)> = match key {
                Key::PageDown if p.page_keys && !m.command => Some((main, self.page(main, pol))),
                Key::PageUp if p.page_keys && !m.command => Some((main, -self.page(main, pol))),
                Key::Home if p.page_keys => Some((main, -self.offset[main] - 1.0)),
                Key::End if p.page_keys => Some((main, max[main] - self.offset[main] + 1.0)),
                Key::ArrowDown if p.arrow_keys && !m.command && p.axes[1] => Some((1, pol.line)),
                Key::ArrowUp if p.arrow_keys && !m.command && p.axes[1] => Some((1, -pol.line)),
                Key::ArrowRight if p.arrow_keys && !m.command && p.axes[0] => Some((0, pol.line)),
                Key::ArrowLeft if p.arrow_keys && !m.command && p.axes[0] => Some((0, -pol.line)),
                _ => None,
            };
            if let Some((d, by)) = step
                && p.axes[d]
            {
                self.offset[d] = (self.offset[d] + by).clamp(0.0, max[d]);
                self.keys_used.push((*key, m));
            }
        }
        if self.offset != before {
            self.last_activity = cx.now;
            out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::Key });
        }
    }

    fn input_a11y(&mut self, p: &ScrollParams, cx: &InputCx<'_>, out: &mut Intents<Scroll>) {
        let before = self.offset;
        for r in cx.a11y {
            let d = match r.action {
                accesskit::Action::ScrollDown => vec2(0.0, A11Y_STEP),
                accesskit::Action::ScrollUp => vec2(0.0, -A11Y_STEP),
                accesskit::Action::ScrollRight => vec2(A11Y_STEP, 0.0),
                accesskit::Action::ScrollLeft => vec2(-A11Y_STEP, 0.0),
                accesskit::Action::SetScrollOffset => {
                    if let Some(accesskit::ActionData::SetScrollOffset(pt)) = &r.data {
                        let want = vec2(pt.x as f32, pt.y as f32);
                        want - self.offset
                    } else {
                        Vec2::ZERO
                    }
                }
                _ => continue,
            };
            self.scroll_by(p, d);
            self.a11y_used = true;
        }
        if self.offset != before {
            self.last_activity = cx.now;
            out.push(Scroll::Scrolled { offset: self.offset, by: ScrollBy::A11y });
        }
    }
}

impl Unit for Scrollable {
    type Params = ScrollParams;
    type Intent = Scroll;
    /// Built-in bits: none (its parts have their own states, `::part(thumb-y):hover`). It sets
    /// the provider states `sv.overflow-*` on the host ([`overflow_bits`], not const).
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &ScrollParams, cx: &mut InputCx<'_>, out: &mut Intents<Scroll>) {
        self.consumed = Vec2::ZERO;
        self.keys_used.clear();
        self.a11y_used = false;
        self.hovered = cx.resp.contains_pointer;
        if self.hovered {
            // hovering keeps overlay bars shown (VS Code: `visible` on mouse over)
            self.last_activity = self.last_activity.max(cx.now - 1.0e-3);
            if cx.input.pointer.is_moving() || cx.input.smooth_scroll_delta != Vec2::ZERO {
                self.last_activity = cx.now;
            }
        }
        if cx.enabled {
            self.input_bars(cx, out);
            self.input_wheel(p, cx, out);
            self.input_keys(p, cx, out);
            self.input_a11y(p, cx, out);
        } else {
            self.drag = None;
            self.hold = None;
            self.flick = false;
        }
        self.last_time = cx.now;
        let ov = self.overflow_state();
        cx.set(overflow_bits(), false);
        cx.set(ov, true);
    }

    fn a11y(&self, p: &ScrollParams, node: &mut accesskit::Node) {
        let max = self.max_offset();
        if p.axes[0] {
            node.set_scroll_x(self.offset.x as f64);
            node.set_scroll_x_min(0.0);
            node.set_scroll_x_max(max.x as f64);
            node.add_action(accesskit::Action::ScrollLeft);
            node.add_action(accesskit::Action::ScrollRight);
        }
        if p.axes[1] {
            node.set_scroll_y(self.offset.y as f64);
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(max.y as f64);
            node.add_action(accesskit::Action::ScrollUp);
            node.add_action(accesskit::Action::ScrollDown);
        }
        node.add_action(accesskit::Action::SetScrollOffset);
    }

    fn claims(&self, p: &ScrollParams, key: Key, m: Modifiers) -> bool {
        !m.alt
            && match key {
                Key::PageUp | Key::PageDown | Key::Home | Key::End => p.page_keys,
                Key::ArrowUp | Key::ArrowDown => p.arrow_keys && p.axes[1] && !m.command,
                Key::ArrowLeft | Key::ArrowRight => p.arrow_keys && p.axes[0] && !m.command,
                _ => false,
            }
    }

    fn next_frame(&self) -> Option<std::time::Duration> {
        if self.flick || self.drag.is_some() {
            return Some(std::time::Duration::from_millis(16));
        }
        self.hold.map(|_| std::time::Duration::from_millis(16))
    }
}

/// The bar mode for a host: `classic` or `overlay` from CSS (`--sv-scrollbar-mode`,
/// `scrollbar-gutter: stable` = classic), else the platform's.
pub fn bar_mode(css: Option<&str>, pol: &ScrollPolicy) -> BarMode {
    match css {
        Some("classic" | "stable") => BarMode::Classic,
        Some("overlay" | "auto") => BarMode::Overlay,
        _ => pol.bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(content: Vec2, view: Vec2) -> Scrollable {
        let mut s = Scrollable::default();
        s.set_geometry(content, view, [None, None]);
        s
    }

    #[test]
    fn scroll_by_clamps_and_returns_the_rest() {
        let mut sc = s(vec2(100.0, 500.0), vec2(100.0, 100.0));
        let p = ScrollParams::default();
        assert_eq!(sc.scroll_by(&p, vec2(0.0, 50.0)), Vec2::ZERO);
        assert_eq!(sc.offset(), vec2(0.0, 50.0));
        assert_eq!(sc.scroll_by(&p, vec2(0.0, 1000.0)), vec2(0.0, 650.0));
        assert_eq!(sc.offset().y, 400.0);
        assert!(sc.away_from_start(1) && !sc.away_from_end(1));
        assert_eq!(sc.overflow_state(), sv::OVERFLOW_TOP.bits());
        // x does not scroll here: all of it is left
        assert_eq!(sc.scroll_by(&p, vec2(30.0, 0.0)), vec2(30.0, 0.0));
    }

    #[test]
    fn thumbs_are_proportional_and_never_shorter_than_the_minimum() {
        let mut sc = s(vec2(100.0, 1000.0), vec2(100.0, 100.0));
        let view = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(100.0, 100.0));
        let [_, y] = sc.bar_geometry(view, 10.0, 20.0, [false, true]);
        let y = y.unwrap();
        assert_eq!(y.track, Rect::from_min_max(egui::pos2(90.0, 0.0), egui::pos2(100.0, 100.0)));
        assert_eq!(y.thumb.height(), 20.0, "10% of 100 is below the minimum");
        sc.set_offset(vec2(0.0, 900.0));
        let [_, y] = sc.bar_geometry(view, 10.0, 20.0, [false, true]);
        assert_eq!(y.unwrap().thumb.max.y, 100.0, "at the end");
        // both bars leave the corner
        let [x, y] = sc.bar_geometry(view, 10.0, 20.0, [true, true]);
        assert_eq!(x.unwrap().track.max.x, 90.0);
        assert_eq!(y.unwrap().track.max.y, 90.0);
    }

    #[test]
    fn reveal_moves_the_least_or_centres() {
        let view = Rect::from_min_size(egui::pos2(0.0, 100.0), vec2(100.0, 100.0));
        let below = Rect::from_min_size(egui::pos2(0.0, 250.0), vec2(50.0, 20.0));
        assert_eq!(Scrollable::reveal_delta(view, below, false, 0.0), vec2(0.0, 70.0));
        let above = Rect::from_min_size(egui::pos2(0.0, 60.0), vec2(50.0, 20.0));
        assert_eq!(Scrollable::reveal_delta(view, above, false, 0.0), vec2(0.0, -40.0));
        let inside = Rect::from_min_size(egui::pos2(0.0, 120.0), vec2(50.0, 20.0));
        assert_eq!(Scrollable::reveal_delta(view, inside, false, 0.0), Vec2::ZERO);
        assert_eq!(Scrollable::reveal_delta(view, inside, true, 0.0), vec2(-25.0, -20.0));
        // scroll-margin keeps a gap
        assert_eq!(Scrollable::reveal_delta(view, below, false, 8.0), vec2(0.0, 78.0));
    }

    #[test]
    fn overlay_bars_hold_then_fade() {
        let pol = crate::native::Native::Gtk.policy().scroll;
        let mut sc = s(vec2(100.0, 500.0), vec2(100.0, 100.0));
        sc.touch(10.0);
        assert_eq!(sc.bar_alpha(10.2, &pol), 1.0);
        let mid = sc.bar_alpha(10.0 + (pol.bar_hold + pol.bar_fade / 2.0) as f64, &pol);
        assert!((mid - 0.5).abs() < 0.01, "{mid}");
        assert_eq!(sc.bar_alpha(12.0, &pol), 0.0);
        assert_eq!(sc.bar_repaint(12.0, &pol), None);
    }
}
