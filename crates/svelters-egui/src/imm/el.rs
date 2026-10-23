//! The virtual element: what an `imm` widget is to the cascade (a tag, an id, classes and
//! attributes, in a chain of ancestors), matched by the scene's [`Stylist`](svelters_style::Stylist)
//! with no scene node behind it.

use crate::provider::StateBits;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU64;
use svelters_style::{Element, StateMask};

/// One element of a virtual chain: `El::new("button").class("primary")`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct El {
    pub tag: Cow<'static, str>,
    /// A second tag type selectors match (`style_as`, WIDGET-PARITY §4.3): an app's own element
    /// (`cad-tab`) styled by a built-in's rules (`tab`).
    pub style_as: Option<Cow<'static, str>>,
    /// `#id`.
    pub id: Option<Cow<'static, str>>,
    pub classes: Vec<Cow<'static, str>>,
    /// Attributes for `[name="value"]` selectors.
    pub attrs: Vec<(Cow<'static, str>, String)>,
}

impl El {
    pub fn new(tag: impl Into<Cow<'static, str>>) -> El {
        El { tag: tag.into(), ..El::default() }
    }
    /// Adds the class `c` (or each of them, space-separated).
    pub fn class(mut self, c: impl Into<Cow<'static, str>>) -> El {
        match c.into() {
            Cow::Borrowed(s) => self.classes.extend(s.split_whitespace().map(Cow::Borrowed)),
            Cow::Owned(s) => self.classes.extend(s.split_whitespace().map(|x| Cow::Owned(x.to_string()))),
        }
        self
    }
    /// Also matched by `tag`'s type selectors and parts.
    pub fn style_as(mut self, tag: impl Into<Cow<'static, str>>) -> El {
        self.style_as = Some(tag.into());
        self
    }
    pub fn id(mut self, id: impl Into<Cow<'static, str>>) -> El {
        self.id = Some(id.into());
        self
    }
    pub fn attr(mut self, name: impl Into<Cow<'static, str>>, value: impl Into<String>) -> El {
        self.attrs.push((name.into(), value.into()));
        self
    }
}

/// One element of a chain, as the engine's tree: `chain[i]`, its parent `chain[i - 1]`, no
/// siblings or children. Only the last element (the widget) has a state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VNode<'a> {
    pub chain: &'a [&'a El],
    pub i: usize,
    pub state: StateBits,
    /// A key for the chain (matching caches are per call, so it only has to be non-zero and
    /// different per element of the chain).
    pub salt: u64,
}

impl<'a> VNode<'a> {
    /// The last element of `chain` in `state`.
    pub fn last(chain: &'a [&'a El], state: StateBits) -> VNode<'a> {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        chain.hash(&mut h);
        VNode { chain, i: chain.len() - 1, state, salt: h.finish() }
    }

    fn el(&self) -> &'a El {
        self.chain[self.i]
    }
}

impl Element for VNode<'_> {
    fn key(&self) -> NonZeroU64 {
        NonZeroU64::new(self.salt.wrapping_add(self.i as u64 + 1)).unwrap_or(NonZeroU64::MIN)
    }
    fn parent(&self) -> Option<Self> {
        (self.i > 0).then(|| VNode { i: self.i - 1, state: StateBits::NONE, ..*self })
    }
    fn prev_sibling(&self) -> Option<Self> {
        None
    }
    fn next_sibling(&self) -> Option<Self> {
        None
    }
    fn first_child(&self) -> Option<Self> {
        None
    }
    fn tag(&self) -> &str {
        &self.el().tag
    }
    fn style_as(&self) -> Option<&str> {
        self.el().style_as.as_deref()
    }
    fn id(&self) -> Option<&str> {
        self.el().id.as_deref()
    }
    fn has_class(&self, name: &str) -> bool {
        self.el().classes.iter().any(|c| c == name)
    }
    fn for_each_class(&self, f: &mut dyn FnMut(&str)) {
        for c in &self.el().classes {
            f(c);
        }
    }
    fn attr(&self, name: &str) -> Option<Cow<'_, str>> {
        self.el().attrs.iter().find(|(n, _)| n == name).map(|(_, v)| Cow::Borrowed(v.as_str()))
    }
    fn state(&self) -> StateMask {
        StateMask(self.state.0)
    }
    /// The bit alone, as scene nodes (Focusable sets it from the focus reason).
    fn is_focus_visible(&self) -> bool {
        self.state.contains(StateBits::FOCUS_VISIBLE)
    }
}
