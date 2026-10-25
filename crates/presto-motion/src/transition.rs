//! svelte/transition's built-ins (`fade`, `fly`, `slide`, `scale`, `blur`, `draw`; svelte@5.57.2
//! `src/transition/index.js`, MIT) evaluated natively from their parameters and the node's computed
//! style, and planned as presto animation tracks.
//!
//! Svelte runs a transition as: `t = t1 + (t2 - t1) * easing(p)` for progress `p` in 0..=1, the
//! node's style is `css(t, 1 - t)`, and the keyframes are that style at `p = i / n`,
//! `n = ceil(duration / (1000 / 60))` (60 Hz, `animate` in `internal/client/dom/elements/transitions.js`).
//! [`Transition::css`] is `css(t, u)` as numbers ([`Frame`]); [`Run`] is the `t1 → t2` part; every
//! built-in is affine in `t` per channel (slide's opacity is piecewise affine), so [`Transition::plan`]
//! turns it into two- or three-keyframe tracks whose timing easing is the transition's easing:
//! presto samples them at the display's refresh rate instead of Svelte's baked 60 Hz frames.

use crate::easing::{SvelteEasing, SvelteEasingExt};
use presto_base::anim::{AnimProperty, AnimValue, FillMode, Keyframe, PlaybackDirection, Timing, TransformFn};
use presto_base::time::Span;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionKind {
    Fade,
    Fly,
    Slide,
    Scale,
    Blur,
    Draw,
}

impl TransitionKind {
    pub fn from_name(name: &str) -> Option<TransitionKind> {
        Some(match name {
            "fade" => TransitionKind::Fade,
            "fly" => TransitionKind::Fly,
            "slide" => TransitionKind::Slide,
            "scale" => TransitionKind::Scale,
            "blur" => TransitionKind::Blur,
            "draw" => TransitionKind::Draw,
            _ => return None,
        })
    }
}

/// A number or a CSS length string as svelte/transition's `split_css_unit` reads it:
/// `12` and `"12"` are px, `"50%"` is (50, `%`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CssLength {
    Number(f64),
    Text(String),
}

impl CssLength {
    /// `split_css_unit`: `[parseFloat(number), unit || 'px']` for a matching string, else `[value, 'px']`.
    pub fn split(&self) -> (f64, String) {
        match self {
            CssLength::Number(n) => (*n, "px".into()),
            CssLength::Text(s) => {
                let t = s.trim();
                let num_end = t.char_indices().find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'))).map_or(t.len(), |(i, _)| i);
                let (num, rest) = t.split_at(num_end);
                // the regex `^\s*(-?[\d.]+)([^\s]*)\s*$`: a number part, then a unit without spaces
                if num.is_empty() || num == "-" || rest.contains(char::is_whitespace) {
                    // JS keeps the string itself as the value; `t * value` is NaN then
                    return (f64::NAN, "px".into());
                }
                let unit = if rest.is_empty() { "px".to_string() } else { rest.to_string() };
                (parse_float(num), unit)
            }
        }
    }
}

/// JS `parseFloat` of the leading number of `s` (`"1.5.2"` is 1.5); NaN when there is none.
pub fn parse_float(s: &str) -> f64 {
    let s = s.trim_start();
    let mut end = 0;
    let mut dot = false;
    let mut exp = false;
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut digits = false;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            digits = true;
            end = i + 1;
        } else if c == b'.' && !dot && !exp {
            dot = true;
        } else if (c == b'e' || c == b'E') && digits && !exp {
            exp = true;
            if i + 1 < b.len() && (b[i + 1] == b'-' || b[i + 1] == b'+') {
                i += 1;
            }
        } else {
            break;
        }
        i += 1;
    }
    if !digits {
        return f64::NAN;
    }
    s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    #[default]
    Y,
}

/// The JSON-able parameters of a built-in transition (functions are resolved in JS: a custom easing
/// keeps the transition in JS; a `duration` function of `draw` arrives resolved in the run).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Params {
    pub delay: Option<f64>,
    pub duration: Option<f64>,
    /// svelte/easing export name; `None` = the transition's default.
    pub easing: Option<String>,
    pub x: Option<CssLength>,
    pub y: Option<CssLength>,
    pub opacity: Option<f64>,
    pub start: Option<f64>,
    pub amount: Option<CssLength>,
    pub axis: Option<Axis>,
    pub speed: Option<f64>,
}

