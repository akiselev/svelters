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
mod shorthand;
mod state;
mod table;
mod transition;
/// The generated style tables of the five toolkits.
#[path = "tables.generated.rs"]
pub mod tables;
/// U0: the generated utility-class table (`tools/metadata/style/gen_utilities.py`); its types are
/// in [`utility_table`].
#[path = "utilities.generated.rs"]
pub mod utilities;
pub mod utility_table;
mod values;
mod var;
mod view;

pub use atom::Atom;
pub use cascade::{MatchCaches, MatchedRule, Origin, RuleId, SheetId, Stylist, matches, select};
pub use computed::{ComputeContext, ComputedStyle};
pub use diag::{Diagnostic, Severity};
pub use element::Element;
pub use invalidation::{Invalidation, Reach, for_each_reached};
pub use media::{ColorScheme, Contrast, MEDIA_FEATURES, MediaEnv, MediaList};
pub use selector::{Impl, Logical, PseudoClass, PseudoElement, Selector, SelectorList, SelectorParser, parse_selector_list};
pub use sheet::{DeclBlock, Declaration, ParseOptions, Rule, Stylesheet};
pub use state::{STATE_PSEUDO_CLASSES, StateMask, StateNames};
pub use shorthand::LINE_STYLES;
pub use table::{Dialect, Prop, Table, ValueType, Via};
pub use transition::Transitions;
pub use view::{BorderSide, Outline, Overlay, Side, StyleView, TextSettings, overlay_get, overlay_get_index};
pub use values::{Color, ComputedValue, Content, Gradient, GradientStop, Length, LinearGradient, RadialExtent, RadialGradient, Shadow, TimingFunction, Transition};
