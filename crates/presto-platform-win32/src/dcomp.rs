//! DirectComposition: one device per process and one visual tree per window (`platform.md` §4).
//!
//! Each window has a target (`CreateTargetForHwnd`, topmost), a root visual and, under the root, the
//! window's main visual (z 0) and one child visual per OS layer (viewports, canvases) ordered by z.
//! wgpu presents into a visual through `SurfaceTargetUnsafe::CompositionVisual`: it sets the content
//! but never commits, so whoever changes the tree or configures a surface commits ([`CommitHandle`]
//! works from the compositor thread; DirectComposition calls are thread-safe).
//!
//! The window is created with `WS_EX_NOREDIRECTIONBITMAP`, so DWM keeps no GDI redirection surface
//! and the visuals are all there is.

use crate::shim::{OsLayerDesc, PlatformError};
use presto_base::geom::Scale;
use presto_base::ids::SurfaceId;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual,
    IDCompositionVisual2,
};
use windows_core::Interface;

/// Commits the process's DirectComposition device from any thread.
#[derive(Clone)]
pub struct CommitHandle(IDCompositionDesktopDevice);

// SAFETY: DirectComposition's API is thread-safe ("call any method from any thread at any time",
// DirectComposition basic concepts).
unsafe impl Send for CommitHandle {}
unsafe impl Sync for CommitHandle {}

impl CommitHandle {
    pub fn commit(&self) -> Result<(), PlatformError> {
        unsafe { self.0.Commit() }.map_err(|e| PlatformError::win("IDCompositionDevice::Commit", e))
    }
}

pub struct DComp {
    device: IDCompositionDesktopDevice,
}

impl DComp {
    /// A device without a rendering device: wgpu's swap chains are the only content.
    pub fn new() -> Result<Self, PlatformError> {
        let device: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(None) }
            .map_err(|e| PlatformError::win("DCompositionCreateDevice2", e))?;
        Ok(Self { device })
    }

    pub fn commit_handle(&self) -> CommitHandle {
        CommitHandle(self.device.clone())
    }

    pub fn commit(&self) -> Result<(), PlatformError> {
        unsafe { self.device.Commit() }.map_err(|e| PlatformError::win("IDCompositionDevice::Commit", e))
    }

    pub fn window_tree(&self, hwnd: HWND) -> Result<WindowTree, PlatformError> {
        let target = unsafe { self.device.CreateTargetForHwnd(hwnd, true) }
            .map_err(|e| PlatformError::win("CreateTargetForHwnd", e))?;
        let root = self.visual()?;
        let main = self.visual()?;
        unsafe {
            root.AddVisual(&main, false, None::<&IDCompositionVisual>)
                .map_err(|e| PlatformError::win("AddVisual", e))?;
            target.SetRoot(&root).map_err(|e| PlatformError::win("SetRoot", e))?;
        }
        self.commit()?;
        Ok(WindowTree { target, root, main, layers: Vec::new() })
    }

    fn visual(&self) -> Result<IDCompositionVisual2, PlatformError> {
        unsafe { self.device.CreateVisual() }.map_err(|e| PlatformError::win("CreateVisual", e))
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

/// The paint order of the main visual (`None`) and the layers: ascending z, the main visual at 0
/// before layers at 0, layers of equal z in creation order.
pub fn stacking_order(layers: &[(SurfaceId, i32)]) -> Vec<Option<SurfaceId>> {
    let mut items: Vec<(i32, u8, usize, Option<SurfaceId>)> = layers
        .iter()
        .enumerate()
        .map(|(i, &(id, z))| (z, 1, i, Some(id)))
        .collect();
    items.push((0, 0, 0, None));
    items.sort_by_key(|&(z, main_last, i, _)| (z, main_last, i));
    items.into_iter().map(|(.., id)| id).collect()
}

/// A layer's bounds in device pixels: (offset x, offset y, clip rect relative to the offset).
pub fn layer_geometry(desc: &OsLayerDesc, scale: Scale) -> (f32, f32, D2D_RECT_F) {
    let s = scale.0;
    let (x, y) = ((desc.bounds.min.x * s).round(), (desc.bounds.min.y * s).round());
    let clip = D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: (desc.bounds.width() * s).round(),
        bottom: (desc.bounds.height() * s).round(),
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
        self.layers.push(Layer { id, visual, desc });
        self.place(id, scale)?;
        self.restack()?;
        dc.commit()
    }

    pub fn update_layer(&mut self, dc: &DComp, id: SurfaceId, desc: OsLayerDesc, scale: Scale) -> Result<(), PlatformError> {
        let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) else { return Ok(()) };
        let restack = layer.desc.z != desc.z;
        layer.desc = desc;
        self.place(id, scale)?;
        if restack {
            self.restack()?;
        }
        dc.commit()
    }

    pub fn remove_layer(&mut self, dc: &DComp, id: SurfaceId) -> Result<(), PlatformError> {
        let Some(i) = self.layers.iter().position(|l| l.id == id) else { return Ok(()) };
        let layer = self.layers.remove(i);
        unsafe { self.root.RemoveVisual(&layer.visual) }.map_err(|e| PlatformError::win("RemoveVisual", e))?;
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
        let (x, y, clip) = layer_geometry(&layer.desc, scale);
        unsafe {
            layer.visual.SetOffsetX2(x).map_err(|e| PlatformError::win("SetOffsetX", e))?;
            layer.visual.SetOffsetY2(y).map_err(|e| PlatformError::win("SetOffsetY", e))?;
            if layer.desc.clip {
                layer.visual.SetClip2(&clip).map_err(|e| PlatformError::win("SetClip", e))?;
            }
        }
        Ok(())
    }

    /// Rebuilds the root's children in [`stacking_order`]: each visual goes above the previous one,
    /// so no call relies on the null-reference placement rules.
    fn restack(&self) -> Result<(), PlatformError> {
        let order = stacking_order(&self.layers.iter().map(|l| (l.id, l.desc.z)).collect::<Vec<_>>());
        let visual_of = |id: Option<SurfaceId>| -> &IDCompositionVisual2 {
            match id {
                None => &self.main,
                Some(id) => &self.layers.iter().find(|l| l.id == id).expect("layer in order").visual,
            }
        };
        unsafe {
            self.root.RemoveAllVisuals().map_err(|e| PlatformError::win("RemoveAllVisuals", e))?;
            let mut prev: Option<&IDCompositionVisual2> = None;
            for id in order {
                let v = visual_of(id);
                match prev {
                    None => self.root.AddVisual(v, false, None::<&IDCompositionVisual>),
                    Some(p) => self.root.AddVisual(v, true, p),
                }
                .map_err(|e| PlatformError::win("AddVisual", e))?;
                prev = Some(v);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::geom::Rect;

    #[test]
    fn main_visual_sits_at_zero() {
        let order = stacking_order(&[(SurfaceId(1), 1), (SurfaceId(2), -1), (SurfaceId(3), 0), (SurfaceId(4), 1)]);
        assert_eq!(order, vec![Some(SurfaceId(2)), None, Some(SurfaceId(3)), Some(SurfaceId(1)), Some(SurfaceId(4))]);
        assert_eq!(stacking_order(&[]), vec![None]);
    }

    #[test]
    fn geometry_in_device_pixels() {
        let desc = OsLayerDesc { bounds: Rect::new(10.0, 20.0, 110.0, 70.0), z: 1, opaque: true, clip: true };
        let (x, y, clip) = layer_geometry(&desc, Scale(1.5));
        assert_eq!((x, y), (15.0, 30.0));
        assert_eq!((clip.right, clip.bottom), (150.0, 75.0));
    }
}
