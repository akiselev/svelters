//! svelters-compiler (experimental, X1): a Rust Svelte compiler that emits JavaScript for the
//! svelters host, lowered through per-backend native semantic specifications.
//!
//! Module map and ownership: docs/notes/native-compiler/modules.md.

pub mod acorn;
pub mod diag;
pub mod estree;
pub mod semantics;
pub mod style;
pub mod svelte;
