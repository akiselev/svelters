//! Small helpers the views share on top of the crate's kind interface ([`crate::shim::core`]): policy
//! times as [`Span`]s, the platform a policy follows, and emitting a signal by name.

use crate::interact::{Platform, Policy};
use crate::shim::core::{EventCx, KindSpec, Value};
use skelter_base::time::Span;

/// Milliseconds (the policy's `*_ms` fields) as a span.
pub fn ms(v: u32) -> Span {
    Span::from_millis(v as i64)
}

/// Seconds (the policy's scroll fields) as a span.
pub fn secs(v: f32) -> Span {
    Span::from_secs_f64(v as f64)
}

/// The platform a policy follows (its own, else the OS this build runs on).
pub fn platform(p: &Policy) -> Platform {
    p.platform.unwrap_or(if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    })
}

/// Emits the signal named `name` of `spec` (a name the spec lacks is a bug: asserted in debug builds,
/// dropped in release).
pub fn emit(cx: &mut EventCx, spec: &KindSpec, name: &str, args: Vec<Value>) {
    match spec.signal(name) {
        Some(id) => cx.emit(id, args),
        None => debug_assert!(false, "{} has no signal {name}", spec.tag),
    }
}

/// [`paint::paint_text`](crate::paint::paint_text) in `color` instead of `s`'s colour.
pub fn paint_text_in(pc: &mut crate::paint::PaintCx, layout: &crate::shim::text::TextLayout, content: skelter_base::geom::Rect, align: crate::paint::Align, color: skelter_base::color::Color) -> skelter_base::geom::Rect {
    use crate::paint::Align;
    use skelter_base::geom::{Point, Rect};
    let size = layout.size();
    let x = match align {
        Align::Start => content.min.x,
        Align::Center => (content.min.x + content.max.x) / 2.0 - size.w / 2.0,
        Align::End => content.max.x - size.w,
    };
    let y = (content.min.y + content.max.y) / 2.0 - size.h / 2.0;
    let pos = pc.snap_point(Point::new(x, y));
    crate::paint::paint_runs(pc, layout, pos, &|_| color);
    Rect::from_origin_size(pos, size)
}
