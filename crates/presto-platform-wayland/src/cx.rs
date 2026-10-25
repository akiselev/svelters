//! [`Cx`]: the `PlatformCx` the app calls back into, and its clipboard.

use crate::app::State;
use crate::layers::Layer;
use crate::present::{FrameSourceConfig, SurfaceLock, WaylandFrameSource};
use crate::shim::{
    Clipboard, DragData, DragId, FrameSource, HitRegion, ImeState, Monitor, OsLayerDesc, PlatformCx, PlatformError, SurfaceTarget, Waker,
    WindowDesc, WindowStateRequest,
};
use crate::time::MonotonicClock;
use crate::window;
use presto_base::cursor::CursorIcon;
use presto_base::event::WindowEvent;
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Vec2};
use presto_base::ids::{SurfaceId, WindowId};
use presto_base::time::{Clock, Time};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use smithay_client_toolkit::activation::RequestData;
use std::ptr::NonNull;
use std::sync::Arc;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{EventQueue, Proxy};

pub struct Cx<'a> {
    state: &'a mut State,
    queue: &'a mut EventQueue<State>,
    waker: &'a Waker,
}

impl<'a> Cx<'a> {
    pub(crate) fn new(state: &'a mut State, queue: &'a mut EventQueue<State>, waker: &'a Waker) -> Self {
        Cx { state, queue, waker }
    }

    fn target_for(&self, surface: &WlSurface) -> SurfaceTarget {
        // SAFETY-relevant invariant: with wayland-backend's `client_system` feature these are libwayland's
        // `wl_proxy`/`wl_display` pointers, valid while the surface (and the connection) live.
        let surface_ptr = NonNull::new(surface.id().as_ptr() as *mut std::ffi::c_void).expect("a live wl_surface has a proxy");
        let display_ptr = NonNull::new(self.state.conn.backend().display_ptr() as *mut std::ffi::c_void).expect("a live connection has a display");
        SurfaceTarget {
            window: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr)),
            display: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr)),
        }
    }

    fn refresh_cursor(&mut self, id: WindowId) {
        let keys: Vec<_> = self
            .state
            .seats
            .iter_mut()
            .filter_map(|s| {
                let p = s.pointer.as_mut()?;
                (p.asm.focus() == Some(id)).then(|| {
                    p.shown = None;
                    s.key
                })
            })
            .collect();
        for k in keys {
            crate::seat::update_cursor(self.state, k, id);
        }
    }
}