/// What the built-ins read from `getComputedStyle(node)` (and `draw` from `getTotalLength()`), as
/// CSS text the way the browser gives it (`"12px"`, `"none"`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Computed {
    pub opacity: String,
    pub transform: String,
    pub filter: String,
    pub width: String,
    pub height: String,
    pub padding_top: String,
    pub padding_bottom: String,
    pub padding_left: String,
    pub padding_right: String,
    pub margin_top: String,
    pub margin_bottom: String,
    pub margin_left: String,
    pub margin_right: String,
    pub border_top_width: String,
    pub border_bottom_width: String,
    pub border_left_width: String,
    pub border_right_width: String,
    pub stroke_linecap: String,
    pub stroke_width: String,
    /// `node.getTotalLength()` (`draw`).
    pub total_length: Option<f64>,
}

impl Default for Computed {
    fn default() -> Self {
        let px0 = || "0px".to_string();
        Computed {
            opacity: "1".into(),
            transform: "none".into(),
            filter: "none".into(),
            width: "auto".into(),
            height: "auto".into(),
            padding_top: px0(),
            padding_bottom: px0(),
            padding_left: px0(),
            padding_right: px0(),
            margin_top: px0(),
            margin_bottom: px0(),
            margin_left: px0(),
            margin_right: px0(),
            border_top_width: px0(),
            border_bottom_width: px0(),
            border_left_width: px0(),
            border_right_width: px0(),
            stroke_linecap: "butt".into(),
            stroke_width: "1px".into(),
            total_length: None,
        }
    }
}

/// JS unary `+` on a string (`+"0.5"`; `+""` is 0).
fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// Why a transition cannot be played from its parameters natively; the caller plays Svelte's
/// keyframes instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotNative {
    /// A length in a unit presto cannot resolve here (`em`, `vw`, ...).
    Unit(String),
    /// The node's computed `filter` or `transform` is not one presto parses.
    Style(&'static str, String),
    UnknownEasing(String),
}

impl std::fmt::Display for NotNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotNative::Unit(u) => write!(f, "unit {u:?} is not resolved natively"),
            NotNative::Style(p, v) => write!(f, "computed {p} {v:?} is not parsed natively"),
            NotNative::UnknownEasing(e) => write!(f, "easing {e:?} is not one of svelte/easing's"),
        }
    }
}

/// A slide dimension: the primary size, then padding, margin and border width of both sides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dim {
    /// The CSS property Svelte writes (`height`, `padding-top`, ...).
    pub property: &'static str,
    /// Its value at `t = 1` in px (`parseFloat` of the computed value).
    pub value: f64,
}

/// A resolved built-in transition: what Svelte's function computes once, before `css` runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub delay: f64,
    /// The transition's own duration (ms); the run scales it by `|t2 - t1|`.
    pub duration: f64,
    pub easing: SvelteEasing,
    /// The node's computed transform, prepended to the animated one (`fly`, `scale`).
    pub transform: TransformList,
    target_opacity: f64,
    /// `od` (`fly`, `scale`, `blur`) or `o` (`fade`, `slide`).
    od: f64,
    /// fly: px offsets; scale: `sd`; blur: amount in px.
    a: f64,
    b: f64,
    /// slide: the dimensions Svelte writes (NaN ones dropped), and the axis.
    dims: Vec<Dim>,
    axis: Axis,
    /// draw: the path length.
    len: f64,
}

/// `css(t, u)` as values, in the order Svelte writes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub opacity: Option<f64>,
    /// The computed transform followed by the animated functions (px).
    pub transform: Option<Vec<Transform64>>,
    /// `blur(<px>)` after the computed filter.
    pub blur: Option<f64>,
    /// slide: `overflow: hidden`, the dimensions, `min-<size>: 0`.
    pub overflow_hidden: bool,
    pub dims: Vec<(&'static str, f64)>,
    /// draw: `stroke-dasharray`, `stroke-dashoffset`.
    pub dash: Option<(f64, f64)>,
}

/// A transform list in `f64`.
pub type TransformList = Vec<Transform64>;

