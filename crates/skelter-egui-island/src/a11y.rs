//! Grafting an island's AccessKit tree into skelter's (architecture §13): AccessKit nested trees.
//!
//! egui 0.37 always emits its update for [`accesskit::TreeId::ROOT`] and resends its whole tree each pass.
//! The host's tree owns the root, so the graft:
//! - rewrites the update's `tree_id` to the island's own [`TreeId`];
//! - replaces the transform egui puts on its root node (`scale(pixels_per_point)`, egui's window
//!   pixels) with the island's placement: subtree transforms compose through the graft node and its
//!   ancestors (accesskit_consumer `NodeRef::transform`), so the host's root already scales points to
//!   pixels and the island adds only its offset;
//! - repairs a focus id that isn't in the update (AccessKit panics on it) to the subtree root.
//!
//! Node ids stay egui's: they are per tree in a nested tree, so they can't collide with skelter's.
//! Actions coming back name the island's tree; [`A11yGraft::map_action`] points them at `ROOT` again,
//! because egui ignores requests for any other tree.

use accesskit::{ActionRequest, Affine, Node, TreeId, TreeInfo, TreeUpdate, Uuid};

/// A tree id for an island, from a value unique among the app's islands (the island's skelter node id
/// works). Never [`TreeId::ROOT`].
pub fn island_tree_id(seed: u64) -> TreeId {
    // A fixed high half keeps island trees apart from other subtree producers (offscreen viewports).
    TreeId(Uuid::from_u128((0x7072_6573_746f_5f69_u128 << 64) | u128::from(seed)))
}

#[derive(Clone, Debug)]
pub struct A11yGraft {
    tree: TreeId,
    /// Island points → the graft node's coordinate space.
    placement: Affine,
}

impl A11yGraft {
    pub fn new(tree: TreeId) -> Self {
        debug_assert_ne!(tree, TreeId::ROOT, "an island can't own the root tree");
        A11yGraft { tree, placement: Affine::IDENTITY }
    }

    pub fn tree_id(&self) -> TreeId {
        self.tree
    }

    /// Where the island's points sit in the graft node's coordinate space (by default the island's
    /// offset in window points, for a graft node without its own transform).
    pub fn set_placement(&mut self, placement: Affine) {
        self.placement = placement;
    }

    pub fn placement(&self) -> Affine {
        self.placement
    }

    /// Mark the host's node for the island as the graft point.
    pub fn graft_node(&self, node: &mut Node) {
        node.set_tree_id(self.tree);
    }

    /// egui's update, as an update of the island's subtree.
    pub fn graft(&self, mut update: TreeUpdate) -> TreeUpdate {
        update.tree_id = self.tree;
        let root = match &update.tree {
            Some(info) => info.root,
            None => {
                let root = egui::accesskit_root_id().accesskit_id();
                update.tree = Some(TreeInfo::new(root));
                root
            }
        };
        if let Some(info) = &mut update.tree {
            info.toolkit_name = Some("egui".to_owned());
        }
        let mut has_focus = false;
        for (id, node) in &mut update.nodes {
            if *id == root {
                if self.placement == Affine::IDENTITY {
                    node.clear_transform();
                } else {
                    node.set_transform(self.placement);
                }
            }
            has_focus |= *id == update.focus;
        }
        if !has_focus {
            update.focus = root;
        }
        update
    }

    /// An action the platform adapter sent to the island's tree, as egui expects it; `None` for an
    /// action aimed at another tree.
    pub fn map_action(&self, req: ActionRequest) -> Option<ActionRequest> {
        (req.target_tree == self.tree).then_some(ActionRequest { target_tree: TreeId::ROOT, ..req })
    }
}
