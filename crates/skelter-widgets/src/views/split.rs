//! `split-view` and the sash it and `panel` resize with (ported from svelters-egui
//! `widgets/split_view.rs` and `interact/resize.rs`).
//!
//! - [`Sash`]: resizes one size along one axis by pointer drag (after the platform's drag
//!   threshold, measured from the press), by keys while focused (WAI-ARIA window splitter: arrows by
//!   `step`, Shift ×5, Home/End, Enter collapses or restores) and by AccessKit
//!   Increment/Decrement/SetValue. Escape during a drag cancels back to the size before it. A double
//!   click restores the size before the last collapse, or collapses. The hover bar shows after
//!   [`SASH_HOVER_DELAY`], at once while dragged or focused.
//! - [`SplitView`]: panes side by side or stacked with a 1 pt sash line between them. Sizes come from
//!   `sizes`, else `divider-position` (two panes), else equal shares; the room left over goes to the
//!   panes by `flex-grow`, else to the last one; each pane keeps its minimum (`min-size` and its own
//!   `min-*`) and maximum. A sash drag moves the two panes beside it (live, or on release with
//!   `commit-mode="release"`, a guide showing meanwhile). The drag is native: the pane sizes change
//!   without JS (architecture §4.5: a splitter's position during a drag is native-owned).

use super::common::{self, ms, secs};
use crate::interact::{Policy, StateBits};
use crate::kind_props;
use crate::paint::{self, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, ChildCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, StyleCx, Styles, Value, ValueType};
use std::cell::Cell;
use skelter_base::accesskit;
use skelter_base::color::{Color, Fill};
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{Prim, Quad};
use skelter_base::event::{KeyState, Modifiers, MouseButton, PointerPhase};
use skelter_base::geom::{Corners, Insets, Point, Rect, Size};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};

/// R11: hovering a sash shows its hover bar and grip after this long when no policy says otherwise
/// (VS Code 300 ms; the policy's `scroll.sash_hover_delay`).
pub const SASH_HOVER_DELAY: Span = Span::from_millis(300);

/// `::part(sash)`'s UA sheet: the 1 pt line in the border colour; hovered (after the delay), dragged
/// or focused, the 4 pt bar in the focus colour with its grip dots in `color` (VS Code's
/// `sash.hoverBorder`).
pub const SASH_UA: Ua = Ua(&[
    UaRule::base(&[("background-color", Token("border")), ("color", Token("accent-fg"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("focus"))]),
    UaRule::when(StateBits::DRAGGING, &[("background-color", Token("focus"))]),
]);

/// `::part(guide)`'s UA sheet: where the sash will land with `commit-mode="release"`.
pub const GUIDE_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("focus"))])]);

/// What a host configures on a [`Sash`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SashParams {
    /// The axis the size is measured on: 0 (width, a vertical sash) or 1 (height).
    pub axis: usize,
    /// +1: moving the pointer towards larger coordinates grows the size; -1: shrinks it (a
    /// right-side panel).
    pub sign: f32,
    pub min: f32,
    pub max: f32,
    /// Points per arrow key.
    pub step: f32,
    /// Dragging below half the minimum collapses to 0 (Qt's `childrenCollapsible`).
    pub collapsible: bool,
    /// A double click restores or collapses.
    pub double_click: bool,
}

impl Default for SashParams {
    fn default() -> Self {
        SashParams { axis: 0, sign: 1.0, min: 0.0, max: f32::INFINITY, step: 10.0, collapsible: false, double_click: true }
    }
}

/// What a [`Sash`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SashIntent {
    /// A drag started (`drag-started`).
    Start,
    /// The size to show now (live).
    Resize { size: f32 },
    /// The drag ended, or a key step or restore: the size to keep.
    Commit { size: f32 },
    /// Escape during a drag: back to `size`.
    Cancel { size: f32 },
}

/// One sash's state.
#[derive(Clone, Debug, Default)]
pub struct Sash {
    size: f32,
    /// (size, pointer coordinate) of a press not yet a drag.
    pressed: Option<(f32, f32)>,
    /// (size, pointer coordinate) at the start of the drag.
    start: Option<(f32, f32)>,
    hover_since: Option<Time>,
    hovered: bool,
    /// The size before the last collapse.
    restore: Option<f32>,
    focused: bool,
    last_click: Option<Time>,
    /// The policy's hover delay (taken at each pointer event; [`SASH_HOVER_DELAY`] before one).
    delay: Option<Span>,
}

fn coord(p: Point, axis: usize) -> f32 {
    if axis == 0 { p.x } else { p.y }
}

