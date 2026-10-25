//! Windows: xdg toplevels and popups.
//!
//! A window's size is in points (the xdg window geometry). Its buffer is `round(size × scale)` device
//! pixels: with `wp_fractional_scale_v1` and `wp_viewporter` the scale is the compositor's preferred
//! fractional scale and the viewport maps the buffer onto the surface; without them it is the integer buffer
//! scale. The buffer-dependent state goes through the window's [`SurfaceLock`] (see `present.rs`).
//!
//! Decorations: the window asks for server-side decorations through `xdg-decoration` when its description
//! wants decorations. Where the compositor has none or picks client-side, the app draws them and
//! `client_side_decorations` says so; our hit-test regions drive the title bar (`csd.rs`).

use crate::app::State;
use crate::csd::CsdState;
use crate::layers::Layer;
use crate::present::{Geometry, SurfaceLock};
use crate::csd::CsdFrame;
use crate::shim::{HitRegion, ImeHints, ImeState, PopupDesc, PopupGravity, WindowDesc};
use skelter_base::cursor::CursorIcon;
use skelter_base::event::WindowEvent;
use skelter_base::geom::{DeviceSize, Scale};
use skelter_base::ids::{SurfaceId, WindowId};
use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::reexports::csd_frame::WindowState;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::popup::{Popup, PopupConfigure, PopupHandler};
use smithay_client_toolkit::shell::xdg::window::{DecorationMode, Window as XdgWindow, WindowConfigure, WindowDecorations, WindowHandler};
use smithay_client_toolkit::shell::xdg::{XdgPositioner, XdgSurface};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{self, WpFractionalScaleV1};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::xdg::shell::client::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};
use wayland_protocols::xdg::shell::client::xdg_surface;

pub enum Role {
    Toplevel(XdgWindow),
    Popup(Popup),
}

pub struct Window {
    pub id: WindowId,
    pub role: Role,
    pub surface: WlSurface,
    pub desc: WindowDesc,
    pub fractional: Option<WpFractionalScaleV1>,
    pub viewport: Option<WpViewport>,
    /// Size in points.
    pub size: (u32, u32),
    /// The compositor's preferred scale in 120ths (fractional-scale-v1), once it sent one.
    pub scale120: Option<u32>,
    /// The integer buffer scale the compositor prefers (or the outputs' maximum).
    pub int_scale: i32,
    pub configured: bool,
    pub decoration: DecorationMode,
    pub states: WindowState,
    pub hit_regions: Vec<HitRegion>,
    pub csd: CsdState,
    pub cursor: CursorIcon,
    pub ime: Option<ImeState>,
    pub ime_hints: ImeHints,
    /// The focused editor, when the app hands it over (`set_text_input`): the IME edits it directly.
    pub text_client: Option<Box<dyn skelter_base::input::TextInputClient>>,
    pub layers: HashMap<SurfaceId, Layer>,
    pub lock: Arc<SurfaceLock>,
    reported: Option<(DeviceSize, f32)>,
}

impl Window {
    pub fn xdg_surface(&self) -> &xdg_surface::XdgSurface {
        match &self.role {
            Role::Toplevel(w) => w.xdg_surface(),
            Role::Popup(p) => p.xdg_surface(),
        }
    }

    /// What the CSD module needs: the size and whether the resize border is active.
    pub fn csd_frame(&self) -> CsdFrame {
        let constrained = self.states.intersects(
            WindowState::MAXIMIZED
                | WindowState::FULLSCREEN
                | WindowState::TILED_LEFT
                | WindowState::TILED_RIGHT
                | WindowState::TILED_TOP
                | WindowState::TILED_BOTTOM,
        );
        CsdFrame {
            size: (self.size.0 as f32, self.size.1 as f32),
            border: self.client_side_decorations() && self.desc.resizable && !constrained,
        }
    }

    pub fn toplevel(&self) -> Option<&XdgWindow> {
        match &self.role {
            Role::Toplevel(w) => Some(w),
            Role::Popup(_) => None,
        }
    }

    /// The scale content renders at.
    pub fn scale(&self) -> f64 {
        match (self.scale120, &self.viewport) {
            (Some(s), Some(_)) => s as f64 / 120.0,
            _ => self.int_scale as f64,
        }
    }

    /// The app draws its own title bar and frame.
    pub fn client_side_decorations(&self) -> bool {
        self.toplevel().is_some()
            && (self.desc.custom_title_bar || (self.desc.decorations && self.decoration == DecorationMode::Client))
    }

