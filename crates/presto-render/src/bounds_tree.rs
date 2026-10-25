// Derived from zed-industries/zed crates/gpui/src/bounds_tree.rs @ a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc
// Copyright 2022 - 2025 Zed Industries, Inc. Licensed under the Apache License, Version 2.0.
// Modifications by the svelters authors: f32 device-pixel rects, index-based search (no `unsafe`),
// presto's `Rect`.

//! Draw-order assignment (architecture §8.2): each primitive's order is 1 + the maximum order of the
//! primitives already inserted that it overlaps, so primitives that don't overlap share an order and
//! batch together. An insert-only R-tree, cleared per frame.

use presto_base::geom::Rect;

/// Maximum children per internal node.
const MAX_CHILDREN: usize = 12;

#[derive(Debug, Default)]
pub(crate) struct BoundsTree {
    nodes: Vec<Node>,
    root: Option<usize>,
    /// The leaf with the highest order (the O(1) fast path).
    max_leaf: Option<usize>,
    insert_path: Vec<usize>,
    search_stack: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Node {
    bounds: Rect,
    max_order: u32,
    kind: NodeKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    Leaf { order: u32 },
    Internal { children: NodeChildren },
}

/// Child indices; the child with the highest `max_order` is kept last.
#[derive(Debug, Clone)]
struct NodeChildren {
    indices: [usize; MAX_CHILDREN],
    len: u8,
}

impl NodeChildren {
    fn new() -> Self {
        Self { indices: [0; MAX_CHILDREN], len: 0 }
    }
    fn push(&mut self, index: usize) {
        debug_assert!((self.len as usize) < MAX_CHILDREN);
        self.indices[self.len as usize] = index;
        self.len += 1;
    }
    fn len(&self) -> usize {
        self.len as usize
    }
    fn as_slice(&self) -> &[usize] {
        &self.indices[..self.len as usize]
    }
}

fn half_perimeter(r: &Rect) -> f32 {
    r.width() + r.height()
}

/// Bounds union that keeps zero-area rects (GPUI's `Bounds::union`; presto's `Rect::union` drops them).
fn union(a: &Rect, b: &Rect) -> Rect {
    Rect::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y), a.max.x.max(b.max.x), a.max.y.max(b.max.y))
}

