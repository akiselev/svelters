//! The restyle: svelters-style's [`Stylist`](svelters_style::Stylist) matched against the document
//! through an [`Element`] view of a node, then mapped to skelter's [`ComputedBox`] and the node's
//! `taffy::Style`. A node is restyled when it is style-dirty or its parent's inherited values changed;
//! the new values are compared with the old ones and each difference marks the work of its class:
//!
//! | Differs | Marks |
//! |---|---|
//! | the taffy style | layout to the boundary (`invalidate_style`) |
//! | font, size, weight, line height, letter spacing, wrapping | the leaf's measure |
//! | colours, background, border, radii, shadows, outline, `visibility` | paint |
//! | opacity, transform, overflow clip, z-index, `will-change` | the property trees |
//!
//! A change of a property listed in the node's `transition` starts an engine animation from the old
//! value to the new (compositor properties only; see `SKELTER-core.md` for the paint ones).

use super::{ComputedBox, Overflow, TextAlign};
use crate::document::Document;
use crate::tree::Dirty;
use skelter_base::anim::{AnimProperty, AnimValue, AnimationCmd, AnimationSpec, Easing, FillMode, Generator, Keyframe, PlaybackDirection, StartTime, StepPosition, Timing, TransformFn};
use skelter_base::color::{Color, Fill, GradientStop};
use skelter_base::cursor::CursorIcon;
use skelter_base::display::BorderStyle;
use skelter_base::geom::{Affine, Corners, Insets, Vec2};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::text::Direction;
use skelter_base::time::Span;
use std::borrow::Cow;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;
use svelters_style::{Atom, ComputeContext, ComputedStyle, ComputedValue, Element, Length, StateMask};

/// A node seen by the selector engine.
#[derive(Clone, Copy)]
pub(crate) struct El<'a> {
    pub doc: &'a Document,
    pub i: usize,
}

impl std::fmt::Debug for El<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "El({})", self.i)
    }
}

impl<'a> El<'a> {
    pub(crate) fn id(&self) -> NodeId {
        self.doc.slots.id_at(self.i).expect("live")
    }
    fn at(&self, id: NodeId) -> Option<El<'a>> {
        self.doc.idx(id).map(|i| El { doc: self.doc, i })
    }
    fn sibling(&self, delta: isize) -> Option<El<'a>> {
        let p = self.doc.core[self.i].parent?;
        let kids = &self.doc.children[self.doc.ix(p)];
        let k = kids.iter().position(|&c| c == self.id())? as isize + delta;
        let c = *kids.get(usize::try_from(k).ok()?)?;
        self.at(c)
    }
}

impl<'a> Element for El<'a> {
    fn key(&self) -> NonZeroU64 {
        NonZeroU64::new(self.id().0).unwrap_or(NonZeroU64::MAX)
    }
    fn parent(&self) -> Option<Self> {
        self.doc.core[self.i].parent.and_then(|p| self.at(p))
    }
    fn prev_sibling(&self) -> Option<Self> {
        self.sibling(-1)
    }
    fn next_sibling(&self) -> Option<Self> {
        self.sibling(1)
    }
    fn first_child(&self) -> Option<Self> {
        self.doc.children[self.i].first().and_then(|&c| self.at(c))
    }
    fn tag(&self) -> &str {
        self.doc.registry.get(self.doc.core[self.i].kind).map_or("", |e| e.spec.name)
    }
    fn id(&self) -> Option<&str> {
        self.doc.style.input(self.i).and_then(|s| s.id.as_deref())
    }
    fn has_class(&self, name: &str) -> bool {
        self.doc.style.input(self.i).is_some_and(|s| s.classes.iter().any(|c| &**c == name))
    }
    fn for_each_class(&self, f: &mut dyn FnMut(&str)) {
        if let Some(s) = self.doc.style.input(self.i) {
            for c in &s.classes {
                f(c);
            }
        }
    }
    fn attr(&self, name: &str) -> Option<Cow<'_, str>> {
        let entry = self.doc.registry.get(self.doc.core[self.i].kind)?;
        let p = entry.spec.prop_named(name).or_else(|| {
            (0..crate::common::COMMON.props.len()).find(|&k| crate::common::COMMON.props[k].name == name).map(|k| crate::common::COMMON.prop_id(k))
        })?;
        let v = self.doc.view_at(self.i).prop(p)?;
        // an enum matches by its value's name (`[dialog-role=accept]`)
        if let crate::value::Value::Enum(e) = v
            && let Some(spec) = entry.prop(p).or_else(|| crate::common::prop(p))
            && let Some(name) = spec.enum_values.get(*e as usize)
        {
            return Some(Cow::Borrowed(name));
        }
        v.to_text().map(Cow::Owned)
    }
    fn state(&self) -> StateMask {
        StateMask(self.doc.state[self.i].0 | self.doc.style.nodes[self.i].forced.0)
    }
    fn is_focus_visible(&self) -> bool {
        self.state().contains(StateMask::FOCUS_VISIBLE)
    }
    fn is_rtl(&self) -> bool {
        self.doc.style.nodes[self.i].dir.is_rtl()
    }
}

fn color(c: svelters_style::Color) -> Color {
    Color::rgba(c.r, c.g, c.b, c.a)
}

fn px(v: Option<&ComputedValue>) -> Option<f32> {
    match v {
        Some(ComputedValue::Length(l)) => Some(l.px),
        Some(ComputedValue::Number(n)) => Some(*n),
        Some(ComputedValue::Int(n)) => Some(*n as f32),
        _ => None,
    }
}

fn kw(v: Option<&ComputedValue>) -> Option<&'static str> {
    match v {
        Some(ComputedValue::Keyword(k)) => Some(k),
        _ => None,
    }
}

fn text(v: Option<&ComputedValue>) -> Option<&str> {
    match v {
        Some(ComputedValue::Raw(t)) | Some(ComputedValue::String(t)) => Some(t),
        _ => None,
    }
}

fn overflow(k: Option<&str>) -> Overflow {
    match k {
        Some("hidden") => Overflow::Hidden,
        Some("clip") => Overflow::Clip,
        Some("scroll") => Overflow::Scroll,
        Some("auto") => Overflow::Auto,
        _ => Overflow::Visible,
    }
}

