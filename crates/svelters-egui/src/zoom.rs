//! ZM1: UI zoom, VS Code style (Ctrl+= / Ctrl+- / Ctrl+0; Cmd on macOS).
//!
//! The zoom is egui's `zoom_factor`: `pixels_per_point = zoom × native_pixels_per_point`. Everything
//! the backend keeps is in points, and CSS px are points, so a zoom changes no style, layout input
//! or hit test: egui scales the frame to physical pixels, re-rasterises the font atlas at the new
//! `pixels_per_point` (text stays crisp; no texture is scaled), and AccessKit bounds go out in points
//! under a root transform of `pixels_per_point`. What the backend adds:
//!
//! - [`EguiBackend::set_zoom`] / [`EguiBackend::zoom`] (and the same on [`crate::Embed`]): the host
//!   API. It takes a factor; levels are the host's (cadatomic maps VS Code's `window.zoomLevel` n to
//!   `1.2^n`, [`level_factor`]).
//! - At the first logic after `pixels_per_point` changed, every measured leaf is remeasured (text
//!   advances snap to physical pixels, so a size in points can move by a fraction): only leaves whose
//!   size changed relayout. The paint caches key on `pixels_per_point` already.
//! - SVG images and icons render at device resolution: egui's image loaders ask for physical pixels
//!   (`SizeHint` × `pixels_per_point`), the texture caches key on that size, and the sizes not drawn
//!   any more are evicted (`assets.rs`). ICR1's registry keys its rasters on physical pixels too.
//! - Who handles the keys is [`ZoomKeys`] ([`crate::embed::EmbedConfig::zoom_keys`]): the host (an
//!   embedder such as cadatomic, which owns the shortcuts: egui's own handler is turned off), the
//!   embed (the standalone runners: steps of 1.2), or egui's built-in handler (steps of 0.1).

use crate::backend::EguiBackend;
use egui::gui_zoom::kb_shortcuts;

/// One zoom level is a factor of 1.2 (VS Code's `window.zoomLevel`).
pub const ZOOM_STEP: f32 = 1.2;
/// The levels the keys step through: `1.2^-8` (≈ 0.23) to `1.2^8` (≈ 4.30).
pub const MIN_LEVEL: i32 = -8;
pub const MAX_LEVEL: i32 = 8;
/// [`EguiBackend::set_zoom`] clamps to egui's own range.
pub const MIN_ZOOM: f32 = 0.2;
pub const MAX_ZOOM: f32 = 5.0;

/// The factor of zoom level `level` (`1.2^level`, the level clamped to [`MIN_LEVEL`]..=[`MAX_LEVEL`]).
pub fn level_factor(level: i32) -> f32 {
    ZOOM_STEP.powi(level.clamp(MIN_LEVEL, MAX_LEVEL))
}

/// The level nearest to `factor`.
pub fn nearest_level(factor: f32) -> i32 {
    if factor.is_nan() || factor <= 0.0 {
        return 0;
    }
    ((factor.ln() / ZOOM_STEP.ln()).round() as i32).clamp(MIN_LEVEL, MAX_LEVEL)
}

/// The next level up (`delta > 0`) or down from `factor`, `delta` levels away. A factor between two
/// levels steps to the neighbouring level in that direction first (1.1 → 1.2 up, 1.0 down).
pub fn step(factor: f32, delta: i32) -> f32 {
    if delta == 0 || factor.is_nan() || factor <= 0.0 {
        return factor;
    }
    let l = factor.ln() / ZOOM_STEP.ln();
    let level = if delta > 0 { (l + 1e-3).floor() as i32 + delta } else { (l - 1e-3).ceil() as i32 + delta };
    level_factor(level)
}

/// Who handles Ctrl+= / Ctrl+- / Ctrl+0 (Cmd on macOS) for an [`crate::Embed`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ZoomKeys {
    /// Nobody in svelters (the [`crate::embed::EmbedConfig`] default): the host owns the shortcuts
    /// (cadatomic's keymap) and calls [`crate::Embed::set_zoom`]. The keys reach the host's ui
    /// unconsumed, and egui's built-in handler is turned off (`Options::zoom_with_keyboard`).
    #[default]
    Host,
    /// The embed steps by [`ZOOM_STEP`] at its `logic` and consumes the keys; egui's handler is off.
    /// [`crate::embed::EmbedConfig::app`] (`svelters-egui-run`, `TestApp`) uses it.
    Embed,
    /// egui's built-in handler (steps of 0.1): svelters leaves `zoom_with_keyboard` as it is.
    Egui,
}

