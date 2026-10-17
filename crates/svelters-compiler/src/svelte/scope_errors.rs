//! Port of the errors of `svelte/src/compiler/errors.js` (5.57.2 @ f3de3f2) that scope creation
//! (`phases/scope.js`) and the scope-related slices of the analysis phase ported in
//! `svelte::scope::analyze` raise. Owner: agent E.
//!
//! Upstream `e(node, code, message)` throws an `InternalCompileError` with `[node.start, node.end]`;
//! here each constructor returns the [`CompileError`] (message without the docs URL, as the
//! `CompileError` contract says). A `None` span (upstream `node` null/undefined) becomes 0..0.

use crate::estree::Span;
use crate::svelte::errors::CompileError;

fn e(span: Option<Span>, code: &'static str, message: String) -> CompileError {
    let span = span.unwrap_or_default();
    CompileError {
        code,
        message,
        start: span.start,
        end: span.end,
    }
}

/// `%name%` has already been declared
pub fn declaration_duplicate(node: Option<Span>, name: &str) -> CompileError {
    e(
        node,
        "declaration_duplicate",
        format!("`{name}` has already been declared"),
    )
}

/// Cannot declare a variable with the same name as an import from `<script module>`
pub fn declaration_duplicate_module_import(node: Option<Span>) -> CompileError {
    e(
        node,
        "declaration_duplicate_module_import",
        "Cannot declare a variable with the same name as an import from `<script module>`".to_string(),
    )
}

/// The $ name is reserved, and cannot be used for variables and imports
pub fn dollar_binding_invalid(node: Option<Span>) -> CompileError {
    e(
        node,
        "dollar_binding_invalid",
        "The $ name is reserved, and cannot be used for variables and imports".to_string(),
    )
}

/// The $ prefix is reserved, and cannot be used for variables and imports
pub fn dollar_prefix_invalid(node: Option<Span>) -> CompileError {
    e(
        node,
        "dollar_prefix_invalid",
        "The $ prefix is reserved, and cannot be used for variables and imports".to_string(),
    )
}

/// `%name%` is an illegal variable name. To reference a global variable called `%name%`, use `globalThis.%name%`
pub fn global_reference_invalid(node: Option<Span>, name: &str) -> CompileError {
    e(
        node,
        "global_reference_invalid",
        format!(
            "`{name}` is an illegal variable name. To reference a global variable called `{name}`, use `globalThis.{name}`"
        ),
    )
}

/// Declaring or accessing a prop starting with `$$` is illegal (they are reserved for Svelte internals)
pub fn props_illegal_name(node: Option<Span>) -> CompileError {
    e(
        node,
        "props_illegal_name",
        "Declaring or accessing a prop starting with `$$` is illegal (they are reserved for Svelte internals)"
            .to_string(),
    )
}

/// `$props()` can only be used with an object destructuring pattern
pub fn props_invalid_identifier(node: Option<Span>) -> CompileError {
    e(
        node,
        "props_invalid_identifier",
        "`$props()` can only be used with an object destructuring pattern".to_string(),
    )
}

/// `$props()` assignment must not contain nested properties or computed keys
pub fn props_invalid_pattern(node: Option<Span>) -> CompileError {
    e(
        node,
        "props_invalid_pattern",
        "`$props()` assignment must not contain nested properties or computed keys".to_string(),
    )
}

/// Cannot use `%rune%` rune in non-runes mode
pub fn rune_invalid_usage(node: Option<Span>, rune: &str) -> CompileError {
    e(
        node,
        "rune_invalid_usage",
        format!("Cannot use `{rune}` rune in non-runes mode"),
    )
}

/// Cannot subscribe to stores that are not declared at the top level of the component
pub fn store_invalid_scoped_subscription(node: Option<Span>) -> CompileError {
    e(
        node,
        "store_invalid_scoped_subscription",
        "Cannot subscribe to stores that are not declared at the top level of the component".to_string(),
    )
}

/// Cannot reference store value inside `<script module>`
pub fn store_invalid_subscription(node: Option<Span>) -> CompileError {
    e(
        node,
        "store_invalid_subscription",
        "Cannot reference store value inside `<script module>`".to_string(),
    )
}

/// Cannot reference store value outside a `.svelte` file
pub fn store_invalid_subscription_module(node: Option<Span>) -> CompileError {
    e(
        node,
        "store_invalid_subscription_module",
        "Cannot reference store value outside a `.svelte` file".to_string(),
    )
}
