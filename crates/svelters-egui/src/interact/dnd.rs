//! `DragSource` and `DropTarget` (ARCH-WIDGETS §1.2 catalogue; OW5a1b, QA9): drag and drop as
//! behaviour units. They used to be the two halves of WG4b's per-node helper (`kinds/dnd.rs`);
//! that file now only feeds them what egui's context knows (the payload in flight, the targets'
//! pointer claims, OS file drops) and turns their intents into the WG4b/WG6 signals, unchanged:
//! `drag-begin`, `drag-end(dropped, action)`, `drag-enter(type)`, `drag-leave`,
//! `drop(type, data, x, y, action)`. List and tree rows run the same units (`virtuallist.rs`).
//!
//! - [`DragSource`] owns `:dragging`. A drag past egui's threshold on the host is a
//!   [`Drag::Begin`]; the host then makes its [`DragPayload`] with [`DragSource::payload`], which
//!   links the payload to the unit, so the pass after the release reports what the target did
//!   ([`Drag::End`]: dropped or not, and the action). It picks the cursor (`grab` over the host,
//!   `grabbing`/`copy`/`alias` over an accepting target by the action, `no-drop` elsewhere).
//! - [`DropTarget`] owns `:drop-target`. It sees an [`Offer`] (the entry of the payload, or the OS
//!   files, it would take) while the pointer is over it and it holds the claim; every
//!   [`DropIntent::Enter`] is matched by a [`DropIntent::Leave`], a drop included (`Leave`, then
//!   [`DropIntent::Drop`]). A host that turns disabled mid-drag still gets its `Leave`.
//!
//! The drop action comes from the modifiers held at the release (Qt's `Qt::DropAction`,
//! [`drop_action`]); a file drop from the OS is always `copy`.
//! Writer lane: OW5a1b.

use super::{FocusCx, InputCx, Intents, PartResponses, Resp, Unit};
use crate::kinds::dnd::{drop_action, DragPayload};
use crate::provider::StateBits;
use egui::CursorIcon;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use svelters_host::platform::MimeData;

/// What a host configures on its [`DragSource`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DragSourceParams {
    /// The host drags (it has a `drag-type`, a `row-drag-type`).
    pub on: bool,
    /// macOS modifiers for the action (Option copies, Cmd+Option links).
    pub mac: bool,
    /// Some accepting target holds the pointer (last pass's claim).
    pub over_target: bool,
}

/// What [`DragSource`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    /// A drag started on the host: set a payload ([`DragSource::payload`]).
    Begin,
    /// The pass after the release: whether a target took it, and with which action (`none`
    /// when not dropped).
    End { dropped: bool, action: &'static str },
}

#[derive(Clone, Debug)]
struct Flight {
    dropped: Arc<AtomicBool>,
    action: Arc<AtomicU8>,
    ended: bool,
}

/// A drag source (see the module doc). Clone: hosts without a unit field keep it in egui's
/// temp data.
#[derive(Clone, Debug, Default)]
pub struct DragSource {
    flight: Option<Flight>,
    cursor: Option<CursorIcon>,
    dragging: bool,
}

impl DragSource {
    /// The payload of the drag that just began, linked to this unit (its `dropped` flag and
    /// action come back as [`Drag::End`]).
    pub fn payload(&self, data: MimeData, source: Option<egui::Id>) -> DragPayload {
        let f = self.flight.clone().unwrap_or(Flight { dropped: Arc::default(), action: Arc::default(), ended: false });
        DragPayload::linked(data, source, f.dropped, f.action)
    }

    /// The cursor this pass (set it while it is `Some`).
    pub fn cursor(&self) -> Option<CursorIcon> {
        self.cursor
    }

    /// The host is being dragged (the ghost follows the pointer).
    pub fn dragging(&self) -> bool {
        self.dragging
    }
}

impl Unit for DragSource {
    type Params = DragSourceParams;
    type Intent = Drag;
    const OWNS: StateBits = StateBits::DRAGGING;

