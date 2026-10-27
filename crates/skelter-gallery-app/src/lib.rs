//! skelter-gallery-app: the interactive gallery (GAL3, `plans/cloud/gallery-b1/LANE-app.md`).
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | [`app::run`]: the window on skelter's app runner, for a host's [`skelter_gallery::Extensions`] |
//! | [`document`] | [`document::GalleryDocument`]: input, frames, zoom and inspect clicks |
//! | [`gallery`] | [`Gallery`]: the state, the chrome (Rust on skelter), events, the source pane |
//! | [`live`] | [`live::LiveStory`]: the selected story's embed; overlay previews and in-place updates |
//! | [`theme`] | per-cell themes: a class per theme redeclaring its token sheet |
//! | [`ui`] | the chrome's skelter-widgets nodes |
pub mod app;
pub mod document;
pub mod gallery;
pub mod live;
pub mod theme;
pub mod ui;

pub use gallery::Gallery;
pub use skelter_quire;
pub use app::run;
pub use document::GalleryDocument;
