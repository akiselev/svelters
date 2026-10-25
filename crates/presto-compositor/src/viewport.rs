//! Viewports on OS layers (architecture §7.6, plan W-V1): a 3D view or an offscreen canvas whose image
//! a producer renders into a registered texture, presented on an OS compositor layer of its own (a
//! DirectComposition child visual, a CAMetalLayer sublayer, a Wayland subsurface) with its own wgpu
//! surface on the compositor's device.
//!
//! On an OS layer, a new image is one [`CompositorHandle::texture_updated`]: the compositor redraws
//! only the layer's surface, so orbiting a view never re-records or redraws the UI. Where the platform
//! has no OS layers (X11, Windows without DirectComposition, headless) or the surface cannot be made,
//! the viewport falls back to a `LayerKind::Texture` layer drawn into the window like any
//! `Prim::Surface`; a new image then redraws the window's surface inside the viewport's bounds only.
//!
//! **Resize without a stretched frame.** A size change runs the handshake
//! ([`CompositorHandle::resize_os_layer_sync`]) ordered around the platform's layer geometry:
//! - when the platform holds the layer's state ([`PlatformCx::begin_os_layer_resize`], Wayland's
//!   sync-mode subsurface), the new bounds are set first and the new-size frame and the bounds become
//!   visible together at the end;
//! - otherwise (DirectComposition, presto's CAMetalLayers, which never scale a buffer to the layer)
//!   the new size is presented first and the layer moved after.
//!
//! Inside the layer the texture is drawn at its own pixel size from the top-left corner, so a texture
//! the producer has not re-rendered at the new size yet is cut or uncovered, never scaled. The UI
//! commits the layout the new bounds come from before calling [`Viewport::set_bounds`]: the
//! handshake's frame applies every commit that has arrived.
//!
//! This is the UI thread's side (it holds the `PlatformCx`); [`layer_descs`] is the pure part the UI
//! puts in its commit.

use crate::CompositorHandle;
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Size};
use presto_base::ids::{LayerId, PropNodeId, SurfaceId, TextureId, WindowId};
use presto_base::layer::{LayerDesc, LayerKind};
use presto_platform::{OsLayerDesc, PlatformCx, SurfaceTarget};
use std::time::Duration;

/// How a viewport reaches the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    /// Its own OS layer and surface.
    OsLayer(SurfaceId),
    /// The fallback: a texture layer drawn into the window's surface.
    Texture,
}

/// What a viewport is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportDesc {
    pub window: WindowId,
    /// Window points.
    pub bounds: Rect,
    pub scale: Scale,
    /// The texture its producer renders into (registered with `CompositorHandle::register_texture`).
    pub texture: TextureId,
    /// The texture's size in pixels.
    pub texture_size: DeviceSize,
    /// The image has alpha that blends with the UI below.
    pub transparent: bool,
    /// Try an OS layer (off: always the texture fallback).
    pub os_layer: bool,
    /// How long a resize waits for the compositor's new-size frame (about one refresh).
    pub resize_wait: Duration,
}

impl ViewportDesc {
    pub fn new(window: WindowId, bounds: Rect, scale: Scale, texture: TextureId, texture_size: DeviceSize) -> Self {
        ViewportDesc {
            window,
            bounds,
            scale,
            texture,
            texture_size,
            transparent: false,
            os_layer: true,
            resize_wait: Duration::from_millis(17),
        }
    }
}

/// The layer ids a viewport's layers use in the UI's tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportLayerIds {
    /// The OS layer (unused in the fallback).
    pub os: LayerId,
    pub texture: LayerId,
}

/// What [`Viewport::set_bounds`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resized {
    /// The size changed (a handshake ran).
    pub resized: bool,
    /// The compositor presented the new size within the wait.
    pub presented: bool,
    /// The platform held the layer's state across the handshake.
    pub held: bool,
}

/// Device pixels of `bounds` at `scale`.
pub fn device_size(bounds: &Rect, scale: Scale) -> DeviceSize {
    DeviceSize { w: (bounds.width() * scale.0).round().max(0.0) as u32, h: (bounds.height() * scale.0).round().max(0.0) as u32 }
}

/// The layers a viewport adds under `parent`, in the space of `transform` (which must map to window
/// points, as the OS layer is placed in them): on an OS layer, that layer with the texture layer
/// inside at the texture's pixel size; in the fallback, the texture layer over `bounds`.
#[allow(clippy::too_many_arguments)]
pub fn layer_descs(
    presentation: Presentation,
    ids: ViewportLayerIds,
    parent: LayerId,
    transform: PropNodeId,
    z: i32,
    bounds: Rect,
    texture: TextureId,
    texture_size: DeviceSize,
    scale: Scale,
    opaque: bool,
) -> Vec<LayerDesc> {
    let desc = |id, parent, kind, bounds| LayerDesc {
        id,
        parent: Some(parent),
        kind,
        transform,
        clip: None,
        effect: None,
        z,
        bounds,
        opaque,
    };
    match presentation {
        Presentation::OsLayer(sid) => {
            let size = Size::new(texture_size.w as f32 / scale.0, texture_size.h as f32 / scale.0);
            let inner = Rect::from_origin_size(Point::new(bounds.min.x, bounds.min.y), size);
            vec![
                desc(ids.os, parent, LayerKind::OsLayer(sid), bounds),
                desc(ids.texture, ids.os, LayerKind::Texture(texture), inner),
            ]
        }
        Presentation::Texture => vec![desc(ids.texture, parent, LayerKind::Texture(texture), bounds)],
    }
}

