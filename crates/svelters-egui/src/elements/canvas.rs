//! The `canvas` kind's display list (X8, plans/milestones/X8-events-draw.md §3) replayed with
//! egui's painter, so the `@svelters/canvas` recorder draws the same list on egui as on GTK (cairo)
//! and Qt (QPainter).
//!
//! The list is the `ops` prop: a JSON array of `[name, ...args]` arrays. Ops (the GTK set):
//! state `save restore translate rotate scale transform setTransform globalAlpha fillStyle
//! strokeStyle lineWidth lineCap lineJoin setLineDash font textAlign textBaseline`; path
//! `beginPath moveTo lineTo bezierCurveTo quadraticCurveTo arc rect closePath`; paint `fill stroke
//! clip fillRect strokeRect clearRect fillText strokeText`. Colors are CSS colors (hex, `rgb()`,
//! `rgba()`, `hsl()`, `hsla()`, names); unknown ops and bad arguments are skipped.
//!
//! egui differences, documented on the kind: curves are flattened to polylines; `fill` fills each
//! subpath as a simple polygon (convex ones anti-aliased, others ear-clipped; no holes, no fill
//! rule); `clip` cuts to the bounding box of the path; `clearRect` paints `clear` (the panel
//! background); `lineCap`/`lineJoin` are not drawn (egui strokes have neither); a dash uses the
//! first two entries of the dash array; text is not rotated (only translated and scaled), and
//! `strokeText` fills.

use egui::{Color32, Pos2, Rect, Shape, Stroke, pos2};
use serde_json::Value as J;
use std::sync::Arc;

/// A `canvas` node's state (the payload of `W::Canvas`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    /// The `ops` prop as written (empty: none).
    pub ops: String,
    /// `ops` parsed.
    pub list: Arc<Vec<J>>,
    pub content_w: f64,
    pub content_h: f64,
    /// The size last posted with `resize`.
    pub reported: Option<(f32, f32)>,
}

impl Canvas {
    /// Sets `ops`; an empty string clears the canvas. Errors on JSON that is not an array.
    pub fn set_ops(&mut self, text: String) -> Result<(), String> {
        let list = if text.is_empty() {
            Vec::new()
        } else {
            match serde_json::from_str::<J>(&text) {
                Ok(J::Array(a)) => a,
                Ok(_) => return Err("ops: expected a JSON array of [name, ...args] arrays".into()),
                Err(e) => return Err(format!("ops: invalid JSON: {e}")),
            }
        };
        self.list = Arc::new(list);
        self.ops = text;
        Ok(())
    }
}

/// A 2D affine transform `[a, b, c, d, e, f]` (Canvas2D order: `x' = a x + c y + e`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tf([f64; 6]);

impl Tf {
    const ID: Tf = Tf([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fn then(self, m: [f64; 6]) -> Tf {
        // self * m (m applied first, as Canvas2D's `transform`)
        let [a, b, c, d, e, f] = self.0;
        let [ma, mb, mc, md, me, mf] = m;
        Tf([a * ma + c * mb, b * ma + d * mb, a * mc + c * md, b * mc + d * md, a * me + c * mf + e, b * me + d * mf + f])
    }
    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
    /// The length scale (for line widths and font sizes).
    fn scale(self) -> f64 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }
}

#[derive(Clone)]
struct St {
    tf: Tf,
    fill: Color32,
    stroke: Color32,
    alpha: f32,
    width: f64,
    dash: Vec<f32>,
    size: f64,
    mono: bool,
    align: String,
    baseline: String,
    clip: Rect,
}

#[derive(Default)]
struct Sub {
    pts: Vec<Pos2>,
    closed: bool,
}

/// What a replay produces: shapes, each with the clip rect it is drawn in (screen coordinates).
pub type Drawn = Vec<(Rect, Shape)>;

fn num(a: &[J], i: usize) -> Option<f64> {
    a.get(i)?.as_f64().filter(|v| v.is_finite())
}

fn nums<const N: usize>(a: &[J]) -> Option<[f64; N]> {
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = num(a, i)?;
    }
    Some(out)
}

fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    c.gamma_multiply(alpha.clamp(0.0, 1.0))
}

