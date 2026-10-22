//! WG2b's part of the Tier 0 catalog (plans/research/EGUI.md §5, plans/milestones/WG2b-tier0-controls.md):
//! the payloads, prop get/set, measure, paint and layout hooks of
//!
//! - `inputs.rs`: `hyperlink`, `radio-button`, `switch`, `range-slider`, `drag-value`;
//! - `media.rs`: `image`, `progress-bar`, `spinner`, `canvas` (its replay is [`canvas`], its PNG
//!   loader [`images`]);
//! - `panes.rs`: the containers `panel`, `sides`, `frame`, `stack`.
//!
//! The kind tables are `kinds/{inputs,media,panes}.rs`. The shared files only dispatch here:
//! `scene/props.rs` (`set_prop`/`get_prop`), `scene/tree.rs` ([`payload`]), `scene/node.rs`
//! ([`is_container`], `text_of`), `layout.rs` ([`style`], [`intrinsic`], the stack layout),
//! `leaf.rs` ([`leaf`], [`container`]).
//!
//! Writer lane: WIDGETS.

pub mod canvas;
pub mod images;
mod inputs;
pub(crate) mod media;
pub use media::fit_rect;
pub(crate) mod panes;

use crate::kinds::{self, P, PropDef};
use crate::layout::Metrics;
use crate::scene::props::{as_bool, as_enum, as_f64, as_i64, as_len, as_str, is_common, len_value};
use crate::scene::{Arena, Common, Dirty, NodeFlags, W};
use crate::walk::Out;
use egui::{Rect, Ui, Vec2};
use svelters_abi::Value;

/// True for WG2b's containers.
pub(crate) fn is_container(w: &W) -> bool {
    matches!(w, W::Panel { .. } | W::Sides | W::Frame { .. } | W::Stack { .. })
}

/// True for WG2b's leaves (painted by [`leaf`]).
pub(crate) fn is_leaf(w: &W) -> bool {
    matches!(
        w,
        W::Hyperlink { .. }
            | W::RadioButton { .. }
            | W::Switch { .. }
            | W::RangeSlider { .. }
            | W::DragValue { .. }
            | W::Image { .. }
            | W::ProgressBar { .. }
            | W::Spinner { .. }
            | W::Canvas(_)
    )
}

/// A new node's payload, by kind id (the defaults of the kind tables).
pub(crate) fn payload(id: &str) -> Option<W> {
    Some(match id {
        "Hyperlink" => W::Hyperlink { text: String::new(), url: String::new(), new_tab: false },
        "RadioButton" => W::RadioButton { text: String::new(), checked: false, value: String::new(), group: String::new() },
        "Switch" => W::Switch { text: String::new(), active: false },
        "RangeSlider" => W::RangeSlider { low: 0.0, high: 100.0, min: 0.0, max: 100.0, step: 0.0, text: String::new(), show_value: true },
        "DragValue" => W::DragValue { value: 0.0, min: None, max: None, speed: 1.0, decimals: None, prefix: String::new(), suffix: String::new() },
        "Panel" => W::Panel { side: "left", size: None, resizable: false, separator: true },
        "Sides" => W::Sides,
        "Frame" => W::Frame { label: String::new(), style: "group" },
        "Stack" => W::Stack { name: String::new(), index: 0, homogeneous: true },
        "Image" => W::Image { source: String::new(), fit: "contain" },
        "ProgressBar" => W::ProgressBar { fraction: 0.0, text: String::new(), show_percentage: false, animate: false },
        "Spinner" => W::Spinner { spinning: true, size: None },
        "Canvas" => W::Canvas(Box::default()),
        _ => return None,
    })
}

/// `checked` of a radio button with a `value` follows its group.
fn sync_radio(checked: &mut bool, value: &str, group: &str) {
    if !value.is_empty() {
        *checked = group == value;
    }
}

/// Writes one of WG2b's own props (`v` already defaulted for a non-nullable null). `None`: not a
/// WG2b prop (a `Widget`/`Box` prop, or another kind), the caller handles it. The caller adds
/// `LAYOUT` for layout props.
pub(crate) fn set_prop(w: &mut W, def: &PropDef, v: &Value) -> Option<Result<Dirty, String>> {
    if is_common(def.key) || !(is_leaf(w) || is_container(w)) {
        return None;
    }
    Some(set(w, def, v))
}

