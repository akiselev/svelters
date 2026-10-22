//! `split-view`, svelters' own (OW2, ARCH-WIDGETS §6.1; WIDGET-PARITY §2.17): box; Resizable per
//! sash; parts `sash` (one per gap, a node-like part with its own id, states and AccessKit
//! `Splitter`) and `grip`. Replaces WG4a's two-pane split view.
//!
//! **Panes.** Every visible child is a pane, laid out side by side (`orientation="horizontal"`)
//! or stacked, with a 1 px gap where the sash line is drawn. Sizes come from `sizes` (one per
//! pane), else `divider-position` (two panes: the first pane), else equal shares. When the split
//! view's size differs from the sum, the difference goes to the panes by their `flex-grow` (Qt's
//! stretch factors), else to the last pane. Each pane keeps `min-size` and its own
//! `min-width`/`max-width` (`min-height`/`max-height` stacked); `collapsible` lets a pane go to 0.
//!
//! **Sashes (R11).** At rest a 1 px `border` line with a 4 px hit area (`sash-w`); the resize
//! cursor at once on hover; after `ScrollPolicy::sash_hover_delay` (300 ms) a 4 px `focus` bar with
//! a grip (three dots) at its middle, at once while dragging or keyboard-focused. A drag moves the
//! two panes beside the sash (VS Code); `commit-mode="release"` draws a guide and moves them on
//! release. Keys on a focused sash: arrows by 10 (Shift: 50), Home/End, Enter collapses or restores;
//! a double click restores or collapses (`restore-on-double-click`). Escape cancels a drag.
//!
//! Signals: `changed(divider-position)` while the first pane changes, `drag-started`,
//! `drag-stopped`, `commit(sizes)` when a resize ends.

use super::{Len, pick};
use crate::interact::{Resizable, Resize, ResizeParams, Unit};
use crate::kinds::COMMIT_MODE;
use crate::paint::{self, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, LayoutCx, MeasureCx, StateBits, Widget, WidgetCx};
use crate::taffy::{self, LayoutInput, LayoutOutput, RequestedAxis, RunMode, Size, SizingMode};
use egui::{Color32, Rect, Vec2, accesskit, pos2, vec2};

/// The split view itself: no look of its own.
pub const UA: Ua = Ua(&[]);

/// `::part(sash)`: the 1 px line at rest (`border`), the 4 px hit area (`sash-w`), the hover bar in
/// `focus` (R11; VS Code's `sash.hoverBorder`).
pub const SASH_UA: Ua = Ua(&[
    UaRule::base(&[("width", Token("stroke")), ("--hit", Token("sash-w")), ("background-color", Token("border")), ("--hover-width", Token("sash-w")), ("--hover-color", Token("focus"))]),
    UaRule::when(StateBits::DRAGGING, &[("--hover-color", Token("focus"))]),
]);

/// `::part(grip)`: three dots at the middle of a shown hover bar.
pub const GRIP_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("accent-fg")), ("width", Px(2.0)), ("height", Px(2.0)), ("gap", Px(2.0))])]);

/// Panes split by draggable sashes (`QSplitter`).
#[svelters::widget(namespace = "sv", replaces = "split-view", caps = CONTAINER | CUSTOM_LAYOUT | PAINTS_BOX)]
pub struct SplitView {
    /// `horizontal` (side by side) or `vertical` (stacked).
    #[prop(default = "horizontal")]
    orientation: String,
    /// The first pane's size; null for its share.
    #[prop(bind)]
    divider_position: Len,
    /// The smallest size of a pane, in points.
    #[prop(default = 32.0)]
    min_size: f64,
    /// The sashes can be dragged.
    #[prop(default = true)]
    resizable: bool,
    /// Every pane's size (null: from `divider-position` or equal).
    #[prop(bind)]
    sizes: Option<Vec<f64>>,
    #[prop]
    collapsible: bool,
    #[prop(default = true)]
    restore_on_double_click: bool,
    #[prop(default = "live")]
    commit_mode: String,
    #[event]
    changed: Event<f64>,
    #[event]
    drag_started: Event<()>,
    #[event]
    drag_stopped: Event<()>,
    #[event]
    commit: Event<Vec<f64>>,
    /// The pane sizes of the last layout (main axis), and the bounds each had.
    #[state]
    laid: Vec<f32>,
    #[state]
    mins: Vec<f32>,
    #[state]
    maxs: Vec<f32>,
    /// `divider-position` as last applied: a different value is a host write (it overrides
    /// `sizes`).
    #[state]
    seen_dp: Option<Len>,
    #[state]
    sashes: Vec<Resizable>,
    #[state]
    sash_state: Vec<StateBits>,
    /// `commit-mode="release"`: the sash being dragged and the size it would give.
    #[state]
    ghost: Option<(usize, f32)>,
}

