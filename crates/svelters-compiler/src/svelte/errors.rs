//! Port of the parser-related errors (and the three parser warnings) of
//! `svelte/src/compiler/errors.js` / `warnings.js` (Svelte 5.57.2 @ f3de3f2). Owner: agent D.
//!
//! Upstream throws an `InternalCompileError`; the port returns the error value and the caller
//! propagates it with `Err(..)?`. `message` is upstream's text without the trailing
//! `\nhttps://svelte.dev/e/<code>` docs link (see [`CompileError::docs_url`]).

use crate::estree::{Span, Spanned};

/// A Svelte CompileError: `code` is Svelte's error code (e.g. `expected_token`), `message` its
/// exact text (without the docs URL), span in byte offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub code: &'static str,
    pub message: String,
    pub start: u32,
    pub end: u32,
}

impl CompileError {
    /// The link upstream appends to `message` (`\n` + this).
    pub fn docs_url(&self) -> String {
        format!("https://svelte.dev/e/{}", self.code)
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for CompileError {}

/// A warning emitted while parsing (`warnings.js`); same shape as [`CompileError`].
/// `svelte-ignore` filtering is not applied here (upstream's ignore stack is empty while parsing;
/// comments are honoured later by analysis).
#[derive(Clone, Debug, PartialEq)]
pub struct CompileWarning {
    pub code: &'static str,
    pub message: String,
    pub start: u32,
    pub end: u32,
}

/// `null | number | NodeLike` of upstream's `e(node, code, message)`: a position or a span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeLike {
    pub start: u32,
    pub end: u32,
}

impl From<u32> for NodeLike {
    fn from(p: u32) -> Self {
        NodeLike { start: p, end: p }
    }
}

impl From<usize> for NodeLike {
    fn from(p: usize) -> Self {
        NodeLike { start: p as u32, end: p as u32 }
    }
}

impl From<Span> for NodeLike {
    fn from(s: Span) -> Self {
        NodeLike { start: s.start, end: s.end }
    }
}

impl From<(u32, u32)> for NodeLike {
    fn from((start, end): (u32, u32)) -> Self {
        NodeLike { start, end }
    }
}

impl From<(usize, usize)> for NodeLike {
    fn from((start, end): (usize, usize)) -> Self {
        NodeLike { start: start as u32, end: end as u32 }
    }
}

impl<T: Spanned> From<&T> for NodeLike {
    fn from(n: &T) -> Self {
        n.span().into()
    }
}

fn e(node: impl Into<NodeLike>, code: &'static str, message: String) -> CompileError {
    let n = node.into();
    CompileError { code, message, start: n.start, end: n.end }
}

pub(crate) fn w(node: impl Into<NodeLike>, code: &'static str, message: String) -> CompileWarning {
    let n = node.into();
    CompileWarning { code, message, start: n.start, end: n.end }
}

// ---------------------------------------------------------------------------------------------
// errors (alphabetical, as in errors.js)

/// Attributes need to be unique
pub fn attribute_duplicate(node: impl Into<NodeLike>) -> CompileError {
    e(node, "attribute_duplicate", "Attributes need to be unique".into())
}

/// Attribute shorthand cannot be empty
pub fn attribute_empty_shorthand(node: impl Into<NodeLike>) -> CompileError {
    e(node, "attribute_empty_shorthand", "Attribute shorthand cannot be empty".into())
}

/// %name% cannot appear more than once within a block
pub fn block_duplicate_clause(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "block_duplicate_clause", format!("{name} cannot appear more than once within a block"))
}

/// {:...} block is invalid at this position (did you forget to close the preceding element or block?)
pub fn block_invalid_continuation_placement(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "block_invalid_continuation_placement",
        "{:...} block is invalid at this position (did you forget to close the preceding element or block?)".into(),
    )
}

/// 'elseif' should be 'else if'
pub fn block_invalid_elseif(node: impl Into<NodeLike>) -> CompileError {
    e(node, "block_invalid_elseif", "'elseif' should be 'else if'".into())
}

/// {#%name% ...} block cannot be %location%
pub fn block_invalid_placement(node: impl Into<NodeLike>, name: &str, location: &str) -> CompileError {
    e(node, "block_invalid_placement", format!("{{#{name} ...}} block cannot be {location}"))
}