    fn uses_viewport(&self) -> bool {
        self.scale120.is_some() && self.viewport.is_some()
    }

    /// `round(points × scale)`, with fractional-scale-v1's rounding (half away from zero).
    pub fn buffer_size(&self) -> DeviceSize {
        buffer_size(self.size, self.scale120.filter(|_| self.viewport.is_some()), self.int_scale)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // Layers and per-surface extension objects go before the surface itself.
        self.lock.clear_children();
        self.layers.clear();
        if let Some(f) = self.fractional.take() {
            f.destroy();
        }
        if let Some(v) = self.viewport.take() {
            v.destroy();
        }
    }
}

/// The buffer size for a surface of `size` points at `scale120` (fractional) or `int_scale`.
pub fn buffer_size(size: (u32, u32), scale120: Option<u32>, int_scale: i32) -> DeviceSize {
    match scale120 {
        Some(s) => DeviceSize { w: ((size.0 as u64 * s as u64 + 60) / 120) as u32, h: ((size.1 as u64 * s as u64 + 60) / 120) as u32 },
        None => {
            let k = int_scale.max(1) as u32;
            DeviceSize { w: size.0 * k, h: size.1 * k }
        }
    }
}

fn clamp_size(size: (u32, u32), desc: &WindowDesc) -> (u32, u32) {
    let (mut w, mut h) = (size.0 as f32, size.1 as f32);
    if let Some(min) = desc.min_size {
        w = w.max(min.w);
        h = h.max(min.h);
    }
    if let Some(max) = desc.max_size {
        w = w.min(max.w);
        h = h.min(max.h);
    }
    (w.round().max(1.0) as u32, h.round().max(1.0) as u32)
}

/// Creates the window's objects and sends the initial commit. The window is usable once its first configure
/// arrived (`configured`); the caller dispatches until then.
pub fn create(
    state: &mut State,
    id: WindowId,
    desc: WindowDesc,
    popup: Option<PopupDesc>,
    parent: Option<WindowId>,
) -> Result<(), crate::shim::PlatformError> {
    let qh = state.qh.clone();
    let initial = clamp_size((desc.size.w.round().max(1.0) as u32, desc.size.h.round().max(1.0) as u32), &desc);
    let (role, surface) = match &popup {
        None => {
            let surface = state.compositor.create_surface(&qh);
            // A custom title bar asks for client-side mode: the app draws it, the compositor draws nothing.
            let decorations = if desc.decorations && !desc.custom_title_bar {
                WindowDecorations::RequestServer
            } else {
                WindowDecorations::RequestClient
            };
            let w = state.xdg.create_window(surface.clone(), decorations, &qh);
            w.set_title(desc.title.clone());
            if let Some(app_id) = &desc.app_id {
                w.set_app_id(app_id.clone());
            }
            let (min, max) = if desc.resizable {
                (desc.min_size.map(round_size), desc.max_size.map(round_size))
            } else {
                (Some(initial), Some(initial))
            };
            w.set_min_size(min);
            w.set_max_size(max);
            if let Some(parent) = parent.and_then(|p| state.windows.get(&p)).and_then(|p| p.toplevel()) {
                w.set_parent(Some(parent));
            }
            if desc.maximized {
                w.set_maximized();
            }
            (Role::Toplevel(w), surface)
        }
        Some(p) => {
            let popup = create_popup(state, p, initial)?;
            let surface = popup.wl_surface().clone();
            (Role::Popup(popup), surface)
        }
    };
    let fractional = state.protocols.fractional_scale.as_ref().map(|m| m.get_fractional_scale(&surface, &qh, id));
    let viewport = state.protocols.viewporter.as_ref().map(|v| v.get_viewport(&surface, &qh, ()));
    state.surfaces.insert(surface.id(), id);
    let window = Window {
        id,
        role,
        surface: surface.clone(),
        desc,
        fractional,
        viewport,
        size: initial,
        scale120: None,
        int_scale: 1,
        configured: false,
        decoration: DecorationMode::Client,
        states: WindowState::empty(),
        hit_regions: Vec::new(),
        csd: CsdState::default(),
        cursor: CursorIcon::Default,
        ime: None,
        ime_hints: ImeHints::default(),
        text_client: None,
        layers: HashMap::new(),
        lock: Arc::new(SurfaceLock::default()),
        reported: None,
    };
    if let Role::Toplevel(w) = &window.role {
        // The initial commit: no buffer until the first configure.
        w.commit();
    }
    state.windows.insert(id, window);
    Ok(())
}

