//! `scroll-area` and `scrollbar` (ported from svelters-egui `interact/scroll.rs`,
//! `widgets/scroll_area.rs`, `widgets/scrollbar.rs`; architecture §7.5, §11.5, §12.2).
//!
//! - [`Scroller`]: the offset over content larger than its viewport, its clamping, the bars'
//!   geometry and overlay fade, thumb drag and track presses, the keys, AccessKit's scroll actions
//!   and the reveal delta (scroll-into-view).
//! - [`ScrollLatch`]: which scroller of the chain under the pointer a wheel or touchpad gesture
//!   scrolls. It picks the innermost one that can move in the gesture's first direction and keeps
//!   it **for the whole gesture**, OS momentum included: a stream that reaches an edge stops there
//!   instead of scrolling the panel around it (browsers and AppKit; svelters-egui chained per
//!   event). A new gesture chains. A wheel without phases latches per burst
//!   ([`WHEEL_LATCH_TIMEOUT`]).
//! - [`ScrollArea`]: the kind. Wheel steps animate per OS through
//!   [`shim::scroll_physics`](super::shim::scroll_physics) (Chromium's curve on Windows and Linux,
//!   none on macOS), touchpads move 1:1 and fling on release where the OS has no momentum. While
//!   every listener on the area is passive, each step is also a [`FastCommit::ScrollOffset`]: the
//!   compositor moves the content without waiting for the UI phases. The offset is native-owned
//!   (architecture §4.5): a JS write made before JS saw the latest native change is refused.
//! - [`Scrollbar`]: a standalone bar over `value` in `min..=max`, `page` sizing the thumb.
//!
//! The scroll offset belongs to the scroll property node (`ScrollNode`), not to the display list:
//! [`ScrollArea::record`](Kind::record) draws only the bars over the content: `::part(thumb-x)` and
//! `::part(thumb-y)` over [`THUMB_UA`] (`:hover` while the pointer is over the area, `:active` while
//! dragged), and for classic bars their tracks `::part(scrollbar-x)`/`::part(scrollbar-y)` and the
//! corner `::part(corner)` over [`TRACK_UA`]. Overlay bars fade by multiplying the thumb's alpha.

use super::common::{self, platform, secs};
use super::shim::fast;
use super::shim::scroll_physics::{self, Fling, ScrollAnimation, SmoothScroll, VelocityTracker};
use crate::interact::{Kinetic, Platform, Policy, Scrollbars, StateBits, TrackClick};
use crate::kind_props;
use crate::paint::{self, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, Styles, Value, ValueType};
use presto_base::accesskit;
use presto_base::color::{Color, Fill};
use presto_base::commit::FastCommit;
use presto_base::display::{Prim, Quad};
use presto_base::geom::{Corners, Insets};
use presto_base::event::{GesturePhase, KeyState, MouseButton, PointerPhase, WheelEvent};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::ids::{InputSeq, NodeId, PropNodeId, Revision, WindowId};
use presto_base::key::Key;
use presto_base::props::OverscrollBehavior;
use presto_base::time::{Span, Time};

/// Points AccessKit's ScrollUp/Down/Left/Right move (egui's distance).
pub const A11Y_STEP: f32 = 100.0;
/// A held track press repeats after this, then every [`TRACK_REPEAT_EVERY`].
pub const TRACK_REPEAT_FIRST: Span = Span::from_millis(300);
pub const TRACK_REPEAT_EVERY: Span = Span::from_millis(50);
/// Animation and repeat frames while the UI thread samples them (the compositor takes this over).
const FRAME: Span = Span::from_micros(16_667);

/// A thumb's UA sheet (`::part(thumb-x)`, `::part(thumb-y)`): VS Code's slider colours, by state.
pub const THUMB_UA: Ua = Ua(&[
    UaRule::base(&[("background-color", Token("scrollbar"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("scrollbar-hover"))]),
    UaRule::when(StateBits::DRAGGING, &[("background-color", Token("scrollbar-active"))]),
]);

/// A classic bar's track and the corner between two bars (`::part(scrollbar-x)`,
/// `::part(scrollbar-y)`, `::part(corner)`).
pub const TRACK_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("bg"))])]);

fn get(v: Vec2, d: usize) -> f32 {
    if d == 0 { v.x } else { v.y }
}
fn set(v: &mut Vec2, d: usize, x: f32) {
    if d == 0 { v.x = x } else { v.y = x }
}
fn rmin(r: &Rect, d: usize) -> f32 {
    if d == 0 { r.min.x } else { r.min.y }
}
fn rmax(r: &Rect, d: usize) -> f32 {
    if d == 0 { r.max.x } else { r.max.y }
}
fn pget(p: Point, d: usize) -> f32 {
    if d == 0 { p.x } else { p.y }
}
fn vmax(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x.max(b.x), a.y.max(b.y))
}
fn vmin(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x.min(b.x), a.y.min(b.y))
}

/// What moved the offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollBy {
    Wheel,
    /// A touchpad's own fling or the OS's momentum.
    Kinetic,
    Thumb,
    Track,
    Key,
    A11y,
    /// A JS or Rust request (`scrollTo`).
    Program,
}