fn set(w: &mut W, def: &PropDef, v: &Value) -> Result<Dirty, String> {
    let measure = Dirty::MEASURE;
    let none = Dirty::default();
    Ok(match (def.key, w) {
        (P::Text, W::Hyperlink { text, .. } | W::RadioButton { text, .. } | W::Switch { text, .. } | W::RangeSlider { text, .. } | W::ProgressBar { text, .. }) => {
            *text = as_str(v)?;
            measure
        }
        (P::Url, W::Hyperlink { url, .. }) => {
            *url = as_str(v)?;
            measure
        }
        (P::NewTab, W::Hyperlink { new_tab, .. }) => {
            *new_tab = as_bool(v)?;
            none
        }
        (P::Checked, W::RadioButton { checked, value, group, .. }) => {
            *checked = as_bool(v)?;
            sync_radio(checked, value, group);
            none
        }
        (P::Choice, W::RadioButton { checked, value, group, .. }) => {
            *value = as_str(v)?;
            sync_radio(checked, value, group);
            none
        }
        (P::Group, W::RadioButton { checked, value, group, .. }) => {
            *group = as_str(v)?;
            sync_radio(checked, value, group);
            none
        }
        (P::Active, W::Switch { active, .. }) => {
            *active = as_bool(v)?;
            none
        }
        (P::Low, W::RangeSlider { low, .. }) => {
            *low = as_f64(v)?;
            none
        }
        (P::High, W::RangeSlider { high, .. }) => {
            *high = as_f64(v)?;
            none
        }
        (P::Min, W::RangeSlider { min, .. }) => {
            *min = as_f64(v)?;
            none
        }
        (P::Max, W::RangeSlider { max, .. }) => {
            *max = as_f64(v)?;
            none
        }
        (P::Step, W::RangeSlider { step, .. }) => {
            *step = finite_or(as_f64(v)?, 0.0).max(0.0);
            none
        }
        (P::ShowValue, W::RangeSlider { show_value, .. }) => {
            *show_value = as_bool(v)?;
            measure
        }
        (P::Value, W::DragValue { value, .. }) => {
            *value = as_f64(v)?;
            measure
        }
        (P::Min, W::DragValue { min, .. }) => {
            *min = as_len(v)?;
            none
        }
        (P::Max, W::DragValue { max, .. }) => {
            *max = as_len(v)?;
            none
        }
        (P::Speed, W::DragValue { speed, .. }) => {
            *speed = as_f64(v)?;
            none
        }
        (P::Decimals, W::DragValue { decimals, .. }) => {
            *decimals = match v {
                Value::Null => None,
                v => Some(as_i64(v)?.clamp(0, 15)),
            };
            measure
        }
        (P::Prefix, W::DragValue { prefix, .. }) => {
            *prefix = as_str(v)?;
            measure
        }
        (P::Suffix, W::DragValue { suffix, .. }) => {
            *suffix = as_str(v)?;
            measure
        }
        (P::Side, W::Panel { side, .. }) => {
            *side = as_enum(v, kinds::SIDE)?;
            Dirty::LAYOUT
        }
        (P::Size, W::Panel { size, .. }) => {
            *size = as_len(v)?.filter(|s| s.is_finite()).map(finite_len);
            Dirty::LAYOUT
        }
        (P::Resizable, W::Panel { resizable, .. }) => {
            *resizable = as_bool(v)?;
            none
        }
        (P::ShowSeparator, W::Panel { separator, .. }) => {
            *separator = as_bool(v)?;
            none
        }
        // the caption takes a row of the frame's padding: a style change
        (P::Text, W::Frame { label, .. }) => {
            *label = as_str(v)?;
            Dirty::LAYOUT
        }
        (P::FrameStyle, W::Frame { style, .. }) => {
            *style = as_enum(v, kinds::FRAME_STYLE)?;
            Dirty::LAYOUT
        }
        (P::VisibleChildName, W::Stack { name, .. }) => {
            *name = as_str(v)?;
            Dirty::LAYOUT
        }
        (P::VisibleIndex, W::Stack { index, .. }) => {
            *index = as_i64(v)?;
            Dirty::LAYOUT
        }
        (P::Homogeneous, W::Stack { homogeneous, .. }) => {
            *homogeneous = as_bool(v)?;
            Dirty::LAYOUT
        }
        (P::Source, W::Image { source, .. }) => {
            *source = as_str(v)?;
            measure
        }
        (P::Fit, W::Image { fit, .. }) => {
            *fit = as_enum(v, kinds::FIT)?;
            none
        }
        (P::Fraction, W::ProgressBar { fraction, .. }) => {
            *fraction = as_f64(v)?;
            none
        }
        (P::ShowPercentage, W::ProgressBar { show_percentage, .. }) => {
            *show_percentage = as_bool(v)?;
            none
        }
        (P::Animate, W::ProgressBar { animate, .. }) => {
            *animate = as_bool(v)?;
            none
        }
        (P::Spinning, W::Spinner { spinning, .. }) => {
            *spinning = as_bool(v)?;
            none
        }
        (P::Size, W::Spinner { size, .. }) => {
            *size = as_len(v)?.filter(|s| s.is_finite() && *s > 0.0).map(finite_len);
            measure
        }
        (P::Ops, W::Canvas(c)) => {
            c.set_ops(match v {
                Value::Null => String::new(),
                v => as_str(v)?,
            })?;
            none
        }
        (P::ContentWidth, W::Canvas(c)) => {
            c.content_w = finite_len(as_f64(v)?);
            measure
        }
        (P::ContentHeight, W::Canvas(c)) => {
            c.content_h = finite_len(as_f64(v)?);
            measure
        }
        (key, w) => return Err(format!("{key:?} is not a prop of {w:?}")),
    })
}

