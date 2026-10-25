//! [`Cx`]: the `PlatformCx` the app calls back into, and its clipboard.

use crate::app::State;
use crate::layers::Layer;
use crate::present::{FrameSourceConfig, SurfaceLock, WaylandFrameSource};
use crate::shim::{
    Clipboard, ClipboardKind, DragData, DragId, DragOutcome, FrameSource, HitRegion, ImeHints, ImeState, Monitor, OsLayerCommitter,
    OsLayerDesc, PlatformCx, PlatformError, PopupDesc, SurfaceTarget, Waker, WaylandCx, WindowDesc, WindowStateRequest,
};
use crate::time::MonotonicClock;
use crate::window;
use presto_base::cursor::CursorIcon;
use presto_base::event::WindowEvent;
use presto_base::geom::{DeviceSize, Rect, Scale, Vec2};
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
        SurfaceTarget::Raw {
            window: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr)),
            display: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr)),
        }
    }

    /// Creates a window and waits for its first configure: presenting before it is a protocol error.
    fn open(&mut self, desc: WindowDesc, popup: Option<PopupDesc>, parent: Option<WindowId>) -> Result<WindowId, PlatformError> {
        let id = WindowId(self.state.next_window);
        self.state.next_window += 1;
        window::create(self.state, id, desc, popup, parent)?;
        while self.state.windows.get(&id).is_some_and(|w| !w.configured) {
            if let Err(e) = self.queue.blocking_dispatch(self.state) {
                return Err(PlatformError::EventLoop(e.to_string()));
            }
        }
        if !self.state.windows.contains_key(&id) {
            return Err(PlatformError::Os("the compositor dismissed the window before configuring it".into()));
        }
        if let Some(theme) = self.state.theme {
            self.state.push_window(id, WindowEvent::ThemeChanged(theme));
        }
        Ok(id)
    }

    fn make_frame_source(&mut self, id: WindowId, layer: Option<SurfaceId>) -> Option<Arc<dyn FrameSource>> {
        let w = self.state.windows.get(&id)?;
        let (surface, lock) = match layer {
            None => (w.surface.clone(), w.lock.clone()),
            Some(l) => {
                let l = w.layers.get(&l)?;
                (l.surface.clone(), l.lock.clone())
            }
        };
        let globals = self.state.frame_globals();
        Some(Arc::new(WaylandFrameSource::new(&self.state.conn, surface, lock, &globals, FrameSourceConfig::default())))
    }

    pub(crate) fn state_mut(&mut self) -> &mut State {
        self.state
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
    fn try_create_window(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError> {
        self.open(desc, None, None)
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
        let layer = Layer { id: layer_id, surface: surface.clone(), subsurface: subsurface.clone(), viewport, rect: desc.bounds, lock: Arc::new(SurfaceLock::default()) };
        if !desc.transparent {
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

    fn set_text_input(&mut self, id: WindowId, client: Option<Box<dyn presto_base::input::TextInputClient>>) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        let off = client.is_none();
        w.text_client = client;
        if off {
            if w.ime.take().is_some() {
                crate::seat::update_ime(self.state, id);
            }
        } else {
            crate::seat::refresh_from_client(self.state, id);
        }
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
                    position: pos,
                    size: DeviceSize { w: w.max(0) as u32, h: h.max(0) as u32 },
                    work_area: None,
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

    fn frame_source(&mut self, id: WindowId) -> Option<Arc<dyn FrameSource>> {
        self.make_frame_source(id, None)
    }

    fn exit(&mut self) {
        self.state.exit = true;
    }

    fn os_layer_committer(&self, _id: WindowId) -> Option<OsLayerCommitter> {
        // Subsurface position and stacking apply with the window's next commit, which the compositor thread
        // makes when it presents the window; layer content commits with each layer present.
        None
    }

    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool {
        crate::seat::end_composition(self.state, id, commit)
    }

    fn set_mouse_capture(&mut self, _id: WindowId, _capture: bool) {
        // Wayland has no explicit capture: a pressed button grabs the pointer implicitly until release, and
        // relative motion (pointer-constraints, relative-pointer) is not bound yet (see the lane record).
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        w.desc.title = title.to_owned();
        if let Some(top) = w.toplevel() {
            top.set_title(title.to_owned());
        }
    }

    fn set_visible(&mut self, _id: WindowId, _visible: bool) {
        // xdg-shell cannot hide a mapped toplevel short of destroying its role; the app closes and recreates
        // windows instead (see the lane record).
    }

    fn drag_window(&mut self, id: WindowId) {
        let Some((seat, serial)) = crate::seat::latest_press(self.state) else { return };
        if let Some(top) = self.state.windows.get(&id).and_then(|w| w.toplevel()) {
            top.move_(&seat, serial);
        }
    }

    fn update_a11y(&mut self, _id: WindowId, _update: presto_base::accesskit::TreeUpdate) {
        // AccessKit on Linux is AT-SPI over D-Bus (accesskit_unix), independent of the display protocol; the
        // consolidation shares presto-platform's adapter with this backend (see the lane record).
    }

    fn window_scale(&self, id: WindowId) -> Option<Scale> {
        self.state.windows.get(&id).map(|w| Scale(w.scale() as f32))
    }

    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, rect: Rect) {
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
}

impl WaylandCx for Cx<'_> {
    fn create_popup(&mut self, desc: WindowDesc, popup: PopupDesc) -> Result<WindowId, PlatformError> {
        self.open(desc, Some(popup), None)
    }

    fn create_child_window(&mut self, desc: WindowDesc, parent: WindowId) -> Result<WindowId, PlatformError> {
        self.open(desc, None, Some(parent))
    }

    fn set_ime_hints(&mut self, id: WindowId, hints: ImeHints) {
        let Some(w) = self.state.windows.get_mut(&id) else { return };
        if w.ime_hints != hints {
            w.ime_hints = hints;
            crate::seat::update_ime(self.state, id);
        }
    }

    fn take_finished_drags(&mut self) -> Vec<(DragId, DragOutcome)> {
        std::mem::take(&mut self.state.finished_drags)
    }

    fn layer_frame_source(&mut self, id: WindowId, layer: SurfaceId) -> Option<Arc<dyn FrameSource>> {
        self.make_frame_source(id, Some(layer))
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
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String> {
        match kind {
            ClipboardKind::Clipboard => crate::data::get_clipboard(self.state),
            ClipboardKind::Primary => crate::data::get_primary(self.state),
        }
    }

    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool {
        let r = match kind {
            ClipboardKind::Clipboard => crate::data::set_clipboard(self.state, text.to_owned()),
            ClipboardKind::Primary => crate::data::set_primary(self.state, text.to_owned()),
        };
        r.is_ok()
    }

    fn has_primary(&self) -> bool {
        crate::data::has_primary(self.state)
    }

    fn clear_primary(&mut self) -> bool {
        crate::data::clear_primary(self.state)
    }
}
