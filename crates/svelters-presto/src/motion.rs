//! Native animations (`OP_ANIMATE`, ABI `SVL_STYLE_ANIMATE`; lane MOTION-CSS's presto side): an
//! animation's tracks come from presto-motion's plan of the op (the svelte/transition built-in or
//! `flip` its `native` descriptor names, else its channel frames) and run on the compositor through
//! presto-core's [`Document::start_animation`](presto_core::Document::start_animation). When the
//! compositor reports the last track's end, JS gets `animationend` with the op's id.
//!
//! The compositor plays opacity, transform and clip. A descriptor plan with other channels (slide's
//! size and padding, blur, a path's dash), held statics or `overflow: hidden` falls back to the op's
//! channel frames, which are exactly those three; the op is always accepted (the runtime stops
//! offering native animations to a host that refuses one). A node in no window has nothing to animate:
//! its op ends at once. The frames' transform origin is the node's own `transform-origin` on presto.

use crate::backend::PrestoBackend;
use presto_base::anim::{AnimProperty, AnimationEvent};
use presto_base::ids::AnimationId;
use presto_motion::op::AnimateOp;
use presto_motion::transition::{Channel, Plan};
use std::collections::HashMap;
use svelters_abi::*;

/// The running ops: op id → (handle, its compositor animations), and each animation's op.
#[derive(Default)]
pub struct Running {
    ops: HashMap<u64, (svl_handle, Vec<AnimationId>)>,
    owner: HashMap<AnimationId, u64>,
    /// Ops that ended, for the next delivery: (handle, op id).
    ended: Vec<(svl_handle, u64)>,
}

fn property(c: Channel) -> Option<AnimProperty> {
    Some(match c {
        Channel::Opacity => AnimProperty::Opacity,
        Channel::Transform => AnimProperty::Transform,
        Channel::ClipSize => AnimProperty::Clip,
        _ => return None,
    })
}

/// The plan presto plays: the descriptor's when the compositor can play all of it, else the frames'.
pub fn playable_plan(op: &AnimateOp) -> Plan {
    match op.native_plan() {
        Some(Ok(p)) if p.statics.is_empty() && !p.overflow_hidden && p.tracks.iter().all(|t| property(t.channel).is_some()) => p,
        _ => op.frames_plan(),
    }
}

impl PrestoBackend {
    /// `SVL_STYLE_ANIMATE` on `h`: answers 1 (presto plays it).
    pub(crate) fn animate(&self, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        let op = AnimateOp::parse(text.ok_or("OP_ANIMATE without a payload")?)?;
        if op.cancel {
            let ids = self.0.motion.borrow_mut().ops.remove(&op.id).map(|(_, ids)| ids).unwrap_or_default();
            let mut doc = self.doc_mut();
            for id in ids {
                self.0.motion.borrow_mut().owner.remove(&id);
                doc.cancel_animation(id);
            }
            return Ok(1);
        }
        let (n, _) = self.live(h)?;
        let plan = playable_plan(&op);
        let mut ids = Vec::new();
        {
            let mut doc = self.doc_mut();
            for (channel, generator) in plan.generators() {
                let Some(prop) = property(channel) else { continue };
                match doc.start_animation(n, prop, generator) {
                    Some(id) => ids.push(id),
                    None => break,
                }
            }
        }
        let mut m = self.0.motion.borrow_mut();
        if ids.is_empty() {
            m.ended.push((h, op.id));
        } else {
            for &id in &ids {
                m.owner.insert(id, op.id);
            }
            m.ops.insert(op.id, (h, ids));
        }
        Ok(1)
    }

    /// The compositor reported `ev` (the runner's `animation_event`): the tree learns it, and an op
    /// whose last animation ended is announced to JS.
    pub fn animation_event(&self, ev: AnimationEvent) {
        self.doc_mut().animation_event(ev);
        let id = match ev {
            AnimationEvent::Started { .. } => return,
            AnimationEvent::Finished { id, .. } | AnimationEvent::Aborted { id } => id,
        };
        let mut m = self.0.motion.borrow_mut();
        let Some(op) = m.owner.remove(&id) else { return };
        let done = match m.ops.get_mut(&op) {
            Some((_, ids)) => {
                ids.retain(|&x| x != id);
                ids.is_empty()
            }
            None => false,
        };
        if done && let Some((h, _)) = m.ops.remove(&op) {
            m.ended.push((h, op));
        }
    }

    /// Posts `animationend` for the ops that ended; returns how many.
    pub(crate) fn deliver_animation_ends(&self) -> usize {
        let ended = std::mem::take(&mut self.0.motion.borrow_mut().ended);
        let Some(host) = self.host() else { return 0 };
        let mut n = 0;
        for (h, op) in ended {
            let Ok(km) = self.kind_map(h) else { continue };
            let Some(sm) = km.signal_named("animationend") else { continue };
            host.post_event(h, sm.wire, &[Value::I64(op as i64)]);
            n += 1;
        }
        n
    }

    /// The ops running now (tests, the inspector): op id → its compositor animations.
    pub fn running_animations(&self) -> Vec<(u64, Vec<AnimationId>)> {
        let mut v: Vec<_> = self.0.motion.borrow().ops.iter().map(|(k, (_, ids))| (*k, ids.clone())).collect();
        v.sort_by_key(|x| x.0);
        v
    }
}