fn cursor(k: &str) -> Option<CursorIcon> {
    use CursorIcon as C;
    Some(match k {
        "default" => C::Default,
        "none" => C::None,
        "context-menu" => C::ContextMenu,
        "help" => C::Help,
        "pointer" => C::Pointer,
        "progress" => C::Progress,
        "wait" => C::Wait,
        "cell" => C::Cell,
        "crosshair" => C::Crosshair,
        "text" => C::Text,
        "vertical-text" => C::VerticalText,
        "alias" => C::Alias,
        "copy" => C::Copy,
        "move" => C::Move,
        "no-drop" => C::NoDrop,
        "not-allowed" => C::NotAllowed,
        "grab" => C::Grab,
        "grabbing" => C::Grabbing,
        "all-scroll" => C::AllScroll,
        "col-resize" => C::ColResize,
        "row-resize" => C::RowResize,
        "n-resize" => C::NResize,
        "e-resize" => C::EResize,
        "s-resize" => C::SResize,
        "w-resize" => C::WResize,
        "ne-resize" => C::NeResize,
        "nw-resize" => C::NwResize,
        "se-resize" => C::SeResize,
        "sw-resize" => C::SwResize,
        "ew-resize" => C::EwResize,
        "ns-resize" => C::NsResize,
        "nesw-resize" => C::NeswResize,
        "nwse-resize" => C::NwseResize,
        "zoom-in" => C::ZoomIn,
        "zoom-out" => C::ZoomOut,
        _ => return None,
    })
}

fn border_style(k: Option<&str>) -> Option<BorderStyle> {
    Some(match k? {
        "dashed" => BorderStyle::Dashed,
        "dotted" => BorderStyle::Dotted,
        "none" | "hidden" => return None,
        _ => BorderStyle::Solid,
    })
}

fn easing(t: &svelters_style::TimingFunction) -> Easing {
    match *t {
        svelters_style::TimingFunction::CubicBezier(a, b, c, d) => Easing::CubicBezier(a, b, c, d),
        svelters_style::TimingFunction::Steps(n, start) => {
            Easing::Steps { count: n, position: if start { StepPosition::Start } else { StepPosition::End } }
        }
    }
}

/// Parses `transform-origin` as fractions of the border box: keywords (`left`, `center`, `top right`)
/// and percentages; one value sets x (or y for `top`/`bottom`), the other axis stays centred. Lengths
/// are not supported (the box size is not known here): the origin stays the centre.
pub(crate) fn parse_origin(src: &str) -> Option<skelter_base::geom::Point> {
    let mut x: Option<f32> = None;
    let mut y: Option<f32> = None;
    for tok in src.split_whitespace().take(2) {
        match tok {
            "left" => x = Some(0.0),
            "right" => x = Some(1.0),
            "top" => y = Some(0.0),
            "bottom" => y = Some(1.0),
            "center" => {
                if x.is_none() {
                    x = Some(0.5);
                } else {
                    y = Some(0.5);
                }
            }
            t => {
                let f = t.strip_suffix('%')?.parse::<f32>().ok()? / 100.0;
                if x.is_none() {
                    x = Some(f);
                } else {
                    y = Some(f);
                }
            }
        }
    }
    Some(skelter_base::geom::Point::new(x.unwrap_or(0.5), y.unwrap_or(0.5)))
}

/// Parses a CSS transform list (`translate(10px, 0) scale(.9) rotate(45deg)`) into an affine.
/// Unknown functions make the whole list identity (as an invalid value does in CSS).
pub(crate) fn parse_transform(src: &str) -> Option<(Affine, Vec<TransformFn>)> {
    let src = src.trim();
    if src.is_empty() || src == "none" {
        return Some((Affine::IDENTITY, Vec::new()));
    }
    let num = |t: &str| -> Option<f32> {
        let t = t.trim();
        let t = t.strip_suffix("px").unwrap_or(t);
        t.parse().ok()
    };
    let angle = |t: &str| -> Option<f32> {
        let t = t.trim();
        if let Some(d) = t.strip_suffix("deg") {
            return d.trim().parse::<f32>().ok().map(f32::to_radians);
        }
        if let Some(r) = t.strip_suffix("rad") {
            return r.trim().parse().ok();
        }
        if let Some(r) = t.strip_suffix("turn") {
            return r.trim().parse::<f32>().ok().map(|v| v * std::f32::consts::TAU);
        }
        if t == "0" { Some(0.0) } else { None }
    };
    let mut fns = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        let name = rest[..open].trim();
        let args: Vec<&str> = rest[open + 1..close].split(',').collect();
        let f = match name {
            "translate" => TransformFn::Translate(num(args[0])?, args.get(1).map_or(Some(0.0), |a| num(a))?),
            "translateX" => TransformFn::Translate(num(args[0])?, 0.0),
            "translateY" => TransformFn::Translate(0.0, num(args[0])?),
            "scale" => {
                let x = num(args[0])?;
                TransformFn::Scale(x, args.get(1).map_or(Some(x), |a| num(a))?)
            }
            "scaleX" => TransformFn::Scale(num(args[0])?, 1.0),
            "scaleY" => TransformFn::Scale(1.0, num(args[0])?),
            "rotate" => TransformFn::Rotate(angle(args[0])?),
            "skew" => TransformFn::Skew(angle(args[0])?, args.get(1).map_or(Some(0.0), |a| angle(a))?),
            "skewX" => TransformFn::Skew(angle(args[0])?, 0.0),
            "skewY" => TransformFn::Skew(0.0, angle(args[0])?),
            "matrix" if args.len() == 6 => {
                let v: Vec<f32> = args.iter().map(|a| num(a)).collect::<Option<_>>()?;
                TransformFn::Matrix(Affine { a: v[0], b: v[1], c: v[2], d: v[3], e: v[4], f: v[5] })
            }
            _ => return None,
        };
        fns.push(f);
        rest = rest[close + 1..].trim_start();
    }
    let m = fns.iter().fold(Affine::IDENTITY, |m, f| m.then(&transform_fn(f)));
    Some((m, fns))
}

pub(crate) fn transform_fn(f: &TransformFn) -> Affine {
    match *f {
        TransformFn::Translate(x, y) => Affine::translate(x, y),
        TransformFn::Scale(x, y) => Affine::scale(x, y),
        TransformFn::Rotate(r) => Affine::rotate(r),
        TransformFn::Skew(x, y) => Affine { a: 1.0, b: y.tan(), c: x.tan(), d: 1.0, e: 0.0, f: 0.0 },
        TransformFn::Matrix(m) => m,
    }
}