impl SplitView {
    fn vertical(&self) -> bool {
        pick(&self.orientation, crate::kinds::ORIENTATION, "horizontal") == "vertical"
    }

    fn line(dark: bool) -> f32 {
        StyleView::ua_only(SASH_UA, StateBits::NONE, dark).px("width").unwrap_or(1.0).max(0.0)
    }

    /// The pane sizes for `room` points (the main size less the sash lines).
    fn resolve(&self, room: f32, mins: &[f32], maxs: &[f32], grows: &[f32]) -> Vec<f32> {
        let n = mins.len();
        if n == 0 {
            return Vec::new();
        }
        let given = self.sizes.as_ref().filter(|s| s.len() == n && s.iter().all(|x| x.is_finite() && *x >= 0.0));
        let mut s: Vec<f32> = match (given, self.divider_position.0) {
            (Some(v), _) => v.iter().map(|x| *x as f32).collect(),
            (None, Some(p)) if n == 2 => {
                // WG4a's rule, kept: the first pane at `p`, each keeping min(min-size, half)
                let lo = mins[0].min(room / 2.0);
                let first = (p as f32).clamp(lo, (room - mins[1].min(room / 2.0)).max(lo));
                return vec![first, (room - first).max(0.0)];
            }
            _ => vec![room / n as f32; n],
        };
        // the difference goes to the panes by flex-grow (else the last pane)
        let diff = room - s.iter().sum::<f32>();
        if diff.abs() > 0.01 {
            let total: f32 = grows.iter().sum();
            if total > 0.0 {
                for (k, g) in grows.iter().enumerate() {
                    s[k] += diff * g / total;
                }
            } else {
                s[n - 1] += diff;
            }
        }
        // bounds (a collapsed pane may stay at 0), then the rest from the last panes that can take it
        for k in 0..n {
            let lo = mins[k].min(room / n as f32);
            if !(self.collapsible && s[k] == 0.0) {
                s[k] = s[k].clamp(lo, maxs[k].max(lo));
            }
        }
        let mut rest = room - s.iter().sum::<f32>();
        for k in (0..n).rev() {
            if rest.abs() < 0.01 {
                break;
            }
            if self.collapsible && s[k] == 0.0 {
                continue;
            }
            let lo = mins[k].min(room / n as f32);
            let new = (s[k] + rest).clamp(lo, maxs[k].max(lo));
            rest -= new - s[k];
            s[k] = new;
        }
        if rest.abs() >= 0.01 {
            s[n - 1] = (s[n - 1] + rest).max(0.0);
        }
        // whole points, rounding the edges (so the sizes still add up to the room)
        let mut out = Vec::with_capacity(n);
        let (mut acc, mut prev) = (0.0f32, 0.0f32);
        for v in s {
            acc += v;
            let edge = acc.round();
            out.push(edge - prev);
            prev = edge;
        }
        out
    }

    /// Writes new pane sizes (the user's): `sizes`, `divider-position`, `changed`.
    fn write(&mut self, cx: &mut WidgetCx<'_>, sizes: Vec<f32>) {
        let v: Vec<f64> = sizes.iter().map(|x| *x as f64).collect();
        let first = v.first().copied();
        cx.set(&mut self.sizes, Some(v));
        if let Some(f) = first {
            let dp = Len(Some(f));
            if dp != self.divider_position {
                cx.set(&mut self.divider_position, dp);
                cx.dispatch(&self.changed, f);
            }
            self.seen_dp = Some(dp);
        }
        self.laid = sizes;
        cx.relayout_children();
        cx.request_measure();
    }
}