/// Reads one of WG2b's own props; `None` as in [`set_prop`].
pub(crate) fn get_prop(w: &W, def: &PropDef) -> Option<Value> {
    if is_common(def.key) || !(is_leaf(w) || is_container(w)) {
        return None;
    }
    let s = |x: &str| Value::Str(x.to_string());
    Some(match (def.key, w) {
        (P::Text, W::Hyperlink { text, .. } | W::RadioButton { text, .. } | W::Switch { text, .. } | W::RangeSlider { text, .. } | W::ProgressBar { text, .. }) => s(text),
        (P::Text, W::Frame { label, .. }) => s(label),
        (P::Url, W::Hyperlink { url, .. }) => s(url),
        (P::NewTab, W::Hyperlink { new_tab, .. }) => Value::Bool(*new_tab),
        (P::Checked, W::RadioButton { checked, .. }) => Value::Bool(*checked),
        (P::Choice, W::RadioButton { value, .. }) => s(value),
        (P::Group, W::RadioButton { group, .. }) => s(group),
        (P::Active, W::Switch { active, .. }) => Value::Bool(*active),
        (P::Low, W::RangeSlider { low, .. }) => Value::F64(*low),
        (P::High, W::RangeSlider { high, .. }) => Value::F64(*high),
        (P::Min, W::RangeSlider { min, .. }) => Value::F64(*min),
        (P::Max, W::RangeSlider { max, .. }) => Value::F64(*max),
        (P::Step, W::RangeSlider { step, .. }) => Value::F64(*step),
        (P::ShowValue, W::RangeSlider { show_value, .. }) => Value::Bool(*show_value),
        (P::Value, W::DragValue { value, .. }) => Value::F64(*value),
        (P::Min, W::DragValue { min, .. }) => len_value(*min),
        (P::Max, W::DragValue { max, .. }) => len_value(*max),
        (P::Speed, W::DragValue { speed, .. }) => Value::F64(*speed),
        (P::Decimals, W::DragValue { decimals, .. }) => decimals.map_or(Value::Null, Value::I64),
        (P::Prefix, W::DragValue { prefix, .. }) => s(prefix),
        (P::Suffix, W::DragValue { suffix, .. }) => s(suffix),
        (P::Side, W::Panel { side, .. }) => s(side),
        (P::Size, W::Panel { size, .. }) => len_value(*size),
        (P::Resizable, W::Panel { resizable, .. }) => Value::Bool(*resizable),
        (P::ShowSeparator, W::Panel { separator, .. }) => Value::Bool(*separator),
        (P::FrameStyle, W::Frame { style, .. }) => s(style),
        (P::VisibleChildName, W::Stack { name, .. }) => s(name),
        (P::VisibleIndex, W::Stack { index, .. }) => Value::I64(*index),
        (P::Homogeneous, W::Stack { homogeneous, .. }) => Value::Bool(*homogeneous),
        (P::Source, W::Image { source, .. }) => s(source),
        (P::Fit, W::Image { fit, .. }) => s(fit),
        (P::Fraction, W::ProgressBar { fraction, .. }) => Value::F64(*fraction),
        (P::ShowPercentage, W::ProgressBar { show_percentage, .. }) => Value::Bool(*show_percentage),
        (P::Animate, W::ProgressBar { animate, .. }) => Value::Bool(*animate),
        (P::Spinning, W::Spinner { spinning, .. }) => Value::Bool(*spinning),
        (P::Size, W::Spinner { size, .. }) => len_value(*size),
        (P::Ops, W::Canvas(c)) => {
            if c.ops.is_empty() {
                Value::Null
            } else {
                s(&c.ops)
            }
        }
        (P::ContentWidth, W::Canvas(c)) => Value::F64(c.content_w),
        (P::ContentHeight, W::Canvas(c)) => Value::F64(c.content_h),
        _ => return None,
    })
}