/// One bar's geometry, in the scroller's local coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarGeom {
    /// The track (the whole bar).
    pub track: Rect,
    /// The thumb inside it.
    pub thumb: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ThumbDrag {
    axis: usize,
    /// Where the pointer grabbed the thumb, from the thumb's start along the axis.
    grab: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TrackHold {
    axis: usize,
    /// +1: towards the end.
    dir: f32,
    next: Time,
    /// The pointer along the axis (the repeat stops when the thumb reaches it).
    at: f32,
}

/// The scroll state of one scrollable: offset, geometry, bars and their pointer state.
#[derive(Clone, Debug, Default)]
pub struct Scroller {
    offset: Vec2,
    content: Size,
    viewport: Size,
    /// The bars' layout: the view they sit in, their width, the shortest thumb, the axes shown.
    bar_cfg: Option<(Rect, f32, f32, [bool; 2])>,
    /// The last time it scrolled or was hovered (overlay bars fade from it).
    last_activity: Time,
    hovered: bool,
    drag: Option<ThumbDrag>,
    hold: Option<TrackHold>,
}

impl Scroller {
    /// The offset (content points scrolled out at the top left).
    pub fn offset(&self) -> Vec2 {
        self.offset
    }
    /// The largest offset on each axis.
    pub fn max_offset(&self) -> Vec2 {
        Vec2::new((self.content.w - self.viewport.w).max(0.0), (self.content.h - self.viewport.h).max(0.0))
    }
    pub fn content(&self) -> Size {
        self.content
    }
    pub fn viewport(&self) -> Size {
        self.viewport
    }
    /// The bars at the current offset (empty until [`Scroller::layout_bars`]).
    pub fn bars(&self) -> [Option<BarGeom>; 2] {
        match self.bar_cfg {
            Some((view, width, min_thumb, show)) => self.compute_bars(view, width, min_thumb, show),
            None => [None, None],
        }
    }
    /// Whether the content overflows on axis `d`.
    pub fn overflows(&self, d: usize) -> bool {
        if d == 0 { self.content.w > self.viewport.w + 0.5 } else { self.content.h > self.viewport.h + 0.5 }
    }
    /// Scrolled away from the start (top or left) of axis `d`: the edge shadow there.
    pub fn away_from_start(&self, d: usize) -> bool {
        get(self.offset, d) > 0.5
    }
    /// Scrolled away from the end of axis `d`.
    pub fn away_from_end(&self, d: usize) -> bool {
        get(self.offset, d) < get(self.max_offset(), d) - 0.5
    }
    /// Whether a delta (positive: towards the end) on axis `d` would move the offset.
    pub fn can_move(&self, d: usize, delta: f32) -> bool {
        if delta > 0.0 {
            self.away_from_end(d)
        } else if delta < 0.0 {
            self.away_from_start(d)
        } else {
            false
        }
    }
    /// The thumb of axis `d` is being dragged.
    pub fn dragging(&self, d: usize) -> bool {
        self.drag.is_some_and(|g| g.axis == d)
    }
    pub fn hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the offset, clamped once the geometry is known.
    pub fn set_offset(&mut self, o: Vec2) {
        self.offset = vmax(o, Vec2::ZERO);
        if self.content != Size::ZERO {
            self.offset = vmin(self.offset, self.max_offset());
        }
    }

    /// The geometry of this layout: content and viewport size. Clamps the offset to it.
    pub fn set_geometry(&mut self, content: Size, viewport: Size) {
        self.content = content;
        self.viewport = viewport;
        self.offset = vmin(vmax(self.offset, Vec2::ZERO), self.max_offset());
    }

    /// Lays the bars out in `view` (local), `width` thick, `min_thumb` the shortest thumb, for the
    /// axes in `show` (each bar leaves room for the other one's corner when both show); they follow
    /// the offset from then on ([`Scroller::bars`]).
    pub fn layout_bars(&mut self, view: Rect, width: f32, min_thumb: f32, show: [bool; 2]) -> [Option<BarGeom>; 2] {
        self.bar_cfg = Some((view, width, min_thumb, show));
        self.bars()
    }

    fn compute_bars(&self, view: Rect, width: f32, min_thumb: f32, show: [bool; 2]) -> [Option<BarGeom>; 2] {
        let mut out = [None, None];
        for d in 0..2 {
            if !show[d] {
                continue;
            }
            let corner = if show[1 - d] { width } else { 0.0 };
            let track = if d == 1 {
                Rect::new(view.max.x - width, view.min.y, view.max.x, view.max.y - corner)
            } else {
                Rect::new(view.min.x, view.max.y - width, view.max.x - corner, view.max.y)
            };
            let len = rmax(&track, d) - rmin(&track, d);
            let content = if d == 0 { self.content.w } else { self.content.h }.max(1.0);
            let vp = if d == 0 { self.viewport.w } else { self.viewport.h };
            let thumb_len = (len * (vp / content).min(1.0)).max(min_thumb.min(len));
            let max = get(self.max_offset(), d);
            let t = if max > 0.0 { (get(self.offset, d) / max).clamp(0.0, 1.0) } else { 0.0 };
            let start = rmin(&track, d) + (len - thumb_len) * t;
            let mut thumb = track;
            if d == 0 {
                thumb.min.x = start;
                thumb.max.x = start + thumb_len;
            } else {
                thumb.min.y = start;
                thumb.max.y = start + thumb_len;
            }
            out[d] = Some(BarGeom { track, thumb });
        }
        out
    }

    /// Overlay bars' opacity at `now`: 1 while hovered, dragged or just scrolled, then fading.
    pub fn bar_alpha(&self, now: Time, p: &Policy) -> f32 {
        if p.scrollbars == Scrollbars::Classic || self.hovered || self.drag.is_some() || self.hold.is_some() {
            return 1.0;
        }
        let t = (now - self.last_activity).as_secs_f64() as f32;
        let (hold, fade) = (p.scroll.bar_hold, p.scroll.bar_fade);
        if t <= hold {
            1.0
        } else if fade > 0.0 && t < hold + fade {
            1.0 - (t - hold) / fade
        } else {
            0.0
        }
    }

    /// When the overlay bars' look next changes without input (`None`: it does not).
    pub fn bar_wake(&self, now: Time, p: &Policy) -> Option<Time> {
        if p.scrollbars == Scrollbars::Classic || self.hovered || self.drag.is_some() {
            return None;
        }
        let hold_end = self.last_activity + secs(p.scroll.bar_hold);
        let fade_end = hold_end + secs(p.scroll.bar_fade);
        if now < hold_end {
            Some(hold_end)
        } else if now < fade_end {
            Some(now + FRAME)
        } else {
            None
        }
    }

    /// Marks activity now (a scroll from outside, a reveal): overlay bars show and fade.
    pub fn touch(&mut self, now: Time) {
        self.last_activity = now;
    }

    /// Scrolls by `delta` (content points; positive: towards the end) on the axes in `axes`;
    /// returns what it could not take (at an edge, or an axis that does not scroll).
    pub fn scroll_by(&mut self, axes: [bool; 2], delta: Vec2) -> Vec2 {
        let max = self.max_offset();
        let mut rest = Vec2::ZERO;
        for d in 0..2 {
            let dd = get(delta, d);
            if dd == 0.0 {
                continue;
            }
            if !axes[d] {
                set(&mut rest, d, dd);
                continue;
            }
            let old = get(self.offset, d);
            let new = (old + dd).clamp(0.0, get(max, d));
            set(&mut rest, d, dd - (new - old));
            set(&mut self.offset, d, new);
        }
        rest
    }

    /// The offset change that brings `target` into `view` (same space): `center` centres it,
    /// otherwise the least scrolling (nothing when it is already inside; its start when larger).
    /// `margin` is CSS `scroll-margin`.
    pub fn reveal_delta(view: Rect, target: Rect, center: bool, margin: f32) -> Vec2 {
        let target = target.inflate(margin);
        let mut out = Vec2::ZERO;
        for a in 0..2 {
            let (v0, v1, t0, t1) = (rmin(&view, a), rmax(&view, a), rmin(&target, a), rmax(&target, a));
            let d = if center {
                (t0 + t1) / 2.0 - (v0 + v1) / 2.0
            } else if t0 < v0 || t1 - t0 > v1 - v0 {
                t0 - v0
            } else if t1 > v1 {
                t1 - v1
            } else {
                0.0
            };
            set(&mut out, a, d);
        }
        out
    }

    /// A page along axis `d`.
    pub fn page(&self, d: usize) -> f32 {
        scroll_physics::page_step(if d == 0 { self.viewport.w } else { self.viewport.h })
    }

    /// The offset on axis `d` that puts the thumb's start at `pos`.
    fn offset_for_thumb(&self, d: usize, pos: f32) -> Option<f32> {
        let b = self.bars()[d]?;
        let len = rmax(&b.track, d) - rmin(&b.track, d);
        let thumb = rmax(&b.thumb, d) - rmin(&b.thumb, d);
        let room = len - thumb;
        if room <= 0.0 {
            return Some(0.0);
        }
        Some(((pos - rmin(&b.track, d)) / room).clamp(0.0, 1.0) * get(self.max_offset(), d))
    }

    /// The pointer over the bars: thumb drag, track press (page or jump, per policy), hold repeat.
    /// Returns whether the event was the bars' (and whether the offset moved).
    pub fn pointer(&mut self, now: Time, policy: &Policy, phase: PointerPhase, primary: bool, pos: Point, shift: bool) -> (bool, bool) {
        let before = self.offset;
        match phase {
            PointerPhase::Move | PointerPhase::Enter => {
                self.hovered = true;
                self.last_activity = now;
                if let Some(g) = self.drag {
                    if let Some(o) = self.offset_for_thumb(g.axis, pget(pos, g.axis) - g.grab) {
                        set(&mut self.offset, g.axis, o);
                    }
                    return (true, self.offset != before);
                }
                if let Some(h) = self.hold.as_mut() {
                    h.at = pget(pos, h.axis);
                    return (true, false);
                }
                (false, false)
            }
            PointerPhase::Leave => {
                self.hovered = false;
                (self.drag.is_some(), false)
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                let ours = self.drag.is_some() || self.hold.is_some();
                self.drag = None;
                self.hold = None;
                (ours, false)
            }
            PointerPhase::Down if primary => {
                let bars = self.bars();
                for d in 0..2 {
                    let Some(b) = bars[d] else { continue };
                    if b.thumb.contains(pos) {
                        self.drag = Some(ThumbDrag { axis: d, grab: pget(pos, d) - rmin(&b.thumb, d) });
                        self.last_activity = now;
                        return (true, false);
                    }
                    if b.track.contains(pos) {
                        let mut mode = policy.scroll.track_click;
                        if policy.scroll.shift_track_click_inverts && shift {
                            mode = if mode == TrackClick::Page { TrackClick::Jump } else { TrackClick::Page };
                        }
                        match mode {
                            TrackClick::Jump => {
                                let half = (rmax(&b.thumb, d) - rmin(&b.thumb, d)) / 2.0;
                                if let Some(o) = self.offset_for_thumb(d, pget(pos, d) - half) {
                                    set(&mut self.offset, d, o);
                                }
                                self.drag = Some(ThumbDrag { axis: d, grab: half });
                            }
                            TrackClick::Page => {
                                let dir = if pget(pos, d) < rmin(&b.thumb, d) { -1.0 } else { 1.0 };
                                self.page_step(d, dir);
                                self.hold = Some(TrackHold { axis: d, dir, next: now + TRACK_REPEAT_FIRST, at: pget(pos, d) });
                            }
                        }
                        self.last_activity = now;
                        return (true, self.offset != before);
                    }
                }
                (false, false)
            }
            PointerPhase::Down => (false, false),
        }
    }

    /// A held track press repeats until the thumb reaches the pointer; returns whether it moved and
    /// when to come back.
    pub fn repeat(&mut self, now: Time) -> (bool, Option<Time>) {
        let Some(h) = self.hold else { return (false, None) };
        if now < h.next {
            return (false, Some(h.next));
        }
        let Some(b) = self.bars()[h.axis] else {
            self.hold = None;
            return (false, None);
        };
        let before_thumb = if h.dir < 0.0 { h.at < rmin(&b.thumb, h.axis) } else { h.at > rmax(&b.thumb, h.axis) };
        if !before_thumb {
            self.hold = None;
            return (false, None);
        }
        let moved = self.page_step(h.axis, h.dir);
        let next = now + TRACK_REPEAT_EVERY;
        self.hold = Some(TrackHold { next, ..h });
        self.last_activity = now;
        (moved, Some(next))
    }

    fn page_step(&mut self, d: usize, dir: f32) -> bool {
        let before = get(self.offset, d);
        let max = get(self.max_offset(), d);
        let new = (before + dir * self.page(d)).clamp(0.0, max);
        set(&mut self.offset, d, new);
        get(self.offset, d) != before
    }

    /// The offset a key asks for (`None`: not a scrolling key here). `page_keys`: PageUp/PageDown,
    /// Home/End; `arrow_keys`: the arrows by `line`.
    pub fn key_target(&self, axes: [bool; 2], key: Key, shift: bool, command: bool, alt: bool, page_keys: bool, arrow_keys: bool, line: f32) -> Option<Vec2> {
        if alt || (shift && key != Key::Space) {
            return None;
        }
        // the vertical axis when it scrolls, else the horizontal one
        let main = if axes[1] { 1 } else { 0 };
        let max = self.max_offset();
        let step: (usize, f32) = match key {
            Key::PageDown if page_keys && !command => (main, self.page(main)),
            Key::PageUp if page_keys && !command => (main, -self.page(main)),
            Key::Space if page_keys && !command => (main, if shift { -self.page(main) } else { self.page(main) }),
            Key::Home if page_keys => (main, -get(self.offset, main)),
            Key::End if page_keys => (main, get(max, main) - get(self.offset, main)),
            Key::ArrowDown if arrow_keys && !command && axes[1] => (1, line),
            Key::ArrowUp if arrow_keys && !command && axes[1] => (1, -line),
            Key::ArrowRight if arrow_keys && !command && axes[0] => (0, line),
            Key::ArrowLeft if arrow_keys && !command && axes[0] => (0, -line),
            _ => return None,
        };
        if !axes[step.0] {
            return None;
        }
        let mut to = self.offset;
        let new = (get(to, step.0) + step.1).clamp(0.0, get(max, step.0));
        set(&mut to, step.0, new);
        Some(to)
    }

    /// An AccessKit scroll action's delta (`None`: not a scroll action).
    pub fn a11y_delta(&self, req: &accesskit::ActionRequest) -> Option<Vec2> {
        Some(match req.action {
            accesskit::Action::ScrollDown => Vec2::new(0.0, A11Y_STEP),
            accesskit::Action::ScrollUp => Vec2::new(0.0, -A11Y_STEP),
            accesskit::Action::ScrollRight => Vec2::new(A11Y_STEP, 0.0),
            accesskit::Action::ScrollLeft => Vec2::new(-A11Y_STEP, 0.0),
            accesskit::Action::SetScrollOffset => match &req.data {
                Some(accesskit::ActionData::SetScrollOffset(p)) => Vec2::new(p.x as f32, p.y as f32) - self.offset,
                _ => return None,
            },
            _ => return None,
        })
    }

    /// The scroll properties and actions of the AccessKit node.
    pub fn a11y(&self, axes: [bool; 2], node: &mut accesskit::Node) {
        let max = self.max_offset();
        if axes[0] {
            node.set_scroll_x(self.offset.x as f64);
            node.set_scroll_x_min(0.0);
            node.set_scroll_x_max(max.x as f64);
            node.add_action(accesskit::Action::ScrollLeft);
            node.add_action(accesskit::Action::ScrollRight);
        }
        if axes[1] {
            node.set_scroll_y(self.offset.y as f64);
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(max.y as f64);
            node.add_action(accesskit::Action::ScrollUp);
            node.add_action(accesskit::Action::ScrollDown);
        }
        node.add_action(accesskit::Action::SetScrollOffset);
    }

    /// Records the bars at `alpha` under the node's styles (thumbs; the tracks and the corner too
    /// for classic bars). `thumb_names`/`track_names` are the parts per axis.
    pub fn record_bars(&self, pc: &mut PaintCx, styles: &Styles, alpha: f32, classic: bool) {
        if alpha <= 0.0 {
            return;
        }
        let host = StyleView::new(styles.computed, Ua::NONE, styles.state, styles.dark);
        let bars = self.bars();
        const THUMBS: [&str; 2] = ["thumb-x", "thumb-y"];
        const TRACKS: [&str; 2] = ["scrollbar-x", "scrollbar-y"];
        for (d, b) in bars.iter().enumerate() {
            let Some(b) = b else { continue };
            if classic {
                let track = host.part(styles.part_css(TRACKS[d]), TRACK_UA).in_state(StateBits::NONE);
                paint::paint_box(pc, &track, b.track);
            }
            let mut st = StateBits::NONE;
            st.set(StateBits::HOVERED, self.hovered);
            st.set(StateBits::DRAGGING, self.dragging(d));
            st.set(StateBits::PRESSED, self.dragging(d));
            let thumb = host.part(styles.part_css(THUMBS[d]), THUMB_UA).in_state(st);
            let c = thumb.color("background-color").unwrap_or(Color::TRANSPARENT);
            let c = c.with_alpha(c.a * alpha * thumb.opacity());
            if c.a <= 0.0 {
                continue;
            }
            let [tl, tr, br, bl] = thumb.radii();
            let r = pc.snap_rect(b.thumb);
            pc.push(Prim::Quad(Quad { rect: r, radii: Corners { tl, tr, br, bl }, fill: Fill::Solid(c), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: Default::default() }), r);
        }
        if classic
            && let (Some(x), Some(y)) = (bars[0], bars[1])
        {
            let corner = host.part(styles.part_css("corner"), TRACK_UA).in_state(StateBits::NONE);
            paint::paint_box(pc, &corner, Rect::new(y.track.min.x, x.track.min.y, y.track.max.x, x.track.max.y));
        }
    }
}

// ---- latching ----

/// A wheel without phases (a mouse wheel, a Windows legacy touchpad) stays latched while its events
/// come less than this apart. Chromium's wheel scroll latching ends a transaction after 500 ms
/// without events (`kDefaultMouseWheelLatchingTransactionMs`, `mouse_wheel_phase_handler.h`;
/// recalled, the research notes recommend 300 ms without a source).
pub const WHEEL_LATCH_TIMEOUT: Span = Span::from_millis(500);

/// One scroller under the pointer, as the latch sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatchCandidate {
    pub node: NodeId,
    /// Per axis: it can move towards the start (`[d][0]`) and towards the end (`[d][1]`).
    pub can_move: [[bool; 2]; 2],
    /// `overscroll-behavior: auto`: a gesture it cannot take goes on to the scrollers around it;
    /// `contain`/`none` keep it here.
    pub chains: bool,
}

