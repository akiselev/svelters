//! `scroll-area`, svelters' own (OW2, ARCH-WIDGETS §3.3, §6.1): box; Scrollable; parts
//! `scrollbar-x`, `scrollbar-y`, `thumb-x`, `thumb-y`, `track-before`, `track-after`, `corner`,
//! `shadow-top`, `shadow-bottom`, `shadow-left`, `shadow-right`. Replaces the `egui::ScrollArea`
//! wrapper.
//!
//! **Layout.** A flex box (its `Box` props and CSS) whose children taffy lays out at their full
//! size: the scrolled axes get `overflow: scroll` (the `direction` prop, or CSS `overflow-x`/
//! `overflow-y`), so the area keeps its own size while its content overflows. The content size is
//! the children's extent plus the padding.
//!
//! **Paint.** Children are drawn moved by the offset in a child `Ui` clipped to the padding box
//! (the walk culls against it and records the clip in the hit list), then the R15 edge shadows on
//! the sides scrolled away from, then the bars: VS Code overlay bars (`scrollbar-w`, 10 px, thumb
//! only, `scrollbar`/`-hover`/`-active` colours) shown while hovered or scrolling and faded
//! afterwards (`ScrollPolicy::bar_hold`, `bar_fade`), or classic bars with a track (Qt's row,
//! `scrollbar-gutter: stable`, `--sv-scrollbar-mode: classic`).
//!
//! **Input.** After its children (post-order, so an inner scrollable takes the wheel first):
//! wheel and trackpad (`smooth_scroll_delta`; Shift+wheel is horizontal, egui's
//! `horizontal_scroll_modifier`), momentum per `native::Policy`, thumb drag, track press (page or
//! jump), PageUp/PageDown/Home/End while focus is inside and unclaimed, AccessKit scroll actions on
//! itself and on its descendants, and the pass's scroll requests ([`Reveal`]: a child gaining
//! focus, `scroll-into-view`, AccessKit `ScrollIntoView`). The wheel delta it takes is removed from
//! egui's input, so an outer scrollable gets only what is left (chaining; `overscroll-behavior:
//! contain` keeps it).
//!
//! **States.** `sv.overflow-top/bottom/left/right` (provider states, `:state(sv.overflow-top)`)
//! while the content overflows that way (R15). Until OW-CSS's `::part()` lands, the parts paint
//! from their UA tables ([`THUMB_UA`], [`TRACK_UA`], [`SHADOW_UA`]).

use crate::interact::{BarGeom, Scroll, ScrollParams, Scrollable, Unit};
use crate::native::Scrollbars as BarMode;
use crate::paint::{self, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Color32, Rect, Vec2, accesskit, pos2, vec2};

/// The area itself: no look of its own (a flex box), the theme may give it one.
pub const UA: Ua = Ua(&[]);

/// `::part(thumb-x)`, `::part(thumb-y)`: VS Code's `scrollbarSlider.*` (VIS1's scrollbar crops).
pub const THUMB_UA: Ua = Ua(&[
    UaRule::base(&[("background-color", Token("scrollbar")), ("min-height", Px(20.0)), ("border-radius", Px(0.0))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("scrollbar-hover"))]),
    UaRule::when(StateBits::DRAGGING, &[("background-color", Token("scrollbar-active"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("scrollbar-active"))]),
]);

/// `::part(scrollbar-x)`, `::part(scrollbar-y)` (the track; VS Code's is transparent) and, in
/// classic mode, the gutter look.
pub const TRACK_UA: Ua = Ua(&[UaRule::base(&[("width", Token("scrollbar-w")), ("background-color", Color(Color32::TRANSPARENT))])]);

/// The classic track and `::part(corner)`.
pub const CLASSIC_TRACK_UA: Ua = Ua(&[UaRule::base(&[("width", Token("scrollbar-w")), ("background-color", Token("bg"))])]);

/// `::part(shadow-top)` and the other edges (R15: VS Code's `scrollbar.shadow`, `inset 0 6px 6px
/// -6px`).
pub const SHADOW_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("scroll-shadow")), ("height", Px(6.0)), ("opacity", Number(0.6))])]);