/// The kind-specific part of a WG2b node's taffy style (after `layout::style_of`'s own).
pub(crate) fn style(c: &Common, w: &W, s: &mut taffy::Style) {
    panes::style(c, w, s);
}

/// The no-wrap size a WG2b leaf asks for (content + chrome); containers measure nothing.
pub(crate) fn intrinsic(m: &Metrics, w: &W) -> Vec2 {
    match w {
        W::Hyperlink { .. } | W::RadioButton { .. } | W::Switch { .. } | W::RangeSlider { .. } | W::DragValue { .. } => inputs::intrinsic(m, w),
        W::Image { .. } | W::ProgressBar { .. } | W::Spinner { .. } | W::Canvas(_) => media::intrinsic(m, w),
        _ => Vec2::ZERO,
    }
}

/// `text` in the body font, unwrapped.
pub(crate) fn text_size(m: &Metrics, text: &str) -> Vec2 {
    let font = egui::TextStyle::Body.resolve(&m.style);
    m.ctx.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, egui::Color32::WHITE).size())
}

/// What a leaf's paint function hands back: its response (decorated and turned into events by
/// [`leaf`]).
pub(crate) struct Painted {
    pub resp: egui::Response,
}

/// Paints WG2b leaf `i` (the walk's call through `leaf::leaf`): the widget, then what every leaf
/// gets (accessible name, anchor, tooltip, `invalid` outline, focus request, the click and drag
/// events). Value changes are announced by the widget functions ([`changed`]).
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    let h = arena.slots.id_at(i).expect("live leaf").to_raw();
    let kind = arena.core[i].kind as usize;
    let sig = |name: &str| kinds::signal_id(kind, name);
    let click_sense = arena.listens(i, sig("clicked"));
    let Painted { resp } = match arena.w(i) {
        W::Hyperlink { .. } | W::RadioButton { .. } | W::Switch { .. } | W::RangeSlider { .. } | W::DragValue { .. } => inputs::paint(arena, i, ui, out),
        _ => media::paint(arena, i, ui, out, click_sense),
    };
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    if !arena.common(i).tooltip.is_empty() {
        resp = resp.on_hover_text(arena.common(i).tooltip.clone());
    }
    if arena.common(i).invalid {
        crate::leaf::invalid_outline(ui, resp.rect);
    }
    let core = &mut arena.core[i];
    if core.flags.contains(NodeFlags::WANT_FOCUS) {
        core.flags.remove(NodeFlags::WANT_FOCUS);
        resp.request_focus();
    }
    core.drawn = resp.rect;
    for (yes, name) in [(resp.clicked(), "clicked"), (resp.secondary_clicked(), "secondary-clicked"), (resp.drag_started(), "drag-started"), (resp.drag_stopped(), "drag-stopped")]
    {
        let s = sig(name);
        if yes && s != 0 && arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![]));
        }
    }
    resp
}

/// The largest length a WG2b prop lays out at (points): more overflows `f32` or taffy.
const MAX_LEN: f64 = 1e7;

/// A length written by the host, as layout takes it: NaN, infinities and negatives are 0, and it
/// is at most [`MAX_LEN`].
fn finite_len(x: f64) -> f64 {
    if x.is_finite() { x.clamp(0.0, MAX_LEN) } else { 0.0 }
}

/// `x` when it is finite, else `fallback`. The numeric props keep what the host wrote (a read
/// returns it); paint and measure use them through this, so NaN or an infinity never reaches egui
/// (whose widgets clamp with `f64::clamp`, which panics on a NaN bound).
pub(crate) fn finite_or(x: f64, fallback: f64) -> f64 {
    if x.is_finite() { x } else { fallback }
}

/// A widget-side change of props `keys` of node `i` (stamped, journaled, `prop_changed` when
/// observed, then `changed` with `args` when listened): WG1's `leaf::widget_changed`.
pub(crate) fn changed(arena: &mut Arena, i: usize, keys: &[P], args: Vec<Value>, out: &mut Vec<Out>) {
    crate::leaf::widget_changed(arena, i, keys, args, out);
}

/// Paints WG2b container `i` at `r` (screen) with its chrome and children (`leaf::container`'s
/// dispatch).
pub(crate) fn container(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    panes::paint(arena, i, r, origin, ui, out);
}