impl Sash {
    /// The host's current size (ignored during a drag: the drag owns it).
    pub fn set_size(&mut self, size: f32) {
        if self.start.is_none() {
            self.size = size;
        }
    }
    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn dragging(&self) -> bool {
        self.start.is_some()
    }
    pub fn hovered(&self) -> bool {
        self.hovered || self.start.is_some()
    }
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }
    /// The hover bar and grip show (after the hover delay, at once while dragged or focused).
    pub fn hover_shown(&self, now: Time) -> bool {
        self.start.is_some() || self.focused || self.hover_since.is_some_and(|t| now - t >= self.hover_delay())
    }
    fn hover_delay(&self) -> Span {
        self.delay.unwrap_or(SASH_HOVER_DELAY)
    }
    /// When the hover bar appears without more input.
    pub fn wake(&self) -> Option<Time> {
        self.hover_since.filter(|_| self.start.is_none()).map(|t| t + self.hover_delay())
    }

    fn clamp(&self, p: &SashParams, v: f32) -> f32 {
        let (lo, hi) = (p.min.max(0.0), p.max.max(p.min.max(0.0)));
        if p.collapsible && v < lo / 2.0 { 0.0 } else { v.clamp(lo, hi) }
    }

    fn set(&mut self, v: f32, out: &mut Vec<SashIntent>, commit: bool) {
        if v > 0.0 && self.size == 0.0 {
            self.restore = None;
        } else if v == 0.0 && self.size > 0.0 {
            self.restore = Some(self.size);
        }
        let changed = v != self.size;
        self.size = v;
        if changed {
            out.push(SashIntent::Resize { size: v });
            if commit {
                out.push(SashIntent::Commit { size: v });
            }
        }
    }

    /// Collapses, or restores the size before the last collapse.
    pub fn toggle_collapse(&mut self, p: &SashParams, out: &mut Vec<SashIntent>) {
        if self.size == 0.0 {
            let back = self.restore.unwrap_or(p.min.max(1.0));
            let v = self.clamp(p, back).max(p.min);
            self.set(v, out, true);
        } else if p.collapsible {
            self.set(0.0, out, true);
        }
    }

    /// Pointer input on the sash's hit area (`pos` in any space the host keeps fixed for the drag).
    pub fn pointer(&mut self, now: Time, policy: &Policy, p: &SashParams, phase: PointerPhase, primary: bool, pos: Point, inside: bool) -> Vec<SashIntent> {
        let mut out = Vec::new();
        self.delay = Some(secs(policy.scroll.sash_hover_delay));
        let at = coord(pos, p.axis);
        match phase {
            PointerPhase::Down if primary && inside => {
                self.pressed = Some((self.size, at));
            }
            PointerPhase::Move | PointerPhase::Enter | PointerPhase::Leave => {
                let hovered = inside && phase != PointerPhase::Leave;
                if hovered && self.hover_since.is_none() {
                    self.hover_since = Some(now);
                } else if !hovered {
                    self.hover_since = None;
                }
                self.hovered = hovered;
                if let Some((s0, p0)) = self.pressed
                    && self.start.is_none()
                    && (at - p0).abs() >= policy.drag_threshold
                {
                    // the drag starts where the press was
                    self.start = Some((s0, p0));
                    self.pressed = None;
                    out.push(SashIntent::Start);
                }
                if let Some((s0, p0)) = self.start {
                    let v = self.clamp(p, s0 + p.sign * (at - p0)).round();
                    self.set(v, &mut out, false);
                }
            }
            PointerPhase::Up => {
                if self.start.take().is_some() {
                    out.push(SashIntent::Commit { size: self.size });
                } else if self.pressed.take().is_some() && inside {
                    // the click count is ours: two clicks within the platform's double-click time
                    let double = self.last_click.is_some_and(|t| now - t <= ms(policy.double_click_ms));
                    self.last_click = if double { None } else { Some(now) };
                    if double && p.double_click {
                        self.toggle_collapse(p, &mut out);
                    }
                }
            }
            PointerPhase::Cancel => {
                self.pressed = None;
                if let Some((s0, _)) = self.start.take() {
                    self.size = s0;
                    out.push(SashIntent::Cancel { size: s0 });
                }
            }
            PointerPhase::Down => {}
        }
        out
    }

    /// A key while the sash is focused (or Escape during a drag).
    pub fn key(&mut self, p: &SashParams, key: Key, m: Modifiers) -> Vec<SashIntent> {
        let mut out = Vec::new();
        if key == Key::Escape {
            if let Some((s0, _)) = self.start.take() {
                self.size = s0;
                out.push(SashIntent::Cancel { size: s0 });
            }
            return out;
        }
        if !self.focused || m.contains(Modifiers::CTRL) || m.contains(Modifiers::ALT) || m.contains(Modifiers::SUPER) {
            return out;
        }
        let (less, more) = if p.axis == 0 { (Key::ArrowLeft, Key::ArrowRight) } else { (Key::ArrowUp, Key::ArrowDown) };
        let mul = if m.contains(Modifiers::SHIFT) { 5.0 } else { 1.0 };
        let v = match key {
            k if k == more => Some(self.size + p.sign * p.step * mul),
            k if k == less => Some(self.size - p.sign * p.step * mul),
            Key::Home => Some(p.min.max(0.0)),
            Key::End if p.max.is_finite() => Some(p.max),
            Key::Enter => {
                self.toggle_collapse(p, &mut out);
                None
            }
            _ => None,
        };
        if let Some(v) = v {
            let v = v.clamp(p.min.max(0.0), p.max.max(p.min.max(0.0))).round();
            self.set(v, &mut out, true);
        }
        out
    }

    /// Whether the focused sash takes `key` itself.
    pub fn claims(&self, p: &SashParams, key: Key) -> bool {
        match key {
            Key::ArrowLeft | Key::ArrowRight => p.axis == 0,
            Key::ArrowUp | Key::ArrowDown => p.axis == 1,
            Key::Home | Key::End => true,
            Key::Enter => p.collapsible,
            Key::Escape => self.start.is_some(),
            _ => false,
        }
    }

    /// An AccessKit action on the sash.
    pub fn a11y_action(&mut self, p: &SashParams, req: &accesskit::ActionRequest) -> Vec<SashIntent> {
        let mut out = Vec::new();
        let v = match req.action {
            accesskit::Action::Increment => Some(self.size + p.step),
            accesskit::Action::Decrement => Some(self.size - p.step),
            accesskit::Action::SetValue => match &req.data {
                Some(accesskit::ActionData::NumericValue(n)) => Some(*n as f32),
                _ => None,
            },
            _ => None,
        };
        if let Some(v) = v {
            let v = v.clamp(p.min.max(0.0), p.max.max(p.min.max(0.0))).round();
            self.set(v, &mut out, true);
        }
        out
    }

    /// The sash's AccessKit node (a `Splitter` with its value).
    pub fn a11y(&self, p: &SashParams, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Splitter);
        node.set_numeric_value(self.size as f64);
        node.set_min_numeric_value(p.min.max(0.0) as f64);
        if p.max.is_finite() {
            node.set_max_numeric_value(p.max as f64);
        }
        node.set_numeric_value_step(p.step as f64);
        // a vertical sash (resizing widths) is a vertical separator
        node.set_orientation(if p.axis == 0 { accesskit::Orientation::Vertical } else { accesskit::Orientation::Horizontal });
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
        node.add_action(accesskit::Action::SetValue);
        node.add_action(accesskit::Action::Focus);
    }
}

