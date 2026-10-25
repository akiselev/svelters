//! Style: svelters-style's cascade run on the document (CSS.md CS5e: the tree implements
//! [`svelters_style::Element`] directly), producing a [`ComputedBox`] and a `taffy::Style` per node.
//! A restyle compares the new computed values with the old ones and marks only the work their change
//! classes need (architecture §5): layout properties relayout, paint properties re-record, compositor
//! properties (opacity, transform) update the property trees. Transitions start engine animations.

mod computed;

pub use computed::{BoxShadow, ComputedBox, Overflow, TextAlign, TransitionSpec};

use std::sync::Arc;
use svelters_style::{DeclBlock, SheetId, StateMask, Stylist};

/// A node's style inputs: what the `class`, `id` and `style` attributes say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleInput {
    pub classes: Vec<Arc<str>>,
    pub id: Option<Arc<str>>,
    /// Inline declarations (`color: red; padding: 4px`).
    pub inline: Option<Arc<str>>,
}

/// One node's style column entry.
#[derive(Default)]
pub(crate) struct NodeStyle {
    pub input: Option<Box<StyleInput>>,
    /// The parsed inline block (from `input.inline`).
    pub inline: Option<Arc<DeclBlock>>,
    /// The engine's computed style (kept for inheritance and the inspector).
    pub css: Option<Arc<svelters_style::ComputedStyle>>,
    pub computed: Arc<ComputedBox>,
}

/// The document's style state: the stylist with its sheets, and the per-node column.
pub(crate) struct DocStyle {
    pub stylist: Stylist,
    pub nodes: Vec<NodeStyle>,
    pub default: Arc<ComputedBox>,
    pub caches: svelters_style::MatchCaches,
}

impl Default for DocStyle {
    fn default() -> Self {
        DocStyle {
            stylist: Stylist::new(&svelters_style::tables::EGUI),
            nodes: Vec::new(),
            default: Arc::new(ComputedBox::default()),
            caches: svelters_style::MatchCaches::new(),
        }
    }
}

impl DocStyle {
    pub fn grow(&mut self) {
        self.nodes.push(NodeStyle { computed: self.default.clone(), ..Default::default() });
    }
    pub fn reset(&mut self, i: usize) {
        self.nodes[i] = NodeStyle { computed: self.default.clone(), ..Default::default() };
    }
    pub fn computed(&self, i: usize) -> &ComputedBox {
        &self.nodes[i].computed
    }
    pub fn input(&self, i: usize) -> Option<&StyleInput> {
        self.nodes[i].input.as_deref()
    }
    /// Stores a node's inputs; false when they are unchanged.
    pub fn set_input(&mut self, i: usize, input: StyleInput) -> bool {
        let n = &mut self.nodes[i];
        if n.input.as_deref() == Some(&input) {
            return false;
        }
        n.inline = input.inline.as_deref().map(|src| Arc::new(svelters_style::DeclBlock::parse_inline(src).0));
        n.input = Some(Box::new(input));
        true
    }
    /// A prop was written: attribute selectors may match it (the restyle re-matches the node).
    pub fn prop_written(&mut self, _i: usize) {}
    pub fn add_sheet(&mut self, css: &str) -> SheetId {
        let sheet = svelters_style::Stylesheet::parse(css, &Default::default());
        let (id, _diags) = self.stylist.add_sheet(&sheet, svelters_style::Origin::Author);
        self.caches.clear();
        id
    }
    pub fn remove_sheet(&mut self, id: SheetId) {
        self.stylist.remove_sheet(id);
        self.caches.clear();
    }
    /// Whether a change of `changed` state bits can restyle descendants (`:hover .x`, `:has`).
    pub fn state_affects_descendants(&self, _changed: StateMask) -> bool {
        true
    }
}
