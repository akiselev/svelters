//! Property trees (architecture §6.2): transform, clip, effect and scroll. Scene nodes point at nodes
//! here; a node gets its own entry only when it needs one. The compositor owns the committed trees and
//! animates them without the UI thread.

use crate::geom::{Affine, Rect, RoundedRect, Size, Vec2};
use crate::ids::PropNodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformNode {
    pub parent: Option<PropNodeId>,
    /// Local transform relative to the parent's space (includes the node's layout offset).
    pub local: Affine,
    /// A scroll node whose offset translates this space (content of a scroll container).
    pub scroll: Option<PropNodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipShape {
    Rect(Rect),
    Rounded(RoundedRect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipNode {
    pub parent: Option<PropNodeId>,
    /// The transform space the shape is expressed in.
    pub space: PropNodeId,
    pub shape: ClipShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectNode {
    pub parent: Option<PropNodeId>,
    pub opacity: f32,
    pub blend: BlendMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverscrollBehavior {
    #[default]
    None,
    /// macOS-style rubber band.
    Elastic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollNode {
    /// The scroll container's own transform space.
    pub space: PropNodeId,
    pub offset: Vec2,
    pub content: Size,
    /// The visible viewport in `space`.
    pub viewport: Rect,
    pub scroll_x: bool,
    pub scroll_y: bool,
    pub overscroll: OverscrollBehavior,
    /// Every listener on this container's scroll path is passive (architecture §11.5): the compositor
    /// may scroll it from a fast commit without waiting for JS.
    pub passive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropOp {
    Transform(PropNodeId, TransformNode),
    Clip(PropNodeId, ClipNode),
    Effect(PropNodeId, EffectNode),
    Scroll(PropNodeId, ScrollNode),
    Remove(PropNodeId),
}

/// Changes to the property trees since the last commit, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertyTreeDelta {
    pub ops: Vec<PropOp>,
}
