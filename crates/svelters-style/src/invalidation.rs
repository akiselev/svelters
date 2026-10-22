//! [`Invalidation`]: which nodes a change can restyle (CSS.md §2.5, v2: dependency maps built from
//! the selectors, modelled on Stylo's invalidation map without depending on it).
//!
//! For every class, id, attribute and state bit some selector tests, the map records where in a
//! selector it sits: the subject compound (restyle the node), an ancestor compound (its
//! descendants), left of a sibling combinator (its later siblings and their subtrees), or inside
//! `:has()` (its ancestors and earlier siblings; the whole tree when the `:has()` itself is not in
//! the subject). Structural pseudo-classes and sibling combinators make insertions and removals
//! restyle the parent's subtree.

use crate::atom::Atom;
use crate::element::Element;
use crate::selector::{Impl, Selector};
use crate::state::StateMask;
use selectors::parser::{Combinator, Component};
use std::collections::HashMap;

/// A set of node relations to restyle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Reach(pub u8);

impl Reach {
    pub const NONE: Reach = Reach(0);
    /// The node itself.
    pub const SELF: Reach = Reach(1 << 0);
    /// Every descendant.
    pub const DESCENDANTS: Reach = Reach(1 << 1);
    /// Every later sibling and its subtree.
    pub const LATER_SIBLINGS: Reach = Reach(1 << 2);
    /// Every ancestor, and the earlier siblings of the node and of each ancestor.
    pub const ANCESTORS: Reach = Reach(1 << 3);
    /// The whole tree.
    pub const TREE: Reach = Reach(1 << 4);

    pub const fn contains(self, other: Reach) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    const fn without(self, other: Reach) -> Reach {
        Reach(self.0 & !other.0)
    }
}

