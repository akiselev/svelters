//! Recording canvas on `GtkDrawingArea` (X8, G8 draw func).
//!
//! JS records Canvas2D-like drawing calls into a display list and sets it as one value, the `ops`
//! property: a JSON array of `[name, ...args]` arrays (`[["fillStyle","#f00"],["fillRect",0,0,10,10]]`).
//! The draw func replays the list with cairo. No JS runs while GTK draws; the widget's `resize` signal
//! tells JS the size so the app can record a list for it.
//!
//! Ops: state `save restore translate rotate scale transform setTransform globalAlpha fillStyle strokeStyle
//! lineWidth lineCap lineJoin setLineDash font textAlign textBaseline`; path `beginPath moveTo lineTo
//! bezierCurveTo quadraticCurveTo arc rect closePath`; paint `fill stroke clip fillRect strokeRect
//! clearRect fillText strokeText`. Colors are CSS colors; unknown ops and bad arguments are skipped.

use gtk::cairo;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value as J;
use std::cell::RefCell;
use std::rc::Rc;

const KEY: &str = "svelters-canvas";

/// The display list of one drawing area.
#[derive(Default)]
pub struct Canvas {
    json: RefCell<String>,
    ops: RefCell<Vec<J>>,
}

fn canvas_of(obj: &glib::Object) -> Option<Rc<Canvas>> {
    unsafe { obj.data::<Rc<Canvas>>(KEY).map(|p| p.as_ref().clone()) }
}

/// Installs the replaying draw func on a new drawing area.
pub fn install(da: &gtk::DrawingArea) {
    let c = Rc::new(Canvas::default());
    unsafe { da.set_data(KEY, c.clone()) };
    da.set_draw_func(move |_, cr, w, h| replay(cr, &c.ops.borrow(), f64::from(w), f64::from(h)));
}

/// `ops` setter. `None` or an empty string clears the canvas.
pub fn set_ops(obj: &glib::Object, text: Option<&str>) -> Result<(), String> {
    let da = obj.downcast_ref::<gtk::DrawingArea>().ok_or("ops: not a drawingarea")?;
    let c = canvas_of(obj).ok_or("ops: the drawing area has no canvas")?;
    let text = text.unwrap_or("");
    let ops: Vec<J> = if text.is_empty() {
        Vec::new()
    } else {
        match serde_json::from_str::<J>(text) {
            Ok(J::Array(a)) => a,
            Ok(_) => return Err("ops: expected a JSON array of [name, ...args] arrays".into()),
            Err(e) => return Err(format!("ops: invalid JSON: {e}")),
        }
    };
    *c.ops.borrow_mut() = ops;
    *c.json.borrow_mut() = text.to_string();
    da.queue_draw();
    Ok(())
}

pub fn get_ops(obj: &glib::Object) -> Option<String> {
    canvas_of(obj).map(|c| c.json.borrow().clone()).filter(|s| !s.is_empty())
}

#[derive(Clone)]
struct St {
    fill: gtk::gdk::RGBA,
    stroke: gtk::gdk::RGBA,
    alpha: f64,
    family: String,
    size: f64,
    bold: bool,
    italic: bool,
    align: String,
    baseline: String,
}

impl Default for St {
    fn default() -> Self {
        let black = gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
        St { fill: black, stroke: black, alpha: 1.0, family: "sans-serif".into(), size: 10.0, bold: false, italic: false, align: "left".into(), baseline: "alphabetic".into() }
    }
}

fn num(a: &[J], i: usize) -> Option<f64> {
    a.get(i)?.as_f64()
}

fn nums<const N: usize>(a: &[J]) -> Option<[f64; N]> {
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = num(a, i)?;
    }
    Some(out)
}

fn color(a: &[J]) -> Option<gtk::gdk::RGBA> {
    gtk::gdk::RGBA::parse(a.first()?.as_str()?).ok()
}

/// CSS font shorthand: `[italic] [bold|100..900] <size>px <family>`.
fn parse_font(spec: &str, st: &mut St) {
    let mut family = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut size = None;
    for tok in spec.split_whitespace() {
        if size.is_none() {
            if let Some(px) = tok.strip_suffix("px").and_then(|n| n.parse::<f64>().ok()) {
                size = Some(px);
            } else if let Some(pt) = tok.strip_suffix("pt").and_then(|n| n.parse::<f64>().ok()) {
                size = Some(pt * 4.0 / 3.0);
            } else {
                match tok {
                    "bold" | "bolder" => bold = true,
                    "italic" | "oblique" => italic = true,
                    t => {
                        if let Ok(w) = t.parse::<u32>() {
                            bold = w >= 600;
                        }
                    }
                }
            }
        } else {
            family.push(tok.trim_matches(|c| c == '"' || c == '\'' || c == ','));
        }
    }
    if let Some(s) = size {
        st.size = s;
        st.bold = bold;
        st.italic = italic;
        if !family.is_empty() {
            st.family = family.join(" ");
        }
    }
}

