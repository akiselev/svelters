//! Parser helpers of `compiler/phases/nodes.js` (Svelte 5.57.2 @ f3de3f2). `create_fragment`
//! is `Parser::create_fragment` (fragments live in the parser's arena).

use crate::estree::Span;
use crate::svelte::ast::{Attribute, AttributeValue};
use crate::svelte::errors as e;
use crate::svelte::parse::{ElementType, FragId, PResult, Parser};

/// All nodes that can appear elsewhere than the top level, have attributes and can contain children
pub fn is_element_node(type_: ElementType) -> bool {
    matches!(
        type_,
        ElementType::SvelteElement
            | ElementType::RegularElement
            | ElementType::SvelteFragment
            | ElementType::Component
            | ElementType::SvelteComponent
            | ElementType::SvelteSelf
            | ElementType::SlotElement
    )
}

/// `disallow_children(node)`; takes the node's fragment and name.
pub fn disallow_children(parser: &Parser, fragment: FragId, name: &str) -> PResult<()> {
    let nodes = &parser.frags[fragment].nodes;

    if let (Some(first), Some(last)) = (nodes.first(), nodes.last()) {
        let start = parser.item_span(first).start;
        let end = parser.item_span(last).end;
        return Err(e::svelte_meta_invalid_content((start, end), name));
    }
    Ok(())
}

pub fn create_attribute(name: String, name_loc: Option<Span>, start: usize, end: usize, value: AttributeValue) -> Attribute {
    Attribute { start: start as u32, end: end as u32, name, name_span: name_loc, value }
}