/// Block was left open
pub fn block_unclosed(node: impl Into<NodeLike>) -> CompileError {
    e(node, "block_unclosed", "Block was left open".into())
}

/// Unexpected block closing tag
pub fn block_unexpected_close(node: impl Into<NodeLike>) -> CompileError {
    e(node, "block_unexpected_close", "Unexpected block closing tag".into())
}

/// {@const ...} must consist of a single variable declaration
pub fn const_tag_invalid_expression(node: impl Into<NodeLike>) -> CompileError {
    e(node, "const_tag_invalid_expression", "{@const ...} must consist of a single variable declaration".into())
}

/// {@debug ...} arguments must be identifiers, not arbitrary expressions
pub fn debug_tag_invalid_arguments(node: impl Into<NodeLike>) -> CompileError {
    e(node, "debug_tag_invalid_arguments", "{@debug ...} arguments must be identifiers, not arbitrary expressions".into())
}

/// Declaration tags must be `let` or `const` declarations
pub fn declaration_tag_invalid_type(node: impl Into<NodeLike>) -> CompileError {
    e(node, "declaration_tag_invalid_type", "Declaration tags must be `let` or `const` declarations".into())
}

/// Directive value must be a JavaScript expression enclosed in curly braces
pub fn directive_invalid_value(node: impl Into<NodeLike>) -> CompileError {
    e(node, "directive_invalid_value", "Directive value must be a JavaScript expression enclosed in curly braces".into())
}

/// `%type%` name cannot be empty
pub fn directive_missing_name(node: impl Into<NodeLike>, type_: &str) -> CompileError {
    e(node, "directive_missing_name", format!("`{type_}` name cannot be empty"))
}

/// `</%name%>` attempted to close an element that was not open
pub fn element_invalid_closing_tag(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "element_invalid_closing_tag", format!("`</{name}>` attempted to close an element that was not open"))
}

/// `</%name%>` attempted to close element that was already automatically closed by `<%reason%>` (cannot nest `<%reason%>` inside `<%name%>`)
pub fn element_invalid_closing_tag_autoclosed(node: impl Into<NodeLike>, name: &str, reason: &str) -> CompileError {
    e(
        node,
        "element_invalid_closing_tag_autoclosed",
        format!(
            "`</{name}>` attempted to close element that was already automatically closed by `<{reason}>` (cannot nest `<{reason}>` inside `<{name}>`)"
        ),
    )
}

/// `<%name%>` was left open
pub fn element_unclosed(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "element_unclosed", format!("`<{name}>` was left open"))
}

/// Expected attribute value
pub fn expected_attribute_value(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_attribute_value", "Expected attribute value".into())
}

/// Expected 'if', 'each', 'await', 'key' or 'snippet'
pub fn expected_block_type(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_block_type", "Expected 'if', 'each', 'await', 'key' or 'snippet'".into())
}

/// Expected an identifier
pub fn expected_identifier(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_identifier", "Expected an identifier".into())
}

/// Expected identifier or destructure pattern
pub fn expected_pattern(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_pattern", "Expected identifier or destructure pattern".into())
}

/// Expected 'html', 'render', 'attach', 'const', or 'debug'
pub fn expected_tag(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_tag", "Expected 'html', 'render', 'attach', 'const', or 'debug'".into())
}

/// Expected token %token%
pub fn expected_token(node: impl Into<NodeLike>, token: &str) -> CompileError {
    e(node, "expected_token", format!("Expected token {token}"))
}

/// Expected whitespace
pub fn expected_whitespace(node: impl Into<NodeLike>) -> CompileError {
    e(node, "expected_whitespace", "Expected whitespace".into())
}

/// %message%
pub fn js_parse_error(node: impl Into<NodeLike>, message: &str) -> CompileError {
    e(node, "js_parse_error", message.to_string())
}

/// `{@render ...}` tags can only contain call expressions
pub fn render_tag_invalid_expression(node: impl Into<NodeLike>) -> CompileError {
    e(node, "render_tag_invalid_expression", "`{@render ...}` tags can only contain call expressions".into())
}

/// A component can have a single top-level `<script>` element and/or a single top-level `<script module>` element
pub fn script_duplicate(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "script_duplicate",
        "A component can have a single top-level `<script>` element and/or a single top-level `<script module>` element".into(),
    )
}