impl LatchCandidate {
    /// From a scroller's state.
    pub fn of(node: NodeId, s: &Scroller, axes: [bool; 2], chains: bool) -> Self {
        let mut can_move = [[false; 2]; 2];
        for d in 0..2 {
            can_move[d] = [axes[d] && s.away_from_start(d), axes[d] && s.away_from_end(d)];
        }
        LatchCandidate { node, can_move, chains }
    }
    fn moves(&self, delta: Vec2) -> bool {
        (0..2).any(|d| {
            let v = get(delta, d);
            (v < 0.0 && self.can_move[d][0]) || (v > 0.0 && self.can_move[d][1])
        })
    }
}

/// Where a gesture currently is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gesture {
    /// A phased touchpad gesture (fingers down), then possibly its OS momentum.
    Phased,
    /// The OS momentum after the fingers lifted.
    Momentum,
    /// A wheel burst without phases.
    Wheel,
}

/// Which scroller a wheel or touchpad gesture scrolls, kept for the whole gesture (one per window).
#[derive(Clone, Debug, Default)]
pub struct ScrollLatch {
    latched: Option<(NodeId, Gesture)>,
    last: Time,
}

impl ScrollLatch {
    /// The scroller the current gesture is latched to.
    pub fn latched(&self) -> Option<NodeId> {
        self.latched.map(|(n, _)| n)
    }

    /// Ends the gesture (the pointer left the window, a key was pressed, the latched node went away).
    pub fn reset(&mut self) {
        self.latched = None;
    }

