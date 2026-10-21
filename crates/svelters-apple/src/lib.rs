//! Shared mechanics of the Apple backends (`docs/notes/apple-gui-backend.md` §7.2).
//!
//! - [`names`], [`enc`], [`handles`], [`testing`]: pure Rust, compiled and unit-tested everywhere.
//! - `objc` (Apple targets only): the CFRunLoop integration, the `SveltersTarget` runtime class
//!   and the dynamic bridge (KVC properties, `NSInvocation` calls, value boxing).

pub mod blocks;
pub mod enc;
pub mod handles;
pub mod names;
pub mod testing;

#[cfg(target_vendor = "apple")]
pub mod objc;