/// Records a sash under the node's styles: the line (`::part(sash)`), then (shown) the hover bar and
/// its three-dot grip.
pub(crate) fn record_sash(pc: &mut PaintCx, styles: &Styles, line: Rect, axis: usize, shown: bool, dragging: bool) {
    let host = StyleView::new(styles.computed, Ua::NONE, styles.state, styles.dark);
    let base = host.part(styles.part_css("sash"), SASH_UA).in_state(StateBits::NONE);
    paint::paint_box(pc, &base, line);
    if !shown {
        return;
    }
    let mut st = StateBits::HOVERED;
    st.set(StateBits::DRAGGING, dragging);
    let hot = host.part(styles.part_css("sash"), SASH_UA).in_state(st);
    let (w, dot, gap) = (4.0, 2.0, 2.0);
    let c = Point::new((line.min.x + line.max.x) / 2.0, (line.min.y + line.max.y) / 2.0);
    let bar = if axis == 0 { Rect::new(c.x - w / 2.0, line.min.y, c.x + w / 2.0, line.max.y) } else { Rect::new(line.min.x, c.y - w / 2.0, line.max.x, c.y + w / 2.0) };
    paint::paint_box(pc, &hot, bar);
    let color = hot.color("color").unwrap_or(Color::WHITE);
    for i in -1..=1 {
        let off = i as f32 * (dot + gap);
        let p = if axis == 0 { Point::new(c.x, c.y + off) } else { Point::new(c.x + off, c.y) };
        let r = Rect::new(p.x - dot / 2.0, p.y - dot / 2.0, p.x + dot / 2.0, p.y + dot / 2.0);
        pc.push(Prim::Quad(Quad { rect: r, radii: Corners::all(dot / 2.0), fill: Fill::Solid(color), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: Default::default() }), r);
    }
}

/// A pane's limits along the split's axis, from its computed style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneLimits {
    pub min: f32,
    pub max: f32,
    pub grow: f32,
}

impl Default for PaneLimits {
    fn default() -> Self {
        PaneLimits { min: 0.0, max: f32::INFINITY, grow: 0.0 }
    }
}

kind_props! {
    /// The split view's props.
    pub struct SplitViewProps;
    /// Panes side by side (`horizontal`) or stacked (`vertical`).
    orientation "orientation": String = Enum("horizontal") ; Enum(&["horizontal", "vertical"]), Layout;
    /// The first pane's size in points (unset: its share).
    divider_position "divider-position": Option<f64> = None ; F64, Layout, bindable;
    /// The smallest size of a pane, in points.
    min_size "min-size": f64 = F64(32.0) ; F64, Layout;
    resizable "resizable": bool = Bool(true) ; Bool, Paint;
    /// Every pane's size (empty: from `divider-position` or equal shares).
    sizes "sizes": Vec<Value> = None ; List, Layout, bindable;
    /// Dragging below half a pane's minimum collapses it.
    collapsible "collapsible": bool = Bool(false) ; Bool, Layout;
    restore_on_double_click "restore-on-double-click": bool = Bool(true) ; Bool, Property;
    /// `release`: the panes move when the drag ends; a guide shows meanwhile.
    commit_mode "commit-mode": String = Enum("live") ; Enum(&["live", "release"]), Property;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("position", ValueType::F64)], "The first pane's size changed (each step of a live drag)."),
    SignalSpec::new("drag-started", &[], ""),
    SignalSpec::new("drag-stopped", &[], ""),
    SignalSpec::new("commit", &[("sizes", ValueType::List)], "A drag ended, or a key or AccessKit step: the sizes to keep."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "split-view",
    props: SplitViewProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Group,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// Panes split by draggable sashes (`QSplitter`).
#[derive(Clone, Debug)]
pub struct SplitView {
    pub props: SplitViewProps,
    pub vertical: bool,
    /// The first pane's size; `None` for its share.
    pub divider_position: Option<f64>,
    /// The smallest size of a pane, in points.
    pub min_size: f64,
    pub resizable: bool,
    /// Every pane's size (`None`: from `divider-position` or equal).
    pub sizes: Option<Vec<f64>>,
    pub collapsible: bool,
    pub restore_on_double_click: bool,
    /// `commit-mode="release"`: panes move when the drag ends; a guide shows meanwhile.
    pub commit_on_release: bool,
    /// The sash line's thickness and its hit area's.
    pub line: f32,
    pub hit: f32,
    size: Size,
    laid: Vec<f32>,
    mins: Vec<f32>,
    maxs: Vec<f32>,
    sashes: Vec<Sash>,
    /// The sash the pointer pressed (it keeps the pointer until release).
    active: Option<usize>,
    /// The sash being dragged (between `drag-started` and `drag-stopped`).
    drag: Option<usize>,
    focused: Option<usize>,
    /// `commit-mode="release"`: the sash being dragged and the size it would give.
    ghost: Option<(usize, f32)>,
    now: Time,
    /// The pane count core last laid out ([`Kind::child_layout`] sees it; [`Kind::prepare`] needs it).
    panes: Cell<usize>,
}

/// How one pane takes its share of the main axis ([`SplitView::weights`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Share {
    /// A fixed size in points (`divider-position` for the first of two panes).
    Fixed(f32),
    /// A share of the room left, by weight.
    Grow(f32),
}

impl Default for SplitView {
    fn default() -> Self {
        SplitView {
            props: SplitViewProps::default(),
            vertical: false,
            divider_position: None,
            min_size: 32.0,
            resizable: true,
            sizes: None,
            collapsible: false,
            restore_on_double_click: true,
            commit_on_release: false,
            line: 1.0,
            hit: 4.0,
            size: Size::ZERO,
            laid: Vec::new(),
            mins: Vec::new(),
            maxs: Vec::new(),
            sashes: Vec::new(),
            active: None,
            drag: None,
            focused: None,
            ghost: None,
            now: Time::ZERO,
            panes: Cell::new(0),
        }
    }
}

impl SplitView {
    fn axis(&self) -> usize {
        if self.vertical { 1 } else { 0 }
    }