fn round_size(s: skelter_base::geom::Size) -> (u32, u32) {
    (s.w.round().max(1.0) as u32, s.h.round().max(1.0) as u32)
}

fn create_popup(state: &mut State, p: &PopupDesc, size: (u32, u32)) -> Result<Popup, crate::shim::PlatformError> {
    let missing = || crate::shim::PlatformError::Os("the compositor lacks xdg_wm_base (popup)".into());
    let parent = state.windows.get(&p.parent).ok_or(crate::shim::PlatformError::Os("popup without a live parent".into()))?;
    let parent_xdg = parent.xdg_surface().clone();
    let positioner = XdgPositioner::new(&state.xdg).map_err(|_| missing())?;
    positioner.set_size(size.0.max(1) as i32, size.1.max(1) as i32);
    let a = p.anchor;
    positioner.set_anchor_rect(
        a.min.x.floor() as i32,
        a.min.y.floor() as i32,
        (a.max.x - a.min.x).ceil().max(1.0) as i32,
        (a.max.y - a.min.y).ceil().max(1.0) as i32,
    );
    let (anchor, gravity, adjust) = match p.gravity {
        PopupGravity::Below => (Anchor::BottomLeft, Gravity::BottomRight, ConstraintAdjustment::FlipY | ConstraintAdjustment::SlideX),
        PopupGravity::Above => (Anchor::TopLeft, Gravity::TopRight, ConstraintAdjustment::FlipY | ConstraintAdjustment::SlideX),
        PopupGravity::Right => (Anchor::TopRight, Gravity::BottomRight, ConstraintAdjustment::FlipX | ConstraintAdjustment::SlideY),
        PopupGravity::Left => (Anchor::TopLeft, Gravity::BottomLeft, ConstraintAdjustment::FlipX | ConstraintAdjustment::SlideY),
    };
    positioner.set_anchor(anchor);
    positioner.set_gravity(gravity);
    positioner.set_constraint_adjustment(adjust | ConstraintAdjustment::ResizeY);
    let surface = Surface::new(&state.compositor, &state.qh).map_err(|_| missing())?;
    let popup = Popup::from_surface(Some(&parent_xdg), &positioner, &state.qh, surface, &state.xdg).map_err(|_| missing())?;
    if p.grab {
        if let Some((seat, serial)) = crate::seat::latest_press(state) {
            popup.xdg_popup().grab(&seat, serial);
        }
    }
    popup.wl_surface().commit();
    Ok(popup)
}

/// Recomputes the buffer size and scale after a configure or scale change: records the surface state for the
/// next matching present, switches layers to sync mode for the resize, and reports `ScaleChanged`/`Resized`.
pub fn apply_geometry(state: &mut State, id: WindowId, interactive: bool) {
    let Some(w) = state.windows.get_mut(&id) else { return };
    if !w.configured {
        return;
    }
    let buffer = w.buffer_size();
    let scale = w.scale();
    let use_vp = w.uses_viewport();
    w.lock.set_geometry(Geometry {
        buffer,
        logical: (w.size.0 as i32, w.size.1 as i32),
        buffer_scale: if use_vp { 1 } else { w.int_scale.max(1) },
        viewport: if use_vp { w.viewport.clone() } else { None },
        xdg_surface: Some(w.xdg_surface().clone()),
    });
    w.lock.begin_resize(buffer, interactive);
    if !w.desc.transparent {
        let region = state.compositor.wl_compositor().create_region(&state.qh, ());
        region.add(0, 0, w.size.0 as i32, w.size.1 as i32);
        w.surface.set_opaque_region(Some(&region));
        region.destroy();
    }
    for layer in w.layers.values() {
        layer.apply_geometry(w.scale120.filter(|_| w.viewport.is_some()), w.int_scale);
    }
    let prev = w.reported;
    let now = (buffer, scale as f32);
    if prev != Some(now) {
        w.reported = Some(now);
        if prev.is_some_and(|(_, s)| s != now.1) {
            state.push_window(id, WindowEvent::ScaleChanged { scale: Scale(now.1) });
        }
        state.push_window(id, WindowEvent::Resized { size: buffer, scale: Scale(now.1) });
    }
}

