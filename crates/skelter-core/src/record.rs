//! Per-node recorder state: the node's display list, the runs it references, and the property-tree
//! nodes and layer the node owns.

use skelter_base::display::DisplayList;
use skelter_base::geom::{Affine, Rect};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, RunId};
use skelter_base::layer::LayerKind;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct NodePaint {
    pub list: Option<Arc<DisplayList>>,
    /// A self-scrolling kind's fixed part ([`crate::RecordCx::fixed`]), sent under [`fixed_key`].
    pub fixed: Option<Arc<DisplayList>>,
    pub runs: Vec<RunId>,
    /// The node's own transform space (its layout offset and CSS transform).
    pub transform: Option<PropNodeId>,
    pub clip: Option<PropNodeId>,
    pub effect: Option<PropNodeId>,
    pub scroll: Option<PropNodeId>,
    /// A scroll container's content space (translated by the scroll offset).
    pub content: Option<PropNodeId>,
    pub layer: Option<LayerId>,
    /// An external layer placed at this node in paint order ([`crate::RecordCx::external_layer`]):
    /// its kind and bounds in the node's border-box space.
    pub external: Option<(LayerKind, Rect)>,
    /// The external layer's id, and the continuation layer that takes what paints after it.
    pub ext_layer: Option<LayerId>,
    pub ext_cont: Option<LayerId>,
    /// The clip node its children draw under (the dirty walk's; absolute descendants whose
    /// containing block this is take it).
    pub child_clip: Option<PropNodeId>,
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
            fixed: None,
            runs: Vec::new(),
            transform: None,
            clip: None,
            effect: None,
            scroll: None,
            content: None,
            layer: None,
            external: None,
            ext_layer: None,
            ext_cont: None,
            child_clip: None,
            world: Rect::ZERO,
            world_tf: Affine::IDENTITY,
            a11y_tf: None,
            a11y_kids: Vec::new(),
        }
    }
}

/// The list key of a node's fixed part: the node id with its top bit set (generations never get
/// there).
pub(crate) const fn fixed_key(node: NodeId) -> NodeId {
    NodeId(node.0 | 1 << 63)
}

impl NodePaint {
    pub fn prop_nodes(&self) -> impl Iterator<Item = PropNodeId> {
        [self.transform, self.clip, self.effect, self.scroll, self.content].into_iter().flatten()
    }
}
