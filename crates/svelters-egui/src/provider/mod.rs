//! Rust-provided kinds (ARCH-EMBED §8): the `Widget` trait, `WidgetCx`, `MeasureCx` and `KindEntry`
//! (collected with `inventory` into [`crate::kinds::KindRegistry`]).
//!
//! Empty at EG0. Written by RUSTAPI (RA1, RA3); EVENTS writes `behavior.rs` and SCENE `pass.rs`.

mod behavior;
mod pass;
mod state;