impl EguiBackend {
    /// ZM1: sets the UI zoom factor, multiplied with the OS scale (egui's `zoom_factor`:
    /// `pixels_per_point = zoom × native_pixels_per_point`). Clamped to [`MIN_ZOOM`]..=[`MAX_ZOOM`];
    /// a non-finite or non-positive factor is ignored. egui applies it at the start of the next pass
    /// (every viewport repaints); a backend not yet bound to a context applies it at its first
    /// logic. The zoom is the context's, so it zooms the host's own egui ui too (VS Code zooms the
    /// whole window).
    pub fn set_zoom(&self, zoom: f32) {
        if !zoom.is_finite() || zoom <= 0.0 {
            return;
        }
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.0.zoom_pending.set(Some(zoom));
        if let Some(ctx) = self.ctx() {
            ctx.set_zoom_factor(zoom);
        }
    }

    /// ZM1: the zoom factor: the one [`set_zoom`](EguiBackend::set_zoom) asked for until egui applies
    /// it, then the context's (1.0 before a context is bound).
    pub fn zoom(&self) -> f32 {
        self.0.zoom_pending.get().or_else(|| self.ctx().map(|c| c.zoom_factor())).unwrap_or(1.0)
    }

    /// At logic: a zoom asked for reaches the context (it may have been bound since), and a changed
    /// `pixels_per_point` of the root viewport (zoom, or the window moved to another monitor)
    /// remeasures every measured leaf.
    pub(crate) fn follow_zoom(&self, ctx: &egui::Context) {
        if let Some(z) = self.0.zoom_pending.get() {
            if ctx.zoom_factor() == z {
                self.0.zoom_pending.set(None);
            } else {
                ctx.set_zoom_factor(z);
            }
        }
        // per viewport the native scale may differ; the root viewport's logic decides
        if ctx.viewport_id() != egui::ViewportId::ROOT {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let seen = self.0.scale_seen.replace(ppp);
        if seen == 0.0 || seen == ppp {
            return;
        }
        let mut s = self.0.scene.borrow_mut();
        let ids: Vec<_> = s.ids().filter(|&id| s.layout[s.ix(id)].intrinsic.is_some()).collect();
        for id in ids {
            s.mark_measure(id);
        }
        drop(s);
        self.touch_all();
    }
}

/// [`ZoomKeys::Embed`] at logic: one step per shortcut pressed this pass (consumed).
pub(crate) fn handle_keys(ctx: &egui::Context, b: &EguiBackend) {
    let mut zoom = b.zoom();
    let start = zoom;
    if ctx.input_mut(|i| i.consume_shortcut(&kb_shortcuts::ZOOM_RESET)) {
        zoom = 1.0;
    } else {
        while ctx.input_mut(|i| i.consume_shortcut(&kb_shortcuts::ZOOM_IN) || i.consume_shortcut(&kb_shortcuts::ZOOM_IN_SECONDARY)) {
            zoom = step(zoom, 1);
        }
        while ctx.input_mut(|i| i.consume_shortcut(&kb_shortcuts::ZOOM_OUT)) {
            zoom = step(zoom, -1);
        }
    }
    if zoom != start {
        b.set_zoom(zoom);
    }
}

/// [`ZoomKeys`] other than `Egui` turn egui's built-in handler off (checked first: no write lock
/// per frame).
pub(crate) fn configure(ctx: &egui::Context, keys: ZoomKeys) {
    if keys != ZoomKeys::Egui && ctx.options(|o| o.zoom_with_keyboard) {
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_step_by_1_2() {
        assert_eq!(level_factor(0), 1.0);
        assert!((level_factor(1) - 1.2).abs() < 1e-6);
        assert!((level_factor(-2) - 1.0 / 1.44).abs() < 1e-6);
        assert_eq!(level_factor(100), level_factor(MAX_LEVEL));
        assert!((step(1.0, 1) - 1.2).abs() < 1e-6);
        assert!((step(1.2, 1) - 1.44).abs() < 1e-5);
        assert!((step(1.44, -1) - 1.2).abs() < 1e-5);
        assert!((step(1.2, -1) - 1.0).abs() < 1e-6);
        // between two levels: the neighbour in that direction
        assert!((step(1.1, 1) - 1.2).abs() < 1e-6);
        assert!((step(1.1, -1) - 1.0).abs() < 1e-6);
        assert_eq!(step(level_factor(MAX_LEVEL), 1), level_factor(MAX_LEVEL));
        assert_eq!(nearest_level(1.44), 2);
        assert_eq!(nearest_level(1.0 / 1.2), -1);
    }
}
