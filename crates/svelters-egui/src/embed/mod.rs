//! Embedding svelters-egui as a library inside a host's egui frame (ARCH-EMBED §4): `Embed`,
//! `EmbedConfig` and the public API (`logic`, `show_slot`, roots and slots).
//!
//! Empty at EG0; EM1 writes it (writer lane: EMBED). Until then the runners use [`crate::EguiBackend`]
//! directly.

mod current;
mod roots;
mod settle;
mod slots;