impl Widget for SplitView {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: LayoutInput) -> LayoutOutput {
        let vertical = self.vertical();
        let main_ax = if vertical { 1 } else { 0 };
        // a host write of `divider-position` overrides `sizes`
        if self.seen_dp != Some(self.divider_position) {
            if self.seen_dp.is_some() {
                self.sizes = None;
            }
            self.seen_dp = Some(self.divider_position);
        }
        let kids: Vec<_> = cx.children().to_vec();
        let panes: Vec<_> = kids.iter().copied().filter(|c| cx.child_common(*c).is_some_and(|c| c.visible)).collect();
        let line = Self::line(true);
        let n = panes.len();
        let gaps = line * n.saturating_sub(1) as f32;
        let style = cx.style().clone();
        let out = taffy::compute_leaf_layout(inputs, &style, |_, _| 0.0, |known, avail| {
            if let Size { width: Some(w), height: Some(h) } = known {
                return Size { width: w, height: h };
            }
            let (mut main, mut cross) = (gaps, 0.0f32);
            for &c in &panes {
                let o = cx.compute_child(
                    c,
                    LayoutInput {
                        run_mode: RunMode::ComputeSize,
                        sizing_mode: SizingMode::InherentSize,
                        axis: RequestedAxis::Both,
                        known_dimensions: Size::NONE,
                        known_dimensions_are_definite: Size { width: false, height: false },
                        parent_size: known,
                        available_space: avail,
                        vertical_margins_are_collapsible: taffy::Line::FALSE,
                    },
                );
                let (m, x) = if vertical { (o.size.height, o.size.width) } else { (o.size.width, o.size.height) };
                main += m;
                cross = cross.max(x);
            }
            let (w, h) = if vertical { (cross, main) } else { (main, cross) };
            Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
        });
        if inputs.run_mode != RunMode::PerformLayout {
            return out;
        }
        let size = vec2(out.size.width, out.size.height);
        let main = size[main_ax];
        let min_size = self.min_size.max(0.0) as f32;
        let mut mins = Vec::with_capacity(n);
        let mut maxs = Vec::with_capacity(n);
        let mut grows = Vec::with_capacity(n);
        for &c in &panes {
            let cc = cx.child_common(c).expect("a live pane");
            let (lo, hi) = if vertical { (cc.min_height, cc.max_height) } else { (cc.min_width, cc.max_width) };
            mins.push((lo.unwrap_or(0.0) as f32).max(min_size));
            maxs.push(hi.filter(|h| h.is_finite()).map_or(f32::INFINITY, |h| h as f32));
            grows.push((cc.flex_grow as f32).max(0.0));
        }
        let sizes = self.resolve((main - gaps).max(0.0), &mins, &maxs, &grows);
        let mut at = 0.0;
        for (k, &c) in panes.iter().enumerate() {
            let s = sizes[k];
            let (p, sz) = if vertical { (pos2(0.0, at), vec2(size.x, s)) } else { (pos2(at, 0.0), vec2(s, size.y)) };
            cx.place(c, p, sz);
            at += s + line;
        }
        for &c in kids.iter().filter(|c| !panes.contains(c)) {
            cx.hide(c);
        }
        self.laid = sizes;
        self.mins = mins;
        self.maxs = maxs;
        out
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let rect = cx.rect();
        let dark = cx.dark();
        let st = cx.style(UA);
        paint::paint_box(&mut cx.paint_cx(ui), &st, rect);
        let area = cx.interact(ui, egui::Sense::hover());
        cx.record_hit(ui);
        cx.children(ui);

