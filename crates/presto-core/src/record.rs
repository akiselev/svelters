//! Per-node recorder state: the node's display list, the runs it references, and the property-tree
//! nodes and layer the node owns.

use presto_base::display::DisplayList;
use presto_base::geom::Rect;
use presto_base::ids::{LayerId, PropNodeId, RunId};
use std::sync::Arc;

#[derive(Default, Clone, Debug)]
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
    /// The border box in window space at the last rebuild (a11y bounds, `scroll_into_view`).
    pub world: Rect,
}

impl NodePaint {
    pub fn prop_nodes(&self) -> impl Iterator<Item = PropNodeId> {
        [self.transform, self.clip, self.effect, self.scroll, self.content].into_iter().flatten()
    }
}
