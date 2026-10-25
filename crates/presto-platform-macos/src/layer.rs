//! `CAMetalLayer`s (`platform.md` §4, macOS): one per window, hosted by the window's view, plus
//! sublayers for OS layers. The UI thread creates and lays them out; layer changes go inside a
//! Core Animation transaction without implicit animations. The configuration follows GPUI's
//! `configure_layer` (`crates/gpui_macos` / `gpui_apple` at a5c4ca8): opaque windows get
//! direct-to-display, three drawables, no next-drawable timeout.

use objc2::rc::Retained;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::{
    CAAutoresizingMask, CALayer, CAMetalLayer, CATransaction, kCAGravityTopLeft,
};
use presto_base::geom::Rect;
use presto_platform::{OsLayerDesc, SurfaceTarget};
use std::ffi::c_void;

pub(crate) fn new_window_layer(opaque: bool, scale: f64) -> Retained<CAMetalLayer> {
    let layer = CAMetalLayer::new();
    configure(&layer, opaque, scale);
    layer.setAutoresizingMask(
        CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable,
    );
    // Sublayer frames use the view's top-left origin.
    layer.setGeometryFlipped(true);
    layer
}

fn configure(layer: &CAMetalLayer, opaque: bool, scale: f64) {
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setOpaque(opaque);
    layer.setMaximumDrawableCount(3);
    layer.setAllowsNextDrawableTimeout(false);
    layer.setContentsScale(scale);
    layer.setPresentsWithTransaction(false);
}

pub(crate) fn target(layer: &CAMetalLayer) -> SurfaceTarget {
    SurfaceTarget::CoreAnimationLayer(layer as *const CAMetalLayer as *mut c_void)
}

pub(crate) fn cg_rect(r: &Rect) -> CGRect {
    CGRect::new(
        CGPoint::new(r.min.x as f64, r.min.y as f64),
        CGSize::new(
            (r.max.x - r.min.x).max(0.0) as f64,
            (r.max.y - r.min.y).max(0.0) as f64,
        ),
    )
}

/// Runs `f` in a transaction with implicit animations off.
pub(crate) fn transaction(f: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    f();
    CATransaction::commit();
}

pub(crate) fn create_os_layer(
    root: &CAMetalLayer,
    desc: &OsLayerDesc,
    scale: f64,
) -> Retained<CAMetalLayer> {
    let layer = CAMetalLayer::new();
    configure(&layer, !desc.transparent, scale);
    // A drawable of the old size stays pinned to the top-left, clipped, until the compositor presents
    // the new size (never stretched).
    // SAFETY: kCAGravityTopLeft is a valid static.
    layer.setContentsGravity(unsafe { kCAGravityTopLeft });
    layer.setMasksToBounds(true);
    transaction(|| {
        layer.setFrame(cg_rect(&desc.bounds));
        root.addSublayer(&layer);
    });
    layer
}

pub(crate) fn set_frame(layer: &CALayer, rect: &Rect) {
    transaction(|| layer.setFrame(cg_rect(rect)));
}

pub(crate) fn remove(layer: &CALayer) {
    transaction(|| layer.removeFromSuperlayer());
}

/// A backing-scale change: the window layer and its OS layers.
pub(crate) fn set_scale<'a>(layers: impl IntoIterator<Item = &'a CAMetalLayer>, scale: f64) {
    transaction(|| {
        for l in layers {
            l.setContentsScale(scale);
        }
    });
}