/// skelter's computed box from the engine's computed style and the parent's box.
pub(crate) fn computed_box(c: &ComputedStyle, parent: Option<&ComputedBox>, default_font: f32) -> ComputedBox {
    let mut b = parent.map(ComputedBox::inherited).unwrap_or_else(|| ComputedBox { font_size: default_font, ..ComputedBox::default() });
    if let Some(ComputedValue::Color(k)) = c.get("color") {
        b.color = color(*k);
    }
    // inherited by the engine too; `auto` (a keyword) or initial = `color`
    b.caret_color = match c.get("caret-color") {
        Some(ComputedValue::Color(k)) => Some(color(*k)),
        Some(_) => None,
        None => b.caret_color,
    };
    if let Some(f) = text(c.get("font-family")) {
        b.font_family = Arc::from(f);
    }
    if let Some(s) = px(c.get("font-size")) {
        b.font_size = s;
    }
    if let Some(ComputedValue::FontWeight(w)) = c.get("font-weight") {
        b.font_weight = *w;
    }
    if let Some(k) = kw(c.get("font-style")) {
        b.italic = k != "normal";
    }
    match c.get("line-height") {
        Some(ComputedValue::Number(n)) => b.line_height = Some(*n),
        Some(ComputedValue::Length(l)) if b.font_size > 0.0 => b.line_height = Some(l.px / b.font_size),
        Some(ComputedValue::Raw(t)) => {
            let t = t.trim();
            if t == "normal" {
                b.line_height = None;
            } else if let Some(p) = t.strip_suffix("px").and_then(|v| v.trim().parse::<f32>().ok()) {
                b.line_height = Some(p / b.font_size.max(1.0));
            } else if let Some(p) = t.strip_suffix('%').and_then(|v| v.trim().parse::<f32>().ok()) {
                b.line_height = Some(p / 100.0);
            } else if let Ok(n) = t.parse::<f32>() {
                b.line_height = Some(n);
            }
        }
        _ => {}
    }
    if let Some(l) = px(c.get("letter-spacing")) {
        b.letter_spacing = l;
    }
    if let Some(k) = kw(c.get("white-space")) {
        b.wrap = !matches!(k, "nowrap" | "pre");
    }
    if let Some(k) = kw(c.get("text-align")) {
        b.text_align = match k {
            "end" => TextAlign::End,
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            "center" => TextAlign::Center,
            "justify" => TextAlign::Justify,
            _ => TextAlign::Start,
        };
    }
    if let Some(k) = kw(c.get("cursor")) {
        b.cursor = cursor(k);
    }
    if let Some(ComputedValue::Bool(v)) = c.get("visible") {
        b.visibility_hidden = !v;
    }
    if let Some(k) = kw(c.get("visibility")) {
        b.visibility_hidden = k != "visible";
    }
    if let Some(k) = kw(c.get("pointer-events")) {
        b.pointer_events_none = k == "none";
    }
    // inherited by the engine (a `dir` or a root sets it on the parent style the node inherits)
    b.direction = if kw(c.get("direction")) == Some("rtl") { Direction::Rtl } else { Direction::Ltr };
    // box
    if let Some(ComputedValue::Color(k)) = c.get("background-color") {
        b.background = Fill::Solid(color(*k));
    }
    if let Some(ComputedValue::Image(Some(g))) = c.get("background-image")
        && let svelters_style::Gradient::Linear(l) = &**g
    {
        let stops = l.stops.iter().map(|s| GradientStop { offset: s.position, color: color(s.color) }).collect();
        b.background = Fill::Linear { angle_deg: l.angle, stops };
    }
    let side = |name: &str| px(c.get(name));
    let all = match c.get("border-width") {
        Some(ComputedValue::Insets([t, r, bo, l])) => Insets { top: t.px, right: r.px, bottom: bo.px, left: l.px },
        Some(ComputedValue::Length(l)) => Insets::all(l.px),
        _ => Insets::ZERO,
    };
    b.border_widths = Insets {
        top: side("border-top-width").unwrap_or(all.top),
        right: side("border-right-width").unwrap_or(all.right),
        bottom: side("border-bottom-width").unwrap_or(all.bottom),
        left: side("border-left-width").unwrap_or(all.left),
    };
    // per side: a `none`/`hidden` style draws no border on that side whatever its width. The box
    // has one style and one colour: the first drawn side's (top, right, bottom, left), so a border
    // on one side only (`border-bottom: 1px solid var(--x)`) keeps its own colour
    const SIDES: [(&str, &str); 4] = [("border-top-style", "border-top-color"), ("border-right-style", "border-right-color"), ("border-bottom-style", "border-bottom-color"), ("border-left-style", "border-left-color")];
    let mut drawn: Option<usize> = None;
    for (k, (style_name, _)) in SIDES.iter().enumerate() {
        let w = match k {
            0 => &mut b.border_widths.top,
            1 => &mut b.border_widths.right,
            2 => &mut b.border_widths.bottom,
            _ => &mut b.border_widths.left,
        };
        let declared = c.get(style_name).is_some();
        match border_style(kw(c.get(style_name))) {
            None if declared => *w = 0.0,
            _ if *w > 0.0 && drawn.is_none() => drawn = Some(k),
            _ => {}
        }
    }
    let k = drawn.unwrap_or(0);
    if let Some(s) = border_style(kw(c.get(SIDES[k].0))) {
        b.border_style = s;
    }
    let side_color = match c.get(SIDES[k].1) {
        Some(ComputedValue::Color(x)) => Some(color(*x)),
        _ => None,
    };
    let uniform = match c.get("border-color") {
        Some(ComputedValue::Color(x)) => Some(color(*x)),
        _ => None,
    };
    if let Some(x) = side_color.or(uniform) {
        b.border_color = x;
    }
    let r = px(c.get("border-radius")).unwrap_or(0.0);
    b.radii = Corners {
        tl: side("border-top-left-radius").unwrap_or(r),
        tr: side("border-top-right-radius").unwrap_or(r),
        br: side("border-bottom-right-radius").unwrap_or(r),
        bl: side("border-bottom-left-radius").unwrap_or(r),
    };
    if let Some(ComputedValue::Shadows(s)) = c.get("box-shadow") {
        let fg = b.color;
        b.shadows = s
            .iter()
            .map(|s| super::BoxShadow { offset: Vec2::new(s.x, s.y), blur: s.blur, spread: s.spread, color: s.color.map_or(fg, color), inset: s.inset })
            .collect();
    }
    if let Some(o) = px(c.get("opacity")) {
        b.opacity = o.clamp(0.0, 1.0);
    }
    if let Some(t) = text(c.get("transform"))
        && let Some((m, _)) = parse_transform(t)
    {
        b.transform = m;
    }
    if let Some(o) = text(c.get("transform-origin")).and_then(parse_origin) {
        b.transform_origin = o;
    }
    b.overflow_x = overflow(kw(c.get("overflow-x")));
    b.overflow_y = overflow(kw(c.get("overflow-y")));
    if let Some(ComputedValue::Int(z)) = c.get("z-index") {
        b.z_index = Some(*z);
    }
    if let Some(t) = text(c.get("will-change")) {
        b.will_change = t.contains("transform") || t.contains("opacity");
    }
    if kw(c.get("outline-style")).is_some_and(|k| k != "none") {
        b.outline_width = px(c.get("outline-width")).unwrap_or(0.0);
        if let Some(ComputedValue::Color(k)) = c.get("outline-color") {
            b.outline_color = color(*k);
        }
        b.outline_offset = px(c.get("outline-offset")).unwrap_or(0.0);
    }
    if let Some(ComputedValue::Transitions(ts)) = c.get("transition") {
        // the engine keeps seconds as f32 (`0.1` is 0.100000001…): round to whole microseconds so
        // `100ms` lasts exactly 100 ms
        let span = |s: f32| Span::from_micros((s as f64 * 1e6).round() as i64);
        b.transitions = ts
            .iter()
            .map(|t| super::TransitionSpec {
                property: Arc::from(&*t.property),
                duration: span(t.duration),
                delay: span(t.delay),
                easing: easing(&t.timing),
            })
            .collect();
    }
    b
}

