//! Toasts: `toast-host` and the Rust API ([`ToastHost::show`], [`ToastQueue`] from any thread).
//!
//! A `toast-host` is a full-window, click-through overlay entry that shows its toasts stacked from a
//! corner, newest nearest the corner (svelters-egui `views/toast.rs`). It computes:
//! - the queue: at most `max-visible` show at once; the rest wait, their clocks not started;
//! - timeouts: a toast closes `duration` after it showed (the host's when it has none; zero: until
//!   closed); hovering it pauses its clock, leaving resumes it with the time it had left;
//! - closing: by its close button, an action button, `dismiss(id)`, `clear()` or the timeout; each
//!   close emits `closed` (`id`, `reason`: `timeout`, `close`, `action`, `method`);
//! - the stacking rects and their parts (close button, action buttons) for hit testing and recording.

use super::draw;
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, Labels, Palette, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::event::{MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size};
use presto_base::time::{Span, Time};
use std::sync::{Arc, Mutex};

/// A toast's severity (its accent colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Success => "success",
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }
    pub fn of(name: &str) -> Option<Level> {
        Some(match name {
            "info" => Level::Info,
            "success" => Level::Success,
            "warning" => Level::Warning,
            "error" => Level::Error,
            _ => return None,
        })
    }
    /// The accent bar's colour (the theme's `info`/`success`/`warning`/`error` tokens once they exist).
    fn accent(self) -> Color {
        match self {
            Level::Info => Color::from_rgba8(0x37, 0x94, 0xff, 0xff),
            Level::Success => Color::from_rgba8(0x3f, 0xb9, 0x50, 0xff),
            Level::Warning => Color::from_rgba8(0xd2, 0x99, 0x22, 0xff),
            Level::Error => Color::from_rgba8(0xf1, 0x4c, 0x4c, 0xff),
        }
    }
    /// Warnings and errors interrupt the screen reader; the rest wait.
    fn live(self) -> accesskit::Live {
        match self {
            Level::Warning | Level::Error => accesskit::Live::Assertive,
            _ => accesskit::Live::Polite,
        }
    }
}

/// The corner toasts stack from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Corner {
    #[default]
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

pub const CORNERS: &[&str] = &["bottom-right", "bottom-left", "top-right", "top-left"];

impl Corner {
    pub fn of(name: &str) -> Option<Corner> {
        Some(match name {
            "bottom-right" => Corner::BottomRight,
            "bottom-left" => Corner::BottomLeft,
            "top-right" => Corner::TopRight,
            "top-left" => Corner::TopLeft,
            _ => return None,
        })
    }
    fn bottom(self) -> bool {
        matches!(self, Corner::BottomRight | Corner::BottomLeft)
    }
    fn right(self) -> bool {
        matches!(self, Corner::BottomRight | Corner::TopRight)
    }
}

/// Why a toast closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Timeout,
    /// Its close button.
    Close,
    /// One of its action buttons.
    Action,
    /// `dismiss`/`clear`.
    Method,
}

impl CloseReason {
    pub fn name(self) -> &'static str {
        match self {
            CloseReason::Timeout => "timeout",
            CloseReason::Close => "close",
            CloseReason::Action => "action",
            CloseReason::Method => "method",
        }
    }
}

/// One toast.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: i64,
    pub text: String,
    pub level: Level,
    /// Zero: until closed.
    pub duration: Span,
    pub actions: Vec<String>,
    /// Time left on its clock while paused or not yet shown.
    remaining: Span,
    /// When its clock last (re)started; `None`: not shown yet or paused.
    running_since: Option<Time>,
    shown: bool,
    hovered: bool,
}

impl Toast {
    pub fn shown(&self) -> bool {
        self.shown
    }
    /// When it times out, if its clock runs.
    pub fn deadline(&self) -> Option<Time> {
        if self.duration == Span::ZERO {
            return None;
        }
        self.running_since.map(|t| t + self.remaining)
    }
}

/// A part of a laid-out toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Body,
    Close,
    Action(usize),
}

/// A toast's place on screen (host-local points) and its parts.
#[derive(Clone, Debug, PartialEq)]
pub struct Laid {
    pub id: i64,
    pub rect: Rect,
    pub close: Rect,
    pub actions: Vec<Rect>,
    text_origin: Point,
    text_width: f32,
}

/// A toast queued from Rust, possibly from another thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub text: String,
    pub level: Level,
    pub duration: Option<Span>,
    pub actions: Vec<String>,
}