/// Replays `ops` for a canvas whose top-left corner is `origin` (screen), cut at `clip`, with
/// `ctx`'s fonts for text. `clear` is what `clearRect` paints.
pub fn shapes(ctx: &egui::Context, origin: Pos2, clip: Rect, ops: &[J], clear: Color32) -> Drawn {
    let mut st = St {
        tf: Tf([1.0, 0.0, 0.0, 1.0, origin.x as f64, origin.y as f64]),
        fill: Color32::BLACK,
        stroke: Color32::BLACK,
        alpha: 1.0,
        width: 1.0,
        dash: Vec::new(),
        size: 10.0,
        mono: false,
        align: "left".into(),
        baseline: "alphabetic".into(),
        clip,
    };
    let base = st.tf;
    let mut stack: Vec<St> = Vec::new();
    let mut path: Vec<Sub> = Vec::new();
    // the current point and the subpath's start, in user space
    let mut cur: Option<(f64, f64)> = None;
    let mut start: Option<(f64, f64)> = None;
    let mut out: Drawn = Vec::new();
    let pt = |st: &St, x: f64, y: f64| {
        let (sx, sy) = st.tf.apply(x, y);
        pos2(sx as f32, sy as f32)
    };
    for op in ops {
        let Some(arr) = op.as_array() else { continue };
        let Some(name) = arr.first().and_then(J::as_str) else { continue };
        let a = &arr[1..];
        match name {
            "save" => stack.push(st.clone()),
            "restore" => {
                if let Some(s) = stack.pop() {
                    st = s;
                }
            }
            "translate" => {
                if let Some([x, y]) = nums::<2>(a) {
                    st.tf = st.tf.then([1.0, 0.0, 0.0, 1.0, x, y]);
                }
            }
            "rotate" => {
                if let Some([r]) = nums::<1>(a) {
                    let (s, c) = r.sin_cos();
                    st.tf = st.tf.then([c, s, -s, c, 0.0, 0.0]);
                }
            }
            "scale" => {
                if let Some([x, y]) = nums::<2>(a) {
                    st.tf = st.tf.then([x, 0.0, 0.0, y, 0.0, 0.0]);
                }
            }
            "transform" => {
                if let Some(m) = nums::<6>(a) {
                    st.tf = st.tf.then(m);
                }
            }
            "setTransform" => {
                if let Some(m) = nums::<6>(a) {
                    st.tf = base.then(m);
                }
            }
            "globalAlpha" => {
                if let Some([x]) = nums::<1>(a) {
                    st.alpha = x.clamp(0.0, 1.0) as f32;
                }
            }
            "fillStyle" | "strokeStyle" => {
                if let Some(c) = a.first().and_then(J::as_str).and_then(css_color) {
                    if name == "fillStyle" {
                        st.fill = c;
                    } else {
                        st.stroke = c;
                    }
                }
            }
            "lineWidth" => {
                if let Some([x]) = nums::<1>(a)
                    && x > 0.0
                {
                    st.width = x;
                }
            }
            "setLineDash" => {
                st.dash = a.first().and_then(J::as_array).map(|v| v.iter().filter_map(J::as_f64).map(|x| x as f32).collect()).unwrap_or_default();
            }
            "font" => {
                if let Some(s) = a.first().and_then(J::as_str) {
                    parse_font(s, &mut st);
                }
            }
            "textAlign" | "textBaseline" => {
                if let Some(s) = a.first().and_then(J::as_str) {
                    if name == "textAlign" {
                        st.align = s.to_string();
                    } else {
                        st.baseline = s.to_string();
                    }
                }
            }
            "beginPath" => {
                path.clear();
                cur = None;
                start = None;
            }
            "moveTo" => {
                if let Some([x, y]) = nums::<2>(a) {
                    path.push(Sub { pts: vec![pt(&st, x, y)], closed: false });
                    cur = Some((x, y));
                    start = cur;
                }
            }
            "lineTo" => {
                if let Some([x, y]) = nums::<2>(a) {
                    line_to(&mut path, &mut cur, &mut start, pt(&st, x, y), (x, y));
                }
            }
            "bezierCurveTo" | "quadraticCurveTo" => {
                let ctrl: Option<Vec<(f64, f64)>> = if name == "bezierCurveTo" {
                    nums::<6>(a).map(|[c1x, c1y, c2x, c2y, x, y]| vec![(c1x, c1y), (c2x, c2y), (x, y)])
                } else {
                    nums::<4>(a).map(|[cx, cy, x, y]| vec![(cx, cy), (x, y)])
                };
                let Some(ctrl) = ctrl else { continue };
                let p0 = match cur {
                    Some(p) => p,
                    None => {
                        let p = ctrl[0];
                        line_to(&mut path, &mut cur, &mut start, pt(&st, p.0, p.1), p);
                        p
                    }
                };
                let n = 24;
                for k in 1..=n {
                    let t = k as f64 / n as f64;
                    let u = 1.0 - t;
                    let (x, y) = if ctrl.len() == 3 {
                        let (c1, c2, e) = (ctrl[0], ctrl[1], ctrl[2]);
                        (
                            u * u * u * p0.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * e.0,
                            u * u * u * p0.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * e.1,
                        )
                    } else {
                        let (c, e) = (ctrl[0], ctrl[1]);
                        (u * u * p0.0 + 2.0 * u * t * c.0 + t * t * e.0, u * u * p0.1 + 2.0 * u * t * c.1 + t * t * e.1)
                    };
                    line_to(&mut path, &mut cur, &mut start, pt(&st, x, y), (x, y));
                }
            }
            "arc" => {
                if let Some([x, y, r, a0, a1]) = nums::<5>(a) {
                    if r < 0.0 {
                        continue;
                    }
                    let ccw = a.get(5).and_then(J::as_bool).unwrap_or(false);
                    let tau = std::f64::consts::TAU;
                    let sweep = if !ccw {
                        if a1 - a0 >= tau { tau } else { (a1 - a0).rem_euclid(tau) }
                    } else if a0 - a1 >= tau {
                        -tau
                    } else {
                        -(a0 - a1).rem_euclid(tau)
                    };
                    // about 4 screen points per segment, at least 8 per full turn
                    let len = sweep.abs() * r * st.tf.scale();
                    let n = ((len / 4.0).ceil() as usize).clamp((8.0 * sweep.abs() / tau).ceil().max(1.0) as usize, 512);
                    for k in 0..=n {
                        let t = a0 + sweep * k as f64 / n as f64;
                        let p = (x + r * t.cos(), y + r * t.sin());
                        line_to(&mut path, &mut cur, &mut start, pt(&st, p.0, p.1), p);
                    }
                }
            }
            "rect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    let pts = vec![pt(&st, x, y), pt(&st, x + w, y), pt(&st, x + w, y + h), pt(&st, x, y + h)];
                    path.push(Sub { pts, closed: true });
                    // a new subpath at the rect's origin
                    path.push(Sub { pts: vec![pt(&st, x, y)], closed: false });
                    cur = Some((x, y));
                    start = cur;
                }
            }
            "closePath" => {
                if let Some(s) = path.last_mut()
                    && s.pts.len() > 1
                {
                    s.closed = true;
                    if let Some(p0) = start {
                        let first = s.pts[0];
                        path.push(Sub { pts: vec![first], closed: false });
                        cur = Some(p0);
                    }
                }
            }
            "fill" => {
                let color = with_alpha(st.fill, st.alpha);
                for s in &path {
                    fill_polygon(&mut out, st.clip, &s.pts, color);
                }
            }
            "stroke" => {
                let stroke = Stroke::new((st.width * st.tf.scale()) as f32, with_alpha(st.stroke, st.alpha));
                for s in &path {
                    stroke_line(&mut out, &st, &s.pts, s.closed, stroke);
                }
            }
            "clip" => {
                let r = path.iter().flat_map(|s| s.pts.iter()).fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)));
                st.clip = st.clip.intersect(r);
            }
            "fillRect" | "strokeRect" | "clearRect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    let pts = [pt(&st, x, y), pt(&st, x + w, y), pt(&st, x + w, y + h), pt(&st, x, y + h)];
                    match name {
                        "fillRect" => fill_polygon(&mut out, st.clip, &pts, with_alpha(st.fill, st.alpha)),
                        "clearRect" => fill_polygon(&mut out, st.clip, &pts, clear),
                        _ => {
                            let stroke = Stroke::new((st.width * st.tf.scale()) as f32, with_alpha(st.stroke, st.alpha));
                            stroke_line(&mut out, &st, &pts, true, stroke);
                        }
                    }
                }
            }
            "fillText" | "strokeText" => {
                if let (Some(t), Some(x), Some(y)) = (a.first().and_then(J::as_str), num(a, 1), num(a, 2)) {
                    let size = (st.size * st.tf.scale()) as f32;
                    if size <= 0.0 {
                        continue;
                    }
                    let font = if st.mono { egui::FontId::monospace(size) } else { egui::FontId::proportional(size) };
                    let color = with_alpha(if name == "fillText" { st.fill } else { st.stroke }, st.alpha);
                    let galley = ctx.fonts_mut(|f| f.layout_no_wrap(t.to_string(), font, color));
                    let gs = galley.size();
                    let p = pt(&st, x, y);
                    let dx = match st.align.as_str() {
                        "center" => -gs.x / 2.0,
                        "right" | "end" => -gs.x,
                        _ => 0.0,
                    };
                    // the galley's top-left from the anchor (alphabetic: about 80% of the row is ascent)
                    let dy = match st.baseline.as_str() {
                        "top" | "hanging" => 0.0,
                        "middle" => -gs.y / 2.0,
                        "bottom" | "ideographic" => -gs.y,
                        _ => -gs.y * 0.8,
                    };
                    out.push((st.clip, Shape::galley(p + egui::vec2(dx, dy), galley, color)));
                }
            }
            // lineCap, lineJoin: egui strokes have neither
            _ => {}
        }
    }
    out
}