/// A computed `transform` (`matrix(1, 0, 0, 1, 10, 0)`, `translate(10px, 0) scale(.9)`), parsed by
/// `presto_anim::css`; `None` for what it refuses or a translation that isn't in px.
pub fn parse_transform(text: &str) -> Option<TransformList> {
    use presto_anim::css::{CssTransformFn, CssValue, Unit, parse_value};
    let CssValue::Transform(fns) = parse_value("transform", text).ok()? else { return None };
    fns.iter()
        .map(|f| {
            Some(match *f {
                CssTransformFn::Translate(x, y) => {
                    if x.unit != Unit::Px || y.unit != Unit::Px {
                        return None;
                    }
                    Transform64::Translate(x.value as f64, y.value as f64)
                }
                CssTransformFn::Scale(x, y) => Transform64::Scale(x as f64, y as f64),
                CssTransformFn::Rotate(a) => Transform64::Rotate(a as f64),
                CssTransformFn::Skew(x, y) => Transform64::Skew(x as f64, y as f64),
                CssTransformFn::Matrix(m) => Transform64::Matrix([m.a, m.b, m.c, m.d, m.e, m.f].map(|v| v as f64)),
            })
        })
        .collect()
}

/// A transform function in `f64` (presto's [`TransformFn`] is `f32`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform64 {
    Translate(f64, f64),
    Scale(f64, f64),
    Rotate(f64),
    Skew(f64, f64),
    Matrix([f64; 6]),
}

impl Transform64 {
    pub fn to_presto(self) -> TransformFn {
        match self {
            Transform64::Translate(x, y) => TransformFn::Translate(x as f32, y as f32),
            Transform64::Scale(x, y) => TransformFn::Scale(x as f32, y as f32),
            Transform64::Rotate(r) => TransformFn::Rotate(r as f32),
            Transform64::Skew(x, y) => TransformFn::Skew(x as f32, y as f32),
            Transform64::Matrix(m) => TransformFn::Matrix(presto_base::geom::Affine {
                a: m[0] as f32,
                b: m[1] as f32,
                c: m[2] as f32,
                d: m[3] as f32,
                e: m[4] as f32,
                f: m[5] as f32,
            }),
        }
    }

    /// The numbers Svelte's CSS text holds for this function, in order.
    pub fn numbers(&self, out: &mut Vec<f64>) {
        match *self {
            Transform64::Translate(x, y) | Transform64::Scale(x, y) | Transform64::Skew(x, y) => out.extend([x, y]),
            Transform64::Rotate(r) => out.push(r),
            Transform64::Matrix(m) => out.extend(m),
        }
    }
}

impl Frame {
    /// The numbers of Svelte's `css(t, u)` text in the order they appear there (the fidelity tests
    /// compare these with the numbers in the JS output). A computed transform or filter prefix is
    /// included as presto holds it.
    pub fn numbers(&self, kind: TransitionKind) -> Vec<f64> {
        let mut out = Vec::new();
        let transform = |out: &mut Vec<f64>| {
            for f in self.transform.iter().flatten() {
                f.numbers(out);
            }
        };
        match kind {
            TransitionKind::Fade => out.extend(self.opacity),
            TransitionKind::Fly => {
                transform(&mut out);
                out.extend(self.opacity);
            }
            TransitionKind::Scale => {
                // Svelte writes `scale(s)` with one argument
                let list = self.transform.as_deref().unwrap_or_default();
                if let Some((last, prefix)) = list.split_last() {
                    prefix.iter().for_each(|f| f.numbers(&mut out));
                    if let Transform64::Scale(s, _) = last {
                        out.push(*s);
                    }
                }
                out.extend(self.opacity);
            }
            TransitionKind::Blur => {
                out.extend(self.opacity);
                out.extend(self.blur);
            }
            TransitionKind::Slide => {
                out.extend(self.opacity);
                out.extend(self.dims.iter().map(|d| d.1));
                out.push(0.0);
            }
            TransitionKind::Draw => {
                let (array, offset) = self.dash.unwrap_or_default();
                out.extend([array, offset]);
            }
        }
        out
    }
}

