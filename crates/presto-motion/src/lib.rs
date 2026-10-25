//! Native Svelte motion on presto (lane MOTION-CSS, `plans/cloud/presto-b2/LANE-motion-css.md`;
//! design: `plans/research/presto/compiler-and-js.md` §3.6).
//!
//! | Module | What |
//! |---|---|
//! | [`easing`] | svelte/easing's curves, bit-for-bit, and as presto easings |
//! | [`transition`] | svelte/transition's `fade`, `fly`, `slide`, `scale`, `blur`, `draw` from their parameters: Svelte's `css(t, u)` and presto tracks |
//! | [`flip`] | svelte/animate's `flip` from presto's layout |
//! | [`spring`] | svelte/motion's spring motion on presto-anim's closed form: Svelte's settle frame and retarget (exact at 60 Hz, frame-rate independent) |
//! | [`waapi`] | the Web Animations slice Svelte uses, timed on presto's clock |
//! | [`op`] | the `OP_ANIMATE` payload: channel keyframes plus the native descriptor |
//!
//! The host (`svelters-presto`) parses each `OP_ANIMATE` with [`op::AnimateOp::parse`], starts a
//! [`waapi::Animation`] and the [`transition::Plan`]'s tracks, and delivers [`waapi::Animations::poll`]'s
//! finishes as `animationend`.

pub mod easing;
pub mod flip;
pub mod op;
pub mod shim;
pub mod spring;
pub mod transition;
pub mod waapi;
