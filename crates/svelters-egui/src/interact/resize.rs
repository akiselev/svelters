//! `Resizable` (OW2, ARCH-WIDGETS §1.2; DESIGN R11): a sash that resizes something along one
//! axis, by pointer drag or by keyboard while it is focused, with WAI-ARIA's window-splitter keys.
//! Used by `split-view` (one per sash) and `panel` (its inner edge); OW5 (table columns) and OW7
//! (inner-window edges) reuse it.
//!
//! Runs on the sash part's own response and state bits (`::part(sash)`: `:hover`, `:active`,
//! `:dragging`, `:focus`, `:focus-visible`). The host sets the current size before each run
//! ([`Resizable::set_size`]) and applies the intents.
//!
//! - Drag: `Start`, `Resize { size }` while moving (clamped to `min`..`max`; below half of `min`
//!   a collapsible pane goes to 0), `Commit` on release. Escape while dragging: `Cancel` with the
//!   size from before the drag.
//! - Keys on the focused sash: arrows along the axis by `step` (Shift: ×5), Home/End to the
//!   minimum and maximum, Enter collapses or restores (collapsible).
//! - Double click: restores the last size before a collapse, or collapses (QW16).
//! - R11: [`Resizable::hover_shown`] turns on after `ScrollPolicy::sash_hover_delay` of hovering
//!   (the hover bar and grip), at once while dragging or focused.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::{Key, Modifiers, accesskit};

/// What a host configures on a [`Resizable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeParams {
    /// The axis the size is measured on: 0 (width, a vertical sash) or 1 (height).
    pub axis: usize,
    /// +1: moving the pointer towards larger coordinates grows the size; -1: shrinks it (a
    /// right-side panel, the second pane of a pair).
    pub sign: f32,
    pub min: f32,
    pub max: f32,
    /// Points per arrow key.
    pub step: f32,
    /// Dragging below half the minimum collapses to 0 (Qt's `childrenCollapsible`).
    pub collapsible: bool,
    /// A double click restores or collapses.
    pub double_click: bool,
}

impl Default for ResizeParams {
    fn default() -> Self {
        ResizeParams { axis: 0, sign: 1.0, min: 0.0, max: f32::INFINITY, step: 10.0, collapsible: false, double_click: true }
    }
}

/// What [`Resizable`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resize {
    /// A drag started (`drag-started`).
    Start,
    /// The size to show now (live).
    Resize { size: f32 },
    /// The drag ended (or a key step, a restore): the size to keep (`commit`, `drag-stopped`).
    Commit { size: f32 },
    /// Escape during a drag: back to `size`.
    Cancel { size: f32 },
}

#[derive(Debug, Default)]
pub struct Resizable {
    size: f32,
    /// (size, pointer coordinate) at the start of the drag.
    start: Option<(f32, f32)>,
    hover_since: Option<f64>,
    hovered: bool,
    /// The size before the last collapse (double click or Enter restores it).
    restore: Option<f32>,
    focused: bool,
    focus_visible: bool,
    last_click: Option<f64>,
    now: f64,
    delay: f32,
}

impl Resizable {
    /// The host's current size (before the run).
    pub fn set_size(&mut self, size: f32) {
        if self.start.is_none() {
            self.size = size;
        }
    }
    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn dragging(&self) -> bool {
        self.start.is_some()
    }
    /// R11: the hover bar and grip show (after the hover delay, or at once while dragged or
    /// focused).
    pub fn hover_shown(&self) -> bool {
        self.start.is_some() || self.focused || self.hover_since.is_some_and(|t| self.now - t >= self.delay as f64)
    }
    /// The pointer is over the sash (the resize cursor shows at once).
    pub fn hovered(&self) -> bool {
        self.hovered
    }

    fn clamp(&self, p: &ResizeParams, v: f32) -> f32 {
        let (lo, hi) = (p.min.max(0.0), p.max.max(p.min.max(0.0)));
        if p.collapsible && v < lo / 2.0 { 0.0 } else { v.clamp(lo, hi) }
    }