// ---- taffy ----

fn calc_lp(l: &Length) -> taffy::LengthPercentage {
    // a mixed `calc(px + %)` has no exact taffy form without a resolver: keep the larger term
    if l.percent != 0.0 && l.px == 0.0 {
        taffy::LengthPercentage::percent(l.percent / 100.0)
    } else {
        taffy::LengthPercentage::length(l.px)
    }
}

fn lpa(v: &ComputedValue) -> Option<taffy::LengthPercentageAuto> {
    Some(match v {
        ComputedValue::Auto => taffy::LengthPercentageAuto::auto(),
        ComputedValue::Length(l) if l.percent != 0.0 && l.px == 0.0 => taffy::LengthPercentageAuto::percent(l.percent / 100.0),
        ComputedValue::Length(l) => taffy::LengthPercentageAuto::length(l.px),
        _ => return None,
    })
}

fn dim(v: &ComputedValue) -> Option<taffy::Dimension> {
    Some(match v {
        ComputedValue::Auto => taffy::Dimension::auto(),
        ComputedValue::Length(l) if l.percent != 0.0 && l.px == 0.0 => taffy::Dimension::percent(l.percent / 100.0),
        ComputedValue::Length(l) => taffy::Dimension::length(l.px),
        _ => return None,
    })
}

fn align(k: &str) -> Option<taffy::AlignItems> {
    Some(match k {
        "start" | "flex-start" => taffy::AlignItems::START,
        "end" | "flex-end" => taffy::AlignItems::END,
        "center" => taffy::AlignItems::CENTER,
        "stretch" => taffy::AlignItems::STRETCH,
        "baseline" => taffy::AlignItems::BASELINE,
        _ => return None,
    })
}

/// The base layout style of every node: a column flexbox (as svelters-egui's boxes).
pub(crate) fn base_layout() -> taffy::Style {
    taffy::Style { display: taffy::Display::Flex, flex_direction: taffy::FlexDirection::Column, ..Default::default() }
}

/// The node's taffy style from its computed style (svelters-egui's `layout_style` over CSS only:
/// skelter has no layout props apart from CSS).
pub(crate) fn layout_style(c: &ComputedStyle, b: &ComputedBox, hidden: bool) -> taffy::Style {
    let mut st = base_layout();
    for (p, v) in c.iter() {
        let k = match v {
            ComputedValue::Keyword(k) => Some(*k),
            _ => None,
        };
        match p.name {
            "width" => st.size.width = dim(v).unwrap_or(st.size.width),
            "height" => st.size.height = dim(v).unwrap_or(st.size.height),
            "min-width" => st.min_size.width = lpa(v).unwrap_or(st.min_size.width),
            "min-height" => st.min_size.height = lpa(v).unwrap_or(st.min_size.height),
            "max-width" => st.max_size.width = lpa(v).unwrap_or(st.max_size.width),
            "max-height" => st.max_size.height = lpa(v).unwrap_or(st.max_size.height),
            "flex-basis" => st.flex_basis = dim(v).unwrap_or(st.flex_basis),
            "flex-grow" => {
                if let ComputedValue::Number(n) = v {
                    st.flex_grow = *n
                }
            }
            "flex-shrink" => {
                if let ComputedValue::Number(n) = v {
                    st.flex_shrink = *n
                }
            }
            "align-self" => st.align_self = k.and_then(align),
            "align-items" => st.align_items = k.and_then(align).unwrap_or(taffy::AlignItems::STRETCH),
            "justify-items" => st.justify_items = k.and_then(align).unwrap_or(taffy::AlignItems::STRETCH),
            "margin" => {
                if let ComputedValue::Insets([t, r, bo, l]) = v {
                    let s = |x: &Length| lpa(&ComputedValue::Length(*x)).unwrap_or(taffy::LengthPercentageAuto::length(0.0));
                    st.margin = taffy::Rect { top: s(t), right: s(r), bottom: s(bo), left: s(l) };
                }
            }
            "margin-top" => st.margin.top = lpa(v).unwrap_or(st.margin.top),
            "margin-right" => st.margin.right = lpa(v).unwrap_or(st.margin.right),
            "margin-bottom" => st.margin.bottom = lpa(v).unwrap_or(st.margin.bottom),
            "margin-left" => st.margin.left = lpa(v).unwrap_or(st.margin.left),
            "padding" => {
                if let ComputedValue::Insets([t, r, bo, l]) = v {
                    st.padding = taffy::Rect { top: calc_lp(t), right: calc_lp(r), bottom: calc_lp(bo), left: calc_lp(l) };
                }
            }
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                if let ComputedValue::Length(l) = v {
                    let x = calc_lp(l);
                    match p.name {
                        "padding-top" => st.padding.top = x,
                        "padding-right" => st.padding.right = x,
                        "padding-bottom" => st.padding.bottom = x,
                        _ => st.padding.left = x,
                    }
                }
            }
            "position" => {
                st.position = match k {
                    Some("absolute") | Some("fixed") => taffy::Position::Absolute,
                    Some("static") => taffy::Position::Static,
                    _ => taffy::Position::Relative,
                }
            }
            "left" => st.inset.left = lpa(v).unwrap_or(st.inset.left),
            "top" => st.inset.top = lpa(v).unwrap_or(st.inset.top),
            "right" => st.inset.right = lpa(v).unwrap_or(st.inset.right),
            "bottom" => st.inset.bottom = lpa(v).unwrap_or(st.inset.bottom),
            "display" => match k {
                Some("none") => st.display = taffy::Display::None,
                Some("grid") => st.display = taffy::Display::Grid,
                Some("block") => st.display = taffy::Display::Block,
                Some("flex") => st.display = taffy::Display::Flex,
                _ => {}
            },
            "flex-direction" => {
                st.flex_direction = match k {
                    Some("row") => taffy::FlexDirection::Row,
                    Some("row-reverse") => taffy::FlexDirection::RowReverse,
                    Some("column-reverse") => taffy::FlexDirection::ColumnReverse,
                    _ => taffy::FlexDirection::Column,
                }
            }
            "flex-wrap" => {
                st.flex_wrap = match k {
                    Some("wrap") => taffy::FlexWrap::Wrap,
                    Some("wrap-reverse") => taffy::FlexWrap::WrapReverse,
                    _ => taffy::FlexWrap::NoWrap,
                }
            }
            "justify-content" => {
                st.justify_content = match k {
                    Some("end") | Some("flex-end") => taffy::AlignContent::END,
                    Some("center") => taffy::AlignContent::CENTER,
                    Some("space-between") => taffy::AlignContent::SPACE_BETWEEN,
                    Some("space-around") => taffy::AlignContent::SPACE_AROUND,
                    Some("space-evenly") => taffy::AlignContent::SPACE_EVENLY,
                    Some("stretch") => taffy::AlignContent::STRETCH,
                    _ => taffy::AlignContent::START,
                }
            }
            "gap" => {
                if let ComputedValue::Length(l) = v {
                    st.gap = taffy::Size { width: calc_lp(l), height: calc_lp(l) };
                }
            }
            "row-gap" => {
                if let ComputedValue::Length(l) = v {
                    st.gap.height = calc_lp(l)
                }
            }
            "column-gap" => {
                if let ComputedValue::Length(l) = v {
                    st.gap.width = calc_lp(l)
                }
            }
            _ => {}
        }
    }
    let ov = |o: Overflow| match o {
        Overflow::Visible => taffy::Overflow::Visible,
        Overflow::Hidden => taffy::Overflow::Hidden,
        Overflow::Clip => taffy::Overflow::Clip,
        Overflow::Scroll | Overflow::Auto => taffy::Overflow::Scroll,
    };
    st.overflow = taffy::Point { x: ov(b.overflow_x), y: ov(b.overflow_y) };
    // a scroll container's own scrollbar gutter is drawn by its kind; taffy reserves none
    st.scrollbar_width = 0.0;
    st.border = taffy::Rect {
        top: taffy::LengthPercentage::length(b.border_widths.top),
        right: taffy::LengthPercentage::length(b.border_widths.right),
        bottom: taffy::LengthPercentage::length(b.border_widths.bottom),
        left: taffy::LengthPercentage::length(b.border_widths.left),
    };
    // flex rows, `start`/`end`, grid columns and a scroller's gutter follow it (taffy's RTL)
    st.direction = if b.direction.is_rtl() { taffy::Direction::Rtl } else { taffy::Direction::Ltr };
    if hidden {
        st.display = taffy::Display::None;
    }
    st
}