    fn sense(&self, _p: &DragSourceParams) -> egui::Sense {
        egui::Sense::drag()
    }

    fn input(&mut self, p: &DragSourceParams, cx: &mut InputCx<'_>, out: &mut Intents<Drag>) {
        // the pass after the release: every target has run, so `dropped` is final
        if let Some(f) = self.flight.as_ref().filter(|f| f.ended) {
            let dropped = f.dropped.load(Ordering::Relaxed);
            let action = match f.action.load(Ordering::Relaxed) {
                0 if dropped => "move",
                k => crate::kinds::dnd::ACTIONS.get(k as usize).copied().unwrap_or("none"),
            };
            out.push(Drag::End { dropped, action });
            self.flight = None;
        }
        self.cursor = None;
        self.dragging = false;
        if !p.on || !cx.enabled {
            cx.set(StateBits::DRAGGING, false);
            return;
        }
        if cx.resp.drag_started {
            self.flight = Some(Flight { dropped: Arc::default(), action: Arc::default(), ended: false });
            out.push(Drag::Begin);
        }
        if cx.resp.dragged {
            self.dragging = true;
            self.cursor = Some(match (p.over_target, drop_action(cx.input.modifiers, p.mac)) {
                (false, _) => CursorIcon::NoDrop,
                (true, "copy") => CursorIcon::Copy,
                (true, "link") => CursorIcon::Alias,
                _ => CursorIcon::Grabbing,
            });
        } else if cx.resp.hovered {
            self.cursor = Some(CursorIcon::Grab);
        }
        if cx.resp.drag_stopped
            && let Some(f) = self.flight.as_mut()
        {
            f.ended = true;
        }
        cx.set(StateBits::DRAGGING, self.dragging);
    }

    fn next_frame(&self) -> Option<std::time::Duration> {
        // `End` is reported the pass after the release
        self.flight.as_ref().filter(|f| f.ended).map(|_| std::time::Duration::ZERO)
    }
}

/// The entry a [`DropTarget`] would take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    /// The type it takes it as (the first of its `drop-types` the payload has).
    pub ty: String,
    /// The data, read only at the release (`None` before).
    pub data: Option<String>,
}

/// What a host feeds its [`DropTarget`] each pass (what egui's context knows).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DropParams {
    /// The payload (or the OS files) it accepts, while a drag is in flight.
    pub offer: Option<Offer>,
    /// The pointer is over the target and the target holds the claim.
    pub inside: bool,
    /// The drag ends this pass (pointer released, files dropped).
    pub released: bool,
    /// The drop point in the target's own points.
    pub at: egui::Vec2,
    /// The action of a drop now (`move`, `copy`, `link`).
    pub action: &'static str,
}

/// What [`DropTarget`] reports.
#[derive(Clone, Debug, PartialEq)]
pub enum DropIntent {
    Enter(String),
    Leave,
    Drop { ty: String, data: String, at: egui::Vec2, action: &'static str },
}

/// A drop target (see the module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DropTarget {
    inside: bool,
}

impl DropTarget {
    /// A drag it accepts is over it (`:drop-target`).
    pub fn active(&self) -> bool {
        self.inside
    }
}

impl Unit for DropTarget {
    type Params = DropParams;
    type Intent = DropIntent;
    const OWNS: StateBits = StateBits::DROP_TARGET;

    fn input(&mut self, p: &DropParams, cx: &mut InputCx<'_>, out: &mut Intents<DropIntent>) {
        let active = cx.enabled && p.inside && p.offer.is_some();
        if active && !self.inside {
            out.push(DropIntent::Enter(p.offer.as_ref().map(|o| o.ty.clone()).unwrap_or_default()));
        }
        let released = active && p.released;
        if self.inside && (!active || released) {
            out.push(DropIntent::Leave);
        }
        if released && let Some(o) = &p.offer {
            out.push(DropIntent::Drop { ty: o.ty.clone(), data: o.data.clone().unwrap_or_default(), at: p.at, action: p.action });
        }
        self.inside = active && !released;
        cx.set(StateBits::DROP_TARGET, self.inside);
    }
}

