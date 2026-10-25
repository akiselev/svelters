//! Per-node recorder state: the node's display list, the runs it references, and the property-tree
//! nodes and layer the node owns.

use presto_base::display::DisplayList;
use presto_base::geom::{Affine, Rect};
use presto_base::ids::{LayerId, NodeId, PropNodeId, RunId};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct NodePaint {
    pub list: Option<Arc<DisplayList>>,
    pub runs: Vec<RunId>,
    /// The node's own transform space (its layout offset and CSS transform).
    pub transform: Option<PropNodeId>,
    pub clip: Option<PropNodeId>,
    pub effect: Option<PropNodeId>,
    pub scroll: Option<PropNodeId>,
    /// A scroll container's content space (translated by the scroll offset).
    pub content: Option<PropNodeId>,
    pub layer: Option<LayerId>,
    /// The border box in window space at the last rebuild (`scroll_into_view`, the IME area).
    pub world: Rect,
    /// The border box's window transform at the last rebuild.
    pub world_tf: Affine,
    /// The transform relative to the tree parent the last accessibility node carried.
    pub a11y_tf: Option<Affine>,
    /// The accessibility children the last accessibility node carried.
    pub a11y_kids: Vec<NodeId>,
}

impl Default for NodePaint {
    fn default() -> Self {
        NodePaint {
            list: None,
            runs: Vec::new(),
            transform: None,
            clip: None,
            effect: None,
            scroll: None,
            content: None,
            layer: None,
            world: Rect::ZERO,
            world_tf: Affine::IDENTITY,
            a11y_tf: None,
            a11y_kids: Vec::new(),
        }
    }
}

impl NodePaint {
    pub fn prop_nodes(&self) -> impl Iterator<Item = PropNodeId> {
        [self.transform, self.clip, self.effect, self.scroll, self.content].into_iter().flatten()
    }
}
