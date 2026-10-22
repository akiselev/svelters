//! Built-in leaf painting: one egui widget per leaf node at its rect, with its response turned into
//! events and widget-side prop changes in the outbox. Also the container chrome of WG1
//! ([`container`]): a container's tooltip, the `collapsing-header` row and the `scroll-area`
//! viewport, and the `invalid` outline ([`invalid_outline`]).
//!
//! Writer lane: WIDGETS.

use crate::kinds;
use crate::scene::{Arena, Dirty, NodeFlags, W};
use crate::walk::Out;
use egui::{Rect, Role, Sense, Ui, UiBuilder, Vec2, WidgetInfo, WidgetText, vec2};
use svelters_abi::Value;

/// `text` in the `text_style` font at `weight` (the plain text, so the style's font, at 400).
fn weighted(ui: &Ui, text: &str, text_style: egui::TextStyle, weight: i64) -> WidgetText {
    if weight == 400 {
        return text.into();
    }
    egui::RichText::new(text).font(crate::fonts::text_font(ui.ctx(), ui.style(), text_style, weight, text)).into()
}

/// WG1: the outline of an `invalid` node (`rect`: what it drew).
pub(crate) fn invalid_outline(ui: &Ui, rect: Rect) {
    if rect == Rect::NOTHING || !rect.is_positive() {
        return;
    }
    let v = ui.visuals();
    ui.painter().rect_stroke(rect, v.widgets.inactive.corner_radius, egui::Stroke::new(1.0, v.error_fg_color), egui::StrokeKind::Outside);
}

/// A widget-side change of props `keys` of node `i`: stamped and journaled, `prop_changed` when
/// observed, then the kind's `changed` event with `args` when listened.
pub(crate) fn widget_changed(arena: &mut Arena, i: usize, keys: &[kinds::P], args: Vec<Value>, out: &mut Vec<Out>) {
    props_changed(arena, i, keys, out);
    let h = arena.slots.id_at(i).expect("live node").to_raw();
    let s = kinds::signal_id(arena.core[i].kind as usize, "changed");
    if arena.listens(i, s) {
        out.push(Out::Event(h, s, args));
    }
}

/// WG5a: the stamp, journal and `prop_changed` half of [`widget_changed`], without the `changed`
/// signal (a prop the user changed that `changed` does not carry: a table's cursor).
pub(crate) fn props_changed(arena: &mut Arena, i: usize, keys: &[kinds::P], out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let t = arena.stamp();
    arena.core[i].changed = t;
    for &key in keys {
        let prop = kinds::prop_id(kind, key);
        arena.record(crate::scene::Change::Prop { node, prop: crate::scene::PropId(prop), origin: crate::scene::Origin::Widget }, crate::scene::Origin::Widget);
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
    }
}

/// Paints container `i` at `r` (screen) and its children (`origin`: the walk's offset of root
/// rects): its tooltip and `invalid` outline, and WG1's chrome.
pub(crate) fn container(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    match arena.w(i) {
        // WG2b: `panel`, `sides`, `frame`, `stack`
        w if crate::elements::is_container(w) => crate::elements::container(arena, i, r, origin, ui, out),
        W::CollapsingHeader { .. } => collapsing(arena, i, r, origin, ui, out),
        W::ScrollArea { .. } => scroll(arena, i, r, origin, ui, out),
        // WG4a: table, tab-view, split-view (`crate::views`)
        W::Table(_) | W::TabView { .. } | W::SplitView { .. } => crate::views::container(arena, i, r, origin, ui, out),
        _ => {
            let c = arena.common(i);
            if !c.tooltip.is_empty() {
                // hover only: it takes no clicks from the children drawn over it
                let tip = c.tooltip.clone();
                ui.interact(r, arena.core[i].egui_id.with("tip"), Sense::hover()).on_hover_text(tip);
            }
            if arena.common(i).invalid {
                invalid_outline(ui, r);
            }
            crate::walk::paint_children(arena, i, origin, ui, out);
        }
    }
}