impl Transition {
    /// What Svelte's `kind(node, params)` computes before returning its config.
    pub fn new(kind: TransitionKind, params: &Params, style: &Computed) -> Result<Transition, NotNative> {
        let default_easing = match kind {
            TransitionKind::Fade => SvelteEasing::Linear,
            TransitionKind::Blur | TransitionKind::Draw => SvelteEasing::CubicInOut,
            _ => SvelteEasing::CubicOut,
        };
        let easing = match &params.easing {
            None => default_easing,
            Some(n) => SvelteEasing::from_name(n).ok_or_else(|| NotNative::UnknownEasing(n.clone()))?,
        };
        let target_opacity = js_number(&style.opacity);
        let opacity_param = params.opacity.unwrap_or(0.0);
        let od = target_opacity * (1.0 - opacity_param);
        let transform = || -> Result<TransformList, NotNative> {
            if style.transform == "none" {
                return Ok(Vec::new());
            }
            parse_transform(&style.transform).ok_or_else(|| NotNative::Style("transform", style.transform.clone()))
        };
        let mut t = Transition {
            kind,
            delay: params.delay.unwrap_or(0.0),
            duration: params.duration.unwrap_or(400.0),
            easing,
            transform: Vec::new(),
            target_opacity,
            od,
            a: 0.0,
            b: 0.0,
            dims: Vec::new(),
            axis: params.axis.unwrap_or_default(),
            len: 0.0,
        };
        match kind {
            TransitionKind::Fade => t.od = target_opacity,
            TransitionKind::Fly => {
                t.transform = transform()?;
                let w = parse_float(&style.width);
                let h = parse_float(&style.height);
                t.a = resolve_length(params.x.as_ref(), w)?;
                t.b = resolve_length(params.y.as_ref(), h)?;
            }
            TransitionKind::Scale => {
                t.transform = transform()?;
                t.a = 1.0 - params.start.unwrap_or(0.0);
            }
            TransitionKind::Blur => {
                if style.filter != "none" && !style.filter.is_empty() {
                    return Err(NotNative::Style("filter", style.filter.clone()));
                }
                let (value, unit) = params.amount.clone().unwrap_or(CssLength::Number(5.0)).split();
                if unit != "px" {
                    return Err(NotNative::Unit(unit));
                }
                t.a = value;
            }
            TransitionKind::Slide => {
                t.od = target_opacity;
                let (primary, primary_value, names, sides) = match t.axis {
                    Axis::Y => (
                        "height",
                        parse_float(&style.height),
                        ["padding-top", "padding-bottom", "margin-top", "margin-bottom", "border-top-width", "border-bottom-width"],
                        [&style.padding_top, &style.padding_bottom, &style.margin_top, &style.margin_bottom, &style.border_top_width, &style.border_bottom_width],
                    ),
                    Axis::X => (
                        "width",
                        parse_float(&style.width),
                        ["padding-left", "padding-right", "margin-left", "margin-right", "border-left-width", "border-right-width"],
                        [&style.padding_left, &style.padding_right, &style.margin_left, &style.margin_right, &style.border_left_width, &style.border_right_width],
                    ),
                };
                let mut dims = vec![Dim { property: primary, value: primary_value }];
                dims.extend(names.iter().zip(sides).map(|(&property, v)| Dim { property, value: parse_float(v) }));
                // `css_dimension` writes nothing for NaN
                dims.retain(|d| !d.value.is_nan());
                t.dims = dims;
            }
            TransitionKind::Draw => {
                let mut len = style.total_length.unwrap_or(0.0);
                if style.stroke_linecap != "butt" {
                    // `parseInt(style.strokeWidth)`
                    len += parse_float(&style.stroke_width).trunc();
                }
                t.len = len;
                t.duration = match (params.duration, params.speed) {
                    (Some(d), _) => d,
                    (None, None) => 800.0,
                    (None, Some(speed)) => len / speed,
                };
            }
        }
        Ok(t)
    }

    /// The axis a slide animates.
    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// Svelte's `css(t, u)` as values.
    pub fn css(&self, t: f64, u: f64) -> Frame {
        let mut f = Frame::default();
        match self.kind {
            TransitionKind::Fade => f.opacity = Some(t * self.od),
            TransitionKind::Fly => {
                let mut list = self.transform.clone();
                list.push(Transform64::Translate((1.0 - t) * self.a, (1.0 - t) * self.b));
                f.transform = Some(list);
                f.opacity = Some(self.target_opacity - self.od * u);
            }
            TransitionKind::Scale => {
                let mut list = self.transform.clone();
                let s = 1.0 - self.a * u;
                list.push(Transform64::Scale(s, s));
                f.transform = Some(list);
                f.opacity = Some(self.target_opacity - self.od * u);
            }
            TransitionKind::Blur => {
                f.opacity = Some(self.target_opacity - self.od * u);
                f.blur = Some(u * self.a);
            }
            TransitionKind::Slide => {
                f.overflow_hidden = true;
                f.opacity = Some((t * 20.0).min(1.0) * self.od);
                f.dims = self.dims.iter().map(|d| (d.property, t * d.value)).collect();
            }
            TransitionKind::Draw => f.dash = Some((self.len, u * self.len)),
        }
        f
    }

