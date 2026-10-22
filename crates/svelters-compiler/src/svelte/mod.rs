//! Verbatim port of Svelte's compiler front end (Svelte 5.57.2 @ f3de3f2).

pub mod ast;
pub mod css;
pub mod errors;
pub mod parse;
pub mod scope;
pub mod scope_errors;
pub mod ts;