fn line_to(path: &mut Vec<Sub>, cur: &mut Option<(f64, f64)>, start: &mut Option<(f64, f64)>, p: Pos2, user: (f64, f64)) {
    match path.last_mut() {
        Some(s) if cur.is_some() && !s.closed => s.pts.push(p),
        _ => {
            path.push(Sub { pts: vec![p], closed: false });
            *start = Some(user);
        }
    }
    *cur = Some(user);
}

fn stroke_line(out: &mut Drawn, st: &St, pts: &[Pos2], closed: bool, stroke: Stroke) {
    if pts.len() < 2 || stroke.width <= 0.0 {
        return;
    }
    let dash: Vec<f32> = st.dash.iter().map(|d| d * st.tf.scale() as f32).collect();
    if dash.len() >= 2 && dash.iter().any(|d| *d > 0.0) {
        let mut line = pts.to_vec();
        if closed {
            line.push(pts[0]);
        }
        for s in Shape::dashed_line(&line, stroke, dash[0].max(0.1), dash[1].max(0.0)) {
            out.push((st.clip, s));
        }
    } else if dash.len() == 1 && dash[0] > 0.0 {
        let mut line = pts.to_vec();
        if closed {
            line.push(pts[0]);
        }
        for s in Shape::dashed_line(&line, stroke, dash[0], dash[0]) {
            out.push((st.clip, s));
        }
    } else if closed {
        out.push((st.clip, Shape::closed_line(pts.to_vec(), stroke)));
    } else {
        out.push((st.clip, Shape::line(pts.to_vec(), stroke)));
    }
}