// ---- the restyle walk ----

/// What a computed-box change needs, by class.
fn box_diff(a: &ComputedBox, b: &ComputedBox) -> (bool, bool, bool) {
    let text = a.font_family != b.font_family
        || a.font_size != b.font_size
        || a.font_weight != b.font_weight
        || a.italic != b.italic
        || a.line_height != b.line_height
        || a.letter_spacing != b.letter_spacing
        || a.wrap != b.wrap
        || a.direction != b.direction;
    let paint = text
        || a.color != b.color
        || a.caret_color != b.caret_color
        || a.background != b.background
        || a.border_widths != b.border_widths
        || a.border_color != b.border_color
        || a.border_style != b.border_style
        || a.radii != b.radii
        || a.shadows != b.shadows
        || a.outline_width != b.outline_width
        || a.outline_color != b.outline_color
        || a.outline_offset != b.outline_offset
        || a.visibility_hidden != b.visibility_hidden
        || a.text_align != b.text_align;
    let property = a.opacity != b.opacity
        || a.transform != b.transform
        || a.transform_origin != b.transform_origin
        || a.overflow_x != b.overflow_x
        || a.overflow_y != b.overflow_y
        || a.z_index != b.z_index
        || a.will_change != b.will_change
        || a.visibility_hidden != b.visibility_hidden
        || a.pointer_events_none != b.pointer_events_none;
    (text, paint, property)
}

fn inherited_differs(a: &ComputedBox, b: &ComputedBox) -> bool {
    a.color != b.color
        || a.caret_color != b.caret_color
        || a.font_family != b.font_family
        || a.font_size != b.font_size
        || a.font_weight != b.font_weight
        || a.italic != b.italic
        || a.line_height != b.line_height
        || a.letter_spacing != b.letter_spacing
        || a.wrap != b.wrap
        || a.text_align != b.text_align
        || a.cursor != b.cursor
        || a.visibility_hidden != b.visibility_hidden
        || a.pointer_events_none != b.pointer_events_none
        || a.direction != b.direction
}

/// What restyling one node found.
struct Restyled {
    /// Its inherited values changed: every child restyles.
    inherited: bool,
    /// The custom properties whose computed value changed (sorted).
    customs: Vec<Atom>,
}

fn transition_for<'a>(b: &'a ComputedBox, prop: &str) -> Option<&'a super::TransitionSpec> {
    b.transitions.iter().rev().find(|t| (&*t.property == prop || &*t.property == "all") && t.duration > Span::ZERO)
}

