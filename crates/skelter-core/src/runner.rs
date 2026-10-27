//! The entries the app runner drives besides input and frames (APP-TEST's requests): whether a
//! window needs a UI frame at all, event-time hover for coalesced pointer moves, AccessKit actions,
//! window events, a GPU device reset, a full accessibility tree on demand, and the IME state.
//!
//! AccessKit ids: the projection uses a node's skelter [`NodeId`] as its AccessKit id
//! (`accesskit::NodeId(node.0)`), so an `ActionRequest`'s `target_node` is the node itself.

use crate::document::Document;
use crate::tree::Dirty;
use skelter_base::event::{Modifiers, PointerEvent, PointerPhase, PointerType, Theme, WindowEvent};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::input::InputPurpose;
use svelters_style::StateMask;

impl Document {
    /// Whether a frame of `window` would do any work: style, layout, recording or property work
    /// queued, a rebuild or removals pending, UI transitions running, a cursor or resources to send.
    /// A dispatch that changed nothing visible (a hover over nodes no rule styles) leaves it false.
    pub fn needs_frame(&self, window: WindowId) -> bool {
        let Some(w) = self.window(window) else { return false };
        if !w.laid_out || w.dirty.full || !w.dirty.relayout.is_empty() || !w.dirty.remeasure.is_empty() || w.rebuild || !w.pending.is_empty() {
            return true;
        }
        if self.idx(w.root).is_some_and(|i| !self.core[i].dirty.is_empty()) {
            return true;
        }
        if self.input.cursor.is_some() || !self.images.added.is_empty() || !self.images.removed.is_empty() || !self.images.paths.is_empty() || !self.images.removed_paths.is_empty() {
            return true;
        }
        if !self.anim.orphaned.is_empty() {
            return true;
        }
        self.anim.ui.keys().any(|&n| self.window_of(n) == Some(window))
    }

    /// Hover at `pos` (window points) now, for continuous input whose JS `pointermove` is
    /// coalesced to the frame (§11.1): `:hover`, the kinds' hover enter/leave and the JS
    /// `pointerenter`/`pointerleave` (queued; [`Document::take_events`]). True when the hovered path
    /// changed.
    ///
    /// The `title` tip steps at the last input's time; a caller that knows when the pointer came
    /// to rest (the move's own time, or the frame's) calls [`Document::hover_at`] instead, so the
    /// tip's delay starts then.
    pub fn hover(&mut self, window: WindowId, pos: Point) -> bool {
        let now = self.input.now;
        self.hover_at(window, pos, now)
    }

    /// [`Document::hover`] at `now`: the document's input clock moves to `now` (never back), so a
    /// `title` tip the new hover starts waits its whole delay from `now` rather than from the
    /// previous input's time.
    pub fn hover_at(&mut self, window: WindowId, pos: Point, now: skelter_base::time::Time) -> bool {
        if now > self.input.now {
            self.input.now = now;
        }
        let p = PointerEvent {
            phase: PointerPhase::Move,
            pointer: 0,
            kind: PointerType::Mouse,
            button: None,
            buttons: 0,
            pos,
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        };
        let before = self.input.hover.get(&0).cloned().unwrap_or_default();
        let captured = self.input.capture.get(&0).copied().filter(|&n| self.contains(n));
        let target = captured.or_else(|| self.hit_test(window, pos));
        let mut out = crate::input::Dispatched { window, revision: self.revision, ..Default::default() };
        let mut emitted = Vec::new();
        self.transaction(|doc| doc.update_hover(window, &p, target, &mut out, &mut emitted));
        for e in emitted {
            crate::input::dispatch::queue_emitted(self, e);
        }
        self.input_events.append(&mut out.events);
        let changed = self.input.hover.get(&0).is_some_and(|now| *now != before);
        if changed {
            self.step_tip(window, false, false, false);
        }
        changed
    }

    /// An assistive technology's action: true when the target's kind or a default (focus, blur,
    /// scroll into view) handled it. The JS events it caused are queued ([`Document::take_events`]).
    pub fn accessibility_action(&mut self, window: WindowId, req: &accesskit::ActionRequest) -> bool {
        let out = self.a11y_action(window, req);
        self.input_events.extend(out.events);
        out.handled
    }