fn apply_font(cr: &cairo::Context, st: &St) {
    let slant = if st.italic { cairo::FontSlant::Italic } else { cairo::FontSlant::Normal };
    let weight = if st.bold { cairo::FontWeight::Bold } else { cairo::FontWeight::Normal };
    cr.select_font_face(&st.family, slant, weight);
    cr.set_font_size(st.size);
}

fn set_source(cr: &cairo::Context, c: &gtk::gdk::RGBA, alpha: f64) {
    cr.set_source_rgba(f64::from(c.red()), f64::from(c.green()), f64::from(c.blue()), f64::from(c.alpha()) * alpha);
}

fn text_origin(cr: &cairo::Context, st: &St, text: &str, x: f64, y: f64) -> (f64, f64) {
    apply_font(cr, st);
    let Ok(ext) = cr.text_extents(text) else { return (x, y) };
    let fe = cr.font_extents().ok();
    let dx = match st.align.as_str() {
        "center" => -ext.width() / 2.0,
        "right" | "end" => -ext.width(),
        _ => 0.0,
    };
    let dy = match (st.baseline.as_str(), fe) {
        ("top", Some(f)) | ("hanging", Some(f)) => f.ascent(),
        ("middle", Some(f)) => (f.ascent() - f.descent()) / 2.0,
        ("bottom", Some(f)) | ("ideographic", Some(f)) => -f.descent(),
        _ => 0.0,
    };
    (x + dx, y + dy)
}