/// If the `%name%` attribute is supplied, it must be a boolean attribute
pub fn script_invalid_attribute_value(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "script_invalid_attribute_value", format!("If the `{name}` attribute is supplied, it must be a boolean attribute"))
}

/// If the context attribute is supplied, its value must be "module"
pub fn script_invalid_context(node: impl Into<NodeLike>) -> CompileError {
    e(node, "script_invalid_context", "If the context attribute is supplied, its value must be \"module\"".into())
}

/// The `%name%` attribute is reserved and cannot be used
pub fn script_reserved_attribute(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "script_reserved_attribute", format!("The `{name}` attribute is reserved and cannot be used"))
}

/// A component can have a single top-level `<style>` element
pub fn style_duplicate(node: impl Into<NodeLike>) -> CompileError {
    e(node, "style_duplicate", "A component can have a single top-level `<style>` element".into())
}

/// Invalid component definition — must be an `{expression}`
pub fn svelte_component_invalid_this(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_component_invalid_this", "Invalid component definition — must be an `{expression}`".into())
}

/// `<svelte:component>` must have a 'this' attribute
pub fn svelte_component_missing_this(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_component_missing_this", "`<svelte:component>` must have a 'this' attribute".into())
}

/// `<svelte:element>` must have a 'this' attribute with a value
pub fn svelte_element_missing_this(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_element_missing_this", "`<svelte:element>` must have a 'this' attribute with a value".into())
}

/// A component can only have one `<%name%>` element
pub fn svelte_meta_duplicate(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "svelte_meta_duplicate", format!("A component can only have one `<{name}>` element"))
}

/// <%name%> cannot have children
pub fn svelte_meta_invalid_content(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "svelte_meta_invalid_content", format!("<{name}> cannot have children"))
}

/// `<%name%>` tags cannot be inside elements or blocks
pub fn svelte_meta_invalid_placement(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "svelte_meta_invalid_placement", format!("`<{name}>` tags cannot be inside elements or blocks"))
}

/// Valid `<svelte:...>` tag names are %list%
pub fn svelte_meta_invalid_tag(node: impl Into<NodeLike>, list: &str) -> CompileError {
    e(node, "svelte_meta_invalid_tag", format!("Valid `<svelte:...>` tag names are {list}"))
}

/// "tag" option is deprecated — use "customElement" instead
pub fn svelte_options_deprecated_tag(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_options_deprecated_tag", "\"tag\" option is deprecated — use \"customElement\" instead".into())
}

/// `<svelte:options>` can only receive static attributes
pub fn svelte_options_invalid_attribute(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_options_invalid_attribute", "`<svelte:options>` can only receive static attributes".into())
}

/// Value must be %list%, if specified
pub fn svelte_options_invalid_attribute_value(node: impl Into<NodeLike>, list: &str) -> CompileError {
    e(node, "svelte_options_invalid_attribute_value", format!("Value must be {list}, if specified"))
}

/// "customElement" must be a string literal defining a valid custom element name or an object of the form ...
pub fn svelte_options_invalid_customelement(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "svelte_options_invalid_customelement",
        "\"customElement\" must be a string literal defining a valid custom element name or an object of the form { tag?: string; shadow?: \"open\" | \"none\" | `ShadowRootInit`; props?: { [key: string]: { attribute?: string; reflect?: boolean; type: .. } } }".into(),
    )
}

/// "props" must be a statically analyzable object literal of the form ...
pub fn svelte_options_invalid_customelement_props(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "svelte_options_invalid_customelement_props",
        "\"props\" must be a statically analyzable object literal of the form \"{ [key: string]: { attribute?: string; reflect?: boolean; type?: \"String\" | \"Boolean\" | \"Number\" | \"Array\" | \"Object\" }\"".into(),
    )
}

/// "shadow" must be either "open", "none" or `ShadowRootInit` object.
pub fn svelte_options_invalid_customelement_shadow(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "svelte_options_invalid_customelement_shadow",
        "\"shadow\" must be either \"open\", \"none\" or `ShadowRootInit` object.".into(),
    )
}

/// Tag name must be lowercase and hyphenated
pub fn svelte_options_invalid_tagname(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_options_invalid_tagname", "Tag name must be lowercase and hyphenated".into())
}

/// Tag name is reserved
pub fn svelte_options_reserved_tagname(node: impl Into<NodeLike>) -> CompileError {
    e(node, "svelte_options_reserved_tagname", "Tag name is reserved".into())
}

