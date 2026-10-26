//! skelter's shared vocabulary.
//!
//! skelter is the retained, low-latency GUI toolkit that replaces egui as svelters' primary backend
//! (design: `plans/research/skelter/`). Every skelter crate speaks the types in this crate, so lanes can
//! build their crates in parallel against one contract:
//!
//! | Module | What |
//! |---|---|
//! | [`geom`] | points, sizes, rects, rounded rects, affine transforms, insets (device-independent `f32` points) |
//! | [`color`] | colours (sRGB-encoded, straight alpha), gradients |
//! | [`ids`] | node, window, layer, surface, run, image, animation ids; `InputSeq`, `Revision`, `FrameId` |
//! | [`time`] | the monotonic clock type all threads share |
//! | [`change`] | the four change classes (structure, layout, paint, property) |
//! | [`display`] | display lists: the primitives a node records |
//! | [`text`] | glyph runs and the [`text::GlyphRasterizer`] / [`text::TextMeasure`] boundaries |
//! | [`props`] | property trees (transform, clip, effect, scroll) and their deltas |
//! | [`layer`] | compositor layers and their deltas |
//! | [`anim`] | animation specs and events (sampling lives in `skelter-anim`) |
//! | [`commit`] | the UI → compositor [`commit::Commit`] and fast commits |
//! | [`scene`] | the compositor → renderer [`scene::FrameScene`] |
//! | [`event`] | platform input events with timestamps and sequence ids |
//! | [`key`] | logical keys, named as egui 0.37 names them |
//! | [`cursor`] | cursor icons |
//! | [`input`] | the text-input client contract between editing kinds and the platform IME |
//!
//! Rules: no GPU, no platform, no JS dependency here; additive changes only once lanes build on it
//! (a breaking change goes through the batch coordinator).

pub mod anim;
pub mod change;
pub mod color;
pub mod commit;
pub mod cursor;
pub mod display;
pub mod event;
pub mod geom;
pub mod ids;
pub mod input;
pub mod key;
pub mod layer;
pub mod props;
pub mod scene;
pub mod text;
pub mod time;

pub use accesskit;
pub use smallvec;
