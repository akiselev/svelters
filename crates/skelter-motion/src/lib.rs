//! Native Svelte motion on skelter (lane MOTION-CSS, `plans/cloud/presto-b2/LANE-motion-css.md`;
//! design: `plans/research/skelter/compiler-and-js.md` §3.6).
//!
//! | Module | What |
//! |---|---|
//! | [`easing`] | svelte/easing's curves, bit-for-bit, and as skelter easings |
//! | [`transition`] | svelte/transition's `fade`, `fly`, `slide`, `scale`, `blur`, `draw` from their parameters: Svelte's `css(t, u)` and skelter tracks |
//! | [`flip`] | svelte/animate's `flip` from skelter's layout |
//! | [`spring`] | svelte/motion's spring motion on skelter-anim's closed form: Svelte's settle frame and retarget (exact at 60 Hz, frame-rate independent) |
//! | [`waapi`] | the Web Animations slice Svelte uses, timed on skelter's clock |
//! | [`op`] | the `OP_ANIMATE` payload: channel keyframes plus the native descriptor |
//!
//! The host (`svelters-skelter`) parses each `OP_ANIMATE` with [`op::AnimateOp::parse`], starts a
//! [`waapi::Animation`] and the [`transition::Plan`]'s tracks, and delivers [`waapi::Animations::poll`]'s
//! finishes as `animationend`.

pub mod easing;
pub mod flip;
pub mod op;
pub mod spring;
pub mod transition;
pub mod waapi;