    /// The pane sizes for `room` points (the main size less the sash lines).
    pub fn resolve(&self, room: f32, limits: &[PaneLimits]) -> Vec<f32> {
        let n = limits.len();
        if n == 0 {
            return Vec::new();
        }
        let given = self.sizes.as_ref().filter(|s| s.len() == n && s.iter().all(|x| x.is_finite() && *x >= 0.0));
        let mut s: Vec<f32> = match (given, self.divider_position) {
            (Some(v), _) => v.iter().map(|x| *x as f32).collect(),
            (None, Some(p)) if n == 2 => {
                // the first pane at `p`, each keeping min(min-size, half)
                let lo = limits[0].min.min(room / 2.0);
                let first = (p as f32).clamp(lo, (room - limits[1].min.min(room / 2.0)).max(lo));
                return vec![first, (room - first).max(0.0)];
            }
            _ => vec![room / n as f32; n],
        };
        // the difference goes to the panes by flex-grow (else the last pane)
        let diff = room - s.iter().sum::<f32>();
        if diff.abs() > 0.01 {
            let total: f32 = limits.iter().map(|l| l.grow).sum();
            if total > 0.0 {
                for (k, l) in limits.iter().enumerate() {
                    s[k] += diff * l.grow / total;
                }
            } else {
                s[n - 1] += diff;
            }
        }
        // bounds (a collapsed pane may stay at 0), then the rest from the last panes that can take it
        for k in 0..n {
            let lo = limits[k].min.min(room / n as f32);
            if !(self.collapsible && s[k] == 0.0) {
                s[k] = s[k].clamp(lo, limits[k].max.max(lo));
            }
        }
        let mut rest = room - s.iter().sum::<f32>();
        for k in (0..n).rev() {
            if rest.abs() < 0.01 {
                break;
            }
            if self.collapsible && s[k] == 0.0 {
                continue;
            }
            let lo = limits[k].min.min(room / n as f32);
            let new = (s[k] + rest).clamp(lo, limits[k].max.max(lo));
            rest -= new - s[k];
            s[k] = new;
        }
        if rest.abs() >= 0.01 {
            s[n - 1] = (s[n - 1] + rest).max(0.0);
        }
        // whole points, rounding the edges (so the sizes still add up to the room)
        let mut out = Vec::with_capacity(n);
        let (mut acc, mut prev) = (0.0f32, 0.0f32);
        for v in s {
            acc += v;
            let edge = acc.round();
            out.push(edge - prev);
            prev = edge;
        }
        out
    }

    /// Lays the visible panes out in `size` (the split view's content box); returns their rects
    /// (local). `limits` are each pane's own `min-*`/`max-*` and `flex-grow` along the axis.
    pub fn layout(&mut self, size: Size, limits: &[PaneLimits]) -> Vec<Rect> {
        let ax = self.axis();
        let n = limits.len();
        let gaps = self.line * n.saturating_sub(1) as f32;
        let main = if ax == 0 { size.w } else { size.h };
        let min_size = self.min_size.max(0.0) as f32;
        let limits: Vec<PaneLimits> = limits.iter().map(|l| PaneLimits { min: l.min.max(min_size), ..*l }).collect();
        let sizes = self.resolve((main - gaps).max(0.0), &limits);
        let mut out = Vec::with_capacity(n);
        let mut at = 0.0;
        for &s in &sizes {
            out.push(if ax == 0 { Rect::new(at, 0.0, at + s, size.h) } else { Rect::new(0.0, at, size.w, at + s) });
            at += s + self.line;
        }
        self.size = size;
        self.mins = limits.iter().map(|l| l.min).collect();
        self.maxs = limits.iter().map(|l| l.max).collect();
        self.laid = sizes;
        self.sashes.resize_with(n.saturating_sub(1), Sash::default);
        out
    }

    /// How each of `n` panes takes the main axis: `sizes` as weights when it lists `n` sizes, else
    /// `divider-position` for the first of two panes, else the last laid sizes, else equal shares.
    /// Weights equal to sizes that fill the room give exactly those sizes; when the room changes the
    /// panes keep their proportions (`QSplitter`'s stretch).
    pub fn weights(&self, n: usize) -> Vec<Share> {
        let pos = |v: &[f32]| v.len() == n && v.iter().all(|x| x.is_finite() && *x >= 0.0) && v.iter().sum::<f32>() > 0.0;
        if let Some(s) = self.sizes.as_ref().map(|s| s.iter().map(|x| *x as f32).collect::<Vec<_>>()).filter(|s| pos(s)) {
            return s.into_iter().map(Share::Grow).collect();
        }
        if let (Some(p), 2) = (self.divider_position, n) {
            return vec![Share::Fixed(p.max(0.0) as f32), Share::Grow(1.0)];
        }
        if pos(&self.laid) {
            return self.laid.iter().map(|x| Share::Grow(*x)).collect();
        }
        vec![Share::Grow(1.0); n]
    }

    /// The pane sizes [`SplitView::weights`] give in `room` points (what taffy computes for the
    /// children [`Kind::child_layout`] styles, before CSS `min-*`/`max-*` on the panes).
    pub fn shares(&self, n: usize, room: f32) -> Vec<f32> {
        let w = self.weights(n);
        let fixed: f32 = w.iter().map(|s| if let Share::Fixed(f) = s { f.min(room) } else { 0.0 }).sum();
        let total: f32 = w.iter().map(|s| if let Share::Grow(g) = s { *g } else { 0.0 }).sum();
        let left = (room - fixed).max(0.0);
        w.iter().map(|s| match s { Share::Fixed(f) => f.min(room), Share::Grow(g) if total > 0.0 => left * g / total, Share::Grow(_) => 0.0 }).collect()
    }

    /// The laid-out pane sizes.
    pub fn laid(&self) -> &[f32] {
        &self.laid
    }

    /// Sash `k`'s line and hit rects (local).
    pub fn sash_rects(&self, k: usize) -> (Rect, Rect) {
        let ax = self.axis();
        let start = self.laid[..=k].iter().sum::<f32>() + self.line * k as f32;
        let grow = (self.hit.max(self.line) - self.line) / 2.0;
        if ax == 0 {
            (Rect::new(start, 0.0, start + self.line, self.size.h), Rect::new(start - grow, 0.0, start + self.line + grow, self.size.h))
        } else {
            (Rect::new(0.0, start, self.size.w, start + self.line), Rect::new(0.0, start - grow, self.size.w, start + self.line + grow))
        }
    }