/// Runs `unit` on a host that has no `egui::Response` of its own (a drop zone: registering a
/// widget there would change what the pointer hits). `rect` is the host's, `id` its egui id.
pub(crate) fn run_bare<U: Unit>(ctx: &egui::Context, id: egui::Id, rect: egui::Rect, enabled: bool, state: &mut StateBits, unit: &mut U, p: &U::Params) -> Intents<U::Intent> {
    let policy = crate::native::policy_of(ctx);
    let mut out = Intents::default();
    let contains = ctx.pointer_latest_pos().is_some_and(|q| rect.contains(q));
    let snap = Resp {
        id,
        rect,
        hovered: contains,
        contains_pointer: contains,
        down_on: false,
        clicked: false,
        double_clicked: false,
        triple_clicked: false,
        secondary_clicked: false,
        middle_clicked: false,
        drag_started: false,
        dragged: false,
        drag_stopped: false,
        drag_delta: egui::Vec2::ZERO,
        interact_pointer_pos: None,
        has_focus: false,
        enabled,
    };
    let mut focus = FocusCx::default();
    let parts = PartResponses::default();
    ctx.input(|inp| {
        let mut cx = InputCx { resp: &snap, input: inp, policy: &policy, now: inp.time, state, enabled, a11y: &[], focus: &mut focus, parts: &parts };
        unit.input(p, &mut cx, &mut out);
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare<U: Unit>(u: &mut U, p: &U::Params, state: &mut StateBits) -> Vec<U::Intent> {
        let ctx = egui::Context::default();
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0));
        run_bare(&ctx, egui::Id::NULL.with("t"), r, true, state, u, p).0
    }

    #[test]
    fn ow5a1b_drop_target_enters_leaves_and_drops_once() {
        let mut t = DropTarget::default();
        let mut st = StateBits::NONE;
        let offer = Some(Offer { ty: "cad/part".into(), data: None });
        let over = DropParams { offer: offer.clone(), inside: true, action: "move", ..DropParams::default() };
        assert_eq!(bare(&mut t, &over, &mut st), vec![DropIntent::Enter("cad/part".into())]);
        assert!(st.contains(StateBits::DROP_TARGET));
        assert!(bare(&mut t, &over, &mut st).is_empty(), "no second enter");
        let release = DropParams { offer: Some(Offer { ty: "cad/part".into(), data: Some("R1".into()) }), released: true, ..over.clone() };
        assert_eq!(bare(&mut t, &release, &mut st), vec![DropIntent::Leave, DropIntent::Drop { ty: "cad/part".into(), data: "R1".into(), at: egui::Vec2::ZERO, action: "move" }]);
        assert!(!st.contains(StateBits::DROP_TARGET));
        bare(&mut t, &over, &mut st);
        assert_eq!(bare(&mut t, &DropParams::default(), &mut st), vec![DropIntent::Leave], "leaving without a drop");
    }

    #[test]
    fn ow5a1b_drag_source_reports_the_targets_action_the_pass_after() {
        let mut s = DragSource::default();
        let mut st = StateBits::NONE;
        let p = DragSourceParams { on: true, ..DragSourceParams::default() };
        assert!(bare(&mut s, &p, &mut st).is_empty());
        // begin by hand (run_bare has no drag); the payload is linked to the unit
        s.flight = Some(Flight { dropped: Arc::default(), action: Arc::default(), ended: true });
        let payload = s.payload(MimeData::single("cad/part", "1"), None);
        payload.accept_as("copy");
        assert_eq!(bare(&mut s, &p, &mut st), vec![Drag::End { dropped: true, action: "copy" }]);
        assert!(bare(&mut s, &p, &mut st).is_empty(), "reported once");
    }
}