    /// The next frame of `window` sends the whole accessibility tree (an assistive technology
    /// activated, or the adapter was recreated).
    pub fn a11y_full_tree(&mut self, window: WindowId) {
        if let Some(w) = self.window_mut(window) {
            w.a11y_sent.clear();
        }
    }

    /// A platform window event: size and scale go to [`Document::resize_window`]; losing the
    /// window's activation sets `:backdrop` on its root (GTK's); the OS theme switches
    /// `prefers-color-scheme`; occlusion and the rest change nothing in the tree.
    pub fn window_event(&mut self, window: WindowId, ev: &WindowEvent) {
        match ev {
            WindowEvent::Resized { size, scale } => {
                let s = if scale.0 > 0.0 { scale.0 } else { 1.0 };
                self.resize_window(window, Size::new(size.w as f32 / s, size.h as f32 / s), *scale);
            }
            WindowEvent::ScaleChanged { scale } => {
                if let Some(size) = self.window(window).map(|w| w.size) {
                    self.resize_window(window, size, *scale);
                }
            }
            WindowEvent::Focused(on) => {
                if let Some(root) = self.window_root(window) {
                    self.set_state(root, StateMask::BACKDROP, !on);
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                let scheme = match theme {
                    Theme::Light => svelters_style::ColorScheme::Light,
                    Theme::Dark => svelters_style::ColorScheme::Dark,
                };
                let env = svelters_style::MediaEnv { color_scheme: scheme, ..*self.style.stylist.media_env() };
                if env != *self.style.stylist.media_env() {
                    let _ = self.style.stylist.set_media(env);
                    self.style.caches.clear();
                    self.restyle_all();
                }
            }
            WindowEvent::Moved { .. }
            | WindowEvent::Occluded(_)
            | WindowEvent::CloseRequested
            | WindowEvent::MonitorChanged => {}
        }
    }

    /// The GPU device was lost and recreated (`ToUi::DeviceReset`): the next frame of every window
    /// re-sends its property trees, layers, display lists, runs, and the live images and paths.
    pub fn device_reset(&mut self) {
        let windows: Vec<(WindowId, NodeId)> = self.windows.iter().map(|w| (w.id, w.root)).collect();
        for (window, root) in windows {
            let Some(ri) = self.idx(root) else { continue };
            let mut stack = vec![ri];
            while let Some(i) = stack.pop() {
                let p = &mut self.record[i];
                p.transform = None;
                p.clip = None;
                p.effect = None;
                p.scroll = None;
                p.content = None;
                p.layer = None;
                stack.extend(self.children[i].iter().map(|&c| self.ix(c)));
            }
            self.mark_subtree(ri, Dirty::PAINT | Dirty::PROPERTY | Dirty::CHILDREN);
            if let Some(w) = self.window_mut(window) {
                w.props = Default::default();
                w.layer_ids = Default::default();
                w.root_transform = None;
                w.root_layer = None;
                w.layers.clear();
                w.items.clear();
                w.scroll_nodes.clear();
                w.pending = Default::default();
                w.rebuild = true;
            }
        }
        self.images.added.extend(self.images.live_images.iter().map(|(id, d)| (*id, d.clone())));
        self.images.paths.extend(self.images.live_paths.iter().map(|(id, d)| (*id, d.clone())));
    }

    /// The IME state of `window`: the focused text kind's candidate area (window points; its
    /// composition or selection, else its box) and input purpose. `None`: nothing editable has
    /// focus (the platform turns the IME off).
    pub fn ime_state(&mut self, window: WindowId) -> Option<(Rect, InputPurpose)> {
        let node = self.focused(window)?;
        let i = self.idx(node)?;
        let mut k = self.kinds[i].take()?;
        let r = k.text_input().map(|c| (c.marked_range().or_else(|| c.selected_range()).and_then(|r| c.bounds_for_range(r)), c.purpose()));
        self.kinds[i] = Some(k);
        let (area, purpose) = r?;
        Some((area.unwrap_or(self.record[i].world), purpose))
    }
}
