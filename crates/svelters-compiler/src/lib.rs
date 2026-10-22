//! svelters-compiler (experimental, NC1): a Rust Svelte compiler that emits JavaScript for the
//! svelters host, lowered through per-backend native semantic specifications.
//!
//! Module map and ownership: docs/notes/native-compiler/modules.md.

pub mod acorn;
pub mod check;
pub mod codegen;
pub mod codes;
pub mod compile;
pub mod diag;
pub mod estree;
mod fixits;
pub mod lower;
pub mod scoped_css;
pub mod semantics;
pub mod style;
pub mod svelte;
pub mod target;
pub mod transform;
