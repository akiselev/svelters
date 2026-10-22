//! [`Element`]: what the engine needs from a tree, and the adapter that lets `selectors` match it.
//!
//! The trait is shaped after the egui scene's `NodeRef` (ARCH-SCENE §6.1–6.2): a `Copy` handle
//! into a borrowed tree, with a stable key (`NodeId::to_raw()`), a tag, light-tree navigation and
//! the node's `StateBits` as a [`StateMask`]. CS5e implements it for `NodeRef<'_>`; CS1 implements
//! it for the host's `StyleTree`. Shadow and attached children are not light children and are not
//! visited, so a provider's shadow subtree is styled as its own tree (`is_root` on its root).

use crate::atom::{Atom, AttrValue};
use crate::selector::{Impl, PseudoClass, PseudoElement};
use crate::state::StateMask;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::MatchingContext;
use selectors::matching::ElementSelectorFlags;
use selectors::OpaqueElement;
use std::borrow::Cow;
use std::fmt::Debug;
use std::num::NonZeroU64;
use std::ptr::NonNull;

/// A node of a styled tree, by handle.
pub trait Element: Copy + Debug {
    /// A key unique among the live nodes of the tree (the scene's `NodeId::to_raw()`). Matching
    /// caches (`:nth-child`, `:has`) are keyed by it.
    fn key(&self) -> NonZeroU64;
    fn parent(&self) -> Option<Self>;
    fn prev_sibling(&self) -> Option<Self>;
    fn next_sibling(&self) -> Option<Self>;
    fn first_child(&self) -> Option<Self>;
    /// The element's tag (`button`, `quantity-field`), matched case-sensitively by type selectors.
    fn tag(&self) -> &str;
    /// The `#id` (the `NAME` style op; the scene's `anchor` is a different thing).
    fn id(&self) -> Option<&str>;
    fn has_class(&self, name: &str) -> bool;
    /// Every class, for the rule index.
    fn for_each_class(&self, f: &mut dyn FnMut(&str));
    /// The value of the prop that an attribute selector names, as text (`[value="3"]`).
    fn attr(&self, name: &str) -> Option<Cow<'_, str>>;
    fn state(&self) -> StateMask;

    /// `:focus-visible`. The default reads [`StateMask::FOCUS_VISIBLE`] (the Focusable unit sets it
    /// from the focus reason, ARCH-WIDGETS §2.2) and, for a tree that never sets that bit, falls
    /// back to "focused" (right for nodes only the keyboard can focus). A tree that sets the bit
    /// for some nodes and not others overrides this to read the bit alone.
    fn is_focus_visible(&self) -> bool {
        let s = self.state();
        s.contains(StateMask::FOCUS_VISIBLE) || s.contains(StateMask::FOCUSED)
    }
}

/// Wraps an [`Element`] for `selectors`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Node<E>(pub E);

impl<E: Element> selectors::Element for Node<E> {
    type Impl = Impl;

    fn opaque(&self) -> OpaqueElement {
        // `selectors` only compares opaque elements (and never dereferences them unless a caller
        // does with `as_const_ptr`, which this crate doesn't). The key is non-zero by contract.
        let raw = self.0.key().get() as usize;
        OpaqueElement::from_non_null_ptr(NonNull::new(raw as *mut ()).unwrap_or(NonNull::dangling()))
    }

    fn parent_element(&self) -> Option<Self> {
        self.0.parent().map(Node)
    }
    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling().map(Node)
    }
    fn next_sibling_element(&self) -> Option<Self> {
        self.0.next_sibling().map(Node)
    }
    fn first_element_child(&self) -> Option<Self> {
        self.0.first_child().map(Node)
    }
    fn is_html_element_in_html_document(&self) -> bool {
        // Not HTML: type selectors and attribute names match case-sensitively.
        false
    }
    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag() == local_name
    }
    fn has_namespace(&self, ns: &str) -> bool {
        ns.is_empty()
    }
    fn is_same_type(&self, other: &Self) -> bool {
        self.0.tag() == other.0.tag()
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Atom>,
        local_name: &Atom,
        operation: &AttrSelectorOperation<&AttrValue>,
    ) -> bool {
        if let NamespaceConstraint::Specific(ns) = ns
            && !ns.is_empty()
        {
            return false;
        }
        match self.0.attr(local_name) {
            Some(v) => operation.eval_str(&v),
            None => false,
        }
    }

    fn match_non_ts_pseudo_class(&self, pc: &PseudoClass, _cx: &mut MatchingContext<'_, Impl>) -> bool {
        let state = self.0.state();
        match pc {
            PseudoClass::State { bit, negated, .. } => state.intersects(*bit) != *negated,
            PseudoClass::FocusVisible => self.0.is_focus_visible(),
            PseudoClass::Provider { bit, .. } => state.intersects(*bit),
        }
    }

    fn match_pseudo_element(&self, _pe: &PseudoElement, _cx: &mut MatchingContext<'_, Impl>) -> bool {
        // A pseudo-element's rules match only through `Stylist::compute_pseudo` (stateless
        // pseudo-element mode, which consumes the pseudo before the element is asked); the
        // element itself is never its own pseudo-element.
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {
        // Invalidation uses the dependency maps of `invalidation.rs`, not per-node flags.
    }

    fn is_link(&self) -> bool {
        false
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &Atom, case: CaseSensitivity) -> bool {
        self.0.id().is_some_and(|mine| case.eq(mine.as_bytes(), id.as_bytes()))
    }

    fn has_class(&self, name: &Atom, case: CaseSensitivity) -> bool {
        match case {
            CaseSensitivity::CaseSensitive => self.0.has_class(name),
            CaseSensitivity::AsciiCaseInsensitive => {
                let mut found = false;
                self.0.for_each_class(&mut |c| found |= c.eq_ignore_ascii_case(name));
                found
            }
        }
    }

    fn has_custom_state(&self, _name: &Atom) -> bool {
        false
    }
    fn imported_part(&self, _name: &Atom) -> Option<Atom> {
        None
    }
    fn is_part(&self, _name: &Atom) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        self.0.first_child().is_none()
    }
    fn is_root(&self) -> bool {
        self.0.parent().is_none()
    }
    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}