/// A scroll request of this pass (`Scene::reveals`), answered by the scrollables around its target
/// after their children drew, innermost first.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reveal {
    /// An AccessKit action's target, until the walk draws it (then `rect` is set).
    pub target: Option<accesskit::NodeId>,
    /// The target's rect on screen, as drawn this pass (moved by each scrollable that scrolls).
    pub rect: Option<Rect>,
    pub kind: RevealKind,
    /// When `rect` was set (`Scene::reveal_seq`): a scrollable answers the requests set while its
    /// children drew.
    pub seq: u64,
    /// A delta was taken (no outer scrollable takes it again).
    pub done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RevealKind {
    /// Bring the rect into view (`center`: centred).
    Into { center: bool },
    /// Scroll by this much (AccessKit ScrollUp/Down/Left/Right on a descendant).
    By(Vec2),
}

/// Starts a pass's scroll requests: the AccessKit scroll actions in this pass's input (their
/// targets are matched as the walk draws them, [`note`]).
pub(crate) fn begin_pass(s: &mut crate::scene::Scene, ctx: &egui::Context) {
    s.reveals.clear();
    ctx.input(|i| {
        for e in &i.events {
            let egui::Event::AccessKitActionRequest(r) = e else { continue };
            let kind = match r.action {
                accesskit::Action::ScrollIntoView => RevealKind::Into { center: true },
                accesskit::Action::ScrollDown => RevealKind::By(vec2(0.0, crate::interact::scroll::A11Y_STEP)),
                accesskit::Action::ScrollUp => RevealKind::By(vec2(0.0, -crate::interact::scroll::A11Y_STEP)),
                accesskit::Action::ScrollRight => RevealKind::By(vec2(crate::interact::scroll::A11Y_STEP, 0.0)),
                accesskit::Action::ScrollLeft => RevealKind::By(vec2(-crate::interact::scroll::A11Y_STEP, 0.0)),
                _ => continue,
            };
            s.reveals.push(Reveal { target: Some(r.target_node), rect: None, kind, seq: 0, done: false });
        }
    });
}

fn next_seq(s: &mut crate::scene::Scene) -> u64 {
    s.reveal_seq += 1;
    s.reveal_seq
}

/// After the walk drew node `i` with response `resp`: a focus it gained scrolls it into view
/// (WG6 QA13: Tab, `request-focus`, a click), and an AccessKit scroll action aimed at it gets its
/// rect.
pub(crate) fn note(s: &mut crate::scene::Scene, resp: &egui::Response) {
    if resp.gained_focus() {
        let seq = next_seq(s);
        s.reveals.push(Reveal { target: None, rect: Some(resp.rect), kind: RevealKind::Into { center: false }, seq, done: false });
    }
    if s.reveals.iter().any(|r| r.rect.is_none()) {
        let id = resp.id.accesskit_id();
        let seq = next_seq(s);
        for r in s.reveals.iter_mut().filter(|r| r.rect.is_none() && r.target == Some(id)) {
            r.rect = Some(resp.rect);
            r.seq = seq;
        }
    }
}

/// WG6 (QA13) `scroll-into-view`: flags node `i` and keeps its ancestors from being culled until
/// the walk reaches it ([`reveal_flagged`]).
pub(crate) fn scroll_into_view(s: &mut crate::scene::Scene, i: usize, center: bool) {
    use crate::scene::NodeFlags;
    s.core[i].flags.insert(NodeFlags::WANT_SCROLL);
    s.core[i].flags.set(NodeFlags::SCROLL_CENTER, center);
    let mut p = s.core[i].parent;
    while let Some(id) = p {
        let Some(k) = s.idx(id) else { break };
        s.core[k].flags.insert(NodeFlags::SCROLL_BELOW);
        p = s.core[k].parent;
    }
}

/// WG6: whether the walk draws node `i` although it is outside the clip: a pass with Tab pressed
/// (so egui's focus can move to a widget scrolled out of view, which then scrolls into view) or
/// a `scroll-into-view` on it or below it, or keyboard focus in it. Clears `SCROLL_BELOW`.
pub(crate) fn keep(s: &mut crate::scene::Scene, i: usize, ui: &egui::Ui) -> bool {
    use crate::scene::NodeFlags;
    let below = s.core[i].flags.contains(NodeFlags::SCROLL_BELOW);
    s.core[i].flags.remove(NodeFlags::SCROLL_BELOW);
    below || s.core[i].flags.contains(NodeFlags::WANT_SCROLL) || ui.input(|i| i.key_pressed(egui::Key::Tab)) || holds_focus(s, i, ui.ctx())
}

