//! The drag in flight: one [`DragSession`] per app (drags cross windows; the pointer is captured by
//! the source until the release, architecture §11.4).
//!
//! Each step gets the drop-target candidates under the pointer, deepest first, from the tree's hit
//! test ([`TargetCandidate`]); the deepest enabled one whose `drop-types` accepts the payload is the
//! target (the source node itself never is). The session turns the steps into the source's and the
//! targets' signals ([`DndEvent`]) in the order Svelte sees them: `drag-enter` / `drag-leave` as the
//! target changes, and at the release `drag-leave`, `drop`, then the source's `drag-end`.
//!
//! OS file drags are sessions too: [`DragSession::files_hovered`] starts one carrying
//! [`URI_LIST`], [`DragSession::files_dropped`] drops it (always `copy`), and
//! [`DragSession::cancel`] ends it when the files leave the window.

use super::{uri_list, DragPayload, DropAction, PayloadData, URI_LIST};
use crate::table::shim::{Emit, Value};
use skelter_base::cursor::CursorIcon;
use skelter_base::event::Modifiers;
use skelter_base::geom::{Point, Rect};
use skelter_base::ids::NodeId;
use std::path::PathBuf;

/// A drop target under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetCandidate<'a> {
    pub node: NodeId,
    pub drop_types: &'a str,
    /// The target's border box in window points (the drop point is reported relative to it).
    pub rect: Rect,
    pub enabled: bool,
}

/// A drag-and-drop signal for a node.
#[derive(Clone, Debug, PartialEq)]
pub enum DndSignal {
    DragBegin,
    DragEnd { dropped: bool, action: DropAction },
    DragEnter { ty: String },
    DragLeave,
    /// `at` in the target's own points.
    Drop { ty: String, data: PayloadData, at: Point, action: DropAction },
}

/// A signal for `node`.
#[derive(Clone, Debug, PartialEq)]
pub struct DndEvent {
    pub node: NodeId,
    pub signal: DndSignal,
}

