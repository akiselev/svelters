//! skelter's component gallery (GAL1, `plans/cloud/gallery-b1/`): every kind's stories, rendered
//! headlessly in every state, theme, density and zoom, for people and agents.
//!
//! | Module | What |
//! |---|---|
//! | [`model`] | [`Story`], [`Variant`], [`State`], [`Theme`], [`Cell`] |
//! | [`catalog`] | `story.toml` parsing, [`discover`], [`Catalog`] and its kind [`Coverage`] |
//! | [`matrix`] | [`Matrix`]: a story × [`Axes`], in a stable order |
//! | [`ext`] | [`Extensions`]: what a host adds (story roots, kinds, setup hooks, token sets) |
//!
//! The story format is `gallery/README.md`; the agent loop and the CLI are
//! `docs/skelter/gallery.md`.

pub mod catalog;
pub mod ext;
pub mod matrix;
pub mod model;

pub use catalog::{Catalog, Coverage, StoryError, discover, discover_with};
pub use ext::{Extensions, SetupCx};
pub use matrix::{Axes, Matrix};
pub use model::{Cell, FAMILIES, State, Story, StoryId, Theme, Variant, VariantId};