/// sctk reports the integer scale (`wl_surface.preferred_buffer_scale`, or the outputs the surface is on).
pub fn integer_scale_changed(state: &mut State, id: WindowId, surface: &WlSurface, factor: i32) {
    let Some(w) = state.windows.get_mut(&id) else { return };
    if &w.surface != surface || w.int_scale == factor {
        return;
    }
    w.int_scale = factor.max(1);
    if !w.uses_viewport() {
        apply_geometry(state, id, false);
    }
}

impl Dispatch<WpFractionalScaleV1, WindowId> for State {
    fn event(state: &mut Self, _: &WpFractionalScaleV1, event: wp_fractional_scale_v1::Event, id: &WindowId, _: &Connection, _: &QueueHandle<Self>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            let Some(w) = state.windows.get_mut(id) else { return };
            if w.scale120 == Some(scale) {
                return;
            }
            w.scale120 = Some(scale);
            apply_geometry(state, *id, false);
        }
    }
}

impl WindowHandler for State {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow) {
        if let Some(id) = self.window_of(window.wl_surface()) {
            self.push_window(id, WindowEvent::CloseRequested);
        }
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow, cfg: WindowConfigure, _: u32) {
        let Some(id) = self.window_of(window.wl_surface()) else { return };
        let Some(w) = self.windows.get_mut(&id) else { return };
        let constrained = cfg.state.intersects(
            WindowState::MAXIMIZED
                | WindowState::FULLSCREEN
                | WindowState::TILED_LEFT
                | WindowState::TILED_RIGHT
                | WindowState::TILED_TOP
                | WindowState::TILED_BOTTOM,
        );
        let proposed = (cfg.new_size.0.map_or(w.size.0, |v| v.get()), cfg.new_size.1.map_or(w.size.1, |v| v.get()));
        w.size = if constrained { proposed } else { clamp_size(proposed, &w.desc) };
        let was_csd = w.client_side_decorations();
        w.decoration = cfg.decoration_mode;
        let csd_changed = w.configured && was_csd != w.client_side_decorations();
        let old = w.states;
        w.states = cfg.state;
        w.configured = true;
        let mut events = Vec::new();
        if old.contains(WindowState::ACTIVATED) != cfg.state.contains(WindowState::ACTIVATED) {
            events.push(WindowEvent::Focused(cfg.state.contains(WindowState::ACTIVATED)));
        }
        if old.contains(WindowState::SUSPENDED) != cfg.state.contains(WindowState::SUSPENDED) {
            events.push(WindowEvent::Occluded(cfg.state.contains(WindowState::SUSPENDED)));
        }
        if csd_changed {
            // The app re-reads `client_side_decorations` on resize; force one.
            w.reported = None;
        }
        for e in events {
            self.push_window(id, e);
        }
        apply_geometry(self, id, cfg.is_resizing());
    }
}

impl PopupHandler for State {
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup, cfg: PopupConfigure) {
        let Some(id) = self.window_of(popup.wl_surface()) else { return };
        let Some(w) = self.windows.get_mut(&id) else { return };
        w.size = (cfg.width.max(1) as u32, cfg.height.max(1) as u32);
        w.configured = true;
        apply_geometry(self, id, false);
    }

    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
        if let Some(id) = self.window_of(popup.wl_surface()) {
            self.push_window(id, WindowEvent::CloseRequested);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_buffer_sizes_round_half_up() {
        // 1.5×: 801 points → 1201.5 → 1202.
        assert_eq!(buffer_size((801, 600), Some(180), 1), DeviceSize { w: 1202, h: 900 });
        // 1.25×
        assert_eq!(buffer_size((1000, 3), Some(150), 1), DeviceSize { w: 1250, h: 4 });
        // Integer scales multiply.
        assert_eq!(buffer_size((640, 480), None, 2), DeviceSize { w: 1280, h: 960 });
        assert_eq!(buffer_size((640, 480), None, 0), DeviceSize { w: 640, h: 480 });
    }

    #[test]
    fn floating_sizes_respect_min_and_max() {
        use skelter_base::geom::Size;
        let desc = WindowDesc { min_size: Some(Size::new(300.0, 200.0)), max_size: Some(Size::new(1000.0, 800.0)), ..WindowDesc::default() };
        assert_eq!(clamp_size((100, 100), &desc), (300, 200));
        assert_eq!(clamp_size((2000, 900), &desc), (1000, 800));
        assert_eq!(clamp_size((500, 500), &desc), (500, 500));
    }
}