/// Node `i` is or contains the focused node: drawn even when scrolled out of view, so egui keeps
/// its focus (a widget not drawn in a pass loses it) and PageDown from a focused button goes on.
fn holds_focus(s: &crate::scene::Scene, i: usize, ctx: &egui::Context) -> bool {
    let Some(f) = crate::focus::focused_node(ctx) else { return false };
    let Some(me) = s.slots.id_at(i) else { return false };
    let mut x = Some(f);
    while let Some(id) = x {
        if id == me {
            return true;
        }
        x = s.idx(id).and_then(|k| s.core[k].parent);
    }
    false
}

/// WG6: the walk reached node `i` (at `r` on screen): a pending `scroll-into-view` becomes a
/// request for the scrollables around it.
pub(crate) fn reveal_flagged(s: &mut crate::scene::Scene, i: usize, r: Rect) {
    use crate::scene::NodeFlags;
    let f = s.core[i].flags;
    if f.contains(NodeFlags::WANT_SCROLL) {
        let center = f.contains(NodeFlags::SCROLL_CENTER);
        s.core[i].flags.remove(NodeFlags::WANT_SCROLL | NodeFlags::SCROLL_CENTER);
        let seq = next_seq(s);
        s.reveals.push(Reveal { target: None, rect: Some(r), kind: RevealKind::Into { center }, seq, done: false });
    }
}

/// A scrolling container (`QScrollArea`, a CSS `overflow: auto` box).
#[svelters::widget(namespace = "sv", replaces = "scroll-area", caps = CONTAINER | PAINTS_BOX)]
pub struct ScrollArea {
    /// The axes that scroll: `vertical`, `horizontal`, `both`.
    #[prop(default = "vertical")]
    direction: String,
    /// Horizontal offset in points.
    #[prop(bind)]
    scroll_x: f64,
    /// Vertical offset in points.
    #[prop(bind)]
    scroll_y: f64,
    /// The user scrolled (or a write was clamped): the new offsets.
    #[event]
    changed: Event<Vec<f64>>,
    #[state]
    scroll: Scrollable,
    /// The offsets as last drawn: a prop that differs is a host write.
    #[state]
    seen: Option<(f64, f64)>,
    /// The thumbs' and tracks' own states (`::part(thumb-y):hover`).
    #[state]
    thumb_state: [StateBits; 2],
}

/// Whether CSS `overflow-<axis>` hides that axis (`hidden`, `clip`) or forces its bar (`scroll`).
fn overflow_of(st: &StyleView, axis: usize, prop_on: bool) -> (bool, bool) {
    let name = if axis == 0 { "overflow-x" } else { "overflow-y" };
    match st.keyword(name) {
        Some("hidden" | "clip") => (false, false),
        Some("scroll") => (true, true),
        Some("auto") => (true, false),
        Some("visible") => (false, false),
        _ => (prop_on, false),
    }
}

impl ScrollArea {
    fn axes_prop(&self) -> [bool; 2] {
        match self.direction.as_str() {
            "horizontal" => [true, false],
            "both" => [true, true],
            _ => [false, true],
        }
    }
}

