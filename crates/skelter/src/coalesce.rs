//! Continuous input coalescing (architecture §11.1): pointer moves and wheel deltas of one window are
//! merged until the next UI frame (or the next discrete event, which flushes them first so order is
//! kept). A merged pointer move keeps every earlier sample in its history; merged wheel deltas and raw
//! motion deltas add up.

use skelter_base::event::{
    GesturePhase, InputEnvelope, InputEvent, PointerEvent, PointerPhase, PointerSample,
    ScrollDelta, WheelEvent,
};
use skelter_base::geom::Vec2;
use skelter_base::ids::{InputSeq, WindowId};
use std::collections::HashMap;

/// What [`Coalescer::push`] did with an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pushed {
    /// Held for the next flush.
    Held,
    /// Held; the pending event with this seq merged into it (that seq terminates as `MergedInto`).
    Merged { old: InputSeq },
}

/// Pending continuous events per window, in arrival order.
#[derive(Default)]
pub struct Coalescer {
    pending: HashMap<WindowId, Vec<InputEnvelope>>,
}

impl Coalescer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold a continuous event. It merges with the newest pending event of its window when the two are
    /// the same stream: the same pointer with the same buttons and modifiers, wheel deltas of the
    /// same unit and gesture state, or raw motion deltas.
    pub fn push(&mut self, ev: InputEnvelope) -> Pushed {
        let q = self.pending.entry(ev.window).or_default();
        if let Some(last) = q.last_mut()
            && let Some(merged) = merge(last, &ev)
        {
            let old = last.seq;
            *last = merged;
            return Pushed::Merged { old };
        }
        q.push(ev);
        Pushed::Held
    }

    /// Take a window's pending events, oldest first.
    pub fn take(&mut self, window: WindowId) -> Vec<InputEnvelope> {
        self.pending.remove(&window).unwrap_or_default()
    }

    pub fn has_pending(&self, window: WindowId) -> bool {
        self.pending.get(&window).is_some_and(|q| !q.is_empty())
    }

    pub fn windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.pending
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(w, _)| *w)
    }
}

/// `new` merged over `old`, or `None` when they are different streams. The result carries `new`'s seq
/// and timestamps (it is the newest sample); `old`'s record terminates as merged into it, as Chrome's
/// coalesced events do.
fn merge(old: &InputEnvelope, new: &InputEnvelope) -> Option<InputEnvelope> {
    let event = match (&old.event, &new.event) {
        (InputEvent::Pointer(a), InputEvent::Pointer(b)) => {
            InputEvent::Pointer(merge_pointer(a, b, old)?)
        }
        (InputEvent::Wheel(a), InputEvent::Wheel(b)) => InputEvent::Wheel(merge_wheel(a, b)?),
        (InputEvent::RawMotion { dx: ax, dy: ay }, InputEvent::RawMotion { dx: bx, dy: by }) => {
            InputEvent::RawMotion { dx: ax + bx, dy: ay + by }
        }
        _ => return None,
    };
    Some(InputEnvelope {
        seq: new.seq,
        window: new.window,
        os_time: new.os_time,
        recv_time: new.recv_time,
        event,
    })
}

fn merge_pointer(a: &PointerEvent, b: &PointerEvent, old: &InputEnvelope) -> Option<PointerEvent> {
    let same = a.phase == PointerPhase::Move
        && b.phase == PointerPhase::Move
        && a.pointer == b.pointer
        && a.kind == b.kind
        && a.buttons == b.buttons
        && a.modifiers == b.modifiers;
    if !same {
        return None;
    }
    let mut out = b.clone();
    let mut history = a.history.clone();
    history.push(PointerSample {
        pos: a.pos,
        time: old.os_time,
        pressure: a.pressure,
    });
    history.extend(b.history.iter().copied());
    out.history = history;
    Some(out)
}

fn merge_wheel(a: &WheelEvent, b: &WheelEvent) -> Option<WheelEvent> {
    // Gesture boundaries (start, end, cancel) are never merged away: the scroll physics needs them.
    let mid = |p: Option<GesturePhase>| matches!(p, None | Some(GesturePhase::Moved));
    if !(mid(a.phase)
        && mid(b.phase)
        && a.phase == b.phase
        && mid(a.momentum)
        && a.momentum == b.momentum)
    {
        return None;
    }
    if a.modifiers != b.modifiers {
        return None;
    }
    let delta = match (a.delta, b.delta) {
        (ScrollDelta::Lines(x), ScrollDelta::Lines(y)) => ScrollDelta::Lines(add(x, y)),
        (ScrollDelta::Pixels(x), ScrollDelta::Pixels(y)) => ScrollDelta::Pixels(add(x, y)),
        (ScrollDelta::Pages(x), ScrollDelta::Pages(y)) => ScrollDelta::Pages(add(x, y)),
        _ => return None,
    };
    Some(WheelEvent {
        pos: b.pos,
        delta,
        phase: b.phase,
        momentum: b.momentum,
        modifiers: b.modifiers,
    })
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}