/// `collapsing-header`: the header row (caret, caption; a click or Enter/Space toggles `open`) at
/// the top of its rect, then the children while they are laid out open.
fn collapsing(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let W::CollapsingHeader { text, open } = arena.w(i) else { unreachable!() };
    let (text, open_before) = (text.clone(), *open);
    // children are drawn only from a layout that saw them open
    let laid_open = open_before && !arena.core[i].dirty.contains(Dirty::LAYOUT);
    let head = arena.layout[i].intrinsic.map_or(r.height(), |v| v.y).min(r.height());
    let hr = Rect::from_min_size(r.min, vec2(r.width(), head));
    let enabled = arena.common(i).enabled;
    let id = arena.core[i].egui_id;
    let mut resp = ui.interact(hr, id, if enabled { Sense::click() } else { Sense::hover() });
    let mut open = open_before;
    if enabled && resp.clicked() {
        open = !open;
        resp.mark_changed();
    }
    resp.widget_info(|| WidgetInfo::labeled(Role::DisclosureTriangle, enabled, &text));
    ui.ctx().accesskit_node_builder(resp.id, |b| b.set_expanded(open));
    if ui.is_rect_visible(hr) {
        let visuals = ui.style().interact_selectable(&resp, false);
        if resp.hovered() || resp.has_focus() {
            ui.painter().rect(hr.expand(visuals.expansion), visuals.corner_radius, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        }
        let indent = ui.spacing().indent;
        let (mut icon_rect, _) = ui.spacing().icon_rectangles(hr);
        icon_rect.set_center(egui::pos2(hr.left() + indent / 2.0, hr.center().y));
        egui::collapsing_header::paint_default_icon(ui, if open { 1.0 } else { 0.0 }, &resp.clone().with_new_rect(icon_rect));
        let galley = WidgetText::from(text.as_str()).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
        let pos = egui::pos2(hr.left() + indent, hr.center().y - galley.size().y / 2.0);
        let color = if enabled { visuals.text_color() } else { ui.visuals().weak_text_color() };
        ui.painter().galley(pos, galley, color);
    }
    let mut resp = crate::a11y::decorate(resp, &arena.n(node.to_raw()));
    let c = arena.common(i);
    if !c.tooltip.is_empty() {
        resp = resp.on_hover_text(c.tooltip.clone());
    }
    if arena.common(i).invalid {
        invalid_outline(ui, hr);
    }
    let core = &mut arena.core[i];
    if core.flags.contains(NodeFlags::WANT_FOCUS) {
        core.flags.remove(NodeFlags::WANT_FOCUS);
        resp.request_focus();
    }
    core.drawn = hr;
    crate::scene::hit::record(arena, i, ui);
    if open != open_before {
        if let W::CollapsingHeader { open: o, .. } = arena.w_mut(i) {
            *o = open;
        }
        // its own size changes: relayout from the parent next frame (SG1's style change)
        arena.invalidate_style(node);
        ui.ctx().request_repaint();
        widget_changed(arena, i, &[kinds::P::Open], vec![Value::Bool(open)], out);
    }
    if laid_open {
        crate::walk::paint_children(arena, i, origin, ui, out);
    }
}

/// `scroll-area`: an `egui::ScrollArea` at its rect, the children (laid out by taffy at their full
/// size) drawn in its content `Ui`; a host write of `scroll-x`/`scroll-y` scrolls, a user scroll
/// (or egui clamping a write) writes them back.
fn scroll(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let _ = origin;
    let W::ScrollArea { direction, x, y, seen } = arena.w(i) else { unreachable!() };
    let (direction, x, y, seen) = (*direction, *x, *y, *seen);
    let (hs, vs) = match direction {
        "horizontal" => (true, false),
        "both" => (true, true),
        _ => (false, true),
    };
    let rr = arena.core[i].rect;
    let pad = arena.common(i).padding as f32;
    // the content's extent from the scroll area's origin (children are placed root-relative)
    let mut content = Vec2::ZERO;
    for &c in &arena.children[i] {
        let cr = arena.core[arena.ix(c)].rect;
        if cr != Rect::NOTHING {
            content = content.max(cr.max - rr.min);
        }
    }
    let content = content + Vec2::splat(pad);
    let id = arena.core[i].egui_id;
    let c = arena.common(i);
    let (enabled, tip, invalid) = (c.enabled, c.tooltip.clone(), c.invalid);
    arena.core[i].drawn = r;
    crate::scene::hit::record(arena, i, ui);
    let mut b = UiBuilder::new().max_rect(r).scope_id(id.with("scope")).layout(egui::Layout::top_down(egui::Align::Min));
    if !enabled {
        b = b.disabled();
    }
    let mut child = ui.new_child(b);
    let mut sa = egui::ScrollArea::new([hs, vs])
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .max_width(r.width())
        .max_height(r.height())
        .content_margin(egui::Margin::ZERO);
    if seen != Some((x, y)) {
        sa = sa.scroll_offset(vec2(x as f32, y as f32));
    }
    let o = sa.show_viewport(&mut child, |cui, _viewport| {
        cui.set_min_size(content);
        let origin2 = cui.max_rect().min - rr.min;
        crate::walk::paint_children(arena, i, origin2, cui, out);
    });
    if !tip.is_empty() {
        ui.interact(r, id.with("tip"), Sense::hover()).on_hover_text(tip);
    }
    if invalid {
        invalid_outline(ui, r);
    }
    let off = o.state.clamped_offset();
    let (nx, ny) = (off.x as f64, off.y as f64);
    if let W::ScrollArea { x: wx, y: wy, seen: s, .. } = arena.w_mut(i) {
        *s = Some((nx, ny));
        *wx = nx;
        *wy = ny;
    }
    if (nx, ny) != (x, y) {
        let mut keys = Vec::new();
        if nx != x {
            keys.push(kinds::P::ScrollX);
        }
        if ny != y {
            keys.push(kinds::P::ScrollY);
        }
        widget_changed(arena, i, &keys, vec![Value::F64(nx), Value::F64(ny)], out);
    }
}

/// Paints the leaf in slot `i` (the walk's index into the scene's columns); returns its response
/// (EV2b: the host response of its behaviours).
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    if matches!(arena.w(i), W::Menu(_)) {
        return crate::menus::leaf(arena, i, ui, out); // WG2a: menu-bar, menu-button
    }
    if crate::elements::is_leaf(arena.w(i)) {
        return crate::elements::leaf(arena, i, ui, out); // WG2b
    }
    if let Some(resp) = crate::views::leaf(arena, i, ui, out) {
        return resp; // WG4a: toast-host, table-column
    }
    let h = arena.slots.id_at(i).expect("live leaf").to_raw();
    let kind = arena.core[i].kind as usize;
    let sig = |name: &str| kinds::signal_id(kind, name);
    let size = arena.core[i].rect.size();
    let node_id = arena.core[i].egui_id;
    let click_sense = arena.listens(i, sig("clicked"));
    let mut text_changed = None;
    let mut text_focus = None; // FX1: the field's focus and selection before it ran
    let before = match arena.w(i) {
        W::Checkbox { checked, .. } | W::ToggleButton { checked, .. } => Some(Value::Bool(*checked)),
        W::Slider { value, .. } => Some(Value::F64(*value)),
        W::ComboBox { selected, .. } => Some(Value::I64(*selected)),
        W::Picker(p) => Some(p.value()), // WG4b
        _ => None,
    };
    let enabled = arena.common(i).enabled;
    // RT2: a label with run children is one paragraph of styled runs (`crate::rich`)
    let rich_wrap = match arena.w(i) {
        W::Label { wrap, .. } if !arena.children[i].is_empty() => Some(*wrap),
        _ => None,
    };
    let painted = rich_wrap.and_then(|wrap| crate::rich::paint(arena, i, ui, wrap, if click_sense { egui::Sense::click() } else { egui::Sense::hover() }));
    let mut links_activated = Vec::new();
    let resp = if let Some(p) = painted {
        links_activated = p.activated;
        p.resp
    } else {
        match arena.w_mut(i) {
        W::Label { text, wrap, selectable, weight, .. } => {
            let mode = match *wrap {
                "truncate" => egui::TextWrapMode::Truncate,
                "extend" => egui::TextWrapMode::Extend,
                _ => egui::TextWrapMode::Wrap,
            };
            let mut l = egui::Label::new(weighted(ui, text, egui::TextStyle::Body, *weight)).wrap_mode(mode).selectable(*selectable);
            if click_sense {
                l = l.sense(egui::Sense::click());
            }
            ui.add(l)
        }
        W::Button { text, selected, frame, small, weight } => {
            crate::native::focus_visible(ui); // FX1
            let mut btn = egui::Button::new(weighted(ui, text, egui::TextStyle::Button, *weight)).selected(*selected).frame(*frame).min_size(size);
            if *small {
                btn = btn.small();
            }
            ui.add(btn)
        }
        W::Checkbox { text, checked } => {
            crate::native::focus_visible(ui); // FX1
            ui.add(egui::Checkbox::new(checked, text.as_str()))
        }
        W::TextEdit { text, hint, multiline, password, char_limit } => {
            // FX1: native Escape and word keys before egui's TextEdit runs (crate::native)
            crate::native::keep_focus_on_escape(ui, node_id);
            crate::focus::before_text(ui, node_id); // EV3: a pending edit command
            text_focus = Some(crate::native::TextFocus::before(ui, node_id));
            if crate::native::text_keys(ui, node_id, text, *password) {
                text_changed = Some(text.clone());
            }
            let mut te = if *multiline { egui::TextEdit::multiline(text).desired_rows(4) } else { egui::TextEdit::singleline(text) };
            te = te.id(node_id).desired_width(size.x).hint_text(hint.clone()).password(*password);
            if *char_limit > 0 {
                te = te.char_limit(*char_limit as usize);
            }
            let o = te.show(ui);
            crate::focus::after_text(ui, node_id); // EV3: a Cut or Copy command to the host's clipboard
            // W5: the Linux primary selection (select-to-copy, middle-click paste)
            let pasted = crate::primary::text_edit(ui, &o, node_id, text, *password, (*char_limit).max(0) as usize);
            let r = o.response.response;
            if r.changed() || pasted {
                text_changed = Some(text.clone());
            }
            r
        }
        W::Slider { value, min, max, step, text, show_value } => {
            // The slider fills its rect: the rail takes what the value box and caption leave.
            let sp = ui.spacing().clone();
            let mut extra = 0.0;
            if *show_value {
                extra += sp.item_spacing.x + sp.interact_size.x;
            }
            if !text.is_empty() {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.clone(), font, egui::Color32::WHITE).size().x);
                extra += sp.item_spacing.x + tw;
            }
            // 4 points of slack: the value box can come out slightly wider than interact_size.x.
            ui.spacing_mut().slider_width = (size.x - extra - 4.0).max(sp.interact_size.x);
            // IN1: the value box shows and reads numbers in the host's locale
            let mut s = crate::numeric::slider(egui::Slider::new(value, *min..=*max).show_value(*show_value));
            if *step > 0.0 {
                s = s.step_by(*step);
            }
            if !text.is_empty() {
                s = s.text(text.as_str());
            }
            ui.add(s)
        }
        // ---- WG1 ----
        W::ToggleButton { text, checked, radio, frame, weight } => {
            let r = ui.add(egui::Button::new(weighted(ui, text, egui::TextStyle::Button, *weight)).selected(*checked).frame(*frame).min_size(size));
            if r.clicked() {
                *checked = *radio || !*checked;
            }
            let role = if *radio { Role::RadioButton } else { Role::Button };
            let (on, label) = (*checked, text.clone());
            r.widget_info(|| WidgetInfo::selected(role, enabled, on, &label));
            r
        }
        W::ComboBox { options, selected, hint } => {
            let mut idx = *selected;
            let shown = usize::try_from(idx).ok().and_then(|k| options.get(k)).unwrap_or(hint).clone();
            let r = egui::ComboBox::from_id_salt(node_id.with("combo"))
                .width(size.x)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (k, o) in options.iter().enumerate() {
                        if ui.selectable_label(k as i64 == idx, o.as_str()).clicked() {
                            idx = k as i64;
                        }
                    }
                })
                .response;
            *selected = idx;
            r
        }
        W::Separator { vertical } => ui.add(if *vertical { egui::Separator::default().vertical() } else { egui::Separator::default().horizontal() }),
        W::Icon { source, tint, frame, selected, .. } if !source.is_empty() => icon_image(ui, source, *tint, *frame, *selected, size, click_sense),
        W::Icon { name, glyph, family, size: pt, weight, frame, selected, .. } => {
            use crate::fonts::IconLook;
            // a family, name or glyph the fonts cannot draw: an outlined box instead (never egui's
            // missing-glyph panic, `crate::fonts`)
            let look = crate::fonts::icon_look(ui.ctx(), ui.style(), family, name, glyph, *weight, *pt);
            let sense = if click_sense { Sense::click() } else { Sense::hover() };
            let r = match (&look, *frame) {
                (IconLook::Glyph(font, text), true) => {
                    ui.add(egui::Button::new(egui::RichText::new(text.as_str()).font(font.clone())).selected(*selected).min_size(size))
                }
                (IconLook::Placeholder(_), true) => ui.add(egui::Button::new("").selected(*selected).min_size(size)),
                (IconLook::Glyph(font, text), false) => {
                    ui.add(egui::Label::new(egui::RichText::new(text.as_str()).font(font.clone())).selectable(false).sense(sense))
                }
                (IconLook::Placeholder(side), false) => ui.allocate_exact_size(Vec2::splat(*side), sense).1,
            };
            if let IconLook::Placeholder(side) = look {
                let rect = Rect::from_center_size(r.rect.center(), Vec2::splat(side));
                let stroke = egui::Stroke::new(1.0, ui.visuals().text_color());
                ui.painter().rect_stroke(rect, 1.0, stroke, egui::StrokeKind::Inside);
            }
            if *frame {
                r
            } else {
                // a glyph is not text: an image, or a button when it takes clicks
                let role = if click_sense { Role::Button } else { Role::Image };
                ui.ctx().accesskit_node_builder(r.id, |b| {
                    b.set_role(role);
                    b.clear_value();
                });
                r
            }
        }
        W::Box | W::Window { .. } | W::Grid { .. } | W::CollapsingHeader { .. } | W::ScrollArea { .. } => unreachable!("containers are not leaves"),
        W::Ext(_) => unreachable!("provider kinds are drawn by provider::paint"),
        W::VirtualList(_) => unreachable!("lists are drawn by kinds::virtuallist::paint"),
        W::Menu(_) | W::Overlay(_) => unreachable!("WG2a kinds are drawn by crate::menus and crate::overlays"),
        W::Hyperlink { .. }
        | W::RadioButton { .. }
        | W::Switch { .. }
        | W::RangeSlider { .. }
        | W::DragValue { .. }
        | W::Panel { .. }
        | W::Sides
        | W::Frame { .. }
        | W::Stack { .. }
        | W::Image { .. }
        | W::ProgressBar { .. }
        | W::Spinner { .. }
        | W::Canvas(_) => unreachable!("WG2b kinds are drawn by crate::elements"),
        W::Table(_) | W::TableColumn { .. } | W::TabView { .. } | W::Tab { .. } | W::SplitView { .. } | W::ToastHost(_) => unreachable!("WG4a views: crate::views"),
        W::Picker(p) => p.ui(ui, size), // WG4b (`kinds/pickers.rs`)
        W::Run { .. } => unreachable!("RT2: runs are drawn by their label (crate::rich)"),
        }
    };
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    if !arena.common(i).tooltip.is_empty() {
        resp = resp.on_hover_text(arena.common(i).tooltip.clone());
    }
    if arena.common(i).invalid {
        invalid_outline(ui, resp.rect);
    }
    let core = &mut arena.core[i];
    let requested = core.flags.contains(NodeFlags::WANT_FOCUS);
    if requested {
        core.flags.remove(NodeFlags::WANT_FOCUS);
        resp.request_focus();
    }
    // FX1: native focus policy after the widget ran (crate::native)
    match (arena.w(i), text_focus) {
        (W::TextEdit { text, multiline, .. }, Some(f)) => f.after(ui, &resp, text.chars().count(), *multiline, requested),
        (W::Button { .. } | W::Checkbox { .. }, _) => crate::native::after_toggle(ui, &resp),
        _ => {}
    }
    if !matches!(arena.w(i), W::TextEdit { .. }) {
        crate::native::keep_focus_on_escape(ui, resp.id);
    }
    let core = &mut arena.core[i];
    core.drawn = resp.rect;
    let mut fired: Vec<&str> = Vec::new();
    // EV3: Enter or Escape on a default or cancel button of the focused scope
    let activated = matches!(arena.w(i), W::Button { .. }) && crate::focus::take_activation(ui, node_id);
    for (yes, name) in [
        (resp.clicked() || activated, "clicked"),
        (resp.double_clicked(), "double-clicked"),
        (resp.secondary_clicked(), "secondary-clicked"),
        (resp.drag_started(), "drag-started"),
        (resp.drag_stopped(), "drag-stopped"),
    ] {
        if yes {
            fired.push(name);
        }
    }
    let focused = resp.has_focus();
    let had_focus = arena.core[i].flags.contains(NodeFlags::HAD_FOCUS);
    if matches!(arena.w(i), W::TextEdit { .. }) && focused != had_focus {
        fired.push(if focused { "gained-focus" } else { "lost-focus" });
    }
    arena.core[i].flags.set(NodeFlags::HAD_FOCUS, focused);
    for name in fired {
        let s = sig(name);
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![]));
        }
    }
    // RT2: an activated `a` run: its `clicked` handler, else its `href` opens (as a hyperlink's)
    for (node, href) in links_activated {
        let Some(j) = arena.idx(node) else { continue };
        let s = kinds::signal_id(arena.core[j].kind as usize, "clicked");
        if arena.listens(j, s) {
            out.push(Out::Event(node.to_raw(), s, vec![]));
        } else if !href.is_empty() {
            ui.ctx().open_url(egui::OpenUrl::same_tab(href));
        }
    }
    // Widget-side value changes: the node already holds the new value (the widget wrote the
    // field); announce it if observed (exact, no echo), then the `changed` event.
    // Compared with the value before the widget ran, so a slider clamping a host-set value into its
    // range is announced too (the binding's `natively_normalizes`).
    let changed = match arena.w(i) {
        W::Checkbox { checked, .. } | W::ToggleButton { checked, .. } if Some(Value::Bool(*checked)) != before => Some((kinds::P::Checked, Value::Bool(*checked))),
        W::ComboBox { selected, .. } if Some(Value::I64(*selected)) != before => Some((kinds::P::SelectedIndex, Value::I64(*selected))),
        W::Slider { value, .. } if Some(Value::F64(*value)) != before => Some((kinds::P::Value, Value::F64(*value))),
        W::TextEdit { .. } => text_changed.map(|t| (kinds::P::Text, Value::Str(t))),
        W::Picker(p) if Some(p.value()) != before => Some((kinds::P::Value, p.value())), // WG4b
        _ => None,
    };
    if let Some((key, v)) = changed {
        // a user edit is a change of the node (ticks, ARCH-SCENE §6.5)
        let t = arena.stamp();
        arena.core[i].changed = t;
        let prop = kinds::prop_id(kind, key);
        // SG2: journaled as a widget's prop write
        let node = arena.slots.id_at(i).expect("live leaf");
        arena.record(crate::scene::Change::Prop { node, prop: crate::scene::PropId(prop), origin: crate::scene::Origin::Widget }, crate::scene::Origin::Widget);
        let s = sig("changed");
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![v]));
        }
    }
    resp
}