    /// Routes `ev` (with `delta` its offset change, positive towards the end) over `chain`, the
    /// scrollers under the pointer innermost first. Returns the node that takes the whole delta, or
    /// `None` when no scroller takes it (the delta is dropped: never chained mid-gesture).
    pub fn route(&mut self, now: Time, ev: &WheelEvent, delta: Vec2, chain: &[LatchCandidate]) -> Option<NodeId> {
        let starts = match (ev.phase, ev.momentum) {
            (Some(GesturePhase::Started), _) => Some(Gesture::Phased),
            (Some(_), _) => None,
            (None, Some(GesturePhase::Started)) => {
                // momentum continues the gesture its fingers made; alone, it starts one
                match self.latched {
                    Some((_, Gesture::Phased | Gesture::Momentum)) => None,
                    _ => Some(Gesture::Momentum),
                }
            }
            (None, Some(_)) => None,
            (None, None) => match self.latched {
                Some((_, Gesture::Wheel)) if now - self.last < WHEEL_LATCH_TIMEOUT => None,
                _ => Some(Gesture::Wheel),
            },
        };
        self.last = now;
        let still_there = |n: NodeId| chain.iter().any(|c| c.node == n);
        if let Some(kind) = starts {
            // a touchpad's first event often carries no delta: pick on its first movement
            self.latched = if delta == Vec2::ZERO && kind == Gesture::Phased { None } else { Self::pick(delta, chain).map(|n| (n, kind)) };
        } else if let Some((n, kind)) = self.latched {
            if !still_there(n) {
                // the latched scroller left the chain (removed, or the pointer moved off it while
                // fingers rested): the rest of the gesture goes nowhere
                self.latched = None;
                return None;
            }
            if ev.momentum.is_some() && kind == Gesture::Phased {
                self.latched = Some((n, Gesture::Momentum));
            }
        } else if matches!(ev.phase, Some(GesturePhase::Moved)) || matches!(ev.momentum, Some(GesturePhase::Moved)) {
            // a gesture whose start no scroller took: still pick on its first real movement
            self.latched = Self::pick(delta, chain).map(|n| (n, if ev.momentum.is_some() { Gesture::Momentum } else { Gesture::Phased }));
        }
        let out = self.latched.map(|(n, _)| n);
        // the gesture's end releases the latch (after this event, which still goes to it)
        if matches!(ev.phase, Some(GesturePhase::Cancelled))
            || matches!(ev.momentum, Some(GesturePhase::Ended | GesturePhase::Cancelled))
        {
            self.latched = None;
        }
        out
    }

    /// The innermost candidate that moves in the delta's direction; a non-chaining one stops the
    /// search (it takes the gesture although it cannot move).
    fn pick(delta: Vec2, chain: &[LatchCandidate]) -> Option<NodeId> {
        for c in chain {
            if c.moves(delta) || !c.chains {
                return Some(c.node);
            }
        }
        // nothing can move: the innermost takes it (a gesture towards an edge everyone is at)
        chain.first().map(|c| c.node)
    }
}

// ---- scroll-area ----

