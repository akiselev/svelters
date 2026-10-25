//! Per-OS shims under winit (platform.md §3.2). Each OS module provides a `WindowShim` with the same
//! inherent methods; OSes without a shim use [`generic::WindowShim`], which does what winit alone can.

use std::sync::Arc;

pub mod generic;

/// Callbacks a shim uses to reach the loop from inside OS callbacks.
#[derive(Clone)]
pub struct ShimHooks {
    /// Run due timers (posted from a modal loop's `WM_TIMER`).
    pub tick: Arc<dyn Fn() + Send + Sync>,
    /// Make OS-layer changes visible, on the main thread.
    pub commit: Arc<dyn Fn() + Send + Sync>,
}

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::{WindowShim, install_app_hooks, monitor_work_area, prepare_attributes};

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::{WindowShim, install_app_hooks, monitor_work_area, prepare_attributes};

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
pub mod wayland;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
pub use self::wayland::{WindowShim, install_app_hooks, monitor_work_area, prepare_attributes};

#[cfg(not(any(target_os = "windows", target_os = "macos", all(unix, not(any(target_os = "ios", target_os = "android"))))))]
pub use self::generic::{WindowShim, install_app_hooks, monitor_work_area, prepare_attributes};