impl Document {
    /// Restyles the dirty part of `window`'s tree.
    ///
    /// SV69: when a node's custom properties change and nothing else it passes down does, its
    /// descendants are not all restyled. Each one whose declarations reference a changed name or
    /// declare custom properties of their own, or whose kind reads a changed one
    /// ([`Kind::reads_custom_property`](crate::Kind::reads_custom_property)), is restyled; the rest
    /// only take the new inherited custom map. Returns how many nodes it restyled.
    pub(crate) fn restyle(&mut self, window: WindowId) -> u64 {
        let Some(root) = self.window_root(window) else { return 0 };
        let Some(ri) = self.idx(root) else { return 0 };
        let mut count = 0;
        let (w, h) = self.window(window).map_or((0.0, 0.0), |w| (w.size.w, w.size.h));
        let env = svelters_style::MediaEnv { width: w, height: h, ..*self.style.stylist.media_env() };
        if env != *self.style.stylist.media_env() {
            if !self.style.stylist.set_media(env).is_empty() {
                self.core[ri].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            }
        }
        // (node, restyle it whatever its flags, the custom properties its parent changed)
        let mut stack: Vec<(usize, bool, Option<Rc<[Atom]>>)> = vec![(ri, false, None)];
        while let Some((i, force, changed)) = stack.pop() {
            let d = self.core[i].dirty;
            if !force && changed.is_none() && !d.intersects(Dirty::STYLE | Dirty::DESC_STYLE | Dirty::STYLE_SUBTREE) {
                continue;
            }
            let mut force_children = force || d.contains(Dirty::STYLE_SUBTREE);
            let mut pass: Option<Rc<[Atom]>> = None;
            let own = force || d.intersects(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            if own || changed.as_deref().is_some_and(|c| self.custom_change_reaches(i, c)) {
                let r = self.restyle_node(i, window);
                count += 1;
                force_children |= r.inherited;
                pass = (!r.customs.is_empty()).then(|| Rc::from(r.customs));
            } else if let Some(c) = changed {
                self.inherit_customs(i);
                pass = Some(c);
            }
            self.core[i].dirty.remove(Dirty::STYLE | Dirty::DESC_STYLE | Dirty::STYLE_SUBTREE);
            for &c in self.children[i].iter().rev() {
                stack.push((self.ix(c), force_children, if force_children { None } else { pass.clone() }));
            }
        }
        count
    }

    /// Whether a change of the inherited custom properties `changed` can change node `i`'s style
    /// or look: its declarations reference one or declare their own, or its kind reads one.
    fn custom_change_reaches(&self, i: usize, changed: &[Atom]) -> bool {
        let n = &self.style.nodes[i];
        n.css.is_none() || n.deps.reached_by(changed) || self.kinds[i].as_ref().is_some_and(|k| changed.iter().any(|c| k.reads_custom_property(c)))
    }

    /// Node `i` (and its parts) take the parent's new custom properties, nothing else changing.
    fn inherit_customs(&mut self, i: usize) {
        let Some(p) = self.core[i].parent.map(|p| self.ix(p)) else { return };
        let Some(parent) = self.style.nodes[p].css.clone() else { return };
        let n = &mut self.style.nodes[i];
        let Some(css) = n.css.as_mut() else { return };
        let css = Arc::make_mut(css);
        css.inherit_customs_from(&parent);
        for (_, part) in n.parts.iter_mut() {
            Arc::make_mut(part).inherit_customs_from(css);
        }
    }

    /// Restyles node `i`: whether its inherited values changed (children must restyle too), and
    /// which custom properties changed.
    fn restyle_node(&mut self, i: usize, window: WindowId) -> Restyled {
        let parent = self.core[i].parent.map(|p| self.ix(p));
        let mut parent_css = parent.and_then(|p| self.style.nodes[p].css.clone());
        // its directionality first (`:dir()` reads it); a `dir` (or a root) is what it inherits as
        // `direction`, below its own rules
        let (dir, hint) = self.directionality(i);
        self.style.nodes[i].dir = dir;
        let inherited_dir = parent_css.as_deref().and_then(|p| kw(p.get("direction"))).unwrap_or("ltr");
        if hint && inherited_dir != dir.name() {
            let mut p = parent_css.as_deref().cloned().unwrap_or_else(|| ComputedStyle::initial(self.style.stylist.table()));
            p.set("direction", Some(ComputedValue::Keyword(dir.name())));
            parent_css = Some(Arc::new(p));
        }
        let parent_box = parent.map(|p| self.style.nodes[p].computed.clone());
        let inline = self.style.nodes[i].inline.clone();
        let cx = ComputeContext { root_font_size: 16.0, default_font_size: 13.0 };
        let mut caches = std::mem::take(&mut self.style.caches);
        let mut deps = svelters_style::CustomDeps::default();
        let css = self.style.stylist.compute_with_deps(El { doc: self, i }, parent_css.as_deref(), inline.as_deref(), &cx, &mut caches, &mut deps);
        // the kind's paint parts inherit from the node's style
        let part_names = self.registry.get(self.core[i].kind).map_or(&[][..], |e| e.spec.parts);
        let parts: Vec<(&'static str, Arc<ComputedStyle>)> = part_names
            .iter()
            .map(|&name| {
                let pseudo = svelters_style::PseudoElement::part(name);
                (name, Arc::new(self.style.stylist.compute_pseudo_with_deps(El { doc: self, i }, &pseudo, &css, &cx, &mut caches, &mut deps)))
            })
            .collect();
        self.style.caches = caches;
        self.style.nodes[i].deps = deps;
        let parts_changed = self.style.nodes[i].parts.len() != parts.len() || self.style.nodes[i].parts.iter().zip(&parts).any(|(a, b)| a.0 != b.0 || *a.1 != *b.1);
        if parts_changed {
            self.style.nodes[i].parts = parts;
        }
        let new_box = computed_box(&css, parent_box.as_deref(), cx.default_font_size);
        let hidden = self.view_at(i).prop_bool(crate::common::HIDDEN);
        let cascaded = layout_style(&css, &new_box, hidden);
        let cascaded_dir = cascaded.direction;
        // the kind's own layout participation over the cascaded style
        let mut tstyle = match self.kinds[i].take() {
            Some(k) => {
                let st = {
                    // the style just computed, not the last restyle's (`None` on the first: inline-only
                    // layout CSS then lost to the kind's prop defaults, A22)
                    let view = crate::cx::NodeView { style: &new_box, css: Some(&css), ..self.view_at(i) };
                    k.layout_style(&crate::cx::StyleCx { node: view, cascaded: &cascaded })
                };
                self.kinds[i] = Some(k);
                st
            }
            None => cascaded,
        };
        // a kind's own style keeps the node's direction
        tstyle.direction = cascaded_dir;
        // chrome the kind paints around its children
        let container = self.registry.get(self.core[i].kind).is_some_and(|e| e.spec.container);
        if container
            && let Some(pad) = crate::layout::with_measure_cx(self, i, Some(&new_box), |k, cx| k.chrome_padding(cx)).flatten()
        {
            crate::layout::add_padding(&mut tstyle, pad);
        }
        if container
            && let Some(min) = crate::layout::with_measure_cx(self, i, Some(&new_box), |k, cx| k.chrome_min_size(cx)).flatten()
        {
            if tstyle.min_size.width.is_auto() {
                tstyle.min_size.width = taffy::LengthPercentageAuto::length(min.w);
            }
            if tstyle.min_size.height.is_auto() {
                tstyle.min_size.height = taffy::LengthPercentageAuto::length(min.h);
            }
        }
        // the caption a container's chrome adds to its intrinsic widths (a header that elides)
        let chrome_w = if container { crate::layout::with_measure_cx(self, i, Some(&new_box), |k, cx| k.chrome_widths(cx)).flatten() } else { None };
        if self.layout[i].chrome_w != chrome_w {
            self.layout[i].chrome_w = chrome_w;
            self.invalidate_style(i);
        }
        // a parent that places its children has the last word
        self.place_child(i, &mut tstyle);
        // a window root's size is the window's (set by layout)
        if self.core[i].flags.contains(crate::tree::NodeFlags::ROOT) {
            tstyle.size = self.layout[i].style.size;
        }
        let first = self.style.nodes[i].css.is_none();
        let old_box = self.style.nodes[i].computed.clone();
        let node = self.slots.id_at(i).expect("live");
        if !first {
            self.start_transitions(window, node, &old_box, &new_box);
            let old_size = self.layout[i].style.size;
            self.start_ui_transitions(node, &old_box, &new_box, old_size, tstyle.size);
        }
        let (mut text, mut paint, property) = box_diff(&old_box, &new_box);
        // custom properties inherit too: a changed map reaches the descendants that read it (SV65,
        // SV69: `restyle` passes the changed names down)
        let customs = self.style.nodes[i].css.as_ref().map(|old| css.changed_customs(old)).unwrap_or_default();
        // the kind's look reads a changed one outside the cascade (a UA token): measure and record
        if !first && !customs.is_empty() && self.kinds[i].as_ref().is_some_and(|k| customs.iter().any(|c| k.reads_custom_property(c))) {
            text = true;
            paint = true;
        }
        let inherited = first || inherited_differs(&old_box, &new_box);
        let new_box = if *old_box == new_box { old_box } else { Arc::new(new_box) };
        self.style.nodes[i].computed = new_box;
        self.style.nodes[i].css = Some(Arc::new(css));
        if first || *self.layout[i].style != tstyle {
            let key = crate::compact::style_key(&tstyle);
            self.layout[i].style = self.styles.intern(tstyle, key);
            self.invalidate_style(i);
            self.layout[i].intrinsic = None;
        }
        if text {
            self.mark_measure(i);
        }
        if text || paint {
            self.fold_up(i);
        }
        if first || paint || parts_changed {
            self.mark(node, Dirty::PAINT | Dirty::A11Y);
        }
        if parts_changed && !first {
            self.fold_up(i);
        }
        if first || property {
            self.mark(node, Dirty::PROPERTY);
            if let Some(ws) = self.window_mut(window) {
                ws.rebuild = true;
            }
        }
        if self.registry.get(self.core[i].kind).is_some_and(|e| e.spec.places_children) {
            self.replace_children(i);
        }
        // the scroll range may have changed with the overflow axes
        if property && self.scroll_axes(i) == (false, false) {
            self.scroll.remove(&node);
        }
        Restyled { inherited, customs }
    }

    /// Child `i`'s parent places its children ([`KindSpec::places_children`](crate::KindSpec::places_children)):
    /// its `child_layout` adjusts the child's taffy style and decides whether it is shown.
    pub(crate) fn place_child(&mut self, i: usize, tstyle: &mut taffy::Style) {
        let Some(p) = self.core[i].parent else { return };
        let pi = self.ix(p);
        if !self.registry.get(self.core[pi].kind).is_some_and(|e| e.spec.places_children) {
            return;
        }
        let node = self.slots.id_at(i).expect("live");
        let Some(k) = self.kinds[pi].take() else { return };
        let shown = {
            let index = self.children[pi].iter().position(|&c| c == node).unwrap_or(0);
            let cx = crate::cx::ChildLayoutCx { parent: self.view_at(pi), child: self.view_at(i), index, count: self.children[pi].len() };
            k.child_layout(&cx, tstyle)
        };
        self.kinds[pi] = Some(k);
        let was = !self.core[i].flags.contains(crate::tree::NodeFlags::CONCEALED);
        if shown != was {
            if shown {
                self.core[i].flags.remove(crate::tree::NodeFlags::CONCEALED);
            } else {
                self.core[i].flags.insert(crate::tree::NodeFlags::CONCEALED);
            }
            self.core[pi].dirty.insert(Dirty::CHILDREN | Dirty::A11Y);
            self.mark_up(pi, Dirty::DESC_RECORD);
            if let Some(ws) = self.core[i].window.and_then(|w| self.window_mut(w)) {
                ws.rebuild = true;
            }
        }
    }

    /// Re-runs the parent's `child_layout` for every child of `pi` (restyles them).
    pub(crate) fn replace_children(&mut self, pi: usize) {
        for c in self.children[pi].clone() {
            let ci = self.ix(c);
            self.core[ci].dirty.insert(Dirty::STYLE);
        }
        self.core[pi].dirty.insert(Dirty::DESC_STYLE);
        self.mark_up(pi, Dirty::DESC_STYLE);
    }

    /// Starts (or retargets) the UI-thread transitions of the paint and layout properties a
    /// `transition` covers: colours re-record, `width`/`height` (both in points) relayout. A restyle
    /// to the value a running transition already heads for keeps it; a change while one runs starts
    /// from the value shown now (the old box holds the last sample).
    fn start_ui_transitions(&mut self, node: NodeId, old: &ComputedBox, new: &ComputedBox, old_size: taffy::Size<taffy::Dimension>, new_size: taffy::Size<taffy::Dimension>) {
        use crate::anim::{UiProp, UiTransition};
        let reduced = self.style.stylist.media_env().reduced_motion;
        let solid = |f: &Fill| match f {
            Fill::Solid(c) => Some(*c),
            Fill::Linear { .. } => None,
        };
        let mut changes: Vec<(UiProp, AnimValue, AnimValue)> = Vec::new();
        let mut color = |p: UiProp, a: Color, b: Color| {
            if a != b {
                changes.push((p, AnimValue::Color(a), AnimValue::Color(b)));
            }
        };
        // a node that draws only from its palette (an icon) animates `color` on the compositor
        if !self.palette_only(node) {
            color(UiProp::Color, old.color, new.color);
        }
        if let (Some(a), Some(b)) = (solid(&old.background), solid(&new.background)) {
            color(UiProp::Background, a, b);
        }
        color(UiProp::BorderColor, old.border_color, new.border_color);
        color(UiProp::OutlineColor, old.outline_color, new.outline_color);
        for (p, a, b) in [(UiProp::Width, old_size.width, new_size.width), (UiProp::Height, old_size.height, new_size.height)] {
            if let (Some(a), Some(b)) = (a.into_option(), b.into_option())
                && a != b
            {
                changes.push((p, AnimValue::Length(a), AnimValue::Length(b)));
            }
        }
        for (prop, from, to) in changes {
            if self.anim.ui_running(node, prop).is_some_and(|t| t.to == to) {
                continue;
            }
            // reduced motion: values jump (a running transition stops)
            let spec = transition_for(new, prop.css()).or_else(|| (prop == UiProp::Background).then(|| transition_for(new, "background")).flatten()).filter(|_| !reduced);
            let list = self.anim.ui.entry(node).or_default();
            list.retain(|t| t.prop != prop);
            let Some(t) = spec else {
                if list.is_empty() {
                    self.anim.ui.remove(&node);
                }
                continue;
            };
            let timing = Timing {
                delay: t.delay,
                end_delay: Span::ZERO,
                duration: t.duration,
                iterations: 1.0,
                iteration_start: 0.0,
                direction: PlaybackDirection::Normal,
                fill: FillMode::Backwards,
                easing: t.easing.clone(),
            };
            let generator = Generator::Keyframes {
                keyframes: vec![Keyframe { offset: 0.0, value: from, easing: None }, Keyframe { offset: 1.0, value: to.clone(), easing: None }],
                timing,
            };
            list.push(UiTransition { prop, generator, start: None, to, length: t.delay + t.duration });
        }
    }

    /// Applies the UI-thread transitions of `window`'s nodes at `now` (the frame's presentation
    /// time): their sampled values replace the computed ones until they end, then `transitionend`
    /// is queued. Inherited colours reach descendants at their own restyle, not per frame.
    pub(crate) fn sample_ui_transitions(&mut self, window: WindowId, now: skelter_base::time::Time) {
        use crate::anim::UiProp;
        if self.anim.ui.is_empty() {
            return;
        }
        let nodes: Vec<NodeId> = self.anim.ui.keys().copied().filter(|&n| self.window_of(n) == Some(window)).collect();
        for node in nodes {
            let Some(i) = self.idx(node) else { continue };
            let list = self.anim.ui.remove(&node).unwrap_or_default();
            let mut keep = Vec::with_capacity(list.len());
            let mut ended = Vec::new();
            for mut t in list {
                let start = *t.start.get_or_insert(now);
                let mut s = skelter_anim::sample(&t.generator, start, now);
                s.done |= now - start >= t.length;
                let v = if s.done { t.to.clone() } else { s.value.clone().unwrap_or_else(|| t.to.clone()) };
                match (t.prop, &v) {
                    (UiProp::Width | UiProp::Height, AnimValue::Length(l)) => {
                        let size = &mut std::sync::Arc::make_mut(&mut self.layout[i].style).size;
                        let d = if t.prop == UiProp::Width { &mut size.width } else { &mut size.height };
                        if d.into_option() != Some(*l) {
                            *d = taffy::Dimension::length(*l);
                            self.mark_layout(node);
                        }
                    }
                    (p, AnimValue::Color(c)) => {
                        let b = Arc::make_mut(&mut self.style.nodes[i].computed);
                        match p {
                            UiProp::Color => b.color = *c,
                            UiProp::Background => b.background = Fill::Solid(*c),
                            UiProp::BorderColor => b.border_color = *c,
                            _ => b.outline_color = *c,
                        }
                        self.mark(node, Dirty::PAINT);
                    }
                    _ => {}
                }
                if s.done {
                    ended.push((t.prop, (now - start).as_secs_f64()));
                } else {
                    keep.push(t);
                }
            }
            if !keep.is_empty() {
                self.anim.ui.insert(node, keep);
            }
            for (prop, elapsed) in ended {
                transition_end(self, node, prop.css(), elapsed);
            }
        }
    }

    /// Whether `node`'s recorded list draws only slotted primitives (an icon whose colours are
    /// palette slots): its `color` transitions run on the compositor as `AnimProperty::Palette(0)`.
    pub(crate) fn palette_only(&self, node: NodeId) -> bool {
        use skelter_base::display::Prim;
        let Some(list) = self.idx(node).and_then(|i| self.record[i].list.as_ref()) else { return false };
        let mut slotted = false;
        for p in &list.items {
            match p {
                Prim::Icon(ic) if ic.slot.is_some() && ic.color.is_some() => slotted |= ic.slot.is_some_and(|s| s.index == 0),
                Prim::Path(pp) if pp.slot.is_some() => slotted |= pp.slot.is_some_and(|s| s.index == 0),
                Prim::Icon(_) | Prim::Path(_) => return false,
                Prim::PushClip(_) | Prim::PopClip => {}
                _ => return false,
            }
        }
        slotted
    }

    /// Starts engine animations for the compositor properties whose change a `transition` covers.
    fn start_transitions(&mut self, window: WindowId, node: NodeId, old: &ComputedBox, new: &ComputedBox) {
        // reduced motion (SV66): no transition starts, the value jumps
        let reduced = self.style.stylist.media_env().reduced_motion;
        let start = |doc: &mut Document, prop: AnimProperty, name: &str, from: AnimValue, to: AnimValue| {
            let Some(t) = transition_for(new, name).filter(|_| !reduced) else { return };
            // a running transition of the same property is replaced (it continues from the old
            // computed value, not from its current sample: see SKELTER-core.md)
            let running = doc.anim.running.entry(node).or_default();
            let mut cancelled = Vec::new();
            running.retain(|&(id, p)| {
                if p == prop {
                    cancelled.push(id);
                    false
                } else {
                    true
                }
            });
            let id = doc.anim.alloc();
            doc.anim.running.entry(node).or_default().push((id, prop));
            let timing = Timing {
                delay: t.delay,
                end_delay: Span::ZERO,
                duration: t.duration,
                iterations: 1.0,
                iteration_start: 0.0,
                direction: PlaybackDirection::Normal,
                fill: FillMode::Backwards,
                easing: t.easing.clone(),
            };
            let generator = Generator::Keyframes {
                keyframes: vec![Keyframe { offset: 0.0, value: from, easing: None }, Keyframe { offset: 1.0, value: to, easing: None }],
                timing,
            };
            let spec = AnimationSpec { id, node, target: None, property: prop, generator, start: StartTime::NextFrame };
            if let Some(ws) = doc.window_mut(window) {
                ws.pending.animations.extend(cancelled.into_iter().map(AnimationCmd::Cancel));
                ws.pending.animations.push(AnimationCmd::Start(spec));
            }
        };
        if old.color != new.color && self.palette_only(node) {
            start(self, AnimProperty::Palette(0), "color", AnimValue::Color(old.color), AnimValue::Color(new.color));
            // the palette lives on the node's effect node, made by the property pass
            if self.animates(node, AnimProperty::Palette(0)) {
                self.mark(node, Dirty::PROPERTY);
                if let Some(ws) = self.window_mut(window) {
                    ws.rebuild = true;
                }
            }
        }
        if old.opacity != new.opacity {
            start(self, AnimProperty::Opacity, "opacity", AnimValue::Number(old.opacity), AnimValue::Number(new.opacity));
        }
        if old.transform != new.transform {
            start(
                self,
                AnimProperty::Transform,
                "transform",
                AnimValue::Transform(vec![TransformFn::Matrix(old.transform)]),
                AnimValue::Transform(vec![TransformFn::Matrix(new.transform)]),
            );
        }
    }
}

/// Queues the `transitionend` event of `property` on `node`.
pub(crate) fn transition_end(doc: &mut Document, node: NodeId, property: &str, elapsed: f64) {
    let rec: Vec<(Arc<str>, crate::value::Value)> = vec![("propertyName".into(), crate::value::Value::from(property)), ("elapsedTime".into(), crate::value::Value::F64(elapsed))];
    let e = crate::cx::Emitted { node, signal: crate::common::TRANSITIONEND, arg: Some(crate::value::Value::Record(rec.into())) };
    crate::input::dispatch::queue_emitted(doc, e);
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::geom::Point;

    #[test]
    fn transforms_parse() {
        let (m, f) = parse_transform("translate(10px, 5px) scale(2)").unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(m.apply(Point::new(1.0, 1.0)), Point::new(12.0, 7.0));
        assert_eq!(parse_transform("none").unwrap().0, Affine::IDENTITY);
        assert!(parse_transform("perspective(10px)").is_none());
        assert_eq!(parse_origin("left top"), Some(Point::new(0.0, 0.0)));
        assert_eq!(parse_origin("25% 100%"), Some(Point::new(0.25, 1.0)));
        assert_eq!(parse_origin("bottom"), Some(Point::new(0.5, 1.0)));
        assert_eq!(parse_origin("10px 4px"), None);
        let (r, _) = parse_transform("rotate(90deg)").unwrap();
        let p = r.apply(Point::new(1.0, 0.0));
        assert!((p.x).abs() < 1e-5 && (p.y - 1.0).abs() < 1e-5);
    }
}