    /// The tracks presto plays for `run`: per channel, the values at `t1` and `t2` (and slide's
    /// opacity knee at `t = 0.05`) as keyframes whose offsets are eased progress, with the
    /// transition's easing as the timing easing.
    pub fn plan(&self, run: &Run) -> Plan {
        let at = |t: f64| self.css(t, 1.0 - t);
        let (a, b) = (at(run.t1), at(run.t2));
        let mut tracks = Vec::new();
        let kf = |offset: f64, value: AnimValue| Keyframe { offset: offset as f32, value, easing: None };
        let pair = |channel: Channel, x: AnimValue, y: AnimValue| Track { channel, keyframes: vec![kf(0.0, x), kf(1.0, y)] };
        if let (Some(x), Some(y)) = (a.opacity, b.opacity) {
            let mut track = pair(Channel::Opacity, AnimValue::Number(x as f32), AnimValue::Number(y as f32));
            if self.kind == TransitionKind::Slide {
                let delta = run.t2 - run.t1;
                if delta != 0.0 {
                    let p = (0.05 - run.t1) / delta;
                    if p > 0.0 && p < 1.0 {
                        track.keyframes.insert(1, kf(p, AnimValue::Number(at(0.05).opacity.unwrap_or(0.0) as f32)));
                    }
                }
            }
            tracks.push(track);
        }
        if let (Some(x), Some(y)) = (&a.transform, &b.transform) {
            let list = |l: &Vec<Transform64>| AnimValue::Transform(l.iter().map(|f| f.to_presto()).collect());
            tracks.push(pair(Channel::Transform, list(x), list(y)));
        }
        if let (Some(x), Some(y)) = (a.blur, b.blur) {
            tracks.push(pair(Channel::Blur, AnimValue::Length(x as f32), AnimValue::Length(y as f32)));
        }
        for (i, &(property, x)) in a.dims.iter().enumerate() {
            let y = b.dims[i].1;
            let channel = match property {
                "height" => Channel::Size(Axis::Y),
                "width" => Channel::Size(Axis::X),
                p => Channel::Css(p),
            };
            tracks.push(pair(channel, AnimValue::Length(x as f32), AnimValue::Length(y as f32)));
        }
        let mut statics = Vec::new();
        if let (Some((len, x)), Some((_, y))) = (a.dash, b.dash) {
            statics.push((Channel::StrokeDashArray, AnimValue::Length(len as f32)));
            tracks.push(pair(Channel::StrokeDashOffset, AnimValue::Length(x as f32), AnimValue::Length(y as f32)));
        }
        if self.kind == TransitionKind::Slide {
            statics.push((Channel::Css(if self.axis == Axis::Y { "min-height" } else { "min-width" }), AnimValue::Length(0.0)));
        }
        Plan { tracks, statics, overflow_hidden: self.kind == TransitionKind::Slide, origin: None, easing: self.easing, timing: run.timing(self.easing) }
    }
}

/// A `%` length resolves against the node's size (`translate` percentages); px as is.
fn resolve_length(v: Option<&CssLength>, size: f64) -> Result<f64, NotNative> {
    let Some(v) = v else { return Ok(0.0) };
    let (value, unit) = v.split();
    match unit.as_str() {
        "px" => Ok(value),
        "%" if !size.is_nan() => Ok(value * size / 100.0),
        _ => Err(NotNative::Unit(unit)),
    }
}

/// One run of a transition: Svelte's `animate` from `t1` to `t2` over `duration` ms (already scaled
/// by `|t2 - t1|`), after `delay` ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub t1: f64,
    pub t2: f64,
    pub duration: f64,
    pub delay: f64,
}

impl Run {
    /// The run Svelte starts for an intro (`t2 = 1`) or outro (`t2 = 0`) from `t1` (a reversed
    /// bidirectional transition starts where the other one was).
    pub fn new(transition: &Transition, t1: f64, t2: f64) -> Run {
        Run { t1, t2, duration: transition.duration * (t2 - t1).abs(), delay: 0.0 }
    }

    /// `t` at `elapsed` ms into the run (clamped to it).
    pub fn t_at(&self, easing: SvelteEasing, elapsed: f64) -> f64 {
        if self.duration <= 0.0 {
            return self.t2;
        }
        let p = (elapsed / self.duration).clamp(0.0, 1.0);
        self.t1 + (self.t2 - self.t1) * easing.eval(p)
    }

