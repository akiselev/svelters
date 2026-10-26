//! Style: svelters-style's cascade run on the document (CSS.md CS5e: the tree implements
//! [`svelters_style::Element`] directly), producing a [`ComputedBox`] and a `taffy::Style` per node.
//! A restyle compares the new computed values with the old ones and marks only the work their change
//! classes need (architecture §5): layout properties relayout, paint properties re-record, compositor
//! properties (opacity, transform) update the property trees. Transitions start engine animations.

pub(crate) mod cascade;
mod computed;
mod inspect;
mod invalidate;
pub mod table;

pub use computed::{BoxShadow, ComputedBox, Overflow, TextAlign, TransitionSpec};

use std::sync::Arc;
use svelters_style::{DeclBlock, SheetId, Stylist};

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
    /// The computed style of each of the kind's paint parts ([`KindSpec::parts`](crate::KindSpec::parts)).
    pub parts: Vec<(&'static str, Arc<svelters_style::ComputedStyle>)>,
    /// State bits a tool forces for matching ([`crate::Document::force_state`]), ORed over the node's.
    pub forced: svelters_style::StateMask,
}

/// What an inspector shows about a sheet ([`crate::Document::set_sheet_info`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct SheetInfo {
    pub name: Option<String>,
    pub source: Option<Arc<svelters_style::SheetSource>>,
}

/// The document's style state: the stylist with its sheets, and the per-node column.
pub(crate) struct DocStyle {
    pub stylist: Stylist,
    pub nodes: Vec<NodeStyle>,
    pub default: Arc<ComputedBox>,
    pub caches: svelters_style::MatchCaches,
    /// Resolves `:state(ns.name)` in sheets to the host's state bits ([`crate::Document::set_state_names`]).
    pub state_names: Option<Box<dyn svelters_style::StateNames>>,
    /// Sheet names and (dev) their `<style>` sources, for matched rules.
    pub sheet_info: std::collections::HashMap<SheetId, SheetInfo>,
}

impl Default for DocStyle {
    fn default() -> Self {
        DocStyle {
            stylist: Stylist::new(table::table()),
            nodes: Vec::new(),
            default: Arc::new(ComputedBox::default()),
            caches: svelters_style::MatchCaches::new(),
            state_names: None,
            sheet_info: std::collections::HashMap::new(),
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
    pub fn add_sheet(&mut self, css: &str) -> SheetId {
        let opts = svelters_style::ParseOptions { states: self.state_names.as_deref() };
        let sheet = svelters_style::Stylesheet::parse(css, &opts);
        self.add_parsed(&sheet)
    }
    pub fn add_parsed(&mut self, sheet: &svelters_style::Stylesheet) -> SheetId {
        let (id, _diags) = self.stylist.add_sheet(sheet, svelters_style::Origin::Author);
        self.caches.clear();
        id
    }
    pub fn preload_utilities(&mut self, utilities: &[svelters_style::Utility]) -> usize {
        let n = self.stylist.preload_utilities(utilities, &Default::default());
        self.caches.clear();
        n
    }
    pub fn remove_sheet(&mut self, id: SheetId) {
        self.stylist.remove_sheet(id);
        self.sheet_info.remove(&id);
        self.caches.clear();
    }
}