        let vertical = self.vertical();
        let ax = if vertical { 1 } else { 0 };
        let n = self.laid.len();
        let nsash = n.saturating_sub(1);
        self.sashes.resize_with(nsash, Resizable::default);
        self.sash_state.resize(nsash, StateBits::NONE);
        let sash = StyleView::ua_only(SASH_UA, StateBits::NONE, dark);
        let line = sash.px("width").unwrap_or(1.0);
        let hit = sash.px("--hit").unwrap_or(4.0).max(line);
        let enabled = cx.enabled() && self.resizable;
        let name = cx.node().common().accessible_name.clone();
        let release = pick(&self.commit_mode, COMMIT_MODE, "live") == "release";
        let mut start = rect.min[ax];
        let mut laid = self.laid.clone();
        for k in 0..nsash {
            start += laid[k];
            let mut lr = rect;
            lr.min[ax] = start;
            lr.max[ax] = start + line;
            let mut hr = lr;
            let grow = (hit - line) / 2.0;
            hr.min[ax] -= grow;
            hr.max[ax] += grow;
            let id = cx.id().with(("sash", k));
            let resp = ui.interact(hr, id, if enabled { egui::Sense::click_and_drag() } else { egui::Sense::hover() });
            let pair = laid[k] + laid[k + 1];
            let p = ResizeParams {
                axis: ax,
                sign: 1.0,
                min: if self.collapsible { 0.0 } else { self.mins[k].min(pair) },
                max: (pair - self.mins.get(k + 1).copied().unwrap_or(0.0)).max(0.0),
                step: 10.0,
                collapsible: self.collapsible,
                double_click: self.restore_on_double_click,
            };
            // with `collapsible`, below half of the pane's own minimum collapses
            let p = if self.collapsible { ResizeParams { min: self.mins[k].min(pair), ..p } } else { p };
            let unit = &mut self.sashes[k];
            unit.set_size(laid[k]);
            let mut fired = crate::interact::Intents::default();
            crate::interact::run(ui, &resp, &mut self.sash_state[k], &crate::interact::PartResponses::default(), |icx| unit.input(&p, icx, &mut fired));
            if enabled {
                cx.record_part_focus(ui, &resp);
            }
            let (shown, hovered) = (unit.hover_shown(), unit.hovered());
            ui.ctx().accesskit_node_builder(resp.id, |nd| {
                unit.a11y(&p, nd);
                nd.set_label(if name.is_empty() { format!("Resize pane {}", k + 1) } else { name.clone() });
            });
            if enabled && hovered {
                ui.ctx().set_cursor_icon(if vertical { egui::CursorIcon::ResizeRow } else { egui::CursorIcon::ResizeColumn });
            }
            if let Some(t) = unit.next_frame() {
                ui.ctx().request_repaint_after(t);
            }
            let mut ended = false;
            for i in fired {
                match i {
                    Resize::Start => {
                        cx.dispatch(&self.drag_started, ());
                    }
                    Resize::Resize { size } => {
                        let mut s = laid.clone();
                        s[k] = size;
                        s[k + 1] = (pair - size).max(0.0);
                        if release {
                            self.ghost = Some((k, size));
                        } else {
                            laid = s.clone();
                            self.write(cx, s);
                        }
                    }
                    Resize::Commit { size } => {
                        let mut s = laid.clone();
                        s[k] = size;
                        s[k + 1] = (pair - size).max(0.0);
                        laid = s.clone();
                        self.ghost = None;
                        self.write(cx, s.clone());
                        cx.dispatch(&self.commit, s.iter().map(|x| *x as f64).collect());
                        ended = true;
                    }
                    Resize::Cancel { size } => {
                        let mut s = laid.clone();
                        s[k] = size;
                        s[k + 1] = (pair - size).max(0.0);
                        self.ghost = None;
                        if !release {
                            laid = s.clone();
                            self.write(cx, s);
                        }
                        ended = true;
                    }
                }
            }
            if ended && resp.drag_stopped() || ended && !resp.dragged() {
                cx.dispatch(&self.drag_stopped, ());
            }
            // paint: the line, then (R11) the hover bar and grip
            let mut pc = cx.paint_cx(ui);
            let mut pp = pc.part("sash");
            let fill = sash.color("background-color").unwrap_or(Color32::TRANSPARENT);
            pp.painter.rect_filled(pp.snap_rect(lr), 0.0, fill);
            pp.record(paint::TraceKind::Box, lr, fill, egui::Stroke::NONE, 0.0);
            if shown && enabled {
                let hs = StyleView::ua_only(SASH_UA, self.sash_state[k], dark);
                let w = hs.px("--hover-width").unwrap_or(4.0);
                let mut bar = lr;
                let c = lr.center()[ax];
                bar.min[ax] = c - w / 2.0;
                bar.max[ax] = c + w / 2.0;
                let col = hs.color("--hover-color").unwrap_or(Color32::TRANSPARENT);
                pp.painter.rect_filled(bar, 0.0, col);
                pp.record(paint::TraceKind::Box, bar, col, egui::Stroke::NONE, 0.0);
                paint_grip(&mut pc, dark, bar, ax);
            }
        }
        // `commit-mode="release"`: the guide where the sash would go
        if let Some((k, size)) = self.ghost {
            let at = rect.min[ax] + self.laid[..k].iter().sum::<f32>() + line * k as f32 + size;
            let mut g = rect;
            g.min[ax] = at - 1.0;
            g.max[ax] = at + 1.0;
            let col = StyleView::ua_only(SASH_UA, StateBits::DRAGGING, dark).color("--hover-color").unwrap_or(Color32::GRAY);
            let mut pc = cx.paint_cx(ui);
            let mut gp = pc.part("ghost");
            gp.painter.rect_filled(g, 0.0, col);
            gp.record(paint::TraceKind::Box, g, col, egui::Stroke::NONE, 0.0);
        }
        area
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Group);
    }
}