    fn sash_params(&self, k: usize) -> SashParams {
        let pair = self.laid[k] + self.laid[k + 1];
        SashParams {
            axis: self.axis(),
            sign: 1.0,
            // with `collapsible`, below half of the pane's own minimum collapses
            min: self.mins[k].min(pair),
            max: (pair - if self.collapsible { 0.0 } else { self.mins.get(k + 1).copied().unwrap_or(0.0) }).max(0.0).min(self.maxs[k]),
            step: 10.0,
            collapsible: self.collapsible,
            double_click: self.restore_on_double_click,
        }
    }

    /// Focus moved to sash `k` (Tab reaches each sash as its own stop), or off the sashes.
    pub fn focus_sash(&mut self, k: Option<usize>) {
        self.focused = k.filter(|k| *k < self.sashes.len());
        for (i, s) in self.sashes.iter_mut().enumerate() {
            s.set_focused(Some(i) == self.focused);
        }
    }

    /// Takes the props into the fields the layout and the sashes read.
    fn sync(&mut self) {
        let p = &self.props;
        self.vertical = p.orientation == "vertical";
        self.divider_position = p.divider_position;
        self.min_size = p.min_size;
        self.resizable = p.resizable;
        self.sizes = (!p.sizes.is_empty()).then(|| p.sizes.iter().map(|v| v.as_f64().unwrap_or(-1.0)).collect());
        self.collapsible = p.collapsible;
        self.restore_on_double_click = p.restore_on_double_click;
        self.commit_on_release = p.commit_mode == "release";
    }

    /// Writes new pane sizes (the user's): `sizes`, `divider-position` (native changes), `changed`.
    fn write(&mut self, cx: &mut EventCx<'_>, sizes: Vec<f32>) {
        let v: Vec<f64> = sizes.iter().map(|x| *x as f64).collect();
        let first = v.first().copied();
        self.props.sizes = v.iter().map(|x| Value::F64(*x)).collect();
        self.sizes = Some(v);
        cx.prop_changed(P::sizes.id());
        if let Some(f) = first
            && Some(f) != self.divider_position
        {
            self.divider_position = Some(f);
            self.props.divider_position = Some(f);
            cx.prop_changed(P::divider_position.id());
            common::emit(cx, &SPEC, "changed", vec![Value::F64(f)]);
        }
        self.laid = sizes;
    }

    fn apply(&mut self, cx: &mut EventCx<'_>, k: usize, intents: Vec<SashIntent>) {
        let pair = self.laid[k] + self.laid[k + 1];
        let with = |laid: &[f32], size: f32| {
            let mut s = laid.to_vec();
            s[k] = size;
            s[k + 1] = (pair - size).max(0.0);
            s
        };
        for i in intents {
            match i {
                SashIntent::Start => {
                    self.drag = Some(k);
                    cx.capture = Some(true);
                    common::emit(cx, &SPEC, "drag-started", Vec::new());
                }
                SashIntent::Resize { size } => {
                    if self.commit_on_release && self.sashes[k].dragging() {
                        self.ghost = Some((k, size));
                        cx.repaint = true;
                    } else {
                        let s = with(&self.laid, size);
                        self.write(cx, s);
                    }
                }
                SashIntent::Commit { size } => {
                    let s = with(&self.laid, size);
                    self.ghost = None;
                    let was_drag = self.drag.take() == Some(k);
                    self.write(cx, s.clone());
                    common::emit(cx, &SPEC, "commit", vec![Value::List(s.iter().map(|x| Value::F64(*x as f64)).collect())]);
                    if was_drag {
                        cx.capture = Some(false);
                        common::emit(cx, &SPEC, "drag-stopped", Vec::new());
                    }
                }
                SashIntent::Cancel { size } => {
                    if self.ghost.take().is_none() {
                        let s = with(&self.laid, size);
                        self.write(cx, s);
                    } else {
                        cx.repaint = true;
                    }
                    self.drag = None;
                    cx.capture = Some(false);
                    common::emit(cx, &SPEC, "drag-stopped", Vec::new());
                }
            }
        }
    }
}