/// `<svelte:options>` unknown attribute '%name%'
pub fn svelte_options_unknown_attribute(node: impl Into<NodeLike>, name: &str) -> CompileError {
    e(node, "svelte_options_unknown_attribute", format!("`<svelte:options>` unknown attribute '{name}'"))
}

/// Expected a valid element or component name. Components must have a valid variable name or dot notation expression
pub fn tag_invalid_name(node: impl Into<NodeLike>) -> CompileError {
    e(
        node,
        "tag_invalid_name",
        "Expected a valid element or component name. Components must have a valid variable name or dot notation expression".into(),
    )
}

/// {@%name% ...} tag cannot be %location%
pub fn tag_invalid_placement(node: impl Into<NodeLike>, name: &str, location: &str) -> CompileError {
    e(node, "tag_invalid_placement", format!("{{@{name} ...}} tag cannot be {location}"))
}

/// Unexpected end of input
pub fn unexpected_eof(node: impl Into<NodeLike>) -> CompileError {
    e(node, "unexpected_eof", "Unexpected end of input".into())
}

/// '%word%' is a reserved word in JavaScript and cannot be used here
pub fn unexpected_reserved_word(node: impl Into<NodeLike>, word: &str) -> CompileError {
    e(node, "unexpected_reserved_word", format!("'{word}' is a reserved word in JavaScript and cannot be used here"))
}

/// Unterminated string constant
pub fn unterminated_string_constant(node: impl Into<NodeLike>) -> CompileError {
    e(node, "unterminated_string_constant", "Unterminated string constant".into())
}

/// Void elements cannot have children or closing tags
pub fn void_element_invalid_content(node: impl Into<NodeLike>) -> CompileError {
    e(node, "void_element_invalid_content", "Void elements cannot have children or closing tags".into())
}

/// TypeScript language features like %feature% are not natively supported, and their use is generally discouraged. Outside of `<script>` tags, these features are not supported. For use within `<script>` tags, you will need to use a preprocessor to convert it to JavaScript before it gets passed to the Svelte compiler. If you are using `vitePreprocess`, make sure to specifically enable preprocessing script tags (`vitePreprocess({ script: true })`)
/// (raised by `remove_typescript_nodes`, i.e. at compile time, not by `parse`)
pub fn typescript_invalid_feature(node: impl Into<NodeLike>, feature: &str) -> CompileError {
    e(
        node,
        "typescript_invalid_feature",
        format!("TypeScript language features like {feature} are not natively supported, and their use is generally discouraged. Outside of `<script>` tags, these features are not supported. For use within `<script>` tags, you will need to use a preprocessor to convert it to JavaScript before it gets passed to the Svelte compiler. If you are using `vitePreprocess`, make sure to specifically enable preprocessing script tags (`vitePreprocess({{ script: true }})`)"),
    )
}

// ---------------------------------------------------------------------------------------------
// warnings emitted by the parser (warnings.js)

/// This element is implicitly closed by the following `%tag%`, which can cause an unexpected DOM structure. Add an explicit `%closing%` to avoid surprises.
pub fn element_implicitly_closed(node: impl Into<NodeLike>, tag: &str, closing: &str) -> CompileWarning {
    w(
        node,
        "element_implicitly_closed",
        format!(
            "This element is implicitly closed by the following `{tag}`, which can cause an unexpected DOM structure. Add an explicit `{closing}` to avoid surprises."
        ),
    )
}

/// Unrecognised attribute — should be one of `generics`, `lang` or `module`. If this exists for a preprocessor, ensure that the preprocessor removes it
pub fn script_unknown_attribute(node: impl Into<NodeLike>) -> CompileWarning {
    w(
        node,
        "script_unknown_attribute",
        "Unrecognised attribute — should be one of `generics`, `lang` or `module`. If this exists for a preprocessor, ensure that the preprocessor removes it".into(),
    )
}

/// `this` should be an `{expression}`. Using a string attribute value will cause an error in future versions of Svelte
pub fn svelte_element_invalid_this(node: impl Into<NodeLike>) -> CompileWarning {
    w(
        node,
        "svelte_element_invalid_this",
        "`this` should be an `{expression}`. Using a string attribute value will cause an error in future versions of Svelte".into(),
    )
}