    fn set(&mut self, v: f32, out: &mut Intents<Resize>, commit: bool) {
        if v > 0.0 && self.size == 0.0 {
            self.restore = None;
        } else if v == 0.0 && self.size > 0.0 {
            self.restore = Some(self.size);
        }
        let changed = v != self.size;
        self.size = v;
        if changed && !commit {
            out.push(Resize::Resize { size: v });
        }
        if commit && changed {
            out.push(Resize::Resize { size: v });
            out.push(Resize::Commit { size: v });
        }
    }

    fn toggle_collapse(&mut self, p: &ResizeParams, out: &mut Intents<Resize>) {
        if self.size == 0.0 {
            let back = self.restore.unwrap_or(p.min.max(1.0));
            let v = self.clamp(p, back).max(p.min);
            self.set(v, out, true);
        } else if p.collapsible {
            self.set(0.0, out, true);
        }
    }
}

impl Unit for Resizable {
    type Params = ResizeParams;
    type Intent = Resize;
    const OWNS: StateBits = StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::DRAGGING.0 | StateBits::FOCUSED.0 | StateBits::FOCUS_VISIBLE.0);

    fn sense(&self, _p: &ResizeParams) -> egui::Sense {
        egui::Sense::click_and_drag()
    }

    fn input(&mut self, p: &ResizeParams, cx: &mut InputCx<'_>, out: &mut Intents<Resize>) {
        let r = *cx.resp;
        self.now = cx.now;
        self.delay = cx.policy.scroll.sash_hover_delay;
        let hovered = cx.enabled && (r.hovered || r.contains_pointer && r.dragged);
        self.hovered = hovered || self.start.is_some();
        match (hovered, self.hover_since) {
            (true, None) => self.hover_since = Some(cx.now),
            (false, Some(_)) => self.hover_since = None,
            _ => {}
        }
        let was_focused = self.focused;
        self.focused = cx.enabled && r.has_focus;
        if !cx.enabled {
            self.start = None;
        } else {
            let axis = p.axis;
            if r.drag_started
                && let Some(pos) = r.interact_pointer_pos
            {
                // the drag starts where the press was (egui reports the start after the threshold)
                let origin = cx.input.pointer.press_origin().unwrap_or(pos);
                self.start = Some((self.size, origin[axis]));
                out.push(Resize::Start);
            }
            if let Some((s0, p0)) = self.start {
                let escape = cx.input.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Escape, pressed: true, .. }));
                if escape {
                    self.start = None;
                    self.size = s0;
                    out.push(Resize::Cancel { size: s0 });
                } else if r.dragged || r.drag_stopped {
                    if let Some(pos) = r.interact_pointer_pos.or(cx.input.pointer.latest_pos()) {
                        let v = self.clamp(p, s0 + p.sign * (pos[axis] - p0)).round();
                        self.set(v, out, false);
                    }
                    if r.drag_stopped {
                        self.start = None;
                        out.push(Resize::Commit { size: self.size });
                    }
                } else if !cx.input.pointer.primary_down() {
                    self.start = None;
                    out.push(Resize::Commit { size: self.size });
                }
            }
            // the click count is ours (W2): two clicks within the platform's double-click time
            let mut double = r.double_clicked;
            if r.clicked {
                let within = self.last_click.is_some_and(|t| (cx.now - t) * 1000.0 <= cx.policy.double_click_ms as f64);
                double |= within;
                self.last_click = if within { None } else { Some(cx.now) };
            }
            if p.double_click && double {
                self.toggle_collapse(p, out);
            }
            if self.focused {
                let (less, more) = if axis == 0 { (Key::ArrowLeft, Key::ArrowRight) } else { (Key::ArrowUp, Key::ArrowDown) };
                for e in &cx.input.events {
                    let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
                    if modifiers.command || modifiers.alt {
                        continue;
                    }
                    let mul = if modifiers.shift { 5.0 } else { 1.0 };
                    let v = match *key {
                        k if k == more => Some(self.size + p.sign * p.step * mul),
                        k if k == less => Some(self.size - p.sign * p.step * mul),
                        Key::Home => Some(p.min.max(0.0)),
                        Key::End if p.max.is_finite() => Some(p.max),
                        Key::Enter => {
                            self.toggle_collapse(p, out);
                            None
                        }
                        _ => None,
                    };
                    if let Some(v) = v {
                        let v = v.clamp(p.min.max(0.0), p.max.max(p.min.max(0.0))).round();
                        self.set(v, out, true);
                    }
                }
            }
            for a in cx.a11y {
                let v = match a.action {
                    accesskit::Action::Increment => Some(self.size + p.step),
                    accesskit::Action::Decrement => Some(self.size - p.step),
                    accesskit::Action::SetValue => match &a.data {
                        Some(accesskit::ActionData::NumericValue(n)) => Some(*n as f32),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(v) = v {
                    let v = v.clamp(p.min.max(0.0), p.max.max(p.min.max(0.0))).round();
                    self.set(v, out, true);
                }
            }
        }
        if self.focused && !was_focused {
            // FX1: focus from a press is not `:focus-visible`
            self.focus_visible = !cx.focus.pointer_pressed && !r.down_on;
        } else if !self.focused {
            self.focus_visible = false;
        }
        cx.set(StateBits::HOVERED, hovered);
        cx.set(StateBits::PRESSED, cx.enabled && r.down_on);
        cx.set(StateBits::DRAGGING, self.start.is_some());
        cx.set(StateBits::FOCUSED, self.focused);
        cx.set(StateBits::FOCUS_VISIBLE, self.focus_visible);
    }

    fn a11y(&self, p: &ResizeParams, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Splitter);
        node.set_numeric_value(self.size as f64);
        node.set_min_numeric_value(p.min.max(0.0) as f64);
        if p.max.is_finite() {
            node.set_max_numeric_value(p.max as f64);
        }
        node.set_numeric_value_step(p.step as f64);
        // a vertical sash (resizing widths) is a vertical separator
        node.set_orientation(if p.axis == 0 { accesskit::Orientation::Vertical } else { accesskit::Orientation::Horizontal });
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
        node.add_action(accesskit::Action::SetValue);
        node.add_action(accesskit::Action::Focus);
    }

    fn claims(&self, p: &ResizeParams, key: Key, m: Modifiers) -> bool {
        !m.command
            && !m.alt
            && match key {
                Key::ArrowLeft | Key::ArrowRight => p.axis == 0,
                Key::ArrowUp | Key::ArrowDown => p.axis == 1,
                Key::Home | Key::End => true,
                Key::Enter => p.collapsible,
                Key::Escape => self.start.is_some(),
                _ => false,
            }
    }

    fn next_frame(&self) -> Option<std::time::Duration> {
        let t = self.hover_since?;
        let left = self.delay as f64 - (self.now - t);
        (left > 0.0 && self.start.is_none()).then(|| std::time::Duration::from_secs_f64(left))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_and_restore() {
        let p = ResizeParams { min: 50.0, max: 400.0, collapsible: true, ..Default::default() };
        let mut r = Resizable::default();
        r.set_size(120.0);
        assert_eq!(r.clamp(&p, 20.0), 0.0, "below half the minimum collapses");
        assert_eq!(r.clamp(&p, 30.0), 50.0);
        assert_eq!(r.clamp(&p, 900.0), 400.0);
        let mut out = Intents::default();
        r.toggle_collapse(&p, &mut out);
        assert_eq!(r.size(), 0.0);
        r.toggle_collapse(&p, &mut out);
        assert_eq!(r.size(), 120.0, "restored to the size before the collapse");
        assert_eq!(out.0.last(), Some(&Resize::Commit { size: 120.0 }));
    }
}