impl std::ops::BitOr for Reach {
    type Output = Reach;
    fn bitor(self, rhs: Reach) -> Reach {
        Reach(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Reach {
    fn bitor_assign(&mut self, rhs: Reach) {
        self.0 |= rhs.0;
    }
}

/// The dependency maps of a set of selectors.
#[derive(Clone, Debug, Default)]
pub struct Invalidation {
    classes: HashMap<Atom, Reach>,
    ids: HashMap<Atom, Reach>,
    attrs: HashMap<Atom, Reach>,
    states: Vec<(StateMask, Reach)>,
    /// What an insertion or removal under a parent reaches, relative to that parent's children.
    structural: Reach,
}

impl Invalidation {
    pub fn add_selector(&mut self, sel: &Selector) {
        self.visit(sel, Reach::SELF, false);
    }

    /// `reach` is where the selector's subject sits in the enclosing selector.
    fn visit(&mut self, sel: &Selector, outer: Reach, in_has: bool) {
        let mut iter = sel.iter();
        let mut own = Reach::NONE; // the relation of the current compound to the subject
        let mut subject = true;
        loop {
            let reach = if in_has {
                if outer.contains(Reach::SELF) { Reach::ANCESTORS } else { Reach::TREE }
            } else if subject {
                outer
            } else {
                outer.without(Reach::SELF) | own
            };
            for c in &mut iter {
                self.component(c, reach, in_has);
            }
            match iter.next_sequence() {
                None => break,
                // `host::part(x)`: the host compound is the subject (a part restyles with its host)
                Some(Combinator::PseudoElement) => continue,
                Some(Combinator::Child | Combinator::Descendant) => own |= Reach::DESCENDANTS,
                Some(Combinator::NextSibling | Combinator::LaterSibling) => {
                    own |= Reach::LATER_SIBLINGS;
                    self.structural |= Reach::DESCENDANTS;
                }
                Some(_) => {}
            }
            subject = false;
        }
    }

    fn component(&mut self, c: &Component<Impl>, reach: Reach, in_has: bool) {
        match c {
            Component::Class(a) => *self.classes.entry(a.clone()).or_default() |= reach,
            Component::ID(a) => *self.ids.entry(a.clone()).or_default() |= reach,
            Component::AttributeInNoNamespaceExists { local_name, .. } | Component::AttributeInNoNamespace { local_name, .. } => {
                *self.attrs.entry(local_name.clone()).or_default() |= reach
            }
            Component::AttributeOther(a) => *self.attrs.entry(a.local_name.clone()).or_default() |= reach,
            // P1: a logical position is set by the widget, which restyles the part itself
            Component::NonTSPseudoClass(crate::selector::PseudoClass::Logical(_)) => {}
            Component::NonTSPseudoClass(pc) => {
                let bits = pc.bits();
                match self.states.iter_mut().find(|(b, _)| *b == bits) {
                    Some((_, r)) => *r |= reach,
                    None => self.states.push((bits, reach)),
                }
            }
            Component::Nth(_) | Component::Empty => self.structural |= Reach::DESCENDANTS,
            Component::NthOf(data) => {
                self.structural |= Reach::DESCENDANTS;
                for s in data.selectors() {
                    self.visit(s, reach, in_has);
                }
            }
            Component::Negation(list) | Component::Is(list) | Component::Where(list) => {
                for s in list.slice() {
                    self.visit(s, reach, in_has);
                }
            }
            Component::Has(rel) => {
                self.structural |= if reach.contains(Reach::SELF) { Reach::ANCESTORS } else { Reach::TREE };
                for r in rel.iter() {
                    self.visit(&r.selector, reach, true);
                }
            }
            _ => {}
        }
    }

    /// A class was added to or removed from a node.
    pub fn class_changed(&self, class: &str) -> Reach {
        self.classes.get(class).copied().unwrap_or_default()
    }

    pub fn id_changed(&self, id: &str) -> Reach {
        self.ids.get(id).copied().unwrap_or_default()
    }

    /// An attribute (a prop some attribute selector names) changed value.
    pub fn attr_changed(&self, name: &str) -> Reach {
        self.attrs.get(name).copied().unwrap_or_default()
    }

    /// Whether some selector names the attribute: only those props need recording (CSS.md §2.2).
    pub fn uses_attr(&self, name: &str) -> bool {
        self.attrs.contains_key(name)
    }

    /// A node's state went from `before` to `after`.
    pub fn state_changed(&self, before: StateMask, after: StateMask) -> Reach {
        let flipped = StateMask(before.0 ^ after.0);
        self.states.iter().filter(|(b, _)| b.intersects(flipped)).fold(Reach::NONE, |a, (_, r)| a | *r)
    }

    /// The state bits any selector tests: the backend tracks only these (CSS.md §2.3).
    pub fn states_used(&self) -> StateMask {
        self.states.iter().fold(StateMask::NONE, |a, (b, _)| a | *b)
    }

    /// A child was inserted under or removed from a parent. The inserted subtree itself always
    /// needs a style; this is what else does, relative to the parent: `DESCENDANTS` (its
    /// children's subtrees), `ANCESTORS` (`:has`) or `TREE`.
    pub fn structure_changed(&self) -> Reach {
        self.structural
    }
}

/// Calls `f` for every node `reach` names from `node` (possibly more than once per node).
pub fn for_each_reached<E: Element>(node: E, reach: Reach, f: &mut dyn FnMut(E)) {
    if reach.contains(Reach::TREE) {
        let mut root = node;
        while let Some(p) = root.parent() {
            root = p;
        }
        f(root);
        descendants(root, f);
        return;
    }
    if reach.contains(Reach::SELF) {
        f(node);
    }
    if reach.contains(Reach::DESCENDANTS) {
        descendants(node, f);
    }
    if reach.contains(Reach::LATER_SIBLINGS) {
        let mut s = node.next_sibling();
        while let Some(n) = s {
            f(n);
            descendants(n, f);
            s = n.next_sibling();
        }
    }
    if reach.contains(Reach::ANCESTORS) {
        let mut cur = Some(node);
        while let Some(n) = cur {
            let mut s = n.prev_sibling();
            while let Some(p) = s {
                f(p);
                s = p.prev_sibling();
            }
            cur = n.parent();
            if let Some(p) = cur {
                f(p);
            }
        }
    }
}

fn descendants<E: Element>(node: E, f: &mut dyn FnMut(E)) {
    let mut c = node.first_child();
    while let Some(n) = c {
        f(n);
        descendants(n, f);
        c = n.next_sibling();
    }
}
