//! What every built-in kind does the same way: the style view over the node's computed style and its UA
//! table, running units on a pass, emitting signals by name, and the UA cursor.

use crate::interact::{self, InputCx, Pass, Sense, StateBits, Tracker};
use crate::paint::{StyleView, Ua};
use crate::shim::core::{Event, EventCx, KindSpec, Styles, Value};
use presto_base::time::Span;

/// The node's style view: its computed style over `ua`, in its state and scheme.
pub fn host_view<'a>(s: &Styles<'a>, ua: Ua) -> StyleView<'a> {
    StyleView::new(s.computed, ua, s.state, s.dark)
}

/// A kind's interaction substrate: one [`Tracker`] for the node.
#[derive(Clone, Debug, Default)]
pub struct Units {
    pub tracker: Tracker,
}

impl Units {
    /// Feeds `ev` to the tracker (sensing `sense`, with the policy's click thresholds), runs `f` (the
    /// kind's units) on the pass, applies their focus requests, and asks for the tracker's wake-up.
    pub fn run(&mut self, cx: &mut EventCx<'_>, ev: &Event, sense: Sense, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
        self.tracker.sense = sense;
        self.tracker.options = interact::ClickOptions::of(cx.policy);
        let pass = self.tracker.pass(cx, ev);
        interact::run(cx, &pass, f);
        if let Some(t) = self.tracker.next_wake() {
            cx.wake_at(t);
        }
        if pass.resp.clicked || pass.resp.drag_started || pass.resp.drag_stopped {
            cx.repaint = true;
        }
        pass
    }
}

/// Emits the signal `name` of `spec` with `args` (a name not in the spec is a bug in the kind).
pub fn emit(cx: &mut EventCx<'_>, spec: &KindSpec, name: &str, args: Vec<Value>) {
    match spec.signal(name) {
        Some(id) => cx.emit(id, args),
        None => debug_assert!(false, "{}: no signal `{name}` in the spec", spec.tag),
    }
}

/// Asks for an [`Event::Timer`] `secs` from now.
pub fn wake_in(cx: &mut EventCx<'_>, secs: f64) {
    let t = cx.now + Span::from_secs_f64(secs.max(0.0));
    cx.wake_at(t);
}

/// Sets the cursor the UA table gives in the node's state while it is hovered (DESIGN R10; a CSS
/// `cursor` on the node is presto-core's, from the computed style).
pub fn cursor(cx: &mut EventCx<'_>, ua: &Ua) {
    if cx.state.contains(StateBits::HOVERED)
        && let Some(c) = StyleView::ua_only(*ua, *cx.state, true).cursor()
    {
        cx.cursor = Some(c);
    }
}