    /// Svelte's keyframe `t` values: `n = ceil(duration / (1000 / 60))`, `t1 + delta * easing(i / n)`.
    pub fn keyframe_ts(&self, easing: SvelteEasing) -> Vec<f64> {
        if self.duration <= 0.0 {
            return Vec::new();
        }
        let n = (self.duration / (1000.0 / 60.0)).ceil() as usize;
        let delta = self.t2 - self.t1;
        (0..=n).map(|i| self.t1 + delta * easing.eval(i as f64 / n as f64)).collect()
    }

    fn timing(&self, easing: SvelteEasing) -> Timing {
        Timing {
            delay: Span::from_secs_f64(self.delay / 1000.0),
            end_delay: Span::ZERO,
            duration: Span::from_secs_f64(self.duration / 1000.0),
            iterations: 1.0,
            iteration_start: 0.0,
            direction: PlaybackDirection::Normal,
            // Svelte plays transitions with `fill: 'forwards'`: the last frame holds until it is cancelled
            fill: FillMode::Forwards,
            easing: easing.to_presto(),
        }
    }
}

/// What a track drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Opacity,
    Transform,
    /// The `blur()` radius (px) after the node's filter.
    Blur,
    /// The slide's primary size.
    Size(Axis),
    StrokeDashArray,
    StrokeDashOffset,
    /// motion.js's clip channel: the visible size from the node's top left as a `Vec2` (`-1` = that
    /// axis unclipped).
    ClipSize,
    /// Another CSS property by name (slide's padding, margin and border widths, `min-height`).
    Css(&'static str),
}

impl Channel {
    /// The presto property; `css_id` maps a CSS property name to svelters-style's property id.
    pub fn property(self, css_id: impl Fn(&str) -> Option<u32>) -> Option<AnimProperty> {
        Some(match self {
            Channel::Opacity => AnimProperty::Opacity,
            Channel::Transform => AnimProperty::Transform,
            Channel::Blur => AnimProperty::Filter,
            Channel::Size(Axis::Y) => AnimProperty::Height,
            Channel::Size(Axis::X) => AnimProperty::Width,
            Channel::StrokeDashOffset => AnimProperty::StrokeDashOffset,
            Channel::ClipSize => AnimProperty::Clip,
            Channel::StrokeDashArray => AnimProperty::Css(css_id("stroke-dasharray")?),
            Channel::Css(name) => AnimProperty::Css(css_id(name)?),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub channel: Channel,
    pub keyframes: Vec<Keyframe>,
}

/// A planned transition run.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub tracks: Vec<Track>,
    /// Values held for the run's length without animating (`stroke-dasharray`, `min-height: 0`).
    pub statics: Vec<(Channel, AnimValue)>,
    /// slide: clip to the animated size (`overflow: hidden`).
    pub overflow_hidden: bool,
    /// The transform origin as fractions of the node's size; `None` = the node's own
    /// (`transform-origin`, the centre by default).
    pub origin: Option<(f64, f64)>,
    /// The curve (also [`Plan::timing`]'s easing, as `Easing::Svelte`).
    pub easing: SvelteEasing,
    pub timing: Timing,
}

impl Plan {
    /// Each track as a presto generator.
    pub fn generators(&self) -> impl Iterator<Item = (Channel, presto_base::anim::Generator)> + '_ {
        self.tracks.iter().map(|t| (t.channel, presto_base::anim::Generator::Keyframes { keyframes: t.keyframes.clone(), timing: self.timing.clone() }))
    }

    /// A track's value at `progress` (0..=1 of the run), with the exact easing: the keyframe pair
    /// around the eased progress, interpolated linearly (extrapolated outside 0..=1, as Web
    /// Animations does for an overshooting easing). Numbers and lengths only (the test oracle).
    pub fn sample_number(&self, track: &Track, progress: f64) -> Option<f64> {
        let p = self.easing.eval(progress.clamp(0.0, 1.0));
        let k = &track.keyframes;
        let i = k.iter().rposition(|f| f.offset as f64 <= p).unwrap_or(0).min(k.len().saturating_sub(2));
        let (a, b) = (&k[i], &k[i + 1]);
        let num = |v: &AnimValue| match v {
            AnimValue::Number(n) | AnimValue::Length(n) => Some(*n as f64),
            _ => None,
        };
        let (x, y) = (num(&a.value)?, num(&b.value)?);
        let span = b.offset as f64 - a.offset as f64;
        let q = if span == 0.0 { 0.0 } else { (p - a.offset as f64) / span };
        Some(x + (y - x) * q)
    }
}
