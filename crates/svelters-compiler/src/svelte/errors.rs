//! Port of the parser-related errors of `svelte/src/compiler/errors.js`. Owner: agent D.

/// A Svelte CompileError: `code` is Svelte's error code (e.g. `expected_token`), `message` its
/// exact text (without the docs URL), span in byte offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub code: &'static str,
    pub message: String,
    pub start: u32,
    pub end: u32,
}
