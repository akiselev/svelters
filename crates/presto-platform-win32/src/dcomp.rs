//! DirectComposition: one device per process and one visual tree per window (`platform.md` §4).
//!
//! Each window has a target (`CreateTargetForHwnd`, topmost), a root visual and, under the root, the
//! window's main visual and, above it, one child visual per OS layer (viewports, canvases) in creation
//! order, each clipped to its bounds.
//! wgpu presents into a visual through `SurfaceTargetUnsafe::CompositionVisual`: it sets the content
//! but never commits, so whoever changes the tree or configures a surface commits ([`CommitHandle`]
//! works from the compositor thread; DirectComposition calls are thread-safe).
//!
//! The window is created with `WS_EX_NOREDIRECTIONBITMAP`, so DWM keeps no GDI redirection surface
//! and the visuals are all there is.

use presto_platform::{OsLayerDesc, PlatformError};
use presto_base::geom::{Rect, Scale};
use presto_base::ids::SurfaceId;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual,
    IDCompositionVisual2,
};
use windows_core::Interface;

fn os_err(call: &str, e: windows_core::Error) -> PlatformError {
    PlatformError::Os(format!("{call}: {e}"))
}

/// Commits the process's DirectComposition device from any thread.
#[derive(Clone)]
pub struct CommitHandle(IDCompositionDesktopDevice);

// SAFETY: DirectComposition's API is thread-safe ("call any method from any thread at any time",
// DirectComposition basic concepts).
unsafe impl Send for CommitHandle {}
unsafe impl Sync for CommitHandle {}

impl CommitHandle {
    pub fn commit(&self) -> Result<(), PlatformError> {
        unsafe { self.0.Commit() }.map_err(|e| os_err("IDCompositionDevice::Commit", e))
    }
}

pub struct DComp {
    device: IDCompositionDesktopDevice,
}

impl DComp {
    /// A device without a rendering device: wgpu's swap chains are the only content.
    pub fn new() -> Result<Self, PlatformError> {
        let device: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(None) }
            .map_err(|e| os_err("DCompositionCreateDevice2", e))?;
        Ok(Self { device })
    }

    pub fn commit_handle(&self) -> CommitHandle {
        CommitHandle(self.device.clone())
    }

    pub fn commit(&self) -> Result<(), PlatformError> {
        unsafe { self.device.Commit() }.map_err(|e| os_err("IDCompositionDevice::Commit", e))
    }

    pub fn window_tree(&self, hwnd: HWND) -> Result<WindowTree, PlatformError> {
        let target = unsafe { self.device.CreateTargetForHwnd(hwnd, true) }
            .map_err(|e| os_err("CreateTargetForHwnd", e))?;
        let root = self.visual()?;
        let main = self.visual()?;
        unsafe {
            root.AddVisual(&main, false, None::<&IDCompositionVisual>)
                .map_err(|e| os_err("AddVisual", e))?;
            target.SetRoot(&root).map_err(|e| os_err("SetRoot", e))?;
        }
        self.commit()?;
        Ok(WindowTree { target, root, main, layers: Vec::new() })
    }

    fn visual(&self) -> Result<IDCompositionVisual2, PlatformError> {
        unsafe { self.device.CreateVisual() }.map_err(|e| os_err("CreateVisual", e))
    }
}

struct Layer {
    id: SurfaceId,
    visual: IDCompositionVisual2,
    desc: OsLayerDesc,
}

/// One window's visuals.
pub struct WindowTree {
    #[allow(dead_code, reason = "keeps the target (and so the tree) alive")]
    target: IDCompositionTarget,
    root: IDCompositionVisual2,
    main: IDCompositionVisual2,
    layers: Vec<Layer>,
}

/// A layer's bounds in device pixels: (offset x, offset y, clip rect relative to the offset).
pub fn layer_geometry(bounds: &Rect, scale: Scale) -> (f32, f32, D2D_RECT_F) {
    let s = scale.0;
    let (x, y) = ((bounds.min.x * s).round(), (bounds.min.y * s).round());
    let clip = D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: (bounds.width() * s).round(),
        bottom: (bounds.height() * s).round(),
    };
    (x, y, clip)
}

impl WindowTree {
    /// The main visual's raw `IDCompositionVisual` for wgpu.
    pub fn main_visual_ptr(&self) -> *mut std::ffi::c_void {
        let v: &IDCompositionVisual = &self.main;
        v.as_raw()
    }

    pub fn layer_visual_ptr(&self, id: SurfaceId) -> Option<*mut std::ffi::c_void> {
        self.layers.iter().find(|l| l.id == id).map(|l| {
            let v: &IDCompositionVisual = &l.visual;
            v.as_raw()
        })
    }

    pub fn add_layer(&mut self, dc: &DComp, id: SurfaceId, desc: OsLayerDesc, scale: Scale) -> Result<(), PlatformError> {
        let visual = dc.visual()?;
        // Above the topmost child, named explicitly so nothing relies on the null-reference rules.
        let top = self.layers.last().map(|l| &l.visual).unwrap_or(&self.main);
        unsafe { self.root.AddVisual(&visual, true, top) }.map_err(|e| os_err("AddVisual", e))?;
        self.layers.push(Layer { id, visual, desc });
        self.place(id, scale)?;
        dc.commit()
    }

    pub fn set_layer_bounds(&mut self, dc: &DComp, id: SurfaceId, bounds: Rect, scale: Scale) -> Result<(), PlatformError> {
        let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) else { return Ok(()) };
        layer.desc.bounds = bounds;
        self.place(id, scale)?;
        dc.commit()
    }

    pub fn remove_layer(&mut self, dc: &DComp, id: SurfaceId) -> Result<(), PlatformError> {
        let Some(i) = self.layers.iter().position(|l| l.id == id) else { return Ok(()) };
        let layer = self.layers.remove(i);
        unsafe { self.root.RemoveVisual(&layer.visual) }.map_err(|e| os_err("RemoveVisual", e))?;
        dc.commit()
    }

    /// Re-places every layer after a DPI change.
    pub fn rescale(&mut self, dc: &DComp, scale: Scale) -> Result<(), PlatformError> {
        let ids: Vec<SurfaceId> = self.layers.iter().map(|l| l.id).collect();
        for id in ids {
            self.place(id, scale)?;
        }
        dc.commit()
    }

    fn place(&self, id: SurfaceId, scale: Scale) -> Result<(), PlatformError> {
        let Some(layer) = self.layers.iter().find(|l| l.id == id) else { return Ok(()) };
        let (x, y, clip) = layer_geometry(&layer.desc.bounds, scale);
        unsafe {
            layer.visual.SetOffsetX2(x).map_err(|e| os_err("SetOffsetX", e))?;
            layer.visual.SetOffsetY2(y).map_err(|e| os_err("SetOffsetY", e))?;
            layer.visual.SetClip2(&clip).map_err(|e| os_err("SetClip", e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_in_device_pixels() {
        let (x, y, clip) = layer_geometry(&Rect::new(10.0, 20.0, 110.0, 70.0), Scale(1.5));
        assert_eq!((x, y), (15.0, 30.0));
        assert_eq!((clip.right, clip.bottom), (150.0, 75.0));
    }
}
