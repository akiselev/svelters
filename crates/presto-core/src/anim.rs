//! UI-side animation bookkeeping: ids of the animations a node started (CSS transitions, kind
//! animations) so they are cancelled with the node, and the commands the next commit carries.

use presto_base::ids::{AnimationId, NodeId};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct AnimState {
    pub next: u64,
    /// Running animations per node, with the CSS property they drive.
    pub running: HashMap<NodeId, Vec<(AnimationId, presto_base::anim::AnimProperty)>>,
    /// Animations cancelled because their node went away (sent with the window's next commit).
    pub orphaned: Vec<AnimationId>,
}

impl AnimState {
    pub fn alloc(&mut self) -> AnimationId {
        self.next += 1;
        AnimationId(self.next)
    }
    pub fn forget(&mut self, node: NodeId) {
        if let Some(v) = self.running.remove(&node) {
            self.orphaned.extend(v.into_iter().map(|(id, _)| id));
        }
    }
}
