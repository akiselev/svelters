//! skelter's component gallery (GAL1, `plans/cloud/gallery-b1/`): every kind's stories, rendered
//! headlessly in every state, theme, density and zoom, for people and agents.
//!
//! | Module | What |
//! |---|---|
//! | [`model`] | [`Story`], [`Variant`], [`State`], [`Theme`], [`Cell`] |
//! | [`catalog`] | `story.toml` parsing, [`discover`], [`Catalog`] and its kind [`Coverage`] |
//! | [`matrix`] | [`Matrix`]: a story × [`Axes`], in a stable order |
//! | [`render`] | [`Renderer`]: the headless renderer (one engine and window for every cell), [`Shot`] |
//! | [`sheet`] | contact sheets: [`Sheet`], [`SheetLayout`], the cell index |
//! | [`inspect`] | one node of a cell as JSON: box, computed style, matched rules, forced state, AccessKit |
//! | [`check`] | every variant compiles and renders one cell: [`check::CheckReport`] |
//! | [`cli`] | [`cli_main`]: the `skelter-gallery` CLI |
//! | [`diff`] | `--diff`: a before / after / changes picture of two images |
//! | [`fonts`] | the bundled font set (Inter, JetBrains Mono, Arabic, CJK and symbol fallbacks) |
//! | [`icons`] | the demo icon set and the story assets (`asset://` imports) |
//! | [`ext`] | [`Extensions`]: what a host adds (story roots, kinds, setup hooks, token sets) |
//!
//! The story format is `gallery/README.md`; the agent loop and the CLI are
//! `docs/skelter/gallery.md`.

pub mod catalog;
pub mod check;
pub mod cli;
pub mod diff;
pub mod ext;
pub mod fonts;
pub mod icons;
pub mod matrix;
pub mod inspect;
pub mod model;
pub mod render;
pub mod sheet;

pub use catalog::{Catalog, Coverage, StoryError, discover, discover_with};
pub use ext::{BackendHook, BusyHook, Extensions, FrameCx, FrameHook, RegisterKinds, SetupCx, SetupHook};
pub use matrix::{Axes, Matrix};
pub use model::{canonicalize, Cell, FAMILIES, State, Story, StoryId, Theme, Variant, VariantId};
pub use render::{Error, Reload, Renderer, Shot, png, save_png};
pub use cli::cli_main;
pub use sheet::{Sheet, SheetCell, SheetLayout};