/// Paints the R15 edge shadow along `edge` of `view` (0 top, 1 bottom, 2 left, 3 right).
fn paint_edge_shadow(cx: &mut paint::PaintCx, st: &StyleView, view: Rect, edge: usize) {
    let Some(col) = st.color("background-color") else { return };
    let a = st.number("opacity").unwrap_or(1.0).clamp(0.0, 1.0);
    let depth = st.px("height").unwrap_or(6.0).max(0.0).min(if edge < 2 { view.height() } else { view.width() } / 2.0);
    if depth <= 0.0 || col.a() == 0 || a <= 0.0 {
        return;
    }
    let strong = col.gamma_multiply(a);
    let clear = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 0);
    let (r, from_start) = match edge {
        0 => (Rect::from_min_size(view.min, vec2(view.width(), depth)), true),
        1 => (Rect::from_min_max(pos2(view.min.x, view.max.y - depth), view.max), false),
        2 => (Rect::from_min_size(view.min, vec2(depth, view.height())), true),
        _ => (Rect::from_min_max(pos2(view.max.x - depth, view.min.y), view.max), false),
    };
    let vertical = edge < 2;
    let mut mesh = egui::Mesh::default();
    let (c0, c1) = if from_start { (strong, clear) } else { (clear, strong) };
    // corners: [min, (max.x, min.y), max, (min.x, max.y)]; the gradient runs along the depth
    let cols = if vertical { [c0, c0, c1, c1] } else { [c0, c1, c1, c0] };
    for (p, c) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(cols) {
        mesh.colored_vertex(p, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    cx.painter.add(mesh);
    cx.record(paint::TraceKind::Box, r, strong, egui::Stroke::NONE, 0.0);
}

impl Widget for ScrollArea {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        Vec2::ZERO
    }

    fn taffy_style(&self, s: &mut crate::taffy::Style) {
        let [x, y] = self.axes_prop();
        let o = |on: bool| if on { crate::taffy::Overflow::Scroll } else { crate::taffy::Overflow::Visible };
        s.overflow = crate::taffy::Point { x: o(x), y: o(y) };
        // overlay bars float over the content; `scrollbar-gutter: stable` reserves a gutter (CSS)
        s.scrollbar_width = 0.0;
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let rect = cx.rect();
        let now = ui.input(|i| i.time);
        let policy = crate::native::policy_of(ui.ctx());
        let pol = policy.scroll;
        let dark = cx.dark();
        let st = cx.style(UA);
        let geom = paint::box_geom(&st, rect);
        let view = geom.padding;
        let [_, pr, _, pb] = st.sides("padding");
        let content = cx.children_extent() + vec2(pr, pb);
        let prop_axes = self.axes_prop();
        let (ax, force_x) = overflow_of(&st, 0, prop_axes[0]);
        let (ay, force_y) = overflow_of(&st, 1, prop_axes[1]);
        let axes = [ax, ay];
        let chain = !matches!(st.keyword("overscroll-behavior"), Some("contain" | "none"));
        let margin = st.px("scroll-margin").unwrap_or(0.0).max(0.0);
        let mode = crate::interact::scroll::bar_mode(st.keyword("scrollbar-gutter").or(st.keyword("--sv-scrollbar-mode")), policy.scrollbars);

        // a host write of the offset (one that differs from what was last drawn) scrolls
        let want = vec2(self.scroll_x as f32, self.scroll_y as f32);
        if self.seen != Some((self.scroll_x, self.scroll_y)) {
            self.scroll.set_offset(want);
        }
        let bar_w = StyleView::ua_only(TRACK_UA, StateBits::NONE, dark).px("width").unwrap_or(10.0);
        let min_thumb = StyleView::ua_only(THUMB_UA, StateBits::NONE, dark).px("min-height").unwrap_or(20.0);
        self.scroll.set_geometry(content, view.size(), [None, None]);
        let show = [axes[0] && (self.scroll.overflows(0) || force_x), axes[1] && (self.scroll.overflows(1) || force_y)];
        let bars = self.scroll.bar_geometry(view, bar_w, min_thumb, show);
        self.scroll.set_geometry(content, view.size(), bars);

        let enabled = cx.enabled();
        paint::paint_box_layers(&mut cx.paint_cx(ui), &st, rect, false);
        let area = cx.interact(ui, egui::Sense::hover());
        cx.record_hit(ui);

        // the children, moved by the offset and clipped to the viewport
        let ppp = ui.ctx().pixels_per_point();
        let off = self.scroll.offset();
        let shown = vec2((off.x * ppp).round() / ppp, (off.y * ppp).round() / ppp);
        let seq0 = cx.scene_mut().reveal_seq;
        {
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(view).scope_id(cx.id().with("content")).layout(egui::Layout::top_down(egui::Align::Min)));
            cui.set_clip_rect(ui.clip_rect().intersect(view));
            cx.children_offset(&mut cui, -shown);
        }

        // R15: edge shadows where the content goes on: the theme's inset `box-shadow` on
        // `:state(sv.overflow-top)` (base.css), over the children; without a theme, the UA's
        let css_shadows = paint::paint_inset_shadows(&mut cx.paint_cx(ui), &cx.style(UA), rect);
        if !css_shadows {
            let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(view));
            let mut pc = cx.paint_cx(ui);
            pc.painter = &painter;
            for (edge, name, on) in [
                // VS Code (`scrollbar.shadow`) shades the top and left edges only
                (0, "shadow-top", axes[1] && self.scroll.away_from_start(1)),
                (2, "shadow-left", axes[0] && self.scroll.away_from_start(0)),
            ] {
                if on {
                    pc.target.1 = Some(name);
                    paint_edge_shadow(&mut pc, &StyleView::ua_only(SHADOW_UA, StateBits::NONE, dark), view, edge);
                }
            }
        }

        // the bars' parts: on top of the children, so they win the pointer
        let alpha = match mode {
            BarMode::Overlay => self.scroll.bar_alpha(now, &pol),
            BarMode::Classic => 1.0,
        };
        let mut parts = crate::interact::PartResponses::default();
        let live = alpha > 0.0 || area.contains_pointer();
        for (d, b) in bars.iter().enumerate() {
            let Some(b) = *b else { continue };
            if !live || !enabled {
                continue;
            }
            let (bn, tn) = if d == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
            let tr = ui.interact(b.track, cx.id().with(bn), egui::Sense::click_and_drag());
            let th = ui.interact(b.thumb, cx.id().with(tn), egui::Sense::drag());
            parts.0.push((bn, crate::interact::Resp::of(&tr)));
            parts.0.push((tn, crate::interact::Resp::of(&th)));
        }

        // the unit, after the children (the innermost scrollable under the pointer goes first)
        let focus_inside = crate::focus::focused_node(ui.ctx()).is_some_and(|f| cx.scene().get(f).is_some_and(|n| n.ancestors().any(|a| a.id() == cx.node().id())));
        let params = ScrollParams { axes, chain, page_keys: focus_inside, arrow_keys: area.has_focus() };
        let mut fired = crate::interact::Intents::default();
        {
            let sc = &mut self.scroll;
            cx.run_units_with(ui, &area, &parts, |icx| sc.input(&params, icx, &mut fired));
        }
        // WG6 QA9 (OW5a): a drag held near an edge scrolls
        if enabled
            && let Some(d) = crate::kinds::dnd::auto_scroll(ui.ctx(), view)
        {
            self.scroll.scroll_by(&params, d);
            ui.ctx().request_repaint();
        }
        // take the wheel delta and keys it used out of egui's input (chaining)
        let consumed = self.scroll.consumed();
        let keys: Vec<_> = self.scroll.keys_used().to_vec();
        if consumed != Vec2::ZERO || !keys.is_empty() {
            ui.ctx().input_mut(|i| {
                i.smooth_scroll_delta -= consumed;
                for (k, m) in &keys {
                    i.consume_key(*m, *k);
                }
            });
        }
        // this pass's scroll requests set while the children drew
        self.answer_reveals(cx, view, &params, seq0, margin, now);
        for (d, s) in self.thumb_state.iter_mut().enumerate() {
            let (_, tn) = if d == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
            let r = parts.get(tn);
            s.set(StateBits::HOVERED, r.is_some_and(|r| r.hovered));
            s.set(StateBits::PRESSED, r.is_some_and(|r| r.down_on));
            s.set(StateBits::DRAGGING, self.scroll.dragging(d));
        }
        for i in fired.iter() {
            if let Scroll::ThumbDrag { .. } = i {
                ui.ctx().request_repaint();
            }
        }

        // the bars, drawn with the offset the children were drawn with
        self.paint_bars(ui, cx, &bars, mode, alpha, dark, view);

        // the offset the user (or a clamp) produced: written back and announced
        let o = self.scroll.offset();
        let (nx, ny) = (o.x as f64, o.y as f64);
        if (nx, ny) != (self.scroll_x, self.scroll_y) {
            cx.set(&mut self.scroll_x, nx);
            cx.set(&mut self.scroll_y, ny);
            cx.dispatch(&self.changed, vec![nx, ny]);
        }
        self.seen = Some((nx, ny));
        if o != off {
            ui.ctx().request_repaint();
        }
        if let Some(t) = self.scroll.next_frame() {
            ui.ctx().request_repaint_after(t);
        }
        if mode == BarMode::Overlay
            && let Some(t) = self.scroll.bar_repaint(now, &pol)
        {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f32(t.max(0.0)));
        }
        area
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::ScrollView);
        // AX1: content scrolled out of the area is clipped (its nodes keep their unclipped bounds)
        node.set_clips_children();
        let axes = self.axes_prop();
        self.scroll.a11y(&ScrollParams { axes, ..Default::default() }, node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        // the scroll area is not a tab stop; it takes PageUp/PageDown/Home/End from a child that
        // did not claim them, after the child ran (see `ui`)
        let _ = (key, m);
        false
    }
}

