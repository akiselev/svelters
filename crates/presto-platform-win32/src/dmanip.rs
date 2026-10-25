// Ported from Zed's `crates/gpui_windows/src/direct_manipulation.rs` at commit a5c4ca8
// (https://github.com/zed-industries/zed), Copyright 2022 - 2025 Zed Industries, Inc.,
// licensed under the Apache License, Version 2.0. Modifications by the svelters authors:
// presto's event types, gesture-activity tracking for the update timer, no logging.

//! DirectManipulation for precision touchpads: two-finger pan with inertia and pinch, which plain
//! `WM_POINTER`/`WM_MOUSEWHEEL` never deliver. A viewport on the window recognises gestures only (its
//! content never draws); each content update becomes a [`WheelEvent`] with a gesture phase (pixels)
//! or a [`GestureEvent::Pinch`]. The viewport is updated manually: the window calls
//! [`DirectManipulation::update`] on each frame and on a short timer while a gesture or its inertia
//! runs ([`DirectManipulation::active`]).

use presto_base::event::{GestureEvent, GesturePhase, InputEvent, Modifiers, ScrollDelta, WheelEvent};
use presto_base::geom::{Point, Vec2};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::Foundation::{E_POINTER, HWND, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectManipulation::*;
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT};
use windows::Win32::UI::Input::Pointer::GetPointerType;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, POINTER_INPUT_TYPE, PT_TOUCHPAD};

/// The viewport's nominal size in pixels; it only recognises gestures, so the size is arbitrary.
const VIEWPORT_SIZE: i32 = 1000;

pub struct DirectManipulation {
    manager: IDirectManipulationManager,
    update_manager: IDirectManipulationUpdateManager,
    viewport: IDirectManipulationViewport,
    _handler_cookie: u32,
    window: HWND,
    scale: Rc<Cell<f32>>,
    active: Rc<Cell<bool>>,
    pending: Rc<RefCell<Vec<InputEvent>>>,
}

impl DirectManipulation {
    pub fn new(window: HWND, scale: f32) -> windows_core::Result<Self> {
        unsafe {
            let manager: IDirectManipulationManager =
                CoCreateInstance(&DirectManipulationManager, None, CLSCTX_INPROC_SERVER)?;
            let update_manager: IDirectManipulationUpdateManager = manager.GetUpdateManager()?;
            let viewport: IDirectManipulationViewport = manager.CreateViewport(None, window)?;
            viewport.ActivateConfiguration(
                DIRECTMANIPULATION_CONFIGURATION_INTERACTION
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_INERTIA
                    | DIRECTMANIPULATION_CONFIGURATION_RAILS_X
                    | DIRECTMANIPULATION_CONFIGURATION_RAILS_Y
                    | DIRECTMANIPULATION_CONFIGURATION_SCALING,
            )?;
            viewport.SetViewportOptions(
                DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE
                    | DIRECTMANIPULATION_VIEWPORT_OPTIONS_DISABLEPIXELSNAPPING,
            )?;
            let mut rect = RECT { left: 0, top: 0, right: VIEWPORT_SIZE, bottom: VIEWPORT_SIZE };
            viewport.SetViewportRect(&mut rect)?;
            manager.Activate(window)?;
            viewport.Enable()?;

            let scale = Rc::new(Cell::new(scale));
            let active = Rc::new(Cell::new(false));
            let pending = Rc::new(RefCell::new(Vec::new()));
            let handler: IDirectManipulationViewportEventHandler = ViewportHandler {
                window,
                scale: scale.clone(),
                active: active.clone(),
                gesture: Cell::new(Gesture::None),
                last_scale: Cell::new(1.0),
                last_x: Cell::new(0.0),
                last_y: Cell::new(0.0),
                scroll_phase: Cell::new(GesturePhase::Started),
                pending: pending.clone(),
            }
            .into();
            let cookie = viewport.AddEventHandler(Some(window), &handler)?;
            update_manager.Update(None)?;
            Ok(Self { manager, update_manager, viewport, _handler_cookie: cookie, window, scale, active, pending })
        }
    }

