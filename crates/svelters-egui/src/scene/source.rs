//! DZ0: where in its `.svelte` source a node was declared, from the `source-loc` prop that the
//! runtime sets in dev builds (`packages/native-runtime/src/index.js` `source_loc`).
//!
//! The prop's text is `file#offset`, then the component call sites the node renders inside, each
//! `file#offset`, innermost first, U+001F-separated. `offset` is the byte offset of the template
//! element (or of the component tag, for a call site) in the file the compiler was given. A node
//! inside `{#each}` shares its location with every other row; the keyed item it belongs to is
//! [`NodeRef::item_key`](super::NodeRef::item_key).
//!
//! Writer lane: SCENE.

use super::access::{NodeRef, SceneRef};

/// One `file#offset` of a [`SourceLoc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceSpot<'a> {
    /// The path the compiler was given for the `.svelte` file.
    pub file: &'a str,
    /// The byte offset of the template element (or component tag) in that file.
    pub offset: u32,
}

impl<'a> SourceSpot<'a> {
    /// Parses `file#offset` (the offset after the last `#`).
    pub fn parse(text: &'a str) -> Option<SourceSpot<'a>> {
        let (file, offset) = text.rsplit_once('#')?;
        Some(SourceSpot { file, offset: offset.parse().ok()? })
    }
}

/// A node's dev-build source location: its own template element, then the component call sites
/// around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLoc<'a> {
    raw: &'a str,
}

impl<'a> SourceLoc<'a> {
    /// Wraps a `source-loc` text; `None` unless its first spot parses.
    pub fn parse(raw: &'a str) -> Option<SourceLoc<'a>> {
        SourceSpot::parse(raw.split('\u{1f}').next()?)?;
        Some(SourceLoc { raw })
    }

    /// The prop's text.
    pub fn raw(self) -> &'a str {
        self.raw
    }

    /// The node's own template element.
    pub fn element(self) -> SourceSpot<'a> {
        self.spots().next().expect("checked in parse")
    }

    /// The component call sites the node renders inside, innermost first.
    pub fn sites(self) -> impl Iterator<Item = SourceSpot<'a>> + 'a {
        self.spots().skip(1)
    }

    /// The element, then the call sites.
    pub fn spots(self) -> impl Iterator<Item = SourceSpot<'a>> + 'a {
        self.raw.split('\u{1f}').filter_map(SourceSpot::parse)
    }
}

impl<'a> NodeRef<'a> {
    /// DZ0: where a dev build declared this node, if the runtime set `source-loc`.
    pub fn source_loc(self) -> Option<SourceLoc<'a>> {
        SourceLoc::parse(self.common().source_loc.as_deref()?)
    }
}

impl<'a> SceneRef<'a> {
    /// DZ0: every live node declared at the template element `file#offset` (each row of an
    /// `{#each}`, each instance of a component), in slot order. A scan of the scene: for tools,
    /// not per frame.
    pub fn by_source(self, file: &'a str, offset: u32) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let want = SourceSpot { file, offset };
        self.nodes().filter(move |n| n.source_loc().is_some_and(|l| l.element() == want))
    }
}
