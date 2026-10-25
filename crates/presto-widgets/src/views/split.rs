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

use super::draw;
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, Policy, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::cursor::CursorIcon;
use presto_base::event::{KeyState, Modifiers, MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size};
use presto_base::key::Key;
use presto_base::time::{Span, Time};

/// R11: hovering a sash shows its hover bar and grip after this long (VS Code 300 ms).
pub const SASH_HOVER_DELAY: Span = Span::from_millis(300);

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
        self.start.is_some() || self.focused || self.hover_since.is_some_and(|t| now - t >= SASH_HOVER_DELAY)
    }
    /// When the hover bar appears without more input.
    pub fn wake(&self) -> Option<Time> {
        self.hover_since.filter(|_| self.start.is_none()).map(|t| t + SASH_HOVER_DELAY)
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
                    let double = self.last_click.is_some_and(|t| now - t <= policy.double_click);
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

/// Records a sash: the line, then (shown) the hover bar and its three-dot grip.
pub(crate) fn record_sash(cx: &mut RecordCx<'_>, line: Rect, axis: usize, shown: bool) {
    let pal = cx.palette;
    draw::fill(cx.list, line, 0.0, pal.border);
    if !shown {
        return;
    }
    let (w, dot, gap) = (4.0, 2.0, 2.0);
    let c = Point::new((line.min.x + line.max.x) / 2.0, (line.min.y + line.max.y) / 2.0);
    let bar = if axis == 0 { Rect::new(c.x - w / 2.0, line.min.y, c.x + w / 2.0, line.max.y) } else { Rect::new(line.min.x, c.y - w / 2.0, line.max.x, c.y + w / 2.0) };
    draw::fill(cx.list, bar, 0.0, pal.accent);
    for i in -1..=1 {
        let off = i as f32 * (dot + gap);
        let p = if axis == 0 { Point::new(c.x, c.y + off) } else { Point::new(c.x + off, c.y) };
        draw::fill(cx.list, Rect::new(p.x - dot / 2.0, p.y - dot / 2.0, p.x + dot / 2.0, p.y + dot / 2.0), dot / 2.0, pal.on_accent);
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

/// Panes split by draggable sashes (`QSplitter`).
#[derive(Clone, Debug)]
pub struct SplitView {
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
}

impl Default for SplitView {
    fn default() -> Self {
        SplitView {
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
        }
    }
}

static SPLIT_VIEW_SPEC: KindSpec = KindSpec {
    name: "split-view",
    props: &[
        prop("orientation", PropType::Enum(&["horizontal", "vertical"]), ChangeClass::Layout),
        prop("divider-position", PropType::F64, ChangeClass::Layout),
        prop("min-size", PropType::F64, ChangeClass::Layout),
        prop("resizable", PropType::Bool, ChangeClass::Paint),
        prop("sizes", PropType::List, ChangeClass::Layout),
        prop("collapsible", PropType::Bool, ChangeClass::Layout),
        prop("restore-on-double-click", PropType::Bool, ChangeClass::Property),
        prop("commit-mode", PropType::Enum(&["live", "release"]), ChangeClass::Property),
    ],
    signals: &[signal("changed", true, false), signal("drag-started", true, false), signal("drag-stopped", true, false), signal("commit", true, false)],
    slots: &[""],
    role: accesskit::Role::Group,
};

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

    /// Writes new pane sizes (the user's): `sizes`, `divider-position`, `changed`.
    fn write(&mut self, cx: &mut EventCx<'_>, sizes: Vec<f32>) {
        let v: Vec<f64> = sizes.iter().map(|x| *x as f64).collect();
        let first = v.first().copied();
        self.sizes = Some(v);
        if let Some(f) = first
            && Some(f) != self.divider_position
        {
            self.divider_position = Some(f);
            cx.emit("changed", Value::F64(f));
        }
        self.laid = sizes;
        cx.mark(ChangeClass::Layout);
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
                    cx.emit("drag-started", Value::Null);
                }
                SashIntent::Resize { size } => {
                    if self.commit_on_release && self.sashes[k].dragging() {
                        self.ghost = Some((k, size));
                        cx.mark(ChangeClass::Paint);
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
                    cx.emit("commit", Value::List(s.iter().map(|x| Value::F64(*x as f64)).collect()));
                    if was_drag {
                        cx.capture = Some(false);
                        cx.emit("drag-stopped", Value::Null);
                    }
                }
                SashIntent::Cancel { size } => {
                    if self.ghost.take().is_none() {
                        let s = with(&self.laid, size);
                        self.write(cx, s);
                    } else {
                        cx.mark(ChangeClass::Paint);
                    }
                    self.drag = None;
                    cx.capture = Some(false);
                    cx.emit("drag-stopped", Value::Null);
                }
            }
        }
    }
}

