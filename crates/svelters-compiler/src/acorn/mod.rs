//! Port of acorn 8.19.0 (MIT, Marijn Haverbeke and contributors) producing `crate::estree`.
//! Owner: agent A (tokenizer + parser); `regexp.rs`: agent B. Contract: docs/notes/native-compiler/modules.md.

pub mod regexp;

use crate::estree::{Comment, Expression, Program, SourceType, Statement};

#[derive(Clone, Debug)]
pub struct Options {
    /// acorn `ecmaVersion`; Svelte uses 16 (ES2025).
    pub ecma_version: u32,
    pub source_type: SourceType,
    /// acorn `preserveParens`: Svelte sets it for template expressions.
    pub preserve_parens: bool,
    /// Svelte's `<script>` hack: exports of names not declared in the script are allowed
    /// (acorn's `undefinedExports` is cleared after every statement).
    pub allow_undefined_exports: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { ecma_version: 16, source_type: SourceType::Module, preserve_parens: false, allow_undefined_exports: false }
    }
}

/// acorn's SyntaxError: `message` includes acorn's ` (line:col)` suffix exactly as acorn does
/// (UTF-16 columns); `pos` is the byte offset of the error.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub pos: u32,
    pub raised_at: u32,
}

/// `acorn.parse(src, opts)`. Comments are appended to `comments` in source order (`onComment`).
pub fn parse(_src: &str, _opts: &Options, _comments: &mut Vec<Comment>) -> Result<Program, ParseError> {
    todo!("agent A")
}

/// `acorn.parseExpressionAt(src, pos, opts)`: parses one expression starting at byte offset
/// `pos` of the full source (positions stay absolute); the caller reads `end` to continue.
pub fn parse_expression_at(_src: &str, _pos: u32, _opts: &Options, _comments: &mut Vec<Comment>) -> Result<Expression, ParseError> {
    todo!("agent A")
}

/// Svelte's `parse_statement_at`: one statement starting at `pos`.
pub fn parse_statement_at(_src: &str, _pos: u32, _opts: &Options, _comments: &mut Vec<Comment>) -> Result<Statement, ParseError> {
    todo!("agent A")
}

/// acorn `isIdentifierStart(code, astral=true)`.
pub fn is_identifier_start(_c: char) -> bool {
    todo!("agent A")
}

/// acorn `isIdentifierChar(code, astral=true)`.
pub fn is_identifier_char(_c: char) -> bool {
    todo!("agent A")
}