/// R11's grip: three dots across the middle of the hover bar.
fn paint_grip(pc: &mut paint::PaintCx<'_>, dark: bool, bar: Rect, ax: usize) {
    let g = StyleView::ua_only(GRIP_UA, StateBits::NONE, dark);
    let col = g.color("background-color").unwrap_or(Color32::WHITE);
    let d = g.px("width").unwrap_or(2.0);
    let gap = g.px("gap").unwrap_or(2.0);
    let c = bar.center();
    let mut gp = pc.part("grip");
    for i in -1..=1 {
        let off = i as f32 * (d + gap);
        let p = if ax == 0 { c + vec2(0.0, off) } else { c + vec2(off, 0.0) };
        gp.painter.circle_filled(p, d / 2.0, col);
    }
    gp.record(paint::TraceKind::Box, Rect::from_center_size(c, if ax == 0 { vec2(d, 3.0 * d + 2.0 * gap) } else { vec2(3.0 * d + 2.0 * gap, d) }), col, egui::Stroke::NONE, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sv() -> SplitView {
        <SplitView as crate::provider::KindDecl>::__svelters_new()
    }

    #[test]
    fn two_panes_follow_the_divider_and_min_size() {
        let mut s = sv();
        assert_eq!(s.resolve(399.0, &[32.0, 32.0], &[f32::INFINITY; 2], &[0.0, 0.0]), vec![200.0, 199.0], "halves, whole points");
        s.divider_position = Len(Some(5.0));
        assert_eq!(s.resolve(399.0, &[32.0, 32.0], &[f32::INFINITY; 2], &[0.0, 0.0])[0], 32.0);
        s.divider_position = Len(Some(100.0));
        assert_eq!(s.resolve(399.0, &[32.0, 32.0], &[f32::INFINITY; 2], &[0.0, 0.0]), vec![100.0, 299.0]);
    }

    #[test]
    fn extra_room_goes_by_flex_grow_and_bounds_hold() {
        let mut s = sv();
        s.sizes = Some(vec![100.0, 100.0, 100.0]);
        let r = s.resolve(400.0, &[32.0; 3], &[f32::INFINITY; 3], &[0.0, 1.0, 1.0]);
        assert_eq!(r, vec![100.0, 150.0, 150.0]);
        let r = s.resolve(400.0, &[32.0; 3], &[f32::INFINITY; 3], &[0.0; 3]);
        assert_eq!(r, vec![100.0, 100.0, 200.0], "the last pane without stretch factors");
        let r = s.resolve(400.0, &[32.0; 3], &[f32::INFINITY, f32::INFINITY, 120.0], &[0.0; 3]);
        assert_eq!(r, vec![100.0, 180.0, 120.0], "max-width holds; the rest moves left");
        let r = s.resolve(100.0, &[32.0; 3], &[f32::INFINITY; 3], &[0.0; 3]);
        assert_eq!(r.iter().sum::<f32>(), 100.0);
        assert!(r.iter().all(|x| *x >= 32.0), "{r:?}");
    }
}
