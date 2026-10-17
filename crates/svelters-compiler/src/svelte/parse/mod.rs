//! Port of `svelte/src/compiler/phases/1-parse` (5.57.2 @ f3de3f2). Owner: agent D.

use super::ast::Root;
use super::errors::CompileError;

#[derive(Clone, Debug, Default)]
pub struct ParseOptions {
    /// Svelte's `loose` mode (error-tolerant parsing for editors).
    pub loose: bool,
    /// Native element mode (svelters deviation): no HTML-only rules. Element names are native
    /// names: no void elements, no implicit closing (`<p>`, `<li>`...), no SVG/MathML by name.
    pub native: bool,
}

/// `parse(source, { modern: true })`.
pub fn parse(_src: &str, _opts: &ParseOptions) -> Result<Root, CompileError> {
    todo!("agent D")
}
