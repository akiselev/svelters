//! svelters' CSS engine (CSS.md option (c), milestone CS0).
//!
//! One Rust engine is the selector and cascade authority for every backend: it parses sheets
//! ([`Stylesheet`]), matches selectors against any tree that implements [`Element`], runs the
//! cascade (origins, `!important`, `@layer`, specificity, source order), resolves custom
//! properties, `var()` and `calc()`, evaluates `@media` against a [`MediaEnv`], and produces a
//! [`ComputedStyle`] typed by a backend's style table ([`tables`], generated from
//! `metadata/style/*.yaml` by `tools/metadata/style/gen_props.py`). [`Invalidation`] says which
//! nodes a class, id, attribute, state or structure change can restyle.
//!
//! It links no toolkit. The egui scene (CS5e) and the host's `StyleTree` (CS1) implement
//! [`Element`]; the compiler can link the parser for diagnostics (Q3).

mod atom;
mod cascade;
mod computed;
mod diag;
mod element;
mod invalidation;
mod media;
mod selector;
mod sheet;
mod state;
mod table;
/// The generated style tables of the five toolkits.
#[path = "tables.generated.rs"]
pub mod tables;
mod values;
mod var;

pub use atom::Atom;
pub use cascade::{MatchCaches, MatchedRule, Origin, RuleId, SheetId, Stylist, matches, select};
pub use computed::{ComputeContext, ComputedStyle};
pub use diag::{Diagnostic, Severity};
pub use element::Element;
pub use invalidation::{Invalidation, Reach, for_each_reached};
pub use media::{ColorScheme, Contrast, MediaEnv, MediaList};
pub use selector::{Impl, PseudoClass, Selector, SelectorList, SelectorParser, parse_selector_list};
pub use sheet::{DeclBlock, Declaration, ParseOptions, Rule, Stylesheet};
pub use state::{STATE_PSEUDO_CLASSES, StateMask, StateNames};
pub use table::{Dialect, Prop, Table, ValueType, Via};
pub use values::{Color, ComputedValue, Length};