/// Twice the signed area (positive: counter-clockwise in y-up, clockwise on screen).
fn area2(p: &[Pos2]) -> f32 {
    let n = p.len();
    (0..n).map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y).sum()
}

fn cross(o: Pos2, a: Pos2, b: Pos2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn is_convex(p: &[Pos2]) -> bool {
    let n = p.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let c = cross(p[i], p[(i + 1) % n], p[(i + 2) % n]);
        if c.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// Fills a simple polygon: convex ones as an anti-aliased `PathShape`, others ear-clipped into a
/// mesh.
fn fill_polygon(out: &mut Drawn, clip: Rect, pts: &[Pos2], color: Color32) {
    // drop repeated points (a closing point equal to the first, flattened curves)
    let mut p: Vec<Pos2> = Vec::with_capacity(pts.len());
    for &q in pts {
        if p.last().is_none_or(|l: &Pos2| l.distance_sq(q) > 1e-8) {
            p.push(q);
        }
    }
    while p.len() > 1 && p[0].distance_sq(*p.last().unwrap()) <= 1e-8 {
        p.pop();
    }
    if p.len() < 3 || color == Color32::TRANSPARENT {
        return;
    }
    if is_convex(&p) {
        out.push((clip, Shape::convex_polygon(p, color, Stroke::NONE)));
        return;
    }
    let mut mesh = egui::Mesh::default();
    for &q in &p {
        mesh.colored_vertex(q, color);
    }
    for [a, b, c] in ear_clip(&p) {
        mesh.add_triangle(a, b, c);
    }
    out.push((clip, Shape::mesh(mesh)));
}

/// Triangulates a simple polygon by ear clipping (O(n²)); a polygon it cannot finish (self
/// intersecting) gets the rest as a fan.
fn ear_clip(p: &[Pos2]) -> Vec<[u32; 3]> {
    let ccw = area2(p) > 0.0;
    let mut idx: Vec<usize> = (0..p.len()).collect();
    let mut tris = Vec::with_capacity(p.len().saturating_sub(2));
    let convex = |a: Pos2, b: Pos2, c: Pos2| {
        let c = cross(a, b, c);
        if ccw { c > 0.0 } else { c < 0.0 }
    };
    let inside = |q: Pos2, a: Pos2, b: Pos2, c: Pos2| {
        let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < p.len() * p.len() {
        guard += 1;
        let n = idx.len();
        let mut found = false;
        for k in 0..n {
            let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            if !convex(a, b, c) {
                continue;
            }
            if idx.iter().any(|&j| j != ia && j != ib && j != ic && inside(p[j], a, b, c)) {
                continue;
            }
            tris.push([ia as u32, ib as u32, ic as u32]);
            idx.remove(k);
            found = true;
            break;
        }
        if !found {
            break;
        }
    }
    for k in 1..idx.len().saturating_sub(1) {
        tris.push([idx[0] as u32, idx[k] as u32, idx[k + 1] as u32]);
    }
    tris
}

/// CSS font shorthand: `[italic] [bold|100..900] <size>px|pt <family>`; `monospace` picks egui's
/// monospace family, anything else the proportional one.
fn parse_font(spec: &str, st: &mut St) {
    let mut size = None;
    let mut family = Vec::new();
    for tok in spec.split_whitespace() {
        if size.is_none() {
            if let Some(px) = tok.strip_suffix("px").and_then(|n| n.parse::<f64>().ok()) {
                size = Some(px);
            } else if let Some(pt) = tok.strip_suffix("pt").and_then(|n| n.parse::<f64>().ok()) {
                size = Some(pt * 4.0 / 3.0);
            }
        } else {
            family.push(tok.trim_matches(|c| c == '"' || c == '\'' || c == ','));
        }
    }
    if let Some(s) = size {
        st.size = s;
        st.mono = family.iter().any(|f| f.eq_ignore_ascii_case("monospace") || f.to_ascii_lowercase().contains("mono"));
    }
}

/// A CSS color: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` (numbers or percentages,
/// commas or spaces, `/ alpha`), `hsl()`/`hsla()`, `transparent` and the CSS named colors that
/// toolkits commonly use.
pub fn css_color(s: &str) -> Option<Color32> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v: Vec<u8> = h.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;
        let (r, g, b, a) = match v.len() {
            3 => (v[0] * 17, v[1] * 17, v[2] * 17, 255),
            4 => (v[0] * 17, v[1] * 17, v[2] * 17, v[3] * 17),
            6 => (v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], 255),
            8 => (v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], v[6] * 16 + v[7]),
            _ => return None,
        };
        return Some(Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    if let Some((f, rest)) = s.split_once('(') {
        let args: Vec<&str> = rest.strip_suffix(')')?.split([',', ' ', '/']).map(str::trim).filter(|x| !x.is_empty()).collect();
        let alpha = |x: Option<&&str>| -> Option<u8> {
            match x {
                None => Some(255),
                Some(a) => {
                    let v = match a.strip_suffix('%') {
                        Some(p) => p.parse::<f32>().ok()? / 100.0,
                        None => a.parse::<f32>().ok()?,
                    };
                    Some((v.clamp(0.0, 1.0) * 255.0).round() as u8)
                }
            }
        };
        return match f.trim() {
            "rgb" | "rgba" if args.len() >= 3 => {
                let ch = |x: &str| -> Option<u8> {
                    let v = match x.strip_suffix('%') {
                        Some(p) => p.parse::<f32>().ok()? * 2.55,
                        None => x.parse::<f32>().ok()?,
                    };
                    Some(v.clamp(0.0, 255.0).round() as u8)
                };
                Some(Color32::from_rgba_unmultiplied(ch(args[0])?, ch(args[1])?, ch(args[2])?, alpha(args.get(3))?))
            }
            "hsl" | "hsla" if args.len() >= 3 => {
                let h = args[0].trim_end_matches("deg").parse::<f32>().ok()?.rem_euclid(360.0) / 360.0;
                let sat = args[1].strip_suffix('%')?.parse::<f32>().ok()? / 100.0;
                let l = args[2].strip_suffix('%')?.parse::<f32>().ok()? / 100.0;
                let (r, g, b) = hsl_to_rgb(h, sat.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                Some(Color32::from_rgba_unmultiplied((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8, alpha(args.get(3))?))
            }
            _ => None,
        };
    }
    let rgb = match s.as_str() {
        "transparent" => return Some(Color32::TRANSPARENT),
        "black" => 0x000000,
        "white" => 0xffffff,
        "red" => 0xff0000,
        "green" => 0x008000,
        "lime" => 0x00ff00,
        "blue" => 0x0000ff,
        "yellow" => 0xffff00,
        "cyan" | "aqua" => 0x00ffff,
        "magenta" | "fuchsia" => 0xff00ff,
        "gray" | "grey" => 0x808080,
        "darkgray" | "darkgrey" => 0xa9a9a9,
        "lightgray" | "lightgrey" => 0xd3d3d3,
        "silver" => 0xc0c0c0,
        "maroon" => 0x800000,
        "olive" => 0x808000,
        "navy" => 0x000080,
        "purple" => 0x800080,
        "teal" => 0x008080,
        "orange" => 0xffa500,
        "pink" => 0xffc0cb,
        "brown" => 0xa52a2a,
        "gold" => 0xffd700,
        "steelblue" => 0x4682b4,
        "cornflowerblue" => 0x6495ed,
        "tomato" => 0xff6347,
        "crimson" => 0xdc143c,
        "indigo" => 0x4b0082,
        "violet" => 0xee82ee,
        "darkred" => 0x8b0000,
        "darkgreen" => 0x006400,
        "darkblue" => 0x00008b,
        "skyblue" => 0x87ceeb,
        "coral" => 0xff7f50,
        "salmon" => 0xfa8072,
        "khaki" => 0xf0e68c,
        "beige" => 0xf5f5dc,
        "ivory" => 0xfffff0,
        "slategray" | "slategrey" => 0x708090,
        "dimgray" | "dimgrey" => 0x696969,
        _ => return None,
    };
    Some(Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_colors_parse() {
        assert_eq!(css_color("#f00"), Some(Color32::RED));
        assert_eq!(css_color("#00ff0080"), Some(Color32::from_rgba_unmultiplied(0, 255, 0, 128)));
        assert_eq!(css_color("rgb(0, 0, 255)"), Some(Color32::from_rgb(0, 0, 255)));
        assert_eq!(css_color("rgba(255,255,255,0.5)"), Some(Color32::from_rgba_unmultiplied(255, 255, 255, 128)));
        assert_eq!(css_color("rgb(0 128 0 / 50%)"), Some(Color32::from_rgba_unmultiplied(0, 128, 0, 128)));
        assert_eq!(css_color("hsl(0, 100%, 50%)"), Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(css_color("SteelBlue"), Some(Color32::from_rgb(0x46, 0x82, 0xb4)));
        assert_eq!(css_color("nonsense"), None);
    }

    #[test]
    fn ear_clipping_covers_a_concave_polygon() {
        // an L shape: 6 vertices, 4 triangles, total area 3
        let p = [pos2(0.0, 0.0), pos2(2.0, 0.0), pos2(2.0, 1.0), pos2(1.0, 1.0), pos2(1.0, 2.0), pos2(0.0, 2.0)];
        assert!(!is_convex(&p));
        let tris = ear_clip(&p);
        assert_eq!(tris.len(), 4);
        let area: f32 = tris.iter().map(|t| cross(p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]).abs() / 2.0).sum();
        assert!((area - 3.0).abs() < 1e-4, "area {area}");
    }

    #[test]
    fn transforms_compose_like_canvas2d() {
        // translate then scale: a point (1, 1) lands at 10 + 2, 20 + 3
        let t = Tf::ID.then([1.0, 0.0, 0.0, 1.0, 10.0, 20.0]).then([2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
        assert_eq!(t.apply(1.0, 1.0), (12.0, 23.0));
    }
}
