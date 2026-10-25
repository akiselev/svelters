//! Hit lists: per window, the nodes drawn by the last frame in paint order, with their world
//! transforms and clips. Input hit-tests against the last committed list (architecture §11.1), so a
//! click lands where the user saw the node, not where an unpainted relayout moved it.

use skelter_base::geom::{Affine, Rect};
use skelter_base::ids::NodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitEntry {
    pub node: NodeId,
    /// Window space → node-local space.
    pub inverse: Affine,
    /// The node's border box in window space (its transformed bounding box).
    pub world: Rect,
    /// The clip chain's intersection in window space (`None`: unclipped).
    pub clip: Option<Rect>,
}

/// Paint order: later entries are on top.
#[derive(Clone, Debug, Default)]
pub struct HitList {
    pub entries: Vec<HitEntry>,
}