impl ScrollArea {
    fn answer_reveals(&mut self, cx: &mut WidgetCx<'_>, view: Rect, p: &ScrollParams, seq0: u64, margin: f32, now: f64) {
        let me = cx.id().accesskit_id();
        let s = cx.scene_mut();
        let mut moved = false;
        for r in s.reveals.iter_mut() {
            // an action aimed at this area itself is the unit's (`InputCx::a11y`)
            if r.target == Some(me) {
                r.done = true;
                continue;
            }
            let Some(rect) = r.rect else { continue };
            if r.seq <= seq0 || r.done {
                continue;
            }
            match r.kind {
                RevealKind::Into { center } => {
                    let d = Scrollable::reveal_delta(view, rect, center, margin);
                    let before = self.scroll.offset();
                    self.scroll.scroll_by(p, d);
                    let applied = self.scroll.offset() - before;
                    if applied != Vec2::ZERO {
                        moved = true;
                    }
                    // for the scrollables around: where it will be, within this viewport
                    let next = rect.translate(-applied);
                    let inside = next.intersect(view);
                    r.rect = Some(if inside.is_positive() { inside } else { view });
                }
                RevealKind::By(d) => {
                    let before = self.scroll.offset();
                    let rest = self.scroll.scroll_by(p, d);
                    if self.scroll.offset() != before {
                        moved = true;
                    }
                    if rest == Vec2::ZERO || !p.chain || self.scroll.offset() != before {
                        r.done = true;
                    } else {
                        r.kind = RevealKind::By(rest);
                    }
                }
            }
        }
        if moved {
            self.scroll.touch(now);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_bars(&self, ui: &egui::Ui, cx: &WidgetCx<'_>, bars: &[Option<BarGeom>; 2], mode: BarMode, alpha: f32, dark: bool, view: Rect) {
        if alpha <= 0.0 {
            return;
        }
        let mut pc = cx.paint_cx(ui);
        let track_ua = if mode == BarMode::Classic { CLASSIC_TRACK_UA } else { TRACK_UA };
        for (d, b) in bars.iter().enumerate() {
            let Some(b) = *b else { continue };
            let (bn, tn) = if d == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
            let track = StyleView::ua_only(track_ua, StateBits::NONE, dark);
            let fill = track.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
            let mut tp = pc.part(bn);
            if fill.a() > 0 {
                tp.painter.rect_filled(b.track, 0.0, fill);
            }
            tp.record(paint::TraceKind::Box, b.track, fill, egui::Stroke::NONE, 0.0);
            let thumb = StyleView::ua_only(THUMB_UA, self.thumb_state[d], dark);
            let fill = thumb.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
            let radius = thumb.px("border-radius").unwrap_or(0.0);
            let mut tp = pc.part(tn);
            tp.painter.rect_filled(b.thumb, radius, fill);
            tp.record(paint::TraceKind::Box, b.thumb, fill, egui::Stroke::NONE, 0.0);
        }
        if mode == BarMode::Classic
            && let (Some(x), Some(y)) = (bars[0], bars[1])
        {
            let corner = Rect::from_min_max(pos2(y.track.min.x, x.track.min.y), view.max);
            let fill = StyleView::ua_only(CLASSIC_TRACK_UA, StateBits::NONE, dark).color("background-color").unwrap_or(Color32::TRANSPARENT);
            let mut cp = pc.part("corner");
            cp.painter.rect_filled(corner, 0.0, fill);
            cp.record(paint::TraceKind::Box, corner, fill, egui::Stroke::NONE, 0.0);
        }
        // AccessKit: each bar is a `ScrollBar` with its value
        for (d, b) in bars.iter().enumerate() {
            if b.is_none() {
                continue;
            }
            let bn = if d == 0 { "scrollbar-x" } else { "scrollbar-y" };
            let id = cx.id().with(bn);
            let (v, max) = (self.scroll.offset()[d] as f64, self.scroll.max_offset()[d] as f64);
            ui.ctx().accesskit_node_builder(id, |n| {
                n.set_role(accesskit::Role::ScrollBar);
                n.set_orientation(if d == 0 { accesskit::Orientation::Horizontal } else { accesskit::Orientation::Vertical });
                n.set_numeric_value(v);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(max);
            });
        }
    }
}
