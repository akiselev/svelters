//! Inline direction (POL4-DIR-CORE): HTML's `dir` and CSS `direction`, as the web has them.
//!
//! - A node's **directionality** (what `:dir()` matches) is its `dir` prop's (`ltr`, `rtl`; `auto`
//!   from the first strong character of its text or its descendants' text, skipping descendants
//!   with a `dir` of their own), else its parent's, else (a window root) the document's
//!   ([`Document::set_direction`]). It is kept per node ([`NodeStyle::dir`](super::NodeStyle)), set
//!   top-down as the restyle reaches each node, before its rules match.
//! - CSS **`direction`** is an inherited property (initial `ltr`). A node with a `dir` (and a window
//!   root) inherits its directionality instead of its parent's value: the cascade sees a copy of the
//!   parent's style with `direction` set, so the node's own author rules still win, as the UA rule
//!   `[dir=rtl] { direction: rtl }` lets them in a browser. Kinds and layout read the computed value
//!   ([`ComputedBox::direction`](super::ComputedBox), [`NodeView::direction`](crate::NodeView::direction)).
//! - A change of a `dir`, of the document's direction, or of the text an `auto` reads restyles the
//!   subtree it reaches.

use crate::common;
use crate::document::Document;
use skelter_base::text::Direction;

/// What a node's `dir` prop says; `None` when it has none (empty, unset or another value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirAttr {
    Ltr,
    Rtl,
    Auto,
}

impl Document {
    /// Sets the document's direction: a window root's, and so every node's without a `dir` above
    /// it. Restyles every window when it changes.
    pub fn set_direction(&mut self, dir: Direction) {
        if self.style.direction != dir {
            self.style.direction = dir;
            self.restyle_all();
            self.stamp();
        }
    }

    /// The document's direction (`ltr` unless [`set_direction`](Self::set_direction) said otherwise).
    pub fn direction(&self) -> Direction {
        self.style.direction
    }

    /// A node's computed CSS `direction`, as of the last restyle (what layout and its kind read).
    pub fn node_direction(&self, node: skelter_base::ids::NodeId) -> Option<Direction> {
        self.idx(node).map(|i| self.style.computed(i).direction)
    }

    /// Node `i`'s `dir` prop.
    pub(crate) fn dir_attr(&self, i: usize) -> Option<DirAttr> {
        let v = self.props[i].get(common::DIR)?.as_str()?;
        let v = v.trim();
        if v.eq_ignore_ascii_case("ltr") {
            Some(DirAttr::Ltr)
        } else if v.eq_ignore_ascii_case("rtl") {
            Some(DirAttr::Rtl)
        } else if v.eq_ignore_ascii_case("auto") {
            Some(DirAttr::Auto)
        } else {
            None
        }
    }

    /// `dir=auto` on node `i`: the first strong character of its text, then of its descendants' in
    /// tree order (those with a `dir` of their own and their subtrees skipped); `ltr` when none has one.
    pub(crate) fn auto_direction(&self, i: usize) -> Direction {
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            if n != i && self.dir_attr(n).is_some() {
                continue;
            }
            if let Some(d) = skelter_base::text::first_strong(self.view_at(n).text()) {
                return d;
            }
            stack.extend(self.children[n].iter().rev().map(|&c| self.ix(c)));
        }
        Direction::Ltr
    }

    /// Node `i`'s directionality from its `dir`, else its parent's (already restyled this pass),
    /// else the document's; and whether it is a hint the cascade must see (a `dir`, or a root).
    pub(crate) fn directionality(&self, i: usize) -> (Direction, bool) {
        let parent = self.core[i].parent.map(|p| self.ix(p));
        match self.dir_attr(i) {
            Some(DirAttr::Ltr) => (Direction::Ltr, true),
            Some(DirAttr::Rtl) => (Direction::Rtl, true),
            Some(DirAttr::Auto) => (self.auto_direction(i), true),
            None => match parent {
                Some(p) => (self.style.nodes[p].dir, false),
                None => (self.style.direction, true),
            },
        }
    }

    /// Node `i`'s `dir` changed: it and its subtree restyle (`direction` and `:dir()`).
    pub(crate) fn dir_changed(&mut self, i: usize) {
        if self.dir_attr(i) == Some(DirAttr::Auto) {
            self.style.any_dir_auto = true;
        }
        self.mark_subtree_style(i);
    }

    /// The text under node `i` changed (its own, or a child came or went): the nearest `dir=auto`
    /// at or above it restyles its subtree when the direction it resolves to changed.
    pub(crate) fn refresh_dir_auto(&mut self, i: usize) {
        if !self.style.any_dir_auto {
            return;
        }
        let mut a = Some(i);
        while let Some(n) = a {
            match self.dir_attr(n) {
                Some(DirAttr::Auto) => {
                    if self.style.nodes[n].css.is_some() && self.auto_direction(n) != self.style.nodes[n].dir {
                        self.mark_subtree_style(n);
                    }
                    return;
                }
                Some(_) => return,
                None => a = self.core[n].parent.map(|p| self.ix(p)),
            }
        }
    }
}
