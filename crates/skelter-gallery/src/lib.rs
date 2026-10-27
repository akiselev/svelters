//! skelter's component gallery (GAL1, `plans/cloud/gallery-b1/`): every kind's stories, rendered
//! headlessly in every state, theme, density and zoom, for people and agents.
//!
//! | Module | What |
//! |---|---|
//! | [`model`] | [`Story`], [`Variant`], [`State`], [`Theme`], [`Cell`] |
//! | [`catalog`] | `story.toml` parsing, [`discover`], [`Catalog`] and its kind [`Coverage`] |
//! | [`matrix`] | [`Matrix`]: a story × [`Axes`], in a stable order |
//! | [`render`] | [`Renderer`]: the headless renderer (one engine and window for every cell), [`Shot`] |
//! | [`cli`] | [`cli_main`]: the `skelter-gallery` CLI |
//! | [`ext`] | [`Extensions`]: what a host adds (story roots, kinds, setup hooks, token sets) |
//!
//! The story format is `gallery/README.md`; the agent loop and the CLI are
//! `docs/skelter/gallery.md`.

pub mod catalog;
pub mod cli;
pub mod ext;
pub mod matrix;
pub mod model;
pub mod render;

pub use catalog::{Catalog, Coverage, StoryError, discover, discover_with};
pub use ext::{Extensions, SetupCx};
pub use matrix::{Axes, Matrix};
pub use model::{Cell, FAMILIES, State, Story, StoryId, Theme, Variant, VariantId};
pub use render::{Error, Renderer, Shot, png, save_png};
pub use cli::cli_main;