kind_props! {
    /// The scroll area's props.
    pub struct ScrollAreaProps;
    /// The axes that scroll.
    direction "direction": String = Enum("vertical") ; Enum(&["vertical", "horizontal", "both"]), Layout;
    /// The offset (native-owned: `bind:scroll-x` observes it; a write JS made before it saw the
    /// latest native scroll is refused).
    scroll_x "scroll-x": f64 = F64(0.0) ; F64, Property, bindable;
    scroll_y "scroll-y": f64 = F64(0.0) ; F64, Property, bindable;
    /// At an edge, a new gesture goes on to the scroll area around (`auto`) or stays (`contain`, `none`).
    overscroll_behavior "overscroll-behavior": String = Enum("auto") ; Enum(&["auto", "contain", "none"]), Property;
    /// Every listener on the area is passive (core writes it from the listeners): each step is also a
    /// fast commit.
    passive "passive": bool = Bool(true) ; Bool, Property;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new(
        "scroll",
        &[("x", ValueType::F64), ("y", ValueType::F64), ("by", ValueType::Str), ("revision", ValueType::I64)],
        "The offset moved natively: by `wheel`, `kinetic`, `thumb`, `track`, `key`, `a11y` or `program`; `revision` is the native revision JS has then seen.",
    ),
    SignalSpec::new("scrollend", &[("x", ValueType::F64), ("y", ValueType::F64)], "A scroll gesture or animation stopped."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "scroll-area",
    props: ScrollAreaProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::ScrollView,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// A scrolling container (`QScrollArea`, a CSS `overflow: auto` box).
#[derive(Clone, Debug)]
pub struct ScrollArea {
    pub props: ScrollAreaProps,
    /// `prefers-reduced-motion`: wheel and key steps jump instead of animating (the host sets it;
    /// the platform policy has no such field yet).
    pub reduced_motion: bool,
    /// The axes that scroll.
    pub axes: [bool; 2],
    pub overscroll: OverscrollBehavior,
    /// `overscroll-behavior: auto` (false: `contain`/`none`).
    pub chains: bool,
    /// Every listener on the area is passive (core sets it from the listeners): steps are fast
    /// commits.
    pub passive: bool,
    pub scroller: Scroller,
    /// A wheel step's or fling's animation, sampled on the UI thread until the compositor runs it.
    anim: Option<ScrollAnimation>,
    velocity: VelocityTracker,
    /// Bumped on each native change of the offset (architecture §4.5).
    native_rev: u64,
    /// The native revision of the last scroll JS has seen ([`ScrollArea::js_saw`]).
    js_rev: u64,
    /// The compositor's ids for fast commits ([`ScrollArea::bind`]).
    window: WindowId,
    scroll_node: Option<PropNodeId>,
    revision: Revision,
    input_seq: InputSeq,
    /// A scroll gesture or animation is running (`scrollend` when it stops).
    scrolling: bool,
    focused: bool,
    /// Focus is inside the area (PageUp/PageDown/Home/End are the area's when no child claimed them).
    focus_within: bool,
    /// The last event's time and policy (recording has neither: the bars' fade reads them).
    now: Time,
    policy: Option<Policy>,
}

impl Default for ScrollArea {
    fn default() -> Self {
        ScrollArea {
            props: ScrollAreaProps::default(),
            reduced_motion: false,
            axes: [false, true],
            overscroll: OverscrollBehavior::None,
            chains: true,
            passive: true,
            scroller: Scroller::default(),
            anim: None,
            velocity: VelocityTracker::default(),
            native_rev: 0,
            js_rev: 0,
            window: WindowId::default(),
            scroll_node: None,
            revision: Revision::default(),
            input_seq: InputSeq::default(),
            scrolling: false,
            focused: false,
            focus_within: false,
            now: Time::ZERO,
            policy: None,
        }
    }
}

impl ScrollArea {
    /// The compositor's ids this area's fast commits address: its window and scroll property node.
    pub fn bind(&mut self, window: WindowId, scroll_node: PropNodeId) {
        self.window = window;
        self.scroll_node = Some(scroll_node);
    }
    /// The tree revision and the input being handled, carried by the next fast commits.
    pub fn set_frame_ids(&mut self, revision: Revision, input_seq: InputSeq) {
        self.revision = revision;
        self.input_seq = input_seq;
    }
    /// The native revision of the offset: each native change bumps it; `scroll` events carry it.
    pub fn native_revision(&self) -> u64 {
        self.native_rev
    }
    /// Core: JS is running with the `scroll` event of native revision `rev` as the newest it saw.
    pub fn js_saw(&mut self, rev: u64) {
        self.js_rev = self.js_rev.max(rev);
    }
    /// Focus is on the area (`focused`) or inside it (`within`).
    pub fn set_focus(&mut self, focused: bool, within: bool) {
        self.focused = focused;
        self.focus_within = within || focused;
    }
    /// The candidate it is for [`ScrollLatch::route`].
    pub fn latch_candidate(&self, node: NodeId) -> LatchCandidate {
        LatchCandidate::of(node, &self.scroller, self.axes, self.chains)
    }
    /// A scroll animation is running.
    pub fn animating(&self) -> bool {
        self.anim.is_some()
    }

    /// The layout's geometry: content size and the padding box (local), the bars laid out over it.
    pub fn set_geometry(&mut self, content: Size, view: Rect, bar_width: f32, min_thumb: f32, force: [bool; 2]) {
        self.scroller.set_geometry(content, view.size());
        let show = [self.axes[0] && (self.scroller.overflows(0) || force[0]), self.axes[1] && (self.scroller.overflows(1) || force[1])];
        self.scroller.layout_bars(view, bar_width, min_thumb, show);
    }

    /// The offset moved natively: bumps the revision, reports `scroll`, and fast-commits it when
    /// every listener is passive.
    fn moved(&mut self, cx: &mut EventCx<'_>, by: ScrollBy) {
        self.native_rev += 1;
        self.scroller.touch(cx.now);
        self.scrolling = true;
        let o = self.scroller.offset();
        if (o.x as f64) != self.props.scroll_x {
            self.props.scroll_x = o.x as f64;
            cx.prop_changed(P::scroll_x.id());
        }
        if (o.y as f64) != self.props.scroll_y {
            self.props.scroll_y = o.y as f64;
            cx.prop_changed(P::scroll_y.id());
        }
        if self.passive
            && let Some(node) = self.scroll_node
        {
            fast::push(FastCommit::ScrollOffset { window: self.window, node, offset: o, revision: self.revision, input_seq: self.input_seq });
        }
        let by = match by {
            ScrollBy::Wheel => "wheel",
            ScrollBy::Kinetic => "kinetic",
            ScrollBy::Thumb => "thumb",
            ScrollBy::Track => "track",
            ScrollBy::Key => "key",
            ScrollBy::A11y => "a11y",
            ScrollBy::Program => "program",
        };
        common::emit(cx, &SPEC, "scroll", vec![Value::F64(o.x as f64), Value::F64(o.y as f64), Value::str(by), Value::I64(self.native_rev as i64)]);
    }

    fn end(&mut self, cx: &mut EventCx<'_>) {
        if std::mem::take(&mut self.scrolling) {
            let o = self.scroller.offset();
            common::emit(cx, &SPEC, "scrollend", vec![Value::F64(o.x as f64), Value::F64(o.y as f64)]);
        }
    }

    /// Applies `to` now (no animation), clamped.
    fn jump(&mut self, cx: &mut EventCx<'_>, to: Vec2, by: ScrollBy) -> bool {
        let before = self.scroller.offset();
        self.scroller.set_offset(to);
        let moved = self.scroller.offset() != before;
        if moved {
            self.moved(cx, by);
        }
        moved
    }

    /// Smoothly (or at once, with reduced motion or a zero duration) towards `to`, retargeting a
    /// running animation with its velocity.
    fn animate_to(&mut self, cx: &mut EventCx<'_>, to: Vec2) {
        let to = vmin(vmax(to, Vec2::ZERO), self.scroller.max_offset());
        if self.reduced_motion {
            self.anim = None;
            if !self.jump(cx, to, ScrollBy::Wheel) {
                self.end(cx);
            }
            return;
        }
        match self.anim.as_mut() {
            Some(a) => a.retarget(cx.now, to),
            None => self.anim = Some(ScrollAnimation::Smooth(SmoothScroll::wheel(cx.now, self.scroller.offset(), to))),
        }
        self.step(cx);
    }

    /// Samples the running animation at `cx.now`.
    fn step(&mut self, cx: &mut EventCx<'_>) {
        let Some(a) = self.anim else { return };
        let by = if matches!(a, ScrollAnimation::Fling(_)) { ScrollBy::Kinetic } else { ScrollBy::Wheel };
        let want = a.offset_at(cx.now);
        let moved = self.jump(cx, want, by);
        // an animation that reached an edge stops there
        let stuck = !moved && self.scroller.offset() != want;
        if a.done_at(cx.now) || stuck {
            self.anim = None;
            self.end(cx);
        } else {
            cx.wake_at(cx.now + FRAME);
        }
    }

    /// The wheel or touchpad event the latch routed here (the whole delta is this area's).
    pub fn wheel(&mut self, cx: &mut EventCx<'_>, ev: &WheelEvent) -> Handled {
        let os = platform(cx.policy);
        let delta = scroll_physics::wheel_delta_points(os, ev.delta, self.scroller.viewport());
        // Shift+wheel scrolls horizontally where only x scrolls or the OS did not do it already
        let delta = if ev.modifiers.contains(presto_base::event::Modifiers::SHIFT) && delta.x == 0.0 && os != Platform::Macos {
            Vec2::new(delta.y, 0.0)
        } else {
            delta
        };
        let mut delta = delta;
        for d in 0..2 {
            if !self.axes[d] {
                set(&mut delta, d, 0.0);
            }
        }
        let precise = matches!(ev.delta, presto_base::event::ScrollDelta::Pixels(_));
        match (ev.phase, ev.momentum) {
            (Some(GesturePhase::Started), _) => {
                // fingers down: a fling stops
                self.anim = None;
                self.velocity.reset();
                self.apply_direct(cx, delta, ScrollBy::Wheel);
            }
            (Some(GesturePhase::Moved), _) => {
                self.velocity.push(cx.now, delta);
                self.apply_direct(cx, delta, ScrollBy::Wheel);
            }
            (Some(GesturePhase::Ended), _) => {
                self.apply_direct(cx, delta, ScrollBy::Wheel);
                // an own fling where the OS sends no momentum
                let v = self.velocity.velocity(cx.now);
                if cx.policy.kinetic == Kinetic::Own && v.length() > scroll_physics::FLING_STOP_SPEED {
                    self.anim = Some(ScrollAnimation::Fling(Fling::new(cx.now, self.scroller.offset(), v, self.scroller.viewport())));
                    self.step(cx);
                } else {
                    self.end(cx);
                }
                self.velocity.reset();
            }
            (Some(GesturePhase::Cancelled), _) => {
                self.velocity.reset();
                self.end(cx);
            }
            (None, Some(m)) => {
                // the OS's momentum, 1:1
                self.apply_direct(cx, delta, ScrollBy::Kinetic);
                if matches!(m, GesturePhase::Ended | GesturePhase::Cancelled) {
                    self.end(cx);
                }
            }
            (None, None) if precise || os == Platform::Macos => {
                // precise deltas without phases, and macOS's accelerated wheel: applied at once
                self.apply_direct(cx, delta, ScrollBy::Wheel);
                self.end(cx);
            }
            (None, None) => {
                let base = self.anim.map_or(self.scroller.offset(), |a| a.target());
                self.animate_to(cx, base + delta);
            }
        }
        Handled::Yes
    }

    fn apply_direct(&mut self, cx: &mut EventCx<'_>, delta: Vec2, by: ScrollBy) {
        if delta == Vec2::ZERO {
            return;
        }
        let before = self.scroller.offset();
        self.scroller.scroll_by(self.axes, delta);
        if self.scroller.offset() != before {
            self.moved(cx, by);
        }
    }

    /// `scrollTo` from JS or Rust (`smooth`: Chromium's programmatic curve is approximated by the
    /// wheel curve until presto-anim has it).
    pub fn scroll_to(&mut self, cx: &mut EventCx<'_>, to: Vec2, smooth: bool) {
        if smooth {
            self.animate_to(cx, to);
        } else {
            self.anim = None;
            self.jump(cx, to, ScrollBy::Program);
            self.end(cx);
        }
    }

    /// Brings `target` (local, as laid out in the content) into view.
    pub fn reveal(&mut self, cx: &mut EventCx<'_>, view: Rect, target: Rect, center: bool, margin: f32) {
        let d = Scroller::reveal_delta(view, target, center, margin);
        let to = self.scroller.offset() + d;
        self.anim = None;
        self.jump(cx, to, ScrollBy::Program);
        self.end(cx);
    }
}

impl ScrollArea {
    /// Takes the props into the fields the scroller and the latch read (not the offset).
    fn sync(&mut self) {
        self.axes = match self.props.direction.as_str() {
            "horizontal" => [true, false],
            "both" => [true, true],
            _ => [false, true],
        };
        let (chains, ov) = match self.props.overscroll_behavior.as_str() {
            "contain" | "none" => (false, OverscrollBehavior::None),
            _ => (true, if cfg!(target_os = "macos") { OverscrollBehavior::Elastic } else { OverscrollBehavior::None }),
        };
        self.chains = chains;
        self.overscroll = ov;
        self.passive = self.props.passive;
    }
}

impl Kind for ScrollArea {
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        let axis = if prop == P::scroll_x.id() {
            Some(0)
        } else if prop == P::scroll_y.id() {
            Some(1)
        } else {
            None
        };
        // native code moved the offset since the event JS saw: JS would paint stale state
        if axis.is_some() && self.native_rev > self.js_rev {
            return Err(PropError::Stale);
        }
        self.props.set(prop, v)?;
        if let Some(d) = axis {
            let mut o = self.scroller.offset();
            set(&mut o, d, if d == 0 { self.props.scroll_x } else { self.props.scroll_y } as f32);
            self.anim = None;
            self.scroller.set_offset(o);
            let o = self.scroller.offset();
            (self.props.scroll_x, self.props.scroll_y) = (o.x as f64, o.y as f64);
        }
        self.sync();
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn record(&self, cx: &mut RecordCx) {
        // the content moves with the scroll property node; the area draws only its bars
        let policy = self.policy.unwrap_or_else(crate::host::policy);
        let styles = Styles { computed: cx.style.computed, parts: cx.style.parts, state: cx.style.state, dark: cx.style.dark };
        let alpha = self.scroller.bar_alpha(self.now, &policy);
        self.scroller.record_bars(&mut PaintCx::of(cx), &styles, alpha, policy.scrollbars == Scrollbars::Classic);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.now = cx.now;
        self.policy = Some(*cx.policy);
        self.focused = cx.focused;
        self.focus_within = cx.focused || cx.state.contains(StateBits::FOCUS_WITHIN);
        // the pointer leaving or the capture ending reach the bars as a leave or a cancel
        let synthetic;
        let ev = match ev {
            Event::Hover(false) => {
                synthetic = Event::Pointer(super::split::leave_event(PointerPhase::Leave));
                &synthetic
            }
            Event::CaptureLost => {
                synthetic = Event::Pointer(super::split::leave_event(PointerPhase::Cancel));
                &synthetic
            }
            e => e,
        };
        match ev {
            Event::Wheel(w) => self.wheel(cx, w),
            Event::Pointer(p) => {
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                if p.phase == PointerPhase::Down {
                    // a press stops a fling or a smooth scroll
                    if self.anim.take().is_some() {
                        self.end(cx);
                    }
                }
                let shift = p.modifiers.contains(presto_base::event::Modifiers::SHIFT);
                let was_dragging = self.scroller.dragging(0) || self.scroller.dragging(1);
                let (ours, moved) = self.scroller.pointer(cx.now, cx.policy, p.phase, primary, p.pos, shift);
                let dragging = self.scroller.dragging(0) || self.scroller.dragging(1);
                if dragging != was_dragging {
                    cx.capture = Some(dragging);
                    cx.repaint = true;
                }
                if moved {
                    let by = if dragging { ScrollBy::Thumb } else { ScrollBy::Track };
                    self.moved(cx, by);
                }
                if ours && matches!(p.phase, PointerPhase::Up | PointerPhase::Cancel) {
                    self.end(cx);
                }
                if let Some(t) = self.scroller.hold.map(|h| h.next) {
                    cx.wake_at(t);
                }
                if let Some(t) = self.scroller.bar_wake(cx.now, cx.policy) {
                    cx.wake_at(t);
                }
                if ours { Handled::Yes } else { Handled::No }
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                let m = k.modifiers;
                use presto_base::event::Modifiers as M;
                let command = m.primary(platform(cx.policy) == Platform::Macos);
                let Some(to) = self.scroller.key_target(
                    self.axes,
                    key,
                    m.contains(M::SHIFT),
                    command,
                    m.contains(M::ALT),
                    self.focus_within,
                    self.focused,
                    cx.policy.scroll.line,
                ) else {
                    return Handled::No;
                };
                if to != self.scroller.offset() {
                    // keys: Chromium's constant 150 ms ease-in-out; the wheel curve stands in
                    self.animate_to(cx, to);
                }
                Handled::Yes
            }
            Event::A11y(req) => {
                let Some(d) = self.scroller.a11y_delta(req) else { return Handled::No };
                self.anim = None;
                let before = self.scroller.offset();
                self.scroller.scroll_by(self.axes, d);
                if self.scroller.offset() != before {
                    self.moved(cx, ScrollBy::A11y);
                }
                self.end(cx);
                Handled::Yes
            }
            Event::Focus { gained, .. } => {
                self.focused = *gained;
                Handled::No
            }
            Event::Timer => {
                if self.anim.is_some() {
                    self.step(cx);
                }
                let (moved, next) = self.scroller.repeat(cx.now);
                if moved {
                    self.moved(cx, ScrollBy::Track);
                }
                if let Some(t) = next {
                    cx.wake_at(t);
                }
                if let Some(t) = self.scroller.bar_wake(cx.now, cx.policy) {
                    cx.wake_at(t);
                }
                // the bars fade between events
                cx.repaint = true;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::ScrollView);
        cx.node.set_clips_children();
        self.scroller.a11y(self.axes, cx.node);
    }
}

// ---- scrollbar ----

/// The scroll bar's props (their own module: each `kind_props!` names its index enum `P`).
pub mod scrollbar_props {
    use crate::kind_props;
    use crate::shim::core::Value;

    kind_props! {
        /// The scroll bar's props.
        pub struct ScrollbarProps;
        /// The position in `min..=max` (native-owned while the user drags).
        value "value": f64 = F64(0.0) ; F64, Paint, bindable;
        min "min": f64 = F64(0.0) ; F64, Paint;
        max "max": f64 = F64(100.0) ; F64, Paint;
        /// The visible amount: the thumb's share of the track.
        page "page": f64 = F64(10.0) ; F64, Paint;
        /// What an arrow key or AccessKit increment moves.
        step "step": f64 = F64(1.0) ; F64, Property;
        orientation "orientation": String = Enum("vertical") ; Enum(&["vertical", "horizontal"]), Layout;
    }
}
pub use scrollbar_props::ScrollbarProps;
use scrollbar_props::P as SbP;

/// The scroll bar's signals, in spec order.
pub const SCROLLBAR_SIGNALS: &[SignalSpec] = &[SignalSpec::new("changed", &[("value", ValueType::F64)], "The user moved the bar.")];

pub static SCROLLBAR_SPEC: KindSpec = KindSpec {
    tag: "scrollbar",
    props: ScrollbarProps::SPECS,
    signals: SCROLLBAR_SIGNALS,
    slots: &[],
    children: false,
    role: accesskit::Role::ScrollBar,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// A standalone scroll bar (`QScrollBar`): `value` in `min..=max`, `page` sizing the thumb.
#[derive(Clone, Debug)]
pub struct Scrollbar {
    pub props: ScrollbarProps,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub page: f64,
    pub step: f64,
    /// 0: horizontal, 1: vertical.
    pub axis: usize,
    scroller: Scroller,
    size: Size,
    min_thumb: f32,
}

impl Default for Scrollbar {
    fn default() -> Self {
        Scrollbar { props: ScrollbarProps::default(), value: 0.0, min: 0.0, max: 100.0, page: 10.0, step: 1.0, axis: 1, scroller: Scroller::default(), size: Size::ZERO, min_thumb: 20.0 }
    }
}

impl Scrollbar {
    /// (min, max, page), sanitised: finite, max ≥ min, page ≥ 0.
    fn range(&self) -> (f64, f64, f64) {
        let f = |v: f64, d: f64| if v.is_finite() { v } else { d };
        let min = f(self.min, 0.0);
        let max = f(self.max, 100.0).max(min);
        (min, max, f(self.page, 10.0).max(0.0))
    }

    /// Maps the range onto a scroller over the bar's `size` (local) and lays its thumb out.
    pub fn layout(&mut self, size: Size, min_thumb: f32) {
        self.size = size;
        self.min_thumb = min_thumb;
        let (min, max, page) = self.range();
        let d = self.axis;
        let (mut content, mut view) = (Size::ZERO, Size::ZERO);
        if d == 0 {
            content.w = (max - min + page) as f32;
            view.w = page as f32;
        } else {
            content.h = (max - min + page) as f32;
            view.h = page as f32;
        }
        self.scroller.set_geometry(content, view);
        let mut off = Vec2::ZERO;
        set(&mut off, d, (self.value.clamp(min, max) - min) as f32);
        self.scroller.set_offset(off);
        let mut show = [false, false];
        show[d] = true;
        let thick = if d == 0 { size.h } else { size.w };
        self.scroller.layout_bars(Rect::from_origin_size(Point::ZERO, size), thick, min_thumb, show);
    }

    fn sync(&mut self, cx: &mut EventCx<'_>) {
        let (min, _, _) = self.range();
        let v = min + get(self.scroller.offset(), self.axis) as f64;
        if v != self.value {
            self.value = v;
            self.props.value = v;
            cx.prop_changed(SbP::value.id());
            common::emit(cx, &SCROLLBAR_SPEC, "changed", vec![Value::F64(v)]);
        }
    }

    fn by(&mut self, cx: &mut EventCx<'_>, delta: f64) {
        let mut d = Vec2::ZERO;
        set(&mut d, self.axis, delta as f32);
        let mut axes = [false, false];
        axes[self.axis] = true;
        self.scroller.scroll_by(axes, d);
        self.sync(cx);
    }
}

impl Kind for Scrollbar {
    fn spec() -> &'static KindSpec {
        &SCROLLBAR_SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SCROLLBAR_SPEC
    }

    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        // the drag owns the value until it ends
        if prop == SbP::value.id() && (self.scroller.dragging(0) || self.scroller.dragging(1)) {
            return Err(PropError::Stale);
        }
        self.props.set(prop, v)?;
        let p = &self.props;
        (self.value, self.min, self.max, self.page, self.step) = (p.value, p.min, p.max, p.page, p.step);
        self.axis = if p.orientation == "horizontal" { 0 } else { 1 };
        self.layout(self.size, self.min_thumb);
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn record(&self, cx: &mut RecordCx) {
        let styles = Styles { computed: cx.style.computed, parts: cx.style.parts, state: cx.style.state, dark: cx.style.dark };
        self.scroller.record_bars(&mut PaintCx::of(cx), &styles, 1.0, true);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        match ev {
            Event::Wheel(w) => {
                let d = scroll_physics::wheel_delta_points(platform(cx.policy), w.delta, self.scroller.viewport());
                // a horizontal bar takes the vertical wheel too
                let along = if self.axis == 0 && d.x == 0.0 { d.y } else { get(d, self.axis) };
                self.by(cx, along as f64);
                Handled::Yes
            }
            Event::Pointer(p) => {
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                let shift = p.modifiers.contains(presto_base::event::Modifiers::SHIFT);
                let (ours, _) = self.scroller.pointer(cx.now, cx.policy, p.phase, primary, p.pos, shift);
                if p.phase == PointerPhase::Down && ours {
                    cx.capture = Some(true);
                }
                if matches!(p.phase, PointerPhase::Up | PointerPhase::Cancel) && ours {
                    cx.capture = Some(false);
                }
                if let Some(h) = self.scroller.hold {
                    cx.wake_at(h.next);
                }
                self.sync(cx);
                if ours { Handled::Yes } else { Handled::No }
            }
            Event::Timer => {
                let (_, next) = self.scroller.repeat(cx.now);
                if let Some(t) = next {
                    cx.wake_at(t);
                }
                self.sync(cx);
                Handled::Yes
            }
            Event::A11y(req) => {
                let delta = match req.action {
                    accesskit::Action::Increment => self.step,
                    accesskit::Action::Decrement => -self.step,
                    accesskit::Action::SetValue => match &req.data {
                        Some(accesskit::ActionData::NumericValue(n)) => *n - self.value,
                        _ => return Handled::No,
                    },
                    _ => return Handled::No,
                };
                self.by(cx, delta);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        let node = &mut *cx.node;
        let (min, max, _) = self.range();
        node.set_role(accesskit::Role::ScrollBar);
        node.set_orientation(if self.axis == 0 { accesskit::Orientation::Horizontal } else { accesskit::Orientation::Vertical });
        node.set_numeric_value(self.value.clamp(min, max));
        node.set_min_numeric_value(min);
        node.set_max_numeric_value(max);
        node.set_numeric_value_step(self.step);
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
        node.add_action(accesskit::Action::SetValue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::event::{Modifiers, PointerEvent, PointerType, ScrollDelta};

    fn sc(content: Size, view: Size) -> Scroller {
        let mut s = Scroller::default();
        s.set_geometry(content, view);
        s
    }

    fn wheel(phase: Option<GesturePhase>, momentum: Option<GesturePhase>, dy: f32) -> WheelEvent {
        WheelEvent { pos: Point::new(10.0, 10.0), delta: ScrollDelta::Pixels(Vec2::new(0.0, dy)), phase, momentum, modifiers: Modifiers::default() }
    }

    fn ms(n: u64) -> Time {
        Time(n * 1_000_000)
    }

    fn area(content: f32, view: f32) -> ScrollArea {
        let mut a = ScrollArea::default();
        a.set_geometry(Size::new(100.0, content), Rect::new(0.0, 0.0, 100.0, view), 10.0, 20.0, [false, false]);
        a
    }

    #[test]
    fn scroll_by_clamps_and_returns_the_rest() {
        let mut s = sc(Size::new(100.0, 500.0), Size::new(100.0, 100.0));
        let axes = [false, true];
        assert_eq!(s.scroll_by(axes, Vec2::new(0.0, 50.0)), Vec2::ZERO);
        assert_eq!(s.offset(), Vec2::new(0.0, 50.0));
        assert_eq!(s.scroll_by(axes, Vec2::new(0.0, 1000.0)), Vec2::new(0.0, 650.0));
        assert_eq!(s.offset().y, 400.0);
        assert!(s.away_from_start(1) && !s.away_from_end(1));
        assert!(s.can_move(1, -1.0) && !s.can_move(1, 1.0));
        // x does not scroll here: all of it is left
        assert_eq!(s.scroll_by(axes, Vec2::new(30.0, 0.0)), Vec2::new(30.0, 0.0));
        // a smaller content clamps the offset
        s.set_geometry(Size::new(100.0, 200.0), Size::new(100.0, 100.0));
        assert_eq!(s.offset().y, 100.0);
    }

    #[test]
    fn thumbs_are_proportional_and_never_shorter_than_the_minimum() {
        let mut s = sc(Size::new(100.0, 1000.0), Size::new(100.0, 100.0));
        let view = Rect::new(0.0, 0.0, 100.0, 100.0);
        let [_, y] = s.layout_bars(view, 10.0, 20.0, [false, true]);
        let y = y.unwrap();
        assert_eq!(y.track, Rect::new(90.0, 0.0, 100.0, 100.0));
        assert_eq!(y.thumb.height(), 20.0, "10% of 100 is below the minimum");
        s.set_offset(Vec2::new(0.0, 900.0));
        let [_, y] = s.layout_bars(view, 10.0, 20.0, [false, true]);
        assert_eq!(y.unwrap().thumb.max.y, 100.0, "at the end");
        let [x, y] = s.layout_bars(view, 10.0, 20.0, [true, true]);
        assert_eq!(x.unwrap().track.max.x, 90.0, "both bars leave the corner");
        assert_eq!(y.unwrap().track.max.y, 90.0);
    }

    #[test]
    fn thumb_drag_and_track_page() {
        let pol = Policy::for_os(Os::Windows);
        let mut s = sc(Size::new(100.0, 1000.0), Size::new(100.0, 100.0));
        s.layout_bars(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 20.0, [false, true]);
        // the thumb is 20 tall at 0..20: grab it at 10 and drag to 50 (thumb start 40 of 80)
        assert_eq!(s.pointer(ms(0), &pol, PointerPhase::Down, true, Point::new(95.0, 10.0), false), (true, false));
        let (ours, moved) = s.pointer(ms(10), &pol, PointerPhase::Move, true, Point::new(95.0, 50.0), false);
        assert!(ours && moved);
        assert_eq!(s.offset().y, 450.0);
        s.pointer(ms(20), &pol, PointerPhase::Up, false, Point::new(95.0, 50.0), false);
        assert!(!s.dragging(1));
        // Windows pages on a track press below the thumb, and repeats while held
        s.set_offset(Vec2::ZERO);
        s.layout_bars(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 20.0, [false, true]);
        let (ours, moved) = s.pointer(ms(100), &pol, PointerPhase::Down, true, Point::new(95.0, 90.0), false);
        assert!(ours && moved);
        assert_eq!(s.offset().y, s.page(1));
        s.layout_bars(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 20.0, [false, true]);
        assert_eq!(s.repeat(ms(200)), (false, Some(ms(100) + TRACK_REPEAT_FIRST)));
        let (moved, _) = s.repeat(ms(400));
        assert!(moved);
    }

    #[test]
    fn reveal_moves_the_least_or_centres() {
        let view = Rect::new(0.0, 100.0, 100.0, 200.0);
        let below = Rect::new(0.0, 250.0, 50.0, 270.0);
        assert_eq!(Scroller::reveal_delta(view, below, false, 0.0), Vec2::new(0.0, 70.0));
        let above = Rect::new(0.0, 60.0, 50.0, 80.0);
        assert_eq!(Scroller::reveal_delta(view, above, false, 0.0), Vec2::new(0.0, -40.0));
        let inside = Rect::new(0.0, 120.0, 50.0, 140.0);
        assert_eq!(Scroller::reveal_delta(view, inside, false, 0.0), Vec2::ZERO);
        assert_eq!(Scroller::reveal_delta(view, inside, true, 0.0), Vec2::new(-25.0, -20.0));
        assert_eq!(Scroller::reveal_delta(view, below, false, 8.0), Vec2::new(-8.0, 78.0));
    }

    #[test]
    fn overlay_bars_hold_then_fade() {
        let pol = Policy::for_os(Os::Linux);
        let mut s = sc(Size::new(100.0, 500.0), Size::new(100.0, 100.0));
        s.touch(ms(10_000));
        assert_eq!(s.bar_alpha(ms(10_200), &pol), 1.0);
        let mid = s.bar_alpha(ms(10_000 + 500 + 400), &pol);
        assert!((mid - 0.5).abs() < 0.01, "{mid}");
        assert_eq!(s.bar_alpha(ms(12_000), &pol), 0.0);
        assert_eq!(s.bar_wake(ms(12_000), &pol), None);
        assert_eq!(s.bar_wake(ms(10_100), &pol), Some(ms(10_500)));
    }

    fn n(i: u32) -> NodeId {
        NodeId::new(i, 1)
    }

    /// inner (node 1) scrolled to its end; outer (node 2) can still scroll down.
    fn chain(inner_at_end: bool) -> Vec<LatchCandidate> {
        vec![
            LatchCandidate { node: n(1), can_move: [[false; 2], [true, !inner_at_end]], chains: true },
            LatchCandidate { node: n(2), can_move: [[false; 2], [true, true]], chains: true },
        ]
    }

    #[test]
    fn a_gesture_stays_latched_through_its_momentum() {
        let mut l = ScrollLatch::default();
        let down = Vec2::new(0.0, 10.0);
        // the gesture starts while the inner one can move: latched to it
        assert_eq!(l.route(ms(0), &wheel(Some(GesturePhase::Started), None, -10.0), down, &chain(false)), Some(n(1)));
        // the inner one reaches its end mid-gesture: still it, the outer one does not move
        assert_eq!(l.route(ms(16), &wheel(Some(GesturePhase::Moved), None, -10.0), down, &chain(true)), Some(n(1)));
        assert_eq!(l.route(ms(32), &wheel(Some(GesturePhase::Ended), None, 0.0), down, &chain(true)), Some(n(1)));
        // the OS momentum belongs to the same gesture
        assert_eq!(l.route(ms(40), &wheel(None, Some(GesturePhase::Started), -10.0), down, &chain(true)), Some(n(1)));
        assert_eq!(l.route(ms(56), &wheel(None, Some(GesturePhase::Moved), -10.0), down, &chain(true)), Some(n(1)));
        assert_eq!(l.route(ms(72), &wheel(None, Some(GesturePhase::Ended), 0.0), down, &chain(true)), Some(n(1)));
        assert_eq!(l.latched(), None);
        // a new gesture chains to the outer one
        assert_eq!(l.route(ms(500), &wheel(Some(GesturePhase::Started), None, -10.0), down, &chain(true)), Some(n(2)));
    }

    #[test]
    fn a_wheel_latches_per_burst() {
        let mut l = ScrollLatch::default();
        let down = Vec2::new(0.0, 120.0);
        assert_eq!(l.route(ms(0), &wheel(None, None, -1.0), down, &chain(false)), Some(n(1)));
        // the inner one hit its end, but the burst goes on: still latched
        assert_eq!(l.route(ms(100), &wheel(None, None, -1.0), down, &chain(true)), Some(n(1)));
        // a pause longer than the timeout starts a new burst, which chains
        assert_eq!(l.route(ms(100) + WHEEL_LATCH_TIMEOUT + Span::from_millis(1), &wheel(None, None, -1.0), down, &chain(true)), Some(n(2)));
    }

    #[test]
    fn contain_keeps_the_gesture_and_a_removed_latch_drops_it() {
        let mut l = ScrollLatch::default();
        let down = Vec2::new(0.0, 10.0);
        let mut c = chain(true);
        c[0].chains = false;
        assert_eq!(l.route(ms(0), &wheel(Some(GesturePhase::Started), None, -10.0), down, &c), Some(n(1)), "overscroll-behavior: contain");
        let only_outer = vec![chain(true)[1]];
        assert_eq!(l.route(ms(16), &wheel(Some(GesturePhase::Moved), None, -10.0), down, &only_outer), None);
    }

    #[test]
    fn steps_fast_commit_only_while_passive() {
        let pol = Policy::for_os(Os::MacOs);
        let mut a = area(1000.0, 100.0);
        a.bind(WindowId(3), PropNodeId(7));
        a.set_frame_ids(Revision(5), InputSeq(42));
        let mut cx = EventCx::new(ms(0), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(wheel(None, None, -30.0)));
        assert_eq!(a.scroller.offset().y, 30.0);
        assert_eq!(
            cx.fast,
            vec![FastCommit::ScrollOffset { window: WindowId(3), node: PropNodeId(7), offset: Vec2::new(0.0, 30.0), revision: Revision(5), input_seq: InputSeq(42) }]
        );
        assert_eq!(cx.emitted("scroll").count(), 1);
        assert_eq!(cx.changed, Some(ChangeClass::Property));
        // a non-passive listener: the step waits for the commit
        a.set_prop("passive", &Value::Bool(false)).unwrap();
        let mut cx = EventCx::new(ms(10), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(wheel(None, None, -30.0)));
        assert_eq!(a.scroller.offset().y, 60.0);
        assert!(cx.fast.is_empty());
        assert_eq!(cx.emitted("scroll").count(), 1);
    }

    #[test]
    fn a_stale_js_write_is_refused() {
        let pol = Policy::for_os(Os::MacOs);
        let mut a = area(1000.0, 100.0);
        assert_eq!(a.set_prop("scroll-y", &Value::F64(100.0)), Ok(ChangeClass::Property), "no native change yet");
        assert_eq!(a.scroller.offset().y, 100.0);
        let mut cx = EventCx::new(ms(0), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(wheel(None, None, -30.0)));
        assert_eq!(a.scroller.offset().y, 130.0);
        // JS has not seen that scroll yet
        assert_eq!(a.set_prop("scroll-y", &Value::F64(0.0)), Err(PropError::Stale));
        assert_eq!(a.scroller.offset().y, 130.0);
        // once it has, its write wins
        a.js_saw(a.native_revision());
        assert_eq!(a.set_prop("scroll-y", &Value::F64(0.0)), Ok(ChangeClass::Property));
        assert_eq!(a.scroller.offset().y, 0.0);
    }

    #[test]
    fn a_wheel_detent_animates_on_windows_and_retargets() {
        let pol = Policy::for_os(Os::Windows);
        let mut a = area(2000.0, 100.0);
        let detent = WheelEvent { pos: Point::ZERO, delta: ScrollDelta::Lines(Vec2::new(0.0, -1.0)), phase: None, momentum: None, modifiers: Modifiers::default() };
        let mut cx = EventCx::new(ms(0), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(detent.clone()));
        assert!(a.animating());
        assert_eq!(a.scroller.offset().y, 0.0, "starts at rest");
        assert!(cx.wake.is_some());
        let mut cx = EventCx::new(ms(100), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Timer);
        let mid = a.scroller.offset().y;
        assert!(mid > 0.0 && mid < 100.0, "{mid}");
        // a second detent heads for 200 from where it is
        a.event(&mut cx, &Event::Wheel(detent));
        let mut cx = EventCx::new(ms(1000), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Timer);
        assert_eq!(a.scroller.offset().y, 200.0);
        assert!(!a.animating());
        assert_eq!(cx.emitted("scrollend").count(), 1);
    }

    #[test]
    fn a_press_stops_an_own_fling() {
        let pol = Policy::for_os(Os::Linux);
        let mut a = area(10_000.0, 100.0);
        let mut t = 0;
        let mut cx = EventCx::new(ms(t), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(wheel(Some(GesturePhase::Started), None, 0.0)));
        for _ in 0..8 {
            t += 10;
            let mut cx = EventCx::new(ms(t), Size::new(100.0, 100.0), &pol);
            a.event(&mut cx, &Event::Wheel(wheel(Some(GesturePhase::Moved), None, -20.0)));
        }
        let mut cx = EventCx::new(ms(t + 5), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Wheel(wheel(Some(GesturePhase::Ended), None, 0.0)));
        assert!(a.animating(), "Linux flings on release");
        let press = PointerEvent {
            phase: PointerPhase::Down,
            pointer: 0,
            kind: PointerType::Mouse,
            button: Some(MouseButton::Primary),
            buttons: 1,
            pos: Point::new(10.0, 10.0),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        };
        let mut cx = EventCx::new(ms(t + 50), Size::new(100.0, 100.0), &pol);
        a.event(&mut cx, &Event::Pointer(press));
        assert!(!a.animating());
        assert_eq!(cx.emitted("scrollend").count(), 1);
    }

    #[test]
    fn scrollbar_maps_its_range() {
        let pol = Policy::for_os(Os::Windows);
        let mut b = Scrollbar { max: 90.0, page: 10.0, ..Default::default() };
        b.layout(Size::new(10.0, 100.0), 20.0);
        let mut cx = EventCx::new(ms(0), Size::new(10.0, 100.0), &pol);
        let req = accesskit::ActionRequest { action: accesskit::Action::SetValue, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(1), data: Some(accesskit::ActionData::NumericValue(200.0)) };
        b.event(&mut cx, &Event::A11y(req));
        assert_eq!(b.value, 90.0, "clamped to max");
        assert_eq!(cx.emitted("changed").count(), 1);
        assert_eq!(b.scroller.bars()[1].unwrap().thumb.max.y, 100.0);
    }
}
