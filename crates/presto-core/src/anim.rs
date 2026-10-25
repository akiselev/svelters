//! UI-side animation bookkeeping: ids of the animations a node started on the compositor (CSS
//! transitions of opacity and transform, kind animations) so they are cancelled with the node, the
//! commands the next commit carries, and the transitions that run on the UI thread (architecture
//! §12.1): colours re-record without layout, `width`/`height` relayout, each sampled with
//! presto-anim at the frame's presentation time.

use presto_base::anim::{AnimValue, Generator};
use presto_base::ids::{AnimationId, NodeId};
use presto_base::time::Time;
use std::collections::HashMap;

/// A property a transition animates on the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiProp {
    Color,
    Background,
    BorderColor,
    OutlineColor,
    Width,
    Height,
}

impl UiProp {
    /// The CSS name (what `transition-property` and `transitionend`'s `propertyName` say).
    pub fn css(self) -> &'static str {
        match self {
            UiProp::Color => "color",
            UiProp::Background => "background-color",
            UiProp::BorderColor => "border-color",
            UiProp::OutlineColor => "outline-color",
            UiProp::Width => "width",
            UiProp::Height => "height",
        }
    }
}

/// A transition sampled on the UI thread.
#[derive(Clone, Debug)]
pub(crate) struct UiTransition {
    pub prop: UiProp,
    pub generator: Generator,
    /// Bound to the first frame that samples it (CSS: the transition starts at the next frame).
    pub start: Option<Time>,
    /// The computed value it ends on (a restyle to the same value does not restart it).
    pub to: AnimValue,
    /// Delay plus duration: it ends at `start + length`, decided in exact time (a float progress
    /// at the boundary can fall a hair short of the end).
    pub length: presto_base::time::Span,
}

#[derive(Default)]
pub(crate) struct AnimState {
    pub next: u64,
    /// Running animations per node, with the CSS property they drive.
    pub running: HashMap<NodeId, Vec<(AnimationId, presto_base::anim::AnimProperty)>>,
    /// Animations cancelled because their node went away (sent with the window's next commit).
    pub orphaned: Vec<AnimationId>,
    /// Transitions sampled on the UI thread, per node.
    pub ui: HashMap<NodeId, Vec<UiTransition>>,
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
        self.ui.remove(&node);
    }
    /// The running UI transition of `prop` on `node`.
    pub fn ui_running(&self, node: NodeId, prop: UiProp) -> Option<&UiTransition> {
        self.ui.get(&node).and_then(|v| v.iter().find(|t| t.prop == prop))
    }
}