/// A handle for showing toasts from anywhere: the host drains it on its next [`ToastHost::tick`].
#[derive(Clone, Debug, Default)]
pub struct ToastQueue(Arc<Mutex<Vec<Queued>>>);

impl ToastQueue {
    pub fn show(&self, text: impl Into<String>, level: Level, duration: Option<Span>) {
        self.push(Queued { text: text.into(), level, duration, actions: Vec::new() });
    }
    pub fn push(&self, q: Queued) {
        if let Ok(mut v) = self.0.lock() {
            v.push(q);
        }
    }
    pub fn len(&self) -> usize {
        self.0.lock().map_or(0, |v| v.len())
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn drain(&self) -> Vec<Queued> {
        self.0.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
    }
}

static SPEC: KindSpec = KindSpec {
    name: "toast-host",
    props: &[
        prop("corner", PropType::Enum(CORNERS), ChangeClass::Layout),
        prop("duration", PropType::F64, ChangeClass::Property),
        prop("max-visible", PropType::I64, ChangeClass::Structure),
    ],
    signals: &[signal("closed", true, false), signal("action", true, false)],
    slots: &[""],
    role: accesskit::Role::Group,
};

const MARGIN: f32 = 12.0;
const GAP: f32 = 6.0;
const MAX_WIDTH: f32 = 360.0;
const PAD: f32 = 10.0;
const ACCENT_W: f32 = 4.0;
const CLOSE: f32 = 16.0;
const ACTION_PAD: f32 = 8.0;

/// A `toast-host` node.
#[derive(Debug)]
pub struct ToastHost {
    pub corner: Corner,
    /// The default duration.
    pub duration: Span,
    pub max_visible: usize,
    /// In show order.
    list: Vec<Toast>,
    next: i64,
    queue: ToastQueue,
    laid: Vec<Laid>,
    /// The part the primary button went down on (a click needs release on the same part).
    down: Option<(i64, Part)>,
}

impl Default for ToastHost {
    fn default() -> Self {
        ToastHost {
            corner: Corner::default(),
            duration: Span::from_millis(4000),
            max_visible: 5,
            list: Vec::new(),
            next: 0,
            queue: ToastQueue::default(),
            laid: Vec::new(),
            down: None,
        }
    }
}

fn closed(cx: &mut EventCx<'_>, id: i64, reason: CloseReason) {
    cx.emit("closed", Value::Record(vec![("id".into(), Value::I64(id)), ("reason".into(), Value::Str(reason.name().into()))]));
    cx.mark(ChangeClass::Structure);
}

impl ToastHost {
    /// A handle that shows toasts on this host from any thread.
    pub fn queue(&self) -> ToastQueue {
        self.queue.clone()
    }
    /// Shown and waiting toasts, in show order.
    pub fn toasts(&self) -> &[Toast] {
        &self.list
    }
    pub fn laid(&self) -> &[Laid] {
        &self.laid
    }

    /// Adds a toast (`duration` `None`: the host's); returns its id. It shows on the next
    /// [`ToastHost::tick`] when a slot is free.
    pub fn show(&mut self, text: impl Into<String>, level: Level, duration: Option<Span>, actions: Vec<String>) -> i64 {
        self.next += 1;
        let duration = duration.unwrap_or(self.duration).max(Span::ZERO);
        self.list.push(Toast {
            id: self.next,
            text: text.into(),
            level,
            duration,
            actions,
            remaining: duration,
            running_since: None,
            shown: false,
            hovered: false,
        });
        self.next
    }

    /// Closes one toast; false when there is none with that id.
    pub fn dismiss(&mut self, cx: &mut EventCx<'_>, id: i64) -> bool {
        self.close(cx, id, CloseReason::Method)
    }

    pub fn clear(&mut self, cx: &mut EventCx<'_>) {
        for t in std::mem::take(&mut self.list) {
            closed(cx, t.id, CloseReason::Method);
        }
        self.laid.clear();
    }

    fn close(&mut self, cx: &mut EventCx<'_>, id: i64, reason: CloseReason) -> bool {
        let Some(i) = self.list.iter().position(|t| t.id == id) else { return false };
        self.list.remove(i);
        self.laid.retain(|l| l.id != id);
        closed(cx, id, reason);
        true
    }