impl Kind for SplitView {
    fn spec() -> &'static KindSpec {
        &SPLIT_VIEW_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "orientation" => {
                self.vertical = match value.as_str() {
                    Some("vertical") => true,
                    Some("horizontal") | None => false,
                    Some(_) => return Err(PropError::Invalid("orientation: horizontal or vertical")),
                };
                Ok(ChangeClass::Layout)
            }
            "divider-position" => {
                // a host write of `divider-position` overrides `sizes`
                if self.sashes.iter().any(Sash::dragging) {
                    return Err(PropError::Stale);
                }
                self.divider_position = match value {
                    Value::Null => None,
                    v => Some(v.as_f64().ok_or(PropError::Invalid("a number of points or null"))?),
                };
                self.sizes = None;
                Ok(ChangeClass::Layout)
            }
            "sizes" => {
                if self.sashes.iter().any(Sash::dragging) {
                    return Err(PropError::Stale);
                }
                self.sizes = match value {
                    Value::Null => None,
                    Value::List(l) => Some(l.iter().map(|v| v.as_f64().ok_or(PropError::Invalid("a list of numbers"))).collect::<Result<_, _>>()?),
                    _ => return Err(PropError::Invalid("a list of numbers or null")),
                };
                Ok(ChangeClass::Layout)
            }
            "min-size" => {
                self.min_size = value.as_f64().ok_or(PropError::Invalid("a number"))?;
                Ok(ChangeClass::Layout)
            }
            "resizable" => {
                self.resizable = value.as_bool().ok_or(PropError::Invalid("a boolean"))?;
                Ok(ChangeClass::Paint)
            }
            "collapsible" => {
                self.collapsible = value.as_bool().ok_or(PropError::Invalid("a boolean"))?;
                Ok(ChangeClass::Layout)
            }
            "restore-on-double-click" => {
                self.restore_on_double_click = value.as_bool().ok_or(PropError::Invalid("a boolean"))?;
                Ok(ChangeClass::Property)
            }
            "commit-mode" => {
                self.commit_on_release = match value.as_str() {
                    Some("release") => true,
                    Some("live") | None => false,
                    Some(_) => return Err(PropError::Invalid("commit-mode: live or release")),
                };
                Ok(ChangeClass::Property)
            }
            _ => Err(PropError::Unknown),
        }
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let ax = self.axis();
        for k in 0..self.sashes.len() {
            let (line, _) = self.sash_rects(k);
            record_sash(cx, line, ax, self.resizable && self.sashes[k].hover_shown(self.now));
        }
        if let Some((k, size)) = self.ghost {
            let at = self.laid[..k].iter().sum::<f32>() + self.line * k as f32 + size;
            let g = if ax == 0 { Rect::new(at - 1.0, 0.0, at + 1.0, self.size.h) } else { Rect::new(0.0, at - 1.0, self.size.w, at + 1.0) };
            draw::fill(cx.list, g, 0.0, cx.palette.accent);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.now = cx.now;
        if !self.resizable || self.sashes.is_empty() {
            return Handled::No;
        }
        match ev {
            Event::Pointer(p) => {
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                let under = (0..self.sashes.len()).find(|k| self.sash_rects(*k).1.contains(p.pos));
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
                        cx.mark(ChangeClass::Paint);
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
            Event::Timer => {
                cx.mark(ChangeClass::Paint);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Group);
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
    use presto_base::event::{KeyEvent, KeyLocation, PointerEvent, PointerType};
    use presto_base::geom::Vec2;
    use super::super::shim::Os;

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
        let pol = Policy::for_os(Os::Windows);
        let mut s = SplitView::default();
        let rects = s.layout(Size::new(401.0, 100.0), &lim(2));
        assert_eq!(rects[0], Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(s.sash_rects(0).0, Rect::new(200.0, 0.0, 201.0, 100.0));
        let mut cx = EventCx::new(Time(0), Size::new(401.0, 100.0), &pol);
        assert_eq!(s.event(&mut cx, &ptr(PointerPhase::Down, 200.5)), Handled::Yes);
        // below the drag threshold (4 pt on Windows): nothing yet
        s.event(&mut cx, &ptr(PointerPhase::Move, 202.0));
        assert_eq!(cx.emitted("drag-started").count(), 0);
        s.event(&mut cx, &ptr(PointerPhase::Move, 250.5));
        assert_eq!(cx.emitted("drag-started").count(), 1);
        assert_eq!(s.laid(), &[250.0, 150.0]);
        assert_eq!(cx.emitted("changed").last().map(|e| e.detail.clone()), Some(Value::F64(250.0)));
        s.event(&mut cx, &ptr(PointerPhase::Up, 250.5));
        assert_eq!(cx.emitted("commit").count(), 1);
        assert_eq!(cx.emitted("drag-stopped").count(), 1);
        assert_eq!(cx.capture, Some(false));
        assert_eq!(s.sizes, Some(vec![250.0, 150.0]));
    }

    #[test]
    fn escape_cancels_and_keys_step_the_focused_sash() {
        let pol = Policy::for_os(Os::Linux);
        let mut s = SplitView::default();
        s.layout(Size::new(401.0, 100.0), &lim(2));
        let mut cx = EventCx::new(Time(0), Size::new(401.0, 100.0), &pol);
        s.event(&mut cx, &ptr(PointerPhase::Down, 200.5));
        s.event(&mut cx, &ptr(PointerPhase::Move, 300.5));
        assert_eq!(s.laid()[0], 300.0);
        s.event(&mut cx, &key(Key::Escape));
        assert_eq!(s.laid()[0], 200.0, "back to the size before the drag");
        s.focus_sash(Some(0));
        s.event(&mut cx, &key(Key::ArrowRight));
        assert_eq!(s.laid(), &[210.0, 190.0]);
        s.event(&mut cx, &key(Key::Home));
        assert_eq!(s.laid()[0], 32.0);
    }

    #[test]
    fn release_mode_moves_on_release() {
        let pol = Policy::for_os(Os::Windows);
        let mut s = SplitView { commit_on_release: true, ..Default::default() };
        s.layout(Size::new(401.0, 100.0), &lim(2));
        let mut cx = EventCx::new(Time(0), Size::new(401.0, 100.0), &pol);
        s.event(&mut cx, &ptr(PointerPhase::Down, 200.5));
        s.event(&mut cx, &ptr(PointerPhase::Move, 260.5));
        assert_eq!(s.laid()[0], 200.0, "panes stay during the drag");
        assert_eq!(s.ghost, Some((0, 260.0)));
        s.event(&mut cx, &ptr(PointerPhase::Up, 260.5));
        assert_eq!(s.laid()[0], 260.0);
        assert_eq!(s.ghost, None);
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