impl DndEvent {
    /// The JS-facing event. A [`PayloadData::Js`] handle goes out as its number: svelters-skelter
    /// swaps it for the value it stands for.
    pub fn emit(&self) -> Emit {
        match &self.signal {
            DndSignal::DragBegin => Emit::new("drag-begin", vec![]),
            DndSignal::DragEnd { dropped, action } => Emit::new("drag-end", vec![Value::Bool(*dropped), Value::Str(action.name().into())]),
            DndSignal::DragEnter { ty } => Emit::new("drag-enter", vec![Value::Str(ty.clone())]),
            DndSignal::DragLeave => Emit::new("drag-leave", vec![]),
            DndSignal::Drop { ty, data, at, action } => {
                let data = match data {
                    PayloadData::Text(s) => Value::Str(s.clone()),
                    PayloadData::Js(h) => Value::I64(*h as i64),
                };
                Emit::new("drop", vec![Value::Str(ty.clone()), data, Value::F64(at.x as f64), Value::F64(at.y as f64), Value::Str(action.name().into())])
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Target {
    node: NodeId,
    rect: Rect,
    ty: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Flight {
    payload: DragPayload,
    /// OS files (the payload's data is built at the drop).
    files: Vec<PathBuf>,
    from_os: bool,
    pos: Point,
    modifiers: Modifiers,
    target: Option<Target>,
}

/// The app's drag in flight, if any.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DragSession {
    flight: Option<Flight>,
    macos: bool,
}

impl DragSession {
    /// `macos` picks the platform's drop-action modifiers.
    pub fn new(macos: bool) -> DragSession {
        DragSession { flight: None, macos }
    }

    pub fn active(&self) -> bool {
        self.flight.is_some()
    }

    /// The payload in flight (the egui bridge reads it; an OS file drag has a [`URI_LIST`] entry
    /// whose data is filled at the drop).
    pub fn payload(&self) -> Option<&DragPayload> {
        self.flight.as_ref().map(|f| &f.payload)
    }

    /// The current target.
    pub fn target(&self) -> Option<NodeId> {
        self.flight.as_ref().and_then(|f| f.target.as_ref().map(|t| t.node))
    }

    /// The pointer's last position (window points), for the ghost.
    pub fn pos(&self) -> Option<Point> {
        self.flight.as_ref().map(|f| f.pos)
    }

    /// The action a drop now would take.
    pub fn action(&self) -> DropAction {
        match &self.flight {
            None => DropAction::None,
            Some(f) if f.from_os => DropAction::Copy,
            Some(f) => DropAction::from_modifiers(f.modifiers, self.macos),
        }
    }

    /// Starts a drag of `payload` at `pos`. A drag already in flight is cancelled first. The
    /// source node (if any) gets `drag-begin`.
    pub fn begin(&mut self, payload: DragPayload, pos: Point, modifiers: Modifiers) -> Vec<DndEvent> {
        let mut out = self.cancel();
        if let Some(s) = payload.source {
            out.push(DndEvent { node: s, signal: DndSignal::DragBegin });
        }
        self.flight = Some(Flight { payload, files: Vec::new(), from_os: false, pos, modifiers, target: None });
        out
    }

    /// Files dragged in from the OS are over the window at `pos` (winit reports one hover event
    /// per file: call it for each; the first starts the session).
    pub fn files_hovered(&mut self, path: PathBuf, pos: Point) {
        match &mut self.flight {
            Some(f) if f.from_os => {
                f.files.push(path);
                f.pos = pos;
            }
            _ => {
                let payload = DragPayload { entries: vec![(URI_LIST.into(), PayloadData::Text(String::new()))], source: None };
                self.flight = Some(Flight { payload, files: vec![path], from_os: true, pos, modifiers: Modifiers::default(), target: None });
            }
        }
    }

    /// The deepest enabled candidate that accepts the payload, as `(node, rect, type)`.
    fn pick(f: &Flight, candidates: &[TargetCandidate<'_>]) -> Option<Target> {
        candidates.iter().find_map(|c| {
            if !c.enabled || Some(c.node) == f.payload.source {
                return None;
            }
            f.payload.best(c.drop_types).map(|(ty, _)| Target { node: c.node, rect: c.rect, ty: ty.to_string() })
        })
    }

    /// The pointer moved to `pos` with `modifiers` over `candidates` (deepest first): the target
    /// may change (`drag-leave` on the old one, then `drag-enter` on the new one).
    pub fn update(&mut self, pos: Point, modifiers: Modifiers, candidates: &[TargetCandidate<'_>]) -> Vec<DndEvent> {
        let mut out = Vec::new();
        let Some(f) = &mut self.flight else { return out };
        f.pos = pos;
        f.modifiers = modifiers;
        let next = Self::pick(f, candidates);
        if next.as_ref().map(|t| t.node) != f.target.as_ref().map(|t| t.node) {
            if let Some(old) = f.target.take() {
                out.push(DndEvent { node: old.node, signal: DndSignal::DragLeave });
            }
            if let Some(t) = &next {
                out.push(DndEvent { node: t.node, signal: DndSignal::DragEnter { ty: t.ty.clone() } });
            }
        }
        f.target = next;
        out
    }

    /// The pointer was released at `pos`: the target (if any) gets `drag-leave` and `drop`, the
    /// source `drag-end(dropped, action)`. The session ends.
    pub fn release(&mut self, pos: Point, modifiers: Modifiers, candidates: &[TargetCandidate<'_>]) -> Vec<DndEvent> {
        let mut out = self.update(pos, modifiers, candidates);
        let action = self.action();
        let Some(f) = self.flight.take() else { return out };
        let dropped = match f.target {
            Some(t) => {
                let data = if f.from_os {
                    PayloadData::Text(uri_list(f.files.iter().map(|p| p.as_path())))
                } else {
                    f.payload.best(&t.ty).map(|(_, d)| d.clone()).unwrap_or_default()
                };
                let at = Point::new(pos.x - t.rect.min.x, pos.y - t.rect.min.y);
                out.push(DndEvent { node: t.node, signal: DndSignal::DragLeave });
                out.push(DndEvent { node: t.node, signal: DndSignal::Drop { ty: t.ty, data, at, action } });
                true
            }
            None => false,
        };
        if let Some(s) = f.payload.source {
            out.push(DndEvent { node: s, signal: DndSignal::DragEnd { dropped, action: if dropped { action } else { DropAction::None } } });
        }
        out
    }

    /// Files from the OS were dropped at `pos` (all of one drop: winit reports one event per file,
    /// the caller batches them per frame). Replaces the hovered list with `paths`.
    pub fn files_dropped(&mut self, paths: Vec<PathBuf>, pos: Point, candidates: &[TargetCandidate<'_>]) -> Vec<DndEvent> {
        match &mut self.flight {
            Some(f) if f.from_os => f.files = paths,
            _ => {
                let mut it = paths.into_iter();
                let Some(first) = it.next() else { return Vec::new() };
                self.files_hovered(first, pos);
                if let Some(f) = &mut self.flight {
                    f.files.extend(it);
                }
            }
        }
        self.release(pos, Modifiers::default(), candidates)
    }

    /// Ends the drag without a drop (Escape, a pointer cancel, files leaving the window): the target
    /// gets `drag-leave`, the source `drag-end(false, "none")`.
    pub fn cancel(&mut self) -> Vec<DndEvent> {
        let mut out = Vec::new();
        let Some(f) = self.flight.take() else { return out };
        if let Some(t) = f.target {
            out.push(DndEvent { node: t.node, signal: DndSignal::DragLeave });
        }
        if let Some(s) = f.payload.source {
            out.push(DndEvent { node: s, signal: DndSignal::DragEnd { dropped: false, action: DropAction::None } });
        }
        out
    }

    /// The cursor during the drag: by the action over an accepting target (`grabbing` for move,
    /// `copy`, `alias` for link), `no-drop` elsewhere.
    pub fn cursor(&self) -> Option<CursorIcon> {
        let f = self.flight.as_ref()?;
        Some(match (&f.target, self.action()) {
            (None, _) => CursorIcon::NoDrop,
            (Some(_), DropAction::Copy) => CursorIcon::Copy,
            (Some(_), DropAction::Link) => CursorIcon::Alias,
            _ => CursorIcon::Grabbing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: NodeId = NodeId(1);
    const OUTER: NodeId = NodeId(2);
    const INNER: NodeId = NodeId(3);

    fn part(data: PayloadData) -> DragPayload {
        DragPayload { entries: vec![("cad/part".into(), data)], source: Some(SRC) }
    }

    fn cand(node: NodeId, types: &str, x: f32) -> TargetCandidate<'_> {
        TargetCandidate { node, drop_types: types, rect: Rect::new(x, 0.0, x + 100.0, 100.0), enabled: true }
    }

    fn signals(ev: &[DndEvent]) -> Vec<(NodeId, &'static str)> {
        ev.iter().map(|e| (e.node, e.emit().signal)).collect()
    }

    #[test]
    fn hover_enters_and_leaves_and_the_deepest_accepting_target_wins() {
        let mut s = DragSession::new(false);
        assert_eq!(signals(&s.begin(part(PayloadData::Js(9)), Point::new(0.0, 0.0), Modifiers::default())), vec![(SRC, "drag-begin")]);
        assert_eq!(s.cursor(), Some(CursorIcon::NoDrop));
        let both = [cand(INNER, "image/png", 10.0), cand(OUTER, "cad/*", 0.0)];
        assert_eq!(signals(&s.update(Point::new(20.0, 5.0), Modifiers::default(), &both)), vec![(OUTER, "drag-enter")], "the inner one doesn't accept");
        assert!(s.update(Point::new(21.0, 5.0), Modifiers::default(), &both).is_empty(), "no second enter");
        assert_eq!(s.cursor(), Some(CursorIcon::Grabbing));
        let deeper = [cand(INNER, "cad/part", 10.0), cand(OUTER, "cad/*", 0.0)];
        assert_eq!(signals(&s.update(Point::new(22.0, 5.0), Modifiers::default(), &deeper)), vec![(OUTER, "drag-leave"), (INNER, "drag-enter")]);
        assert_eq!(signals(&s.update(Point::new(500.0, 5.0), Modifiers::default(), &[])), vec![(INNER, "drag-leave")]);
        assert_eq!(s.target(), None);
    }

    #[test]
    fn a_drop_reports_leave_drop_then_the_sources_end() {
        let mut s = DragSession::new(false);
        s.begin(part(PayloadData::Js(9)), Point::new(0.0, 0.0), Modifiers::default());
        let t = [cand(OUTER, "cad/part", 50.0)];
        s.update(Point::new(60.0, 10.0), Modifiers::CTRL, &t);
        assert_eq!(s.cursor(), Some(CursorIcon::Copy));
        let ev = s.release(Point::new(60.0, 10.0), Modifiers::CTRL, &t);
        assert_eq!(signals(&ev), vec![(OUTER, "drag-leave"), (OUTER, "drop"), (SRC, "drag-end")]);
        assert_eq!(ev[1].signal, DndSignal::Drop { ty: "cad/part".into(), data: PayloadData::Js(9), at: Point::new(10.0, 10.0), action: DropAction::Copy });
        assert_eq!(ev[2].signal, DndSignal::DragEnd { dropped: true, action: DropAction::Copy });
        assert!(!s.active());
    }

    #[test]
    fn released_nowhere_or_cancelled_reports_not_dropped() {
        let mut s = DragSession::new(false);
        s.begin(part(PayloadData::Text("R1".into())), Point::ZERO, Modifiers::default());
        let ev = s.release(Point::new(5.0, 5.0), Modifiers::default(), &[]);
        assert_eq!(ev, vec![DndEvent { node: SRC, signal: DndSignal::DragEnd { dropped: false, action: DropAction::None } }]);
        s.begin(part(PayloadData::Text("R1".into())), Point::ZERO, Modifiers::default());
        s.update(Point::new(5.0, 5.0), Modifiers::default(), &[cand(OUTER, "*", 0.0)]);
        assert_eq!(signals(&s.cancel()), vec![(OUTER, "drag-leave"), (SRC, "drag-end")]);
    }

    #[test]
    fn the_source_and_disabled_targets_never_take_the_drop() {
        let mut s = DragSession::new(false);
        s.begin(part(PayloadData::Js(1)), Point::ZERO, Modifiers::default());
        let own = [cand(SRC, "cad/part", 0.0), TargetCandidate { enabled: false, ..cand(INNER, "cad/part", 0.0) }];
        assert!(s.update(Point::new(5.0, 5.0), Modifiers::default(), &own).is_empty());
        assert_eq!(s.target(), None);
    }

    #[test]
    fn os_files_drop_as_a_copy_of_a_uri_list() {
        let mut s = DragSession::new(false);
        s.files_hovered(PathBuf::from("/a.step"), Point::new(5.0, 5.0));
        let t = [cand(OUTER, "text/uri-list", 0.0)];
        assert_eq!(signals(&s.update(Point::new(5.0, 5.0), Modifiers::default(), &t)), vec![(OUTER, "drag-enter")]);
        assert_eq!(s.cursor(), Some(CursorIcon::Copy));
        let ev = s.files_dropped(vec![PathBuf::from("/a.step"), PathBuf::from("/b c.step")], Point::new(5.0, 6.0), &t);
        assert_eq!(signals(&ev), vec![(OUTER, "drag-leave"), (OUTER, "drop")], "no source: no drag-end");
        assert_eq!(ev[1].signal, DndSignal::Drop { ty: URI_LIST.into(), data: PayloadData::Text("file:///a.step\r\nfile:///b%20c.step".into()), at: Point::new(5.0, 6.0), action: DropAction::Copy });
    }
}
