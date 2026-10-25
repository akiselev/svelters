//! Peer groups ([`PeerGroup`]): sibling nodes of one kind that act as one control (radio buttons,
//! the toggle buttons of a group), what svelters-egui reached through `WidgetCx::sibling_ids` and
//! `with_peer`. The document gives a group one Tab stop and applies the members' peer requests
//! after the event that made them, in order; a member's own requests queue behind them.

use crate::cx::{Emitted, EventCx};
use crate::document::Document;
use crate::kind::{FocusReason, Handled, PeerGroup, PeerMsg, PeerTarget};
use skelter_base::ids::NodeId;
use svelters_style::StateMask;

fn group_of(doc: &Document, node: NodeId) -> Option<PeerGroup> {
    let i = doc.idx(node)?;
    doc.kinds[i].as_deref()?.peer_group(&doc.view_at(i))
}

/// The members of `node`'s peer group in tree order, the node included (empty without a group).
pub(crate) fn members(doc: &Document, node: NodeId) -> Vec<NodeId> {
    let Some(g) = group_of(doc, node) else { return Vec::new() };
    let Some(kind) = doc.kind_of(node) else { return Vec::new() };
    let Some(parent) = doc.parent(node) else { return vec![node] };
    doc.children(parent)
        .iter()
        .copied()
        .filter(|&c| {
            c == node
                || doc.kind_of(c) == Some(kind)
                    && match &g.key {
                        Some((p, v)) => doc.view(c).and_then(|cv| cv.prop(*p)) == Some(v),
                        None => true,
                    }
        })
        .collect()
}

/// The group's Tab stop when `node` is in a group of two or more focusable members: the first
/// member whose stop prop is true, else the first member. `None`: not in such a group.
pub(crate) fn tab_stop(doc: &Document, node: NodeId) -> Option<NodeId> {
    let g = group_of(doc, node)?;
    let focusable: Vec<NodeId> = members(doc, node).into_iter().filter(|&m| crate::input::focus::can_focus(doc, m)).collect();
    if focusable.len() < 2 {
        return None;
    }
    let stop = g.stop.and_then(|p| focusable.iter().copied().find(|&m| doc.view(m).is_some_and(|v| v.prop_bool(p))));
    stop.or_else(|| focusable.first().copied())
}

/// Calls member `to`'s [`Kind::peer`](crate::Kind::peer) (disabled members ignore it).
fn call_peer(doc: &mut Document, to: NodeId, from: NodeId, msg: &PeerMsg, emitted: &mut Vec<Emitted>) -> Handled {
    if doc.state(to).contains(StateMask::DISABLED) {
        return Handled::No;
    }
    let Some(i) = doc.idx(to) else { return Handled::No };
    let Some(mut k) = doc.kinds[i].take() else { return Handled::No };
    let h = {
        let mut cx = EventCx { doc, node: to, emitted, pointer: None };
        k.peer(&mut cx, from, msg)
    };
    doc.restore_kind(to, k);
    h
}

/// Applies the queued peer requests (and the ones their handling queues), unless an outer call is
/// already doing so.
pub(crate) fn run(doc: &mut Document, emitted: &mut Vec<Emitted>) {
    if doc.peer_running {
        return;
    }
    doc.peer_running = true;
    while let Some((from, to, msg)) = doc.peer_queue.pop_front() {
        if !doc.contains(from) {
            continue;
        }
        let members = members(doc, from);
        match to {
            PeerTarget::Others => {
                for m in members.into_iter().filter(|&m| m != from) {
                    call_peer(doc, m, from, &msg, emitted);
                }
            }
            PeerTarget::Rove(r) => {
                // roving skips members that cannot take focus (Qt, GTK)
                let focusable: Vec<NodeId> = members.into_iter().filter(|&m| m == from || crate::input::focus::can_focus(doc, m)).collect();
                let Some(at) = focusable.iter().position(|&m| m == from) else { continue };
                let target = focusable[r.target(at, focusable.len())];
                if target == from {
                    continue;
                }
                crate::input::focus::set_focus(doc, Some(target), FocusReason::Keyboard);
                call_peer(doc, target, from, &msg, emitted);
            }
        }
    }
    doc.peer_running = false;
}