/// Replays a display list onto `cr`. Public so tests can render a list without a widget.
pub fn replay(cr: &cairo::Context, ops: &[J], _w: f64, _h: f64) {
    let mut st = St::default();
    let mut stack: Vec<St> = Vec::new();
    cr.set_line_width(1.0);
    cr.set_antialias(cairo::Antialias::Default);
    for op in ops {
        let Some(arr) = op.as_array() else { continue };
        let Some(name) = arr.first().and_then(J::as_str) else { continue };
        let a = &arr[1..];
        match name {
            "save" => {
                cr.save().ok();
                stack.push(st.clone());
            }
            "restore" => {
                if let Some(s) = stack.pop() {
                    cr.restore().ok();
                    st = s;
                }
            }
            "translate" => {
                if let Some([x, y]) = nums::<2>(a) {
                    cr.translate(x, y);
                }
            }
            "rotate" => {
                if let Some([r]) = nums::<1>(a) {
                    cr.rotate(r);
                }
            }
            "scale" => {
                if let Some([x, y]) = nums::<2>(a) {
                    cr.scale(x, y);
                }
            }
            "transform" => {
                if let Some([m0, m1, m2, m3, m4, m5]) = nums::<6>(a) {
                    cr.transform(cairo::Matrix::new(m0, m1, m2, m3, m4, m5));
                }
            }
            "setTransform" => {
                if let Some([m0, m1, m2, m3, m4, m5]) = nums::<6>(a) {
                    cr.set_matrix(cairo::Matrix::new(m0, m1, m2, m3, m4, m5));
                }
            }
            "globalAlpha" => {
                if let Some([x]) = nums::<1>(a) {
                    st.alpha = x.clamp(0.0, 1.0);
                }
            }
            "fillStyle" => {
                if let Some(c) = color(a) {
                    st.fill = c;
                }
            }
            "strokeStyle" => {
                if let Some(c) = color(a) {
                    st.stroke = c;
                }
            }
            "lineWidth" => {
                if let Some([x]) = nums::<1>(a) {
                    if x > 0.0 {
                        cr.set_line_width(x);
                    }
                }
            }
            "lineCap" => match a.first().and_then(J::as_str) {
                Some("round") => cr.set_line_cap(cairo::LineCap::Round),
                Some("square") => cr.set_line_cap(cairo::LineCap::Square),
                Some("butt") => cr.set_line_cap(cairo::LineCap::Butt),
                _ => {}
            },
            "lineJoin" => match a.first().and_then(J::as_str) {
                Some("round") => cr.set_line_join(cairo::LineJoin::Round),
                Some("bevel") => cr.set_line_join(cairo::LineJoin::Bevel),
                Some("miter") => cr.set_line_join(cairo::LineJoin::Miter),
                _ => {}
            },
            "setLineDash" => {
                let d: Vec<f64> = a.first().and_then(J::as_array).map(|v| v.iter().filter_map(J::as_f64).collect()).unwrap_or_default();
                cr.set_dash(&d, 0.0);
            }
            "font" => {
                if let Some(s) = a.first().and_then(J::as_str) {
                    parse_font(s, &mut st);
                }
            }
            "textAlign" => {
                if let Some(s) = a.first().and_then(J::as_str) {
                    st.align = s.to_string();
                }
            }
            "textBaseline" => {
                if let Some(s) = a.first().and_then(J::as_str) {
                    st.baseline = s.to_string();
                }
            }
            "beginPath" => cr.new_path(),
            "moveTo" => {
                if let Some([x, y]) = nums::<2>(a) {
                    cr.move_to(x, y);
                }
            }
            "lineTo" => {
                if let Some([x, y]) = nums::<2>(a) {
                    if cr.has_current_point().unwrap_or(false) {
                        cr.line_to(x, y);
                    } else {
                        cr.move_to(x, y);
                    }
                }
            }
            "bezierCurveTo" => {
                if let Some([c1x, c1y, c2x, c2y, x, y]) = nums::<6>(a) {
                    if !cr.has_current_point().unwrap_or(false) {
                        cr.move_to(c1x, c1y);
                    }
                    cr.curve_to(c1x, c1y, c2x, c2y, x, y);
                }
            }
            "quadraticCurveTo" => {
                if let Some([cx, cy, x, y]) = nums::<4>(a) {
                    let (px, py) = cr.current_point().unwrap_or((cx, cy));
                    cr.curve_to(px + 2.0 / 3.0 * (cx - px), py + 2.0 / 3.0 * (cy - py), x + 2.0 / 3.0 * (cx - x), y + 2.0 / 3.0 * (cy - y), x, y);
                }
            }
            "arc" => {
                if let Some([x, y, r, a0, a1]) = nums::<5>(a) {
                    if a.get(5).and_then(J::as_bool).unwrap_or(false) {
                        cr.arc_negative(x, y, r, a0, a1);
                    } else {
                        cr.arc(x, y, r, a0, a1);
                    }
                }
            }
            "rect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    cr.rectangle(x, y, w, h);
                }
            }
            "closePath" => cr.close_path(),
            "fill" => {
                set_source(cr, &st.fill, st.alpha);
                cr.fill_preserve().ok();
            }
            "stroke" => {
                set_source(cr, &st.stroke, st.alpha);
                cr.stroke_preserve().ok();
            }
            "clip" => {
                cr.clip_preserve();
            }
            "fillRect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    set_source(cr, &st.fill, st.alpha);
                    cr.rectangle(x, y, w, h);
                    cr.fill().ok();
                }
            }
            "strokeRect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    set_source(cr, &st.stroke, st.alpha);
                    cr.rectangle(x, y, w, h);
                    cr.stroke().ok();
                }
            }
            "clearRect" => {
                if let Some([x, y, w, h]) = nums::<4>(a) {
                    cr.save().ok();
                    cr.set_operator(cairo::Operator::Clear);
                    cr.rectangle(x, y, w, h);
                    cr.fill().ok();
                    cr.restore().ok();
                }
            }
            "fillText" | "strokeText" => {
                if let (Some(t), Some(x), Some(y)) = (a.first().and_then(J::as_str), num(a, 1), num(a, 2)) {
                    let (tx, ty) = text_origin(cr, &st, t, x, y);
                    // The current path belongs to the app; text must not disturb it.
                    let saved = cr.copy_path().ok();
                    cr.new_path();
                    cr.move_to(tx, ty);
                    if name == "fillText" {
                        set_source(cr, &st.fill, st.alpha);
                        cr.show_text(t).ok();
                    } else {
                        set_source(cr, &st.stroke, st.alpha);
                        cr.text_path(t);
                        cr.stroke().ok();
                    }
                    cr.new_path();
                    if let Some(p) = saved {
                        cr.append_path(&p);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Renders a display list into an ARGB image (tests, screenshots).
pub fn render_image(ops_json: &str, w: i32, h: i32) -> Result<cairo::ImageSurface, String> {
    let ops: Vec<J> = serde_json::from_str(ops_json).map_err(|e| e.to_string())?;
    let surf = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).map_err(|e| e.to_string())?;
    {
        let cr = cairo::Context::new(&surf).map_err(|e| e.to_string())?;
        replay(&cr, &ops, f64::from(w), f64::from(h));
    }
    surf.flush();
    Ok(surf)
}
