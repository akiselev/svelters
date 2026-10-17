//! Small helpers `phases/scope.js` imports from other upstream files (5.57.2 @ f3de3f2):
//! - `svelte/src/utils.js`: `RESERVED_WORDS`/`is_reserved`, `RUNES`/`is_rune`,
//!   `STATE_CREATION_RUNES`/`is_state_creation_rune`
//! - `compiler/phases/nodes.js`: `is_element_node`, `is_component_node`
//! - `compiler/utils/slot.js`: `determine_slot`
//! - `compiler/utils/ast.js`: `is_text_attribute`

use super::walk::NodeRef;
use crate::svelte::ast::{Attribute, AttributeLike, AttributeValue, TextOrExpressionTag};

const RESERVED_WORDS: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Returns `true` if `word` is a reserved JavaScript keyword
pub fn is_reserved(word: &str) -> bool {
    RESERVED_WORDS.contains(&word)
}

const STATE_CREATION_RUNES: &[&str] = &["$state", "$state.raw", "$derived", "$derived.by"];

pub const RUNES: &[&str] = &[
    "$state",
    "$state.raw",
    "$derived",
    "$derived.by",
    "$state.eager",
    "$state.snapshot",
    "$props",
    "$props.id",
    "$bindable",
    "$effect",
    "$effect.pre",
    "$effect.tracking",
    "$effect.root",
    "$effect.pending",
    "$inspect",
    "$inspect().with",
    "$inspect.trace",
    "$host",
];

pub fn is_rune(name: &str) -> bool {
    RUNES.contains(&name)
}

/// The `'static` spelling of a rune name (`None` if `name` is not a rune).
pub fn as_rune(name: &str) -> Option<&'static str> {
    RUNES.iter().copied().find(|r| *r == name)
}

pub fn is_state_creation_rune(name: &str) -> bool {
    STATE_CREATION_RUNES.contains(&name)
}

/// Returns true for all nodes that can appear elsewhere than the top level, have attributes and can contain children
pub fn is_element_node(node: NodeRef<'_>) -> bool {
    matches!(
        node,
        NodeRef::SvelteElement(_)
            | NodeRef::RegularElement(_)
            | NodeRef::SvelteFragment(_)
            | NodeRef::Component(_)
            | NodeRef::SvelteComponent(_)
            | NodeRef::SvelteSelf(_)
            | NodeRef::SlotElement(_)
    )
}

/// Returns true for all component-like nodes
pub fn is_component_node(node: NodeRef<'_>) -> bool {
    matches!(
        node,
        NodeRef::Component(_) | NodeRef::SvelteComponent(_) | NodeRef::SvelteSelf(_)
    )
}

/// Returns true if the attribute contains a single static text node.
pub fn is_text_attribute(attribute: &Attribute) -> bool {
    matches!(&attribute.value, AttributeValue::Sequence(v) if v.len() == 1 && matches!(v[0], TextOrExpressionTag::Text(_)))
}

fn element_attributes<'a>(node: NodeRef<'a>) -> Option<&'a [AttributeLike]> {
    match node {
        NodeRef::SvelteElement(n) => Some(&n.attributes),
        NodeRef::SvelteComponent(n) => Some(&n.attributes),
        NodeRef::RegularElement(n)
        | NodeRef::SvelteFragment(n)
        | NodeRef::Component(n)
        | NodeRef::SvelteSelf(n)
        | NodeRef::SlotElement(n) => Some(&n.attributes),
        _ => None,
    }
}

/// `determine_slot(node)` (utils/slot.js).
pub fn determine_slot<'a>(node: NodeRef<'a>) -> Option<&'a str> {
    if !is_element_node(node) {
        return None;
    }

    for attribute in element_attributes(node)? {
        let AttributeLike::Attribute(attribute) = attribute else {
            continue;
        };
        if attribute.name != "slot" {
            continue;
        }
        if !is_text_attribute(attribute) {
            continue;
        }

        if let AttributeValue::Sequence(v) = &attribute.value
            && let TextOrExpressionTag::Text(t) = &v[0]
        {
            return Some(&t.data);
        }
    }

    None
}
