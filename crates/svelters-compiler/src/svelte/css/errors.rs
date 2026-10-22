//! The CSS errors and warnings of `errors.js` / `warnings.js` (Svelte 5.57.2), messages without
//! the docs URL line.

pub use crate::svelte::errors::{expected_token, unexpected_eof};
use crate::svelte::errors::{CompileError, NodeLike};

fn e(node: impl Into<NodeLike>, code: &'static str, message: &str) -> CompileError {
    let n = node.into();
    CompileError { code, message: message.to_string(), start: n.start, end: n.end }
}

pub fn css_empty_declaration(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_empty_declaration", "Declaration cannot be empty")
}
pub fn css_expected_identifier(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_expected_identifier", "Expected a valid CSS identifier")
}
pub fn css_global_block_invalid_combinator(node: impl Into<NodeLike>, name: &str) -> CompileError {
    let mut err = e(node, "css_global_block_invalid_combinator", "");
    err.message = format!("A `:global` selector cannot follow a `{name}` combinator");
    err
}
pub fn css_global_block_invalid_declaration(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_block_invalid_declaration", "A top-level `:global {...}` block can only contain rules, not declarations")
}
pub fn css_global_block_invalid_list(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_block_invalid_list", "A `:global` selector cannot be part of a selector list with entries that don't contain `:global`")
}
pub fn css_global_block_invalid_modifier(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_block_invalid_modifier", "A `:global` selector cannot modify an existing selector")
}
pub fn css_global_block_invalid_modifier_start(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_block_invalid_modifier_start", "A `:global` selector can only be modified if it is a descendant of other selectors")
}
pub fn css_global_block_invalid_placement(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_block_invalid_placement", "A `:global` selector cannot be inside a pseudoclass")
}
pub fn css_global_invalid_placement(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_invalid_placement", "`:global(...)` can be at the start or end of a selector sequence, but not in the middle")
}
pub fn css_global_invalid_selector(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_invalid_selector", "`:global(...)` must contain exactly one selector")
}
pub fn css_global_invalid_selector_list(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_global_invalid_selector_list", "`:global(...)` must not contain type or universal selectors when used in a compound selector")
}
pub fn css_nesting_selector_invalid_placement(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_nesting_selector_invalid_placement", "Nesting selectors can only be used inside a rule or as the first selector inside a lone `:global(...)`")
}
pub fn css_selector_invalid(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_selector_invalid", "Invalid selector")
}
pub fn css_type_selector_invalid_placement(node: impl Into<NodeLike>) -> CompileError {
    e(node, "css_type_selector_invalid_placement", "`:global(...)` must not be followed by a type selector")
}