    /// Takes queued toasts, times out expired ones, shows waiting ones into free slots and asks for
    /// a wake-up at the next deadline.
    pub fn tick(&mut self, cx: &mut EventCx<'_>) {
        let now = cx.now;
        for q in self.queue.drain() {
            self.show(q.text, q.level, q.duration, q.actions);
        }
        let expired: Vec<i64> = self.list.iter().filter(|t| t.deadline().is_some_and(|d| now >= d)).map(|t| t.id).collect();
        for id in expired {
            self.close(cx, id, CloseReason::Timeout);
        }
        let mut visible = self.list.iter().filter(|t| t.shown).count();
        for t in self.list.iter_mut().filter(|t| !t.shown) {
            if visible >= self.max_visible {
                break;
            }
            t.shown = true;
            t.running_since = if t.hovered { None } else { Some(now) };
            visible += 1;
            cx.mark(ChangeClass::Structure);
        }
        if let Some(d) = self.list.iter().filter_map(Toast::deadline).min() {
            cx.wake_at(d);
        }
    }

    /// The pointer is over toast `id` (or none): pauses its clock and resumes the one it left.
    pub fn hover(&mut self, now: Time, id: Option<i64>) {
        for t in &mut self.list {
            let over = Some(t.id) == id;
            if over == t.hovered {
                continue;
            }
            t.hovered = over;
            if over {
                if let Some(since) = t.running_since.take() {
                    t.remaining = (t.remaining - (now - since)).max(Span::ZERO);
                }
            } else if t.shown {
                t.running_since = Some(now);
            }
        }
    }

    /// Lays the shown toasts out in a host of `size`, stacked from the corner.
    pub fn layout(&mut self, size: Size, labels: &mut dyn Labels, palette: &Palette) {
        self.laid.clear();
        let max_w = MAX_WIDTH.min(size.w - 2.0 * MARGIN).max(120.0);
        let mut edge = if self.corner.bottom() { size.h - MARGIN } else { MARGIN };
        // newest nearest the corner
        for t in self.list.iter().filter(|t| t.shown).rev() {
            let lead = PAD + ACCENT_W + PAD;
            let trail = PAD + CLOSE + PAD;
            let text_w = (max_w - lead - trail).max(40.0);
            let ts = labels.measure(&t.text, palette.font_size, Some(text_w));
            let action_sizes: Vec<Size> = t.actions.iter().map(|a| labels.measure(a, palette.font_size, None)).collect();
            let actions_h = action_sizes.iter().map(|s| s.h + 2.0 * 4.0).fold(0.0, f32::max);
            let actions_w: f32 = action_sizes.iter().map(|s| s.w + 2.0 * ACTION_PAD + GAP).sum();
            let w = (lead + ts.w.max(actions_w) + trail).min(max_w);
            let h = PAD + ts.h.max(CLOSE) + if actions_h > 0.0 { GAP + actions_h } else { 0.0 } + PAD;
            let x = if self.corner.right() { size.w - MARGIN - w } else { MARGIN };
            let y = if self.corner.bottom() { edge - h } else { edge };
            let rect = Rect::from_origin_size(Point::new(x, y), Size::new(w, h));
            edge = if self.corner.bottom() { y - GAP } else { y + h + GAP };
            let close = Rect::from_origin_size(Point::new(rect.max.x - PAD - CLOSE, y + PAD), Size::new(CLOSE, CLOSE));
            let mut ax = x + lead;
            let ay = y + PAD + ts.h.max(CLOSE) + GAP;
            let actions = action_sizes
                .iter()
                .map(|s| {
                    let r = Rect::from_origin_size(Point::new(ax, ay), Size::new(s.w + 2.0 * ACTION_PAD, actions_h));
                    ax = r.max.x + GAP;
                    r
                })
                .collect();
            self.laid.push(Laid { id: t.id, rect, close, actions, text_origin: Point::new(x + lead, y + PAD), text_width: text_w });
        }
    }