impl PlatformCx for Cx<'_> {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId {
        let id = WindowId(self.state.next_window);
        self.state.next_window += 1;
        if window::create(self.state, id, desc).is_err() {
            // The app hears about it as an immediate close.
            self.state.push_window(id, WindowEvent::CloseRequested);
            return id;
        }
        // Wait for the first configure: presenting before it is a protocol error.
        while self.state.windows.get(&id).is_some_and(|w| !w.configured) {
            if self.queue.blocking_dispatch(self.state).is_err() {
                break;
            }
        }
        if let Some(theme) = self.state.theme {
            self.state.push_window(id, WindowEvent::ThemeChanged(theme));
        }
        id
    }

    fn close_window(&mut self, id: WindowId) {
        let Some(w) = self.state.windows.remove(&id) else { return };
        self.state.surfaces.remove(&w.surface.id());
        for l in w.layers.values() {
            self.state.surfaces.remove(&l.surface.id());
        }
        drop(w);
        let _ = self.queue.flush();
    }

    fn surface_target(&self, id: WindowId) -> SurfaceTarget {
        let w = self.state.windows.get(&id).unwrap_or_else(|| panic!("surface_target for {id:?}, which is not open"));
        self.target_for(&w.surface)
    }

    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)> {
        let sub = self.state.subcompositor.as_ref()?;
        let w = self.state.windows.get(&id)?;
        let (subsurface, surface) = sub.create_subsurface(w.surface.clone(), &self.state.qh);
        let empty = self.state.compositor.wl_compositor().create_region(&self.state.qh, ());
        surface.set_input_region(Some(&empty));
        empty.destroy();
        let layer_id = SurfaceId(self.state.next_layer);
        self.state.next_layer += 1;
        let viewport = self.state.protocols.viewporter.as_ref().map(|v| v.get_viewport(&surface, &self.state.qh, ()));
        let layer = Layer { id: layer_id, surface: surface.clone(), subsurface: subsurface.clone(), viewport, rect: desc.rect, lock: Arc::new(SurfaceLock::default()) };
        if desc.opaque {
            let (lw, lh) = layer.size();
            let region = self.state.compositor.wl_compositor().create_region(&self.state.qh, ());
            region.add(0, 0, lw as i32, lh as i32);
            surface.set_opaque_region(Some(&region));
            region.destroy();
        }
        layer.set_position();
        let scale120 = w.scale120.filter(|_| w.viewport.is_some());
        layer.apply_geometry(scale120, w.int_scale);
        w.lock.add_child(subsurface);
        let target = self.target_for(&surface);
        self.state.surfaces.insert(surface.id(), id);
        self.state.windows.get_mut(&id)?.layers.insert(layer_id, layer);
        Some((layer_id, target))
    }

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        if w.cursor == c {
            return;
        }
        w.cursor = c;
        self.refresh_cursor(id);
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        if w.ime == state {
            return;
        }
        w.ime = state;
        crate::seat::update_ime(self.state, id);
    }

    fn clipboard(&mut self) -> &mut dyn Clipboard {
        self
    }

    fn set_timer(&mut self, at: Option<Time>) {
        self.state.timer = at;
    }

    fn waker(&self) -> Waker {
        self.waker.clone()
    }

    fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(MonotonicClock)
    }

    fn monitors(&self) -> Vec<Monitor> {
        let outputs = &self.state.outputs;
        outputs
            .outputs()
            .filter_map(|o| outputs.info(&o))
            .map(|info| {
                let mode = info.modes.iter().find(|m| m.current).or(info.modes.first());
                let (w, h) = mode.map_or((0, 0), |m| m.dimensions);
                let pos = info.logical_position.unwrap_or(info.location);
                Monitor {
                    name: info.name.clone().or_else(|| info.description.clone()),
                    position: Point::new(pos.0 as f32, pos.1 as f32),
                    size: DeviceSize { w: w.max(0) as u32, h: h.max(0) as u32 },
                    scale: Scale(info.scale_factor as f32),
                    refresh_millihertz: mode.and_then(|m| u32::try_from(m.refresh_rate).ok()).filter(|&r| r > 0),
                }
            })
            .collect()
    }

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        if let Some(w) = self.state.windows.get_mut(&id) {
            w.hit_regions = regions.to_vec();
        }
    }

    fn exit(&mut self) {
        self.state.exit = true;
    }

    fn set_os_layer_rect(&mut self, id: WindowId, layer: SurfaceId, rect: Rect) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        let scale120 = w.scale120.filter(|_| w.viewport.is_some());
        let int_scale = w.int_scale;
        let Some(l) = w.layers.get_mut(&layer) else { return };
        l.rect = rect;
        l.set_position();
        l.apply_geometry(scale120, int_scale);
    }

    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        if let Some(l) = w.layers.remove(&layer) {
            w.lock.remove_child(&l.subsurface);
            self.state.surfaces.remove(&l.surface.id());
        }
    }

    fn frame_source(&mut self, id: WindowId, layer: Option<SurfaceId>) -> Option<Box<dyn FrameSource>> {
        let w = self.state.windows.get(&id)?;
        let (surface, lock) = match layer {
            None => (w.surface.clone(), w.lock.clone()),
            Some(l) => {
                let l = w.layers.get(&l)?;
                (l.surface.clone(), l.lock.clone())
            }
        };
        let globals = self.state.frame_globals();
        Some(Box::new(WaylandFrameSource::new(&self.state.conn, surface, lock, &globals, FrameSourceConfig::default())))
    }

    fn client_side_decorations(&self, id: WindowId) -> bool {
        self.state.windows.get(&id).is_some_and(|w| w.client_side_decorations())
    }

    fn set_window_state(&mut self, id: WindowId, req: WindowStateRequest) {
        let Some(w) = self.state.windows.get(&id) else { return };
        if req == WindowStateRequest::Activate {
            if let Some(a) = &self.state.activation {
                let seat_and_serial = crate::seat::latest_press(self.state);
                a.request_token(&self.state.qh, RequestData { app_id: w.desc.app_id.clone(), seat_and_serial, surface: Some(w.surface.clone()) });
            }
            return;
        }
        let Some(top) = w.toplevel() else { return };
        match req {
            WindowStateRequest::Maximize => top.set_maximized(),
            WindowStateRequest::Unmaximize => top.unset_maximized(),
            WindowStateRequest::Minimize => top.set_minimized(),
            WindowStateRequest::Fullscreen => top.set_fullscreen(None),
            WindowStateRequest::Unfullscreen => top.unset_fullscreen(),
            WindowStateRequest::Activate => {}
        }
    }

    fn start_drag(&mut self, window: WindowId, data: DragData) -> Result<DragId, PlatformError> {
        crate::data::start_drag(self.state, window, data)
    }

    fn attach_window_to_drag(&mut self, drag: DragId, window: WindowId, offset: Vec2) -> Result<(), PlatformError> {
        crate::data::attach_window(self.state, drag, window, offset)
    }
}

impl Clipboard for Cx<'_> {
    fn get_text(&mut self) -> Option<String> {
        crate::data::get_clipboard(self.state)
    }

    fn set_text(&mut self, text: String) -> Result<(), PlatformError> {
        crate::data::set_clipboard(self.state, text)
    }

    fn get_primary(&mut self) -> Option<String> {
        crate::data::get_primary(self.state)
    }

    fn set_primary(&mut self, text: String) -> Result<(), PlatformError> {
        crate::data::set_primary(self.state, text)
    }
}