impl Kind for SplitView {
    fn parts() -> &'static [&'static str] {
        &["sash", "guide"]
    }
    fn places_children() -> bool {
        true
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        // a drag owns the pane sizes until it ends
        if (prop == P::divider_position.id() || prop == P::sizes.id()) && self.sashes.iter().any(Sash::dragging) {
            return Err(PropError::Stale);
        }
        self.props.set(prop, v)?;
        if prop == P::sizes.id()
            && self.props.sizes.iter().any(|x| x.as_f64().is_none())
        {
            self.props.sizes.clear();
            return Err(PropError::Type { prop: "sizes", expected: ValueType::List });
        }
        // a host write of `divider-position` overrides `sizes`
        if prop == P::divider_position.id() {
            self.props.sizes.clear();
        }
        self.sync();
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// A flex row (column when vertical) with the sash lines as gaps; the panes are placed by
    /// [`Kind::child_layout`].
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        s.display = taffy::Display::Flex;
        s.flex_direction = if self.vertical { taffy::FlexDirection::Column } else { taffy::FlexDirection::Row };
        s.flex_wrap = taffy::FlexWrap::NoWrap;
        s.gap = taffy::Size { width: taffy::LengthPercentage::length(self.line), height: taffy::LengthPercentage::length(self.line) };
        s
    }

    /// Each pane fills the cross axis and takes its [`SplitView::weights`] share of the main axis,
    /// never below `min-size`.
    fn child_layout(&self, cx: &ChildCx, style: &mut taffy::Style) -> bool {
        self.panes.set(cx.count());
        let share = self.weights(cx.count()).get(cx.index).copied().unwrap_or(Share::Grow(1.0));
        match share {
            Share::Fixed(f) => {
                style.flex_basis = taffy::Dimension::length(f);
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
            }
            Share::Grow(g) => {
                style.flex_basis = taffy::Dimension::length(0.0);
                style.flex_grow = g;
                style.flex_shrink = 1.0;
            }
        }
        let min = taffy::LengthPercentageAuto::length(self.min_size.max(0.0) as f32);
        if self.vertical {
            style.min_size.height = min;
        } else {
            style.min_size.width = min;
        }
        style.align_self = Some(taffy::AlignSelf::STRETCH);
        true
    }

    /// Takes the laid size: the pane sizes (as [`SplitView::shares`] gives them) and the sashes
    /// between them.
    fn prepare(&mut self, cx: &mut PrepareCx) {
        let n = self.panes.get();
        let main = if self.vertical { cx.size.h } else { cx.size.w };
        let room = (main - self.line * n.saturating_sub(1) as f32).max(0.0);
        self.laid = self.shares(n, room);
        self.size = cx.size;
        let min = self.min_size.max(0.0) as f32;
        self.mins = vec![min; n];
        self.maxs = vec![f32::INFINITY; n];
        self.sashes.resize_with(n.saturating_sub(1), Sash::default);
    }

    fn record(&self, cx: &mut RecordCx) {
        let ax = self.axis();
        let styles = Styles { computed: cx.style.computed, parts: cx.style.parts, children: cx.style.children, state: cx.style.state, dark: cx.style.dark };
        let mut pc = PaintCx::of(cx);
        for k in 0..self.sashes.len() {
            let (line, _) = self.sash_rects(k);
            record_sash(&mut pc, &styles, line, ax, self.resizable && self.sashes[k].hover_shown(self.now), self.sashes[k].dragging());
        }
        if let Some((k, size)) = self.ghost {
            let at = self.laid[..k].iter().sum::<f32>() + self.line * k as f32 + size;
            let g = if ax == 0 { Rect::new(at - 1.0, 0.0, at + 1.0, self.size.h) } else { Rect::new(0.0, at - 1.0, self.size.w, at + 1.0) };
            let host = StyleView::new(styles.computed, Ua::NONE, styles.state, styles.dark);
            let guide = host.part(styles.part_css("guide"), GUIDE_UA);
            paint::paint_box(&mut pc, &guide, g);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.now = cx.now;
        if !self.resizable || self.sashes.is_empty() {
            return Handled::No;
        }
        // the pointer leaving or the capture ending reach the sashes as a leave or a cancel
        let synthetic;
        let ev = match ev {
            Event::Hover(false) => {
                synthetic = Event::Pointer(leave_event(PointerPhase::Leave));
                &synthetic
            }
            Event::CaptureLost => {
                synthetic = Event::Pointer(leave_event(PointerPhase::Cancel));
                &synthetic
            }
            e => e,
        };
        match ev {
            Event::Pointer(p) => {
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                let under = (0..self.sashes.len()).find(|k| p.phase != PointerPhase::Leave && self.sash_rects(*k).1.contains(p.pos));
                if p.phase == PointerPhase::Down && primary {
                    self.active = under;
                }
                let mut handled = Handled::No;
                for k in 0..self.sashes.len() {
                    let inside = under == Some(k);
                    // the pressed sash keeps the pointer; the others only see hover
                    if self.active.is_some_and(|a| a != k) && !matches!(p.phase, PointerPhase::Move | PointerPhase::Leave) {
                        continue;
                    }
                    let was_shown = self.sashes[k].hover_shown(cx.now);
                    let params = self.sash_params(k);
                    self.sashes[k].set_size(self.laid[k]);
                    let intents = self.sashes[k].pointer(cx.now, cx.policy, &params, p.phase, primary, p.pos, inside);
                    if self.sashes[k].hover_shown(cx.now) != was_shown {
                        cx.repaint = true;
                    }
                    if let Some(t) = self.sashes[k].wake() {
                        cx.wake_at(t);
                    }
                    if inside || self.active == Some(k) {
                        handled = Handled::Yes;
                    }
                    self.apply(cx, k, intents);
                }
                if under.is_some() || self.active.is_some() {
                    cx.cursor = Some(if self.vertical { CursorIcon::RowResize } else { CursorIcon::ColResize });
                }
                if matches!(p.phase, PointerPhase::Up | PointerPhase::Cancel) {
                    self.active = None;
                }
                handled
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                let target = if key == Key::Escape { (0..self.sashes.len()).find(|i| self.sashes[*i].dragging()) } else { self.focused };
                let Some(i) = target else { return Handled::No };
                let params = self.sash_params(i);
                if !self.sashes[i].claims(&params, key) {
                    return Handled::No;
                }
                self.sashes[i].set_size(self.laid[i]);
                let intents = self.sashes[i].key(&params, key, k.modifiers);
                self.apply(cx, i, intents);
                Handled::Yes
            }
            Event::A11y(req) => {
                let Some(k) = self.focused else { return Handled::No };
                self.sash_action(cx, k, req);
                Handled::Yes
            }
            Event::Focus { gained: false, .. } => {
                self.focus_sash(None);
                cx.repaint = true;
                Handled::No
            }
            Event::Timer => {
                cx.repaint = true;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Group);
    }
}

/// A pointer event with no position of its own (the pointer left, or the capture ended).
pub(crate) fn leave_event(phase: PointerPhase) -> skelter_base::event::PointerEvent {
    skelter_base::event::PointerEvent {
        phase,
        pointer: 0,
        kind: skelter_base::event::PointerType::Mouse,
        button: None,
        buttons: 0,
        pos: Point::new(f32::NAN, f32::NAN),
        modifiers: Modifiers::default(),
        pressure: 0.0,
        tilt: skelter_base::geom::Vec2::ZERO,
        history: Default::default(),
    }
}

impl SplitView {
    /// Sash `k`'s AccessKit node (each sash is its own `Splitter`).
    pub fn sash_a11y(&self, k: usize, label: &str, node: &mut accesskit::Node) {
        self.sashes[k].a11y(&self.sash_params(k), node);
        node.set_label(if label.is_empty() { format!("Resize pane {}", k + 1) } else { label.to_string() });
    }

    /// An AccessKit action on sash `k`.
    pub fn sash_action(&mut self, cx: &mut EventCx<'_>, k: usize, req: &accesskit::ActionRequest) {
        let params = self.sash_params(k);
        self.sashes[k].set_size(self.laid[k]);
        let intents = self.sashes[k].a11y_action(&params, req);
        self.apply(cx, k, intents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{Native, Platform};
    use skelter_base::event::{KeyEvent, KeyLocation, PointerEvent, PointerType};
    use skelter_base::geom::Vec2;

    fn lim(n: usize) -> Vec<PaneLimits> {
        vec![PaneLimits { min: 32.0, ..Default::default() }; n]
    }

    fn ptr(phase: PointerPhase, x: f32) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: if matches!(phase, PointerPhase::Down | PointerPhase::Move) { 1 } else { 0 },
            pos: Point::new(x, 50.0),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    fn key(k: Key) -> Event {
        Event::Key(KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() })
    }

    #[test]
    fn child_layout_gives_each_pane_its_share_and_prepare_takes_it() {
        let mut s = SplitView::default();
        let anchors = ["", "", ""];
        let mut styles = Vec::new();
        for i in 0..3 {
            let mut st = taffy::Style::default();
            assert!(s.child_layout(&ChildCx { index: i, anchors: &anchors }, &mut st), "every pane shows");
            styles.push(st);
        }
        assert!(styles.iter().all(|st| st.flex_grow == 1.0 && st.flex_basis == taffy::Dimension::length(0.0)), "equal shares");
        assert_eq!(styles[0].min_size.width, taffy::LengthPercentageAuto::length(32.0));
        // 302 wide, two 1-point sash lines: 100 each
        let mut text = crate::shim::testing::MonoShaper;
        let style = Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true };
        s.prepare(&mut PrepareCx { size: Size::new(302.0, 50.0), style, text: &mut text, emitted: Vec::new(), layouts: None });
        assert_eq!(s.laid(), &[100.0, 100.0, 100.0]);
        assert_eq!(s.sash_rects(0).0, Rect::new(100.0, 0.0, 101.0, 50.0));
        // `sizes` become weights: they hold while the room is theirs and scale with it
        s.sizes = Some(vec![50.0, 150.0, 100.0]);
        assert_eq!(s.shares(3, 300.0), vec![50.0, 150.0, 100.0]);
        assert_eq!(s.shares(3, 600.0), vec![100.0, 300.0, 200.0]);
        // `divider-position` fixes the first of two panes
        s.sizes = None;
        s.divider_position = Some(80.0);
        assert_eq!(s.weights(2), vec![Share::Fixed(80.0), Share::Grow(1.0)]);
        assert_eq!(s.shares(2, 299.0), vec![80.0, 219.0]);
        let mut st = taffy::Style::default();
        s.child_layout(&ChildCx { index: 0, anchors: &["", ""] }, &mut st);
        assert_eq!((st.flex_basis, st.flex_grow, st.flex_shrink), (taffy::Dimension::length(80.0), 0.0, 0.0));
    }

    #[test]
    fn two_panes_follow_the_divider_and_min_size() {
        let mut s = SplitView::default();
        assert_eq!(s.resolve(399.0, &lim(2)), vec![200.0, 199.0], "halves, whole points");
        s.divider_position = Some(5.0);
        assert_eq!(s.resolve(399.0, &lim(2))[0], 32.0);
        s.divider_position = Some(100.0);
        assert_eq!(s.resolve(399.0, &lim(2)), vec![100.0, 299.0]);
    }

    #[test]
    fn extra_room_goes_by_flex_grow_and_bounds_hold() {
        let mut s = SplitView { sizes: Some(vec![100.0, 100.0, 100.0]), ..Default::default() };
        let mut l = lim(3);
        l[1].grow = 1.0;
        l[2].grow = 1.0;
        assert_eq!(s.resolve(400.0, &l), vec![100.0, 150.0, 150.0]);
        assert_eq!(s.resolve(400.0, &lim(3)), vec![100.0, 100.0, 200.0], "the last pane without stretch factors");
        let mut l = lim(3);
        l[2].max = 120.0;
        assert_eq!(s.resolve(400.0, &l), vec![100.0, 180.0, 120.0], "max holds; the rest moves left");
        s.sizes = None;
        let r = s.resolve(100.0, &lim(3));
        assert_eq!(r.iter().sum::<f32>(), 100.0);
        assert!(r.iter().all(|x| *x >= 32.0), "{r:?}");
    }

    #[test]
    fn a_drag_moves_the_two_panes_beside_the_sash() {
        let pol = *Native::for_os(Platform::Windows).policy();
        let mut s = SplitView::default();
        let rects = s.layout(Size::new(401.0, 100.0), &lim(2));
        assert_eq!(rects[0], Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(s.sash_rects(0).0, Rect::new(200.0, 0.0, 201.0, 100.0));
        let mut t = Probe::new(pol, Size::new(401.0, 100.0));
        assert_eq!(t.send(&mut s, ptr(PointerPhase::Down, 200.5)), Handled::Yes);
        // below the drag threshold (4 pt on Windows): nothing yet
        t.send(&mut s, ptr(PointerPhase::Move, 202.0));
        assert_eq!(t.count("drag-started"), 0);
        t.send(&mut s, ptr(PointerPhase::Move, 250.5));
        assert_eq!(t.count("drag-started"), 1);
        assert_eq!(t.capture, Some(true));
        assert_eq!(s.laid(), &[250.0, 150.0]);
        assert_eq!(t.last("changed"), Some(vec![Value::F64(250.0)]));
        assert!(t.changed.contains(&"sizes") && t.changed.contains(&"divider-position"), "native changes for bind:");
        t.send(&mut s, ptr(PointerPhase::Up, 250.5));
        assert_eq!(t.count("commit"), 1);
        assert_eq!(t.count("drag-stopped"), 1);
        assert_eq!(t.capture, Some(false));
        assert_eq!(s.sizes, Some(vec![250.0, 150.0]));
        assert_eq!(s.get_prop(P::sizes.id()), Some(Value::List(vec![Value::F64(250.0), Value::F64(150.0)])));
    }

    #[test]
    fn escape_cancels_and_keys_step_the_focused_sash() {
        let pol = *Native::for_os(Platform::Linux).policy();
        let mut s = SplitView::default();
        s.layout(Size::new(401.0, 100.0), &lim(2));
        let mut t = Probe::new(pol, Size::new(401.0, 100.0));
        t.send(&mut s, ptr(PointerPhase::Down, 200.5));
        t.send(&mut s, ptr(PointerPhase::Move, 300.5));
        assert_eq!(s.laid()[0], 300.0);
        // a JS write while the drag owns the sizes is refused
        assert_eq!(s.set_prop(P::divider_position.id(), &Value::F64(10.0)), Err(PropError::Stale));
        t.send(&mut s, key(Key::Escape));
        assert_eq!(s.laid()[0], 200.0, "back to the size before the drag");
        s.focus_sash(Some(0));
        t.send(&mut s, key(Key::ArrowRight));
        assert_eq!(s.laid(), &[210.0, 190.0]);
        t.send(&mut s, key(Key::Home));
        assert_eq!(s.laid()[0], 32.0);
        assert_eq!(s.set_prop(P::divider_position.id(), &Value::F64(10.0)), Ok(()));
        assert_eq!(s.sizes, None, "divider-position overrides sizes");
    }

    #[test]
    fn release_mode_moves_on_release() {
        let pol = *Native::for_os(Platform::Windows).policy();
        let mut s = SplitView::default();
        s.set_prop(P::commit_mode.id(), &Value::str("release")).unwrap();
        assert!(s.commit_on_release);
        s.layout(Size::new(401.0, 100.0), &lim(2));
        let mut t = Probe::new(pol, Size::new(401.0, 100.0));
        t.send(&mut s, ptr(PointerPhase::Down, 200.5));
        t.send(&mut s, ptr(PointerPhase::Move, 260.5));
        assert_eq!(s.laid()[0], 200.0, "panes stay during the drag");
        assert_eq!(s.ghost, Some((0, 260.0)));
        let (list, _) = crate::controls::testkit::record(&s, Size::new(401.0, 100.0), StateBits::NONE);
        // the sash line, its hover bar and three grip dots, then the guide
        assert_eq!(crate::controls::testkit::kinds(&list), ["quad", "quad", "quad", "quad", "quad", "quad"]);
        t.send(&mut s, ptr(PointerPhase::Up, 260.5));
        assert_eq!(s.laid()[0], 260.0);
        assert_eq!(s.ghost, None);
    }

    #[test]
    fn the_hover_bar_waits_for_the_policy_delay() {
        let pol = *Native::for_os(Platform::Windows).policy();
        let mut s = SplitView::default();
        s.layout(Size::new(401.0, 100.0), &lim(2));
        let mut t = Probe::new(pol, Size::new(401.0, 100.0));
        t.send(&mut s, Event::Pointer(PointerEvent { buttons: 0, button: None, ..match ptr(PointerPhase::Move, 200.5) { Event::Pointer(p) => p, _ => unreachable!() } }));
        assert!(!s.sashes[0].hover_shown(t.now));
        let wake = t.wake.expect("asks for the delay");
        assert_eq!(wake - t.now, secs(pol.scroll.sash_hover_delay));
        t.now = wake;
        assert!(s.sashes[0].hover_shown(t.now));
        // the pointer leaves the view: the bar goes
        t.send(&mut s, Event::Hover(false));
        assert!(!s.sashes[0].hover_shown(t.now));
    }

    #[test]
    fn collapse_and_restore() {
        let p = SashParams { min: 50.0, max: 400.0, collapsible: true, ..Default::default() };
        let mut r = Sash::default();
        r.set_size(120.0);
        assert_eq!(r.clamp(&p, 20.0), 0.0, "below half the minimum collapses");
        assert_eq!(r.clamp(&p, 30.0), 50.0);
        assert_eq!(r.clamp(&p, 900.0), 400.0);
        let mut out = Vec::new();
        r.toggle_collapse(&p, &mut out);
        assert_eq!(r.size(), 0.0);
        r.toggle_collapse(&p, &mut out);
        assert_eq!(r.size(), 120.0, "restored to the size before the collapse");
        assert_eq!(out.last(), Some(&SashIntent::Commit { size: 120.0 }));
    }
}

/// A test harness for one view kind: sends events at a virtual time with a policy, and keeps the
/// signals (with their arguments), native prop changes, capture and wake-up each event produced.
#[cfg(test)]
pub(crate) struct Probe {
    pub policy: Policy,
    pub size: Size,
    pub now: Time,
    pub state: StateBits,
    pub focused: bool,
    pub signals: Vec<(&'static str, Vec<Value>)>,
    pub changed: Vec<&'static str>,
    pub capture: Option<bool>,
    pub wake: Option<Time>,
    pub cursor: Option<CursorIcon>,
    pub repaint: bool,
    /// The scroll offsets the kind handed the host (`EventCx::scroll_to`), one per event that moved it.
    pub scroll_to: Vec<skelter_base::geom::Vec2>,
}

#[cfg(test)]
impl Probe {
    pub fn new(policy: Policy, size: Size) -> Probe {
        Probe { policy, size, now: Time(1_000_000_000), state: StateBits::NONE, focused: false, signals: Vec::new(), changed: Vec::new(), capture: None, wake: None, cursor: None, repaint: false, scroll_to: Vec::new() }
    }
    /// Sends `ev` to `k`; capture, wake, cursor and repaint are this event's.
    pub fn send<K: Kind>(&mut self, k: &mut K, ev: Event) -> Handled {
        let spec = k.kind_spec();
        let policy = self.policy;
        let mut cx = EventCx::new(self.now, self.size, &mut self.state, &policy);
        cx.focused = self.focused;
        let h = k.event(&mut cx, &ev);
        let (emitted, changed) = (std::mem::take(&mut cx.emitted), std::mem::take(&mut cx.changed));
        let (capture, wake, cursor, repaint) = (cx.capture, cx.wake, cx.cursor, cx.repaint);
        self.scroll_to.extend(cx.scroll_to);
        self.signals.extend(emitted.into_iter().map(|e| (spec.signals[e.signal.0 as usize].name, e.args)));
        self.changed.extend(changed.into_iter().map(|p| spec.props[p.0 as usize].name));
        if capture.is_some() {
            self.capture = capture;
        }
        (self.wake, self.cursor, self.repaint) = (wake, cursor, repaint);
        h
    }
    pub fn advance(&mut self, ms: i64) {
        self.now = self.now + Span::from_millis(ms);
    }
    /// How many times `name` was emitted.
    pub fn count(&self, name: &str) -> usize {
        self.signals.iter().filter(|(n, _)| *n == name).count()
    }
    /// The arguments of the last `name`.
    pub fn last(&self, name: &str) -> Option<Vec<Value>> {
        self.signals.iter().rev().find(|(n, _)| *n == name).map(|(_, a)| a.clone())
    }
}
