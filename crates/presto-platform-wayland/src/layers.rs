//! OS layers: viewports and canvases rendered into their own wgpu surfaces, composited by the compositor
//! above the window's content. On Wayland each is a `wl_subsurface` of the window's surface with its own
//! `wp_viewport`, and:
//! - desync mode normally, so the layer presents at its own rate; sync mode while the window resizes (its
//!   parent's [`SurfaceLock`] switches it), so layer and window change size in one compositor frame;
//! - an empty input region: input goes to the window, which hit-tests in window coordinates;
//! - its position is parent state (`wl_subsurface.set_position` applies on the parent's next commit).

use crate::present::{Geometry, SurfaceLock};
use crate::window::buffer_size;
use presto_base::geom::Rect;
use presto_base::ids::SurfaceId;
use std::sync::Arc;
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;

pub struct Layer {
    pub id: SurfaceId,
    pub surface: WlSurface,
    pub subsurface: WlSubsurface,
    pub viewport: Option<WpViewport>,
    /// Position and size in window points.
    pub rect: Rect,
    pub lock: Arc<SurfaceLock>,
}

impl Layer {
    /// Points size, at least one point each way.
    pub fn size(&self) -> (u32, u32) {
        ((self.rect.max.x - self.rect.min.x).round().max(1.0) as u32, (self.rect.max.y - self.rect.min.y).round().max(1.0) as u32)
    }

    /// Records the buffer-dependent state for the layer's next present at the window's scale.
    pub fn apply_geometry(&self, scale120: Option<u32>, int_scale: i32) {
        let size = self.size();
        let use_vp = scale120.is_some() && self.viewport.is_some();
        self.lock.set_geometry(Geometry {
            buffer: buffer_size(size, scale120.filter(|_| use_vp), int_scale),
            logical: (size.0 as i32, size.1 as i32),
            buffer_scale: if use_vp { 1 } else { int_scale.max(1) },
            viewport: if use_vp { self.viewport.clone() } else { None },
            xdg_surface: None,
        });
    }

    pub fn set_position(&self) {
        self.subsurface.set_position(self.rect.min.x.round() as i32, self.rect.min.y.round() as i32);
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        if let Some(vp) = self.viewport.take() {
            vp.destroy();
        }
        self.subsurface.destroy();
        self.surface.destroy();
    }
}