/// AS1: an `icon` with a `source`: the image fitted inside the leaf's square (inside the padding of
/// a framed one, whose button draws the state visuals), tinted with the text colour of its state;
/// the placeholder box when the source does not load.
fn icon_image(ui: &mut Ui, source: &str, tint: bool, frame: bool, selected: bool, size: Vec2, click_sense: bool) -> egui::Response {
    use egui::load::TexturePoll;
    let sense = if click_sense { Sense::click() } else { Sense::hover() };
    let r = if frame { ui.add(egui::Button::new("").selected(selected).min_size(size)) } else { ui.allocate_exact_size(size, sense).1 };
    let inner = if frame { r.rect.shrink2(ui.spacing().button_padding) } else { r.rect };
    let color = if !tint {
        egui::Color32::WHITE
    } else if frame {
        ui.visuals().override_text_color.unwrap_or_else(|| ui.style().interact_selectable(&r, selected).fg_stroke.color)
    } else {
        ui.visuals().text_color()
    };
    if inner.width() < 1.0 || inner.height() < 1.0 || !ui.is_rect_visible(r.rect) {
        return r;
    }
    let img = egui::Image::new(crate::elements::images::uri_of(source)).fit_to_exact_size(inner.size());
    match img.load_for_size(ui.ctx(), inner.size()) {
        Ok(TexturePoll::Ready { texture }) => {
            let (dest, uv) = crate::elements::media::fit_rect("contain", inner, texture.size);
            ui.painter().with_clip_rect(r.rect.intersect(ui.clip_rect())).image(texture.id, dest, uv, color);
        }
        Ok(TexturePoll::Pending { .. }) => {}
        Err(_) => {
            let side = inner.width().min(inner.height());
            let rect = Rect::from_center_size(inner.center(), Vec2::splat(side));
            ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0, ui.visuals().text_color()), egui::StrokeKind::Inside);
        }
    }
    r
}
