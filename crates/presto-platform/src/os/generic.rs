//! What winit alone gives: no OS layers, no pointer history, no OS timestamps, no composition
//! control. Hit-test regions are kept for the app's own caption handling (`drag_window`).

use super::ShimHooks;
use crate::api::{HitRegion, ImeState, OsLayerCommitter, OsLayerDesc, SurfaceTarget, WindowDesc};
use presto_base::event::PointerSample;
use presto_base::geom::Rect;
use presto_base::ids::SurfaceId;
use presto_base::time::Time;
use smallvec::SmallVec;
use winit::window::{Window, WindowAttributes};

pub struct WindowShim;

#[allow(unused_variables)]
impl WindowShim {
    pub fn install(window: &Window, desc: &WindowDesc, hooks: ShimHooks) -> Self {
        WindowShim
    }
    pub fn window_event(&mut self, ev: &winit::event::WindowEvent) {}
    pub fn surface_target(&self) -> Option<SurfaceTarget> {
        None
    }
    pub fn create_layer(&mut self, window: &Window, id: SurfaceId, desc: OsLayerDesc, scale: f32) -> Option<SurfaceTarget> {
        None
    }
    pub fn set_layer_bounds(&mut self, id: SurfaceId, bounds: Rect, scale: f32) {}
    pub fn remove_layer(&mut self, id: SurfaceId) {}
    pub fn committer(&self) -> Option<OsLayerCommitter> {
        None
    }
    pub fn commit_layers(&mut self) {}
    pub fn set_timer(&mut self, at: Option<Time>) {}
    pub fn take_history(&mut self) -> SmallVec<[PointerSample; 4]> {
        SmallVec::new()
    }
    pub fn event_time(&self) -> Option<Time> {
        None
    }
    pub fn set_hit_regions(&mut self, regions: &[HitRegion]) {}
    pub fn set_ime(&mut self, state: Option<&ImeState>) {}
    /// `None`: this OS cannot end a composition here (the backend falls back).
    pub fn end_composition(&mut self, commit: bool) -> Option<bool> {
        None
    }
}

pub fn install_app_hooks(quit: Box<dyn Fn() + Send + Sync>) {
    let _ = quit;
}

pub fn prepare_attributes(a: WindowAttributes, desc: &WindowDesc) -> WindowAttributes {
    let _ = desc;
    a
}

pub fn monitor_work_area(m: &winit::monitor::MonitorHandle) -> Option<presto_base::geom::DeviceRect> {
    let _ = m;
    None
}