/// Makes a wgpu surface from a platform surface target, on the main thread.
///
/// # Safety
/// The target's handles must stay valid until the surface is dropped (the compositor drops an OS
/// layer's surface on `remove_os_layer`, which must precede the platform's `remove_os_layer`).
pub unsafe fn create_surface(instance: &wgpu::Instance, target: SurfaceTarget) -> Option<wgpu::Surface<'static>> {
    let t = match target {
        SurfaceTarget::Raw { window, display } => {
            wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle: Some(display), raw_window_handle: window }
        }
        #[cfg(windows)]
        SurfaceTarget::CompositionVisual(v) => wgpu::SurfaceTargetUnsafe::CompositionVisual(v),
        #[cfg(target_os = "macos")]
        SurfaceTarget::CoreAnimationLayer(l) => wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(l),
        _ => return None,
    };
    // SAFETY: the caller keeps the handles alive for the surface's lifetime.
    match unsafe { instance.create_surface_unsafe(t) } {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("presto-compositor: no surface for an OS layer: {e}");
            None
        }
    }
}

/// A viewport, held by the UI thread.
#[derive(Debug)]
pub struct Viewport {
    desc: ViewportDesc,
    presentation: Presentation,
    size: DeviceSize,
}

impl Viewport {
    /// Promotes the viewport to an OS layer when the platform has one for this window and a surface
    /// can be made from it; otherwise the texture fallback.
    pub fn new(cx: &mut dyn PlatformCx, compositor: &CompositorHandle, desc: ViewportDesc) -> Viewport {
        let size = device_size(&desc.bounds, desc.scale);
        let mut presentation = Presentation::Texture;
        if desc.os_layer {
            let layer = OsLayerDesc { bounds: desc.bounds, transparent: desc.transparent };
            if let Some((sid, target)) = cx.create_os_layer(desc.window, layer) {
                // SAFETY: the platform keeps the layer until `remove`, which removes it from the
                // compositor first.
                match unsafe { create_surface(compositor.instance(), target) } {
                    Some(surface) => {
                        compositor.add_os_layer(desc.window, sid, surface, size);
                        presentation = Presentation::OsLayer(sid);
                    }
                    None => cx.remove_os_layer(desc.window, sid),
                }
            }
        }
        Viewport { desc, presentation, size }
    }

    /// A viewport that never asks for an OS layer (tests, or apps that want the texture path).
    pub fn texture_only(desc: ViewportDesc) -> Viewport {
        Viewport { size: device_size(&desc.bounds, desc.scale), desc, presentation: Presentation::Texture }
    }

    pub fn presentation(&self) -> Presentation {
        self.presentation
    }

    pub fn bounds(&self) -> Rect {
        self.desc.bounds
    }

    /// The device size the producer should render at.
    pub fn size(&self) -> DeviceSize {
        self.size
    }

    /// The layers for the UI's commit (see [`layer_descs`]).
    pub fn layers(&self, ids: ViewportLayerIds, parent: LayerId, transform: PropNodeId, z: i32) -> Vec<LayerDesc> {
        let d = &self.desc;
        layer_descs(self.presentation, ids, parent, transform, z, d.bounds, d.texture, d.texture_size, d.scale, !d.transparent)
    }

    /// The producer drew a new image: redraw what shows it (only the OS layer, when there is one).
    pub fn frame_ready(&self, compositor: &CompositorHandle) {
        compositor.texture_updated(self.desc.window, self.desc.texture);
    }

    /// The producer moved to another texture (re-created at a new size): the next commit's layers use it.
    pub fn set_texture(&mut self, texture: TextureId, size: DeviceSize) {
        self.desc.texture = texture;
        self.desc.texture_size = size;
    }

    /// Moves or resizes the viewport (window points). A size change runs the resize handshake in the
    /// order that never shows a stretched frame (module docs).
    pub fn set_bounds(&mut self, cx: &mut dyn PlatformCx, compositor: &CompositorHandle, bounds: Rect, scale: Scale) -> Resized {
        let size = device_size(&bounds, scale);
        self.desc.bounds = bounds;
        self.desc.scale = scale;
        let Presentation::OsLayer(sid) = self.presentation else {
            self.size = size;
            return Resized::default();
        };
        let w = self.desc.window;
        if size == self.size {
            cx.set_os_layer_bounds(w, sid, bounds);
            return Resized::default();
        }
        self.size = size;
        let held = cx.begin_os_layer_resize(w, sid);
        if held {
            cx.set_os_layer_bounds(w, sid, bounds);
        }
        let presented = compositor.resize_os_layer_sync(w, sid, size, self.desc.resize_wait);
        if held {
            cx.end_os_layer_resize(w, sid);
        } else {
            cx.set_os_layer_bounds(w, sid, bounds);
        }
        Resized { resized: true, presented, held }
    }

    /// Removes the viewport's OS layer: the compositor drops its surface first, then the platform
    /// destroys the layer. If the compositor does not answer in time the layer is left to the window
    /// (destroyed with it) rather than pulled from under a surface still in use.
    pub fn remove(self, cx: &mut dyn PlatformCx, compositor: &CompositorHandle) {
        if let Presentation::OsLayer(sid) = self.presentation {
            if compositor.remove_os_layer_sync(self.desc.window, sid, Duration::from_millis(250)) {
                cx.remove_os_layer(self.desc.window, sid);
            } else {
                log::warn!("presto-compositor: the compositor did not drop OS layer {sid:?}; it stays until its window closes");
            }
        }
    }
}