    /// The toast and part at `p` (host-local).
    pub fn hit(&self, p: Point) -> Option<(i64, Part)> {
        let l = self.laid.iter().find(|l| l.rect.contains(p))?;
        if l.close.contains(p) {
            return Some((l.id, Part::Close));
        }
        if let Some(i) = l.actions.iter().position(|r| r.contains(p)) {
            return Some((l.id, Part::Action(i)));
        }
        Some((l.id, Part::Body))
    }
}

impl Kind for ToastHost {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "corner" => self.corner = v.as_str().and_then(Corner::of).ok_or(PropError::Invalid("unknown corner"))?,
            "duration" => self.duration = Span::from_secs_f64(v.as_f64().ok_or(PropError::Invalid("expected seconds"))?.max(0.0)),
            "max-visible" => self.max_visible = v.as_i64().filter(|n| *n >= 1).ok_or(PropError::Invalid("expected a count of at least 1"))? as usize,
            _ => return Err(PropError::Unknown),
        }
        Ok(SPEC.prop(name).map_or(ChangeClass::Paint, |p| p.change))
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let p = cx.palette;
        for l in &self.laid {
            let Some(t) = self.list.iter().find(|t| t.id == l.id) else { continue };
            draw::shadow(cx.list, l.rect, p.radius, p.shadow, 8.0, presto_base::geom::Vec2::new(0.0, 2.0));
            draw::boxed(cx.list, l.rect, p.radius, p.surface_raised, 1.0, p.border);
            let bar = Rect::new(l.rect.min.x + PAD, l.rect.min.y + PAD, l.rect.min.x + PAD + ACCENT_W, l.rect.max.y - PAD);
            draw::fill(cx.list, bar, ACCENT_W / 2.0, t.level.accent());
            cx.labels.record(cx.list, &t.text, l.text_origin, p.font_size, p.text, Some(l.text_width));
            // the close glyph: an ×
            let c = l.close.inflate(-4.0);
            draw::line(cx.list, c.min, c.max, 1.5, p.text_muted);
            draw::line(cx.list, Point::new(c.max.x, c.min.y), Point::new(c.min.x, c.max.y), 1.5, p.text_muted);
            for (r, a) in l.actions.iter().zip(&t.actions) {
                draw::boxed(cx.list, *r, p.radius, p.surface, 1.0, p.border);
                cx.labels.record(cx.list, a, Point::new(r.min.x + ACTION_PAD, r.min.y + 4.0), p.font_size, p.accent, None);
            }
        }
    }

    /// Hover pauses a toast; a click on its close or action button closes it. Presses elsewhere fall
    /// through (the host is click-through outside its toasts).
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        match ev {
            Event::Pointer(pe) => {
                let at = self.hit(pe.pos);
                match pe.phase {
                    PointerPhase::Move | PointerPhase::Enter => self.hover(cx.now, at.map(|a| a.0)),
                    PointerPhase::Leave | PointerPhase::Cancel => {
                        self.hover(cx.now, None);
                        self.down = None;
                    }
                    PointerPhase::Down if pe.button == Some(MouseButton::Primary) => self.down = at,
                    PointerPhase::Up if pe.button == Some(MouseButton::Primary) => {
                        let down = self.down.take();
                        if let Some((id, part)) = at
                            && down == at
                        {
                            match part {
                                Part::Close => {
                                    self.close(cx, id, CloseReason::Close);
                                }
                                Part::Action(i) => {
                                    cx.emit("action", Value::Record(vec![("id".into(), Value::I64(id)), ("index".into(), Value::I64(i as i64))]));
                                    self.close(cx, id, CloseReason::Action);
                                }
                                Part::Body => {}
                            }
                        }
                    }
                    _ => {}
                }
                if at.is_some() { Handled::Yes } else { Handled::No }
            }
            Event::Timer => {
                self.tick(cx);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Group);
        let live = self.list.iter().filter(|t| t.shown).map(|t| t.level.live()).max_by_key(|l| matches!(l, accesskit::Live::Assertive));
        node.set_live(live.unwrap_or(accesskit::Live::Polite));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::shim::{FixedLabels, Os, Policy};
    use presto_base::event::{Modifiers, PointerEvent, PointerType};
    use presto_base::geom::Vec2;

    fn ms(n: i64) -> Time {
        Time(n as u64 * 1_000_000)
    }

    fn ptr(phase: PointerPhase, pos: Point) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: 0,
            pos,
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    fn reasons(cx: &EventCx<'_>) -> Vec<(i64, String)> {
        cx.emitted("closed")
            .map(|e| (e.detail.field("id").and_then(Value::as_i64).unwrap(), e.detail.field("reason").and_then(Value::as_str).unwrap().to_string()))
            .collect()
    }

    #[test]
    fn times_out_and_pauses_while_hovered() {
        let pol = Policy::for_os(Os::Linux);
        let mut h = ToastHost::default();
        let id = h.show("Saved", Level::Success, Some(Span::from_millis(1000)), Vec::new());
        let mut cx = EventCx::new(ms(0), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert!(h.toasts()[0].shown());
        assert_eq!(cx.wake, Some(ms(1000)));
        // hovered at 400 ms: 600 ms left, frozen
        h.hover(ms(400), Some(id));
        let mut cx = EventCx::new(ms(5000), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert_eq!(h.toasts().len(), 1, "paused while hovered");
        assert_eq!(cx.wake, None);
        // left at 5000 ms: times out at 5600
        h.hover(ms(5000), None);
        let mut cx = EventCx::new(ms(5599), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert_eq!((h.toasts().len(), cx.wake), (1, Some(ms(5600))));
        let mut cx = EventCx::new(ms(5600), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert!(h.toasts().is_empty());
        assert_eq!(reasons(&cx), vec![(id, "timeout".into())]);
    }

    #[test]
    fn max_visible_queues_the_rest_and_zero_duration_stays() {
        let pol = Policy::for_os(Os::Linux);
        let mut h = ToastHost { max_visible: 2, ..ToastHost::default() };
        let a = h.show("a", Level::Info, Some(Span::from_millis(100)), Vec::new());
        h.show("b", Level::Info, Some(Span::ZERO), Vec::new());
        let c = h.show("c", Level::Info, Some(Span::from_millis(100)), Vec::new());
        let mut cx = EventCx::new(ms(0), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert_eq!(h.toasts().iter().map(|t| t.shown()).collect::<Vec<_>>(), vec![true, true, false]);
        let mut cx = EventCx::new(ms(100), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert_eq!(reasons(&cx), vec![(a, "timeout".into())]);
        // c takes the free slot; its clock starts now
        let t = h.toasts().iter().find(|t| t.id == c).unwrap();
        assert_eq!((t.shown(), t.deadline()), (true, Some(ms(200))));
        assert_eq!(h.toasts().iter().find(|t| t.id != c).unwrap().deadline(), None, "zero: until closed");
    }

    #[test]
    fn queue_from_rust_dismiss_and_clear() {
        let pol = Policy::for_os(Os::Linux);
        let mut h = ToastHost::default();
        let q = h.queue();
        q.show("from a job", Level::Warning, None);
        assert_eq!(q.len(), 1);
        let mut cx = EventCx::new(ms(0), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        assert!(q.is_empty());
        assert_eq!(h.toasts()[0].duration, Span::from_millis(4000), "the host's duration");
        let id = h.toasts()[0].id;
        assert!(h.dismiss(&mut cx, id));
        assert!(!h.dismiss(&mut cx, id));
        h.show("x", Level::Info, None, Vec::new());
        h.show("y", Level::Info, None, Vec::new());
        h.clear(&mut cx);
        assert_eq!(reasons(&cx).iter().filter(|r| r.1 == "method").count(), 3);
    }

    #[test]
    fn stacks_from_the_corner_and_closes_by_its_buttons() {
        let pol = Policy::for_os(Os::Linux);
        let mut h = ToastHost::default();
        let a = h.show("first", Level::Info, None, Vec::new());
        let b = h.show("second", Level::Error, None, vec!["Retry".into()]);
        let mut cx = EventCx::new(ms(0), Size::new(800.0, 600.0), &pol);
        h.tick(&mut cx);
        h.layout(Size::new(800.0, 600.0), &mut FixedLabels, &Palette::default());
        let (la, lb) = (h.laid().iter().find(|l| l.id == a).unwrap().clone(), h.laid().iter().find(|l| l.id == b).unwrap().clone());
        assert_eq!(lb.rect.max.y, 600.0 - MARGIN, "newest nearest the corner");
        assert_eq!(lb.rect.max.x, 800.0 - MARGIN);
        assert_eq!(la.rect.max.y, lb.rect.min.y - GAP);
        // a click on the action emits it and closes the toast
        let at = lb.actions[0].min + Vec2::new(2.0, 2.0);
        let mut cx = EventCx::new(ms(10), Size::new(800.0, 600.0), &pol);
        assert_eq!(h.event(&mut cx, &ptr(PointerPhase::Down, at)), Handled::Yes);
        h.event(&mut cx, &ptr(PointerPhase::Up, at));
        assert_eq!(cx.emitted("action").count(), 1);
        assert_eq!(reasons(&cx), vec![(b, "action".into())]);
        // the close button
        let at = la.close.min + Vec2::new(2.0, 2.0);
        let mut cx = EventCx::new(ms(20), Size::new(800.0, 600.0), &pol);
        h.event(&mut cx, &ptr(PointerPhase::Down, at));
        h.event(&mut cx, &ptr(PointerPhase::Up, at));
        assert_eq!(reasons(&cx), vec![(a, "close".into())]);
        // outside any toast: click-through
        let mut cx = EventCx::new(ms(30), Size::new(800.0, 600.0), &pol);
        assert_eq!(h.event(&mut cx, &ptr(PointerPhase::Down, Point::new(5.0, 5.0))), Handled::No);
    }
}