impl BoundsTree {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.max_leaf = None;
        self.insert_path.clear();
        self.search_stack.clear();
    }

    /// Inserts `bounds` and returns its order: one more than the highest order it intersects.
    pub fn insert(&mut self, bounds: Rect) -> u32 {
        let ordering = self.find_max_ordering(&bounds) + 1;
        let new_leaf_idx = self.insert_leaf(bounds, ordering);
        self.max_leaf = match self.max_leaf {
            None => Some(new_leaf_idx),
            Some(old) if self.nodes[old].max_order < ordering => Some(new_leaf_idx),
            some => some,
        };
        ordering
    }

    fn find_max_ordering(&mut self, query: &Rect) -> u32 {
        let Some(root_idx) = self.root else {
            return 0;
        };
        if let Some(max_idx) = self.max_leaf {
            let max_node = &self.nodes[max_idx];
            if query.intersects(&max_node.bounds) {
                return max_node.max_order;
            }
        }

        self.search_stack.clear();
        self.search_stack.push(root_idx);
        let mut max_found = 0u32;
        while let Some(idx) = self.search_stack.pop() {
            let node = &self.nodes[idx];
            if node.max_order <= max_found || !query.intersects(&node.bounds) {
                continue;
            }
            match &node.kind {
                NodeKind::Leaf { order } => max_found = max_found.max(*order),
                NodeKind::Internal { children } => {
                    // The highest child is last, so it is popped first.
                    for &c in children.as_slice() {
                        if self.nodes[c].max_order > max_found {
                            self.search_stack.push(c);
                        }
                    }
                }
            }
        }
        max_found
    }

    fn insert_leaf(&mut self, bounds: Rect, order: u32) -> usize {
        let new_leaf_idx = self.nodes.len();
        self.nodes.push(Node { bounds, max_order: order, kind: NodeKind::Leaf { order } });

        let Some(root_idx) = self.root else {
            self.root = Some(new_leaf_idx);
            return new_leaf_idx;
        };

        if matches!(self.nodes[root_idx].kind, NodeKind::Leaf { .. }) {
            let root_bounds = self.nodes[root_idx].bounds;
            let root_order = self.nodes[root_idx].max_order;
            let mut children = NodeChildren::new();
            if order > root_order {
                children.push(root_idx);
                children.push(new_leaf_idx);
            } else {
                children.push(new_leaf_idx);
                children.push(root_idx);
            }
            let new_root_idx = self.nodes.len();
            self.nodes.push(Node {
                bounds: union(&root_bounds, &bounds),
                max_order: root_order.max(order),
                kind: NodeKind::Internal { children },
            });
            self.root = Some(new_root_idx);
            return new_leaf_idx;
        }

        self.insert_path.clear();
        let mut current_idx = root_idx;
        loop {
            let NodeKind::Internal { children } = &self.nodes[current_idx].kind else {
                unreachable!("only internal nodes are traversed");
            };
            self.insert_path.push(current_idx);

            let mut best_child_idx = children.as_slice()[0];
            let mut best_child_pos = 0;
            let mut best_cost = half_perimeter(&union(&bounds, &self.nodes[best_child_idx].bounds));
            for (pos, &child_idx) in children.as_slice().iter().enumerate().skip(1) {
                let cost = half_perimeter(&union(&bounds, &self.nodes[child_idx].bounds));
                if cost < best_cost {
                    best_cost = cost;
                    best_child_idx = child_idx;
                    best_child_pos = pos;
                }
            }

            if matches!(self.nodes[best_child_idx].kind, NodeKind::Leaf { .. }) {
                if children.len() < MAX_CHILDREN {
                    let node = &mut self.nodes[current_idx];
                    if let NodeKind::Internal { children } = &mut node.kind {
                        children.push(new_leaf_idx);
                        if order <= node.max_order {
                            let last = children.len() - 1;
                            children.indices.swap(last - 1, last);
                        }
                    }
                    node.bounds = union(&node.bounds, &bounds);
                    node.max_order = node.max_order.max(order);
                } else {
                    let sibling_bounds = self.nodes[best_child_idx].bounds;
                    let sibling_order = self.nodes[best_child_idx].max_order;
                    let mut new_children = NodeChildren::new();
                    if order > sibling_order {
                        new_children.push(best_child_idx);
                        new_children.push(new_leaf_idx);
                    } else {
                        new_children.push(new_leaf_idx);
                        new_children.push(best_child_idx);
                    }
                    let new_internal_idx = self.nodes.len();
                    let new_internal_max = sibling_order.max(order);
                    self.nodes.push(Node {
                        bounds: union(&sibling_bounds, &bounds),
                        max_order: new_internal_max,
                        kind: NodeKind::Internal { children: new_children },
                    });
                    let parent = &mut self.nodes[current_idx];
                    if let NodeKind::Internal { children } = &mut parent.kind {
                        let len = children.len();
                        children.indices[best_child_pos] = new_internal_idx;
                        if new_internal_max > parent.max_order {
                            children.indices.swap(best_child_pos, len - 1);
                        }
                    }
                }
                break;
            }
            current_idx = best_child_idx;
        }

        // Propagate bounds and max_order up the insertion path.
        let mut updated_child_idx = None;
        for i in (0..self.insert_path.len()).rev() {
            let node_idx = self.insert_path[i];
            let node = &mut self.nodes[node_idx];
            node.bounds = union(&node.bounds, &bounds);
            if node.max_order < order {
                node.max_order = order;
                if let (Some(child_idx), NodeKind::Internal { children }) = (updated_child_idx, &mut node.kind)
                    && let Some(pos) = children.as_slice().iter().position(|&c| c == child_idx)
                {
                    let last = children.len() - 1;
                    if pos != last {
                        children.indices.swap(pos, last);
                    }
                }
            }
            updated_child_idx = Some(node_idx);
        }
        new_leaf_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    #[test]
    fn insert_assigns_orders() {
        let mut tree = BoundsTree::default();
        assert_eq!(tree.insert(r(0.0, 0.0, 10.0, 10.0)), 1);
        assert_eq!(tree.insert(r(5.0, 5.0, 10.0, 10.0)), 2);
        assert_eq!(tree.insert(r(10.0, 10.0, 10.0, 10.0)), 3);
        assert_eq!(tree.insert(r(20.0, 20.0, 10.0, 10.0)), 1);
        assert_eq!(tree.insert(r(40.0, 40.0, 10.0, 10.0)), 1);
        assert_eq!(tree.insert(r(25.0, 25.0, 10.0, 10.0)), 2);
    }

    #[test]
    fn random_inserts_match_brute_force() {
        // A small LCG keeps the test free of a `rand` dependency.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |lo: f32, hi: f32| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            lo + (hi - lo) * ((state >> 40) as f32 / (1u64 << 24) as f32)
        };
        for _ in 0..300 {
            let mut tree = BoundsTree::default();
            let mut expected: Vec<(Rect, u32)> = Vec::new();
            let n = next(1.0, 100.0) as usize;
            for _ in 0..n {
                let b = r(next(-100.0, 100.0), next(-100.0, 100.0), next(0.0, 50.0), next(0.0, 50.0));
                let want = expected.iter().filter(|q| q.0.intersects(&b)).map(|q| q.1).max().unwrap_or(0) + 1;
                expected.push((b, want));
                assert_eq!(tree.insert(b), want);
            }
        }
    }
}