    pub fn set_scale(&self, scale: f32) {
        self.scale.set(scale);
    }

    /// `DM_POINTERHITTEST`: hand touchpad contacts to the viewport.
    pub fn on_pointer_hit_test(&self, wparam: WPARAM) {
        let pointer_id = (wparam.0 & 0xFFFF) as u32;
        let mut ty = POINTER_INPUT_TYPE::default();
        unsafe {
            if GetPointerType(pointer_id, &mut ty).is_ok() && ty == PT_TOUCHPAD {
                let _ = self.viewport.SetContact(pointer_id);
            }
        }
    }

    /// Runs the recogniser (manual-update viewport) and returns the gestures it produced.
    pub fn update(&self) -> Vec<InputEvent> {
        unsafe {
            let _ = self.update_manager.Update(None);
        }
        std::mem::take(&mut *self.pending.borrow_mut())
    }

    /// A gesture or its inertia is running: keep calling [`Self::update`].
    pub fn active(&self) -> bool {
        self.active.get()
    }
}

impl Drop for DirectManipulation {
    fn drop(&mut self) {
        unsafe {
            let _ = self.viewport.Stop();
            let _ = self.viewport.Abandon();
            let _ = self.manager.Deactivate(self.window);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesture {
    None,
    Scroll,
    Pinch,
}

/// The modifiers held now (gestures carry no key state of their own).
pub fn os_modifiers() -> Modifiers {
    let held = |vk: u16| unsafe { GetKeyState(vk as i32) } < 0;
    let mut m = Modifiers::default();
    if held(VK_SHIFT.0) {
        m = m.union(Modifiers::SHIFT);
    }
    if held(VK_CONTROL.0) {
        m = m.union(Modifiers::CTRL);
    }
    if held(VK_MENU.0) {
        m = m.union(Modifiers::ALT);
    }
    if held(VK_LWIN.0) || held(VK_RWIN.0) {
        m = m.union(Modifiers::SUPER);
    }
    m
}

#[windows_core::implement(IDirectManipulationViewportEventHandler)]
struct ViewportHandler {
    window: HWND,
    scale: Rc<Cell<f32>>,
    active: Rc<Cell<bool>>,
    gesture: Cell<Gesture>,
    last_scale: Cell<f32>,
    last_x: Cell<f32>,
    last_y: Cell<f32>,
    scroll_phase: Cell<GesturePhase>,
    pending: Rc<RefCell<Vec<InputEvent>>>,
}

impl ViewportHandler {
    fn push(&self, ev: InputEvent) {
        self.pending.borrow_mut().push(ev);
    }

    fn end_gesture(&self) {
        match self.gesture.get() {
            Gesture::Scroll => self.push(InputEvent::Wheel(WheelEvent {
                pos: self.cursor(),
                delta: ScrollDelta::Pixels(Vec2::ZERO),
                phase: Some(GesturePhase::Ended),
                momentum: None,
                modifiers: os_modifiers(),
            })),
            Gesture::Pinch => self.push(InputEvent::Gesture(GestureEvent::Pinch { delta: 0.0, phase: GesturePhase::Ended })),
            Gesture::None => {}
        }
        self.gesture.set(Gesture::None);
    }

    fn cursor(&self) -> Point {
        let s = self.scale.get().max(0.01);
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
            let _ = ScreenToClient(self.window, &mut p);
        }
        Point::new(p.x as f32 / s, p.y as f32 / s)
    }
}

impl IDirectManipulationViewportEventHandler_Impl for ViewportHandler_Impl {
    fn OnViewportStatusChanged(
        &self,
        viewport: windows_core::Ref<'_, IDirectManipulationViewport>,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) -> windows_core::Result<()> {
        if current == previous {
            return Ok(());
        }
        self.active.set(current == DIRECTMANIPULATION_RUNNING || current == DIRECTMANIPULATION_INERTIA);
        // A new gesture interrupted inertia: end the old sequence.
        if current == DIRECTMANIPULATION_RUNNING && previous == DIRECTMANIPULATION_INERTIA {
            self.end_gesture();
        }
        if current == DIRECTMANIPULATION_READY {
            self.end_gesture();
            // Reset the content transform for the next gesture. ZoomToRect itself causes another
            // RUNNING -> READY cycle, hence the check.
            if self.last_scale.get() != 1.0 || self.last_x.get() != 0.0 || self.last_y.get() != 0.0 {
                if let Some(viewport) = viewport.as_ref() {
                    unsafe {
                        let _ = viewport.ZoomToRect(0.0, 0.0, VIEWPORT_SIZE as f32, VIEWPORT_SIZE as f32, false);
                    }
                }
            }
            self.last_scale.set(1.0);
            self.last_x.set(0.0);
            self.last_y.set(0.0);
        }
        Ok(())
    }

    fn OnViewportUpdated(&self, _viewport: windows_core::Ref<'_, IDirectManipulationViewport>) -> windows_core::Result<()> {
        Ok(())
    }

    fn OnContentUpdated(
        &self,
        _viewport: windows_core::Ref<'_, IDirectManipulationViewport>,
        content: windows_core::Ref<'_, IDirectManipulationContent>,
    ) -> windows_core::Result<()> {
        let content = content.as_ref().ok_or(E_POINTER)?;
        // [scale, 0, 0, scale, tx, ty]
        let mut xform = [0.0f32; 6];
        unsafe { content.GetContentTransform(&mut xform)? };
        let scale = xform[0];
        let s = self.scale.get().max(0.01);
        let (x, y) = (xform[4] / s, xform[5] / s);
        if scale == 0.0 {
            return Ok(());
        }
        let (last_scale, last_x, last_y) = (self.last_scale.get(), self.last_x.get(), self.last_y.get());
        if float_equals(scale, last_scale) && float_equals(x, last_x) && float_equals(y, last_y) {
            return Ok(());
        }
        // Every update carries both translation and scale (the zoom centre moves during a pinch), so
        // each gesture is classified once: scroll may turn into pinch, never the reverse.
        if !float_equals(scale, 1.0) {
            if self.gesture.get() != Gesture::Pinch {
                self.end_gesture();
                self.gesture.set(Gesture::Pinch);
                self.push(InputEvent::Gesture(GestureEvent::Pinch { delta: 0.0, phase: GesturePhase::Started }));
            }
        } else if self.gesture.get() == Gesture::None {
            self.gesture.set(Gesture::Scroll);
            self.scroll_phase.set(GesturePhase::Started);
        }
        match self.gesture.get() {
            Gesture::Scroll => {
                let phase = self.scroll_phase.get();
                self.scroll_phase.set(GesturePhase::Moved);
                self.push(InputEvent::Wheel(WheelEvent {
                    pos: self.cursor(),
                    delta: ScrollDelta::Pixels(Vec2::new(x - last_x, y - last_y)),
                    phase: Some(phase),
                    momentum: None,
                    modifiers: os_modifiers(),
                }));
            }
            Gesture::Pinch => {
                self.push(InputEvent::Gesture(GestureEvent::Pinch { delta: scale / last_scale - 1.0, phase: GesturePhase::Moved }));
            }
            Gesture::None => {}
        }
        self.last_scale.set(scale);
        self.last_x.set(x);
        self.last_y.set(y);
        Ok(())
    }
}

pub fn float_equals(a: f32, b: f32) -> bool {
    const EPSILON: f32 = 0.00001;
    (a - b).abs() < EPSILON * a.abs().max(b.abs()).max(EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_float_comparison() {
        assert!(float_equals(1.0, 1.000001));
        assert!(!float_equals(1.0, 1.001));
        assert!(float_equals(0.0, 0.0));
        assert!(!float_equals(0.0, 0.001));
    }
}
