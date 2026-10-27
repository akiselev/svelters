//! [`GalleryDocument`]: the gallery's document as the tree skelter's [`App`](skelter::App) drives.
//!
//! Input goes through the selected story's backend (so its roots' JS gets its events at event time)
//! and the chrome reads what landed on its own nodes from the backend's event tap. In inspect mode a
//! press over a cell picks the node under it and reaches nothing else. Zoom multiplies the window's
//! scale (skelter's UI zoom: the whole window, chrome included, as `skelter-gallery --zoom` scales a
//! shot).

use crate::gallery::Gallery;
use skelter::seams::{Dispatched, DocumentApi};
use skelter_base::accesskit::ActionRequest;
use skelter_base::anim::AnimationEvent;
use skelter_base::commit::Commit;
use skelter_base::event::{InputEnvelope, InputEvent, KeyState, Modifiers, PointerPhase, WindowEvent};
use skelter_base::geom::{Scale, Size, Vec2};
use skelter_base::ids::{PropNodeId, Revision, WindowId};
use skelter_base::key::Key;
use skelter_base::time::Time;
use svelters_skelter::Output;

pub struct GalleryDocument {
    pub gallery: Gallery,
    window: Option<WindowId>,
    /// The revision the window last ran a frame at.
    framed: Option<Revision>,
    /// The platform's size (points at the OS scale) and scale.
    os: (Size, Scale),
    /// The zoom applied to the window.
    zoom: f32,
}

impl GalleryDocument {
    pub fn new(gallery: Gallery) -> Self {
        GalleryDocument { gallery, window: None, framed: None, os: (Size::new(1.0, 1.0), Scale(1.0)), zoom: 1.0 }
    }

    pub fn window(&self) -> Option<WindowId> {
        self.window
    }

    fn revision(&self) -> Revision {
        self.gallery.document().borrow().revision()
    }

    /// The window's size and scale with the zoom.
    fn zoomed(&self) -> (Size, Scale) {
        let z = self.gallery.settings.zoom.max(0.1);
        let (s, sc) = self.os;
        (Size::new(s.w / z, s.h / z), Scale(sc.0 * z))
    }
}

impl DocumentApi for GalleryDocument {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        let before = self.revision();
        // Ctrl+S (Cmd+S) saves the source pane wherever focus is
        if let InputEvent::Key(k) = &ev.event
            && k.state == KeyState::Pressed
            && (k.modifiers.contains(Modifiers::CTRL) || k.modifiers.contains(Modifiers::SUPER))
            && k.key == Some(Key::S)
        {
            self.gallery.save();
            return Dispatched { dirty: true, js_events: 0, fast: Vec::new() };
        }
        if self.gallery.settings.inspect
            && let InputEvent::Pointer(p) = &ev.event
            && p.phase == PointerPhase::Down
            && self.gallery.inspect_at(ev.window, p.pos)
        {
            return Dispatched { dirty: true, js_events: 0, fast: Vec::new() };
        }
        let (events, js) = match self.gallery.backend().cloned() {
            Some(b) => {
                let (d, _) = b.dispatch(ev);
                let mut all = self.gallery.take_tapped();
                if all.is_empty() {
                    all = d.events;
                }
                let n = all.len();
                (all, n)
            }
            None => {
                let d = self.gallery.document().borrow_mut().dispatch(ev);
                let n = d.events.len();
                (d.events, n)
            }
        };
        self.gallery.handle(&events);
        // an edit in the source pane previews now (the editor's own repaint is not a tree change)
        if self.gallery.edit_pending() {
            self.gallery.sync_buffer();
            self.gallery.dirty = true;
        }
        let fast = self.gallery.document().borrow_mut().take_fast_commits();
        Dispatched { dirty: self.revision() != before || self.gallery.dirty, js_events: js, fast }
    }

    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        if self.zoom != self.gallery.settings.zoom {
            self.zoom = self.gallery.settings.zoom;
            let (s, sc) = self.zoomed();
            self.gallery.document().borrow_mut().resize_window(window, s, sc);
        }
        let timed = self.gallery.document().borrow_mut().run_timers(target);
        if let Some(b) = self.gallery.backend().cloned() {
            if !timed.is_empty() {
                b.queue(Output { events: timed.clone(), changed: Vec::new(), revision: None });
            }
            b.frame();
            b.pump();
        }
        self.gallery.handle(&timed);
        self.gallery.before_frame(target);
        let c = self.gallery.document().borrow_mut().frame(window, target);
        if let Some(b) = self.gallery.backend().cloned() {
            b.pump();
        }
        self.gallery.dirty = false;
        self.framed = Some(self.revision());
        c
    }

    fn needs_frame(&self, window: WindowId) -> bool {
        if Some(window) != self.window {
            return false;
        }
        self.gallery.dirty
            || self.gallery.edit_pending()
            || self.zoom != self.gallery.settings.zoom
            || self.gallery.backend().is_some_and(|b| b.frame_pending())
            || self.framed.is_none_or(|r| self.revision() > r)
    }

    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        self.os = (size, scale);
        self.zoom = self.gallery.settings.zoom;
        let (s, sc) = self.zoomed();
        let doc = self.gallery.document().clone();
        let mut d = doc.borrow_mut();
        if self.window == Some(window) {
            d.resize_window(window, s, sc);
        } else if self.window.is_none() {
            if let Err(e) = d.attach_window(window, self.gallery.root(), s, sc) {
                eprintln!("skelter-gallery-app: cannot attach the window: {e:?}");
                return;
            }
            self.window = Some(window);
        }
    }

    fn close_window(&mut self, window: WindowId) {
        if self.window == Some(window) {
            self.gallery.document().borrow_mut().close_window(window);
            self.window = None;
        }
    }

    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.gallery.document().borrow_mut().apply_scroll(window, node, offset);
    }

    fn animation_event(&mut self, _window: WindowId, ev: AnimationEvent) {
        if let Some(b) = self.gallery.backend().cloned() {
            b.animation_event(ev);
            b.pump();
        }
    }

    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool {
        let d = self.gallery.document().borrow_mut().a11y_action(window, req);
        let handled = d.handled;
        self.gallery.handle(&d.events);
        if let Some(b) = self.gallery.backend().cloned() {
            b.queue(Output { events: d.events, changed: Vec::new(), revision: Some(d.revision) });
            b.pump();
        }
        handled
    }

    fn install_clipboard(&mut self, clipboard: Box<dyn skelter_platform::Clipboard>) {
        self.gallery.set_clipboard(clipboard);
    }

    fn accessibility_activated(&mut self, _window: WindowId) {}
    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}
    fn device_reset(&mut self) {}

    fn next_wakeup(&self, _window: WindowId) -> Option<Time> {
        self.gallery.document().borrow().next_timer()
    }
}
