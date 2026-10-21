//! egui-jsdraw probe: what a JS-authored egui widget costs per frame.
//!
//! Reproduce (from the repo root):
//!   CARGO_TARGET_DIR=<dir> cargo run --release \
//!     --manifest-path docs/notes/scripts/egui-jsdraw-probe/Cargo.toml
//! Optional: `-- quick` for fewer samples.
//!
//! All JS runs in rquickjs 0.14.0 (QuickJS-NG 0.16.2+), the host's engine, with an interrupt handler
//! installed as the host does. Each row is the median of several samples; one sample runs the frame
//! closure enough times to take >= 20 ms.

use egui::epaint::{CircleShape, PathShape, RectShape};
use egui::{Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
#[cfg(feature = "jit")]
use rquickjs_jit as rquickjs;
#[cfg(not(feature = "jit"))]
use rquickjs_stock as rquickjs;

use rquickjs::{ArrayBuffer, Context, Ctx, Function, Object, Runtime};
use std::cell::RefCell;
use std::hint::black_box;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------
// Decoded draw commands (what every JS path produces, and what the Rust baseline produces too).

#[derive(Clone, Copy, Debug)]
enum Cmd {
    Rect { x: f32, y: f32, w: f32, h: f32, rgba: u32 },
    Line { start: u32, count: u32, width: f32, rgba: u32 },
    Circle { cx: f32, cy: f32, r: f32, rgba: u32 },
}

#[derive(Default)]
struct Sink {
    cmds: Vec<Cmd>,
    pts: Vec<Pos2>,
}

impl Sink {
    fn clear(&mut self) {
        self.cmds.clear();
        self.pts.clear();
    }
    fn to_shapes(&self, out: &mut Vec<Shape>) {
        for c in &self.cmds {
            out.push(match *c {
                Cmd::Rect { x, y, w, h, rgba } => Shape::Rect(RectShape::filled(
                    Rect::from_min_size(pos2(x, y), vec2(w, h)),
                    CornerRadius::ZERO,
                    color(rgba),
                )),
                Cmd::Line { start, count, width, rgba } => Shape::Path(PathShape::line(
                    self.pts[start as usize..(start + count) as usize].to_vec(),
                    Stroke::new(width, color(rgba)),
                )),
                Cmd::Circle { cx, cy, r, rgba } => Shape::Circle(CircleShape::filled(pos2(cx, cy), r, color(rgba))),
            });
        }
    }
}

fn color(rgba: u32) -> Color32 {
    let [r, g, b, a] = rgba.to_be_bytes();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Decodes the bench.js command buffer: u32 opcode words, f32/u32 args, little-endian.
fn decode(bytes: &[u8], words: usize, sink: &mut Sink) {
    let w = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
    let f = |i: usize| f32::from_bits(w(i));
    let mut k = 0;
    while k < words {
        match w(k) {
            1 => {
                sink.cmds.push(Cmd::Rect { x: f(k + 1), y: f(k + 2), w: f(k + 3), h: f(k + 4), rgba: w(k + 5) });
                k += 6;
            }
            2 => {
                let count = w(k + 1);
                let start = sink.pts.len() as u32;
                sink.cmds.push(Cmd::Line { start, count, width: f(k + 2), rgba: w(k + 3) });
                k += 4;
                for _ in 0..count {
                    sink.pts.push(pos2(f(k), f(k + 1)));
                    k += 2;
                }
            }
            3 => {
                sink.cmds.push(Cmd::Circle { cx: f(k + 1), cy: f(k + 2), r: f(k + 3), rgba: w(k + 4) });
                k += 5;
            }
            op => panic!("bad opcode {op} at {k}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Harness

struct Row {
    group: &'static str,
    name: String,
    n: usize,
    ns_frame: f64,
}

struct Bench<'a> {
    rows: Vec<Row>,
    samples: usize,
    only: Option<String>,
    /// Called between iterations (quickjs-jit installs compiled code from `Jit::poll`).
    poll: Option<&'a dyn Fn()>,
}

/// Thread CPU time: the host is shared with other builds, so wall time would count the time this
/// thread spends descheduled. (Cache and SMT contention still show up.)
fn cpu_ns() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 * 1e9 + ts.tv_nsec as f64
}

impl Bench<'_> {
    fn run(&mut self, group: &'static str, name: impl Into<String>, n: usize, mut f: impl FnMut()) {
        let name = name.into();
        if let Some(o) = &self.only {
            if !format!("{group} {name}").contains(o.as_str()) {
                return;
            }
        }
        // warm up and calibrate: iterations per sample so that one sample takes >= 20 ms CPU
        let t = cpu_ns();
        let mut iters = 0u64;
        while cpu_ns() - t < 30e6 {
            f();
            if let Some(p) = self.poll {
                p();
            }
            iters += 1;
        }
        let per = (cpu_ns() - t) / iters as f64;
        let iters = ((20e6 / per).ceil() as u64).max(1);
        let mut s: Vec<f64> = (0..self.samples)
            .map(|_| {
                if let Some(p) = self.poll {
                    p();
                }
                let t = cpu_ns();
                for _ in 0..iters {
                    f();
                }
                (cpu_ns() - t) / iters as f64
            })
            .collect();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = s[s.len() / 2];
        eprintln!("  {group:8} {name:40} n={n:<6} {:>10.2} us/frame (min {:.2})", med / 1e3, s[0] / 1e3);
        self.rows.push(Row { group, name, n, ns_frame: med });
    }

    fn print(&self, ghz: f64) {
        println!("| group | case | N (ops or widgets) | µs / frame | ns / op | cycles / op | ops in 16 ms |");
        println!("|---|---|---:|---:|---:|---:|---:|");
        for r in &self.rows {
            let per_op = r.ns_frame / r.n as f64;
            let fit = 16e6 / per_op;
            println!(
                "| {} | {} | {} | {:.1} | {:.1} | {:.0} | {} |",
                r.group,
                r.name,
                r.n,
                r.ns_frame / 1e3,
                per_op,
                per_op * ghz,
                fmt_count(fit)
            );
        }
    }
}

fn fmt_count(x: f64) -> String {
    if x >= 1e6 {
        format!("{:.1}M", x / 1e6)
    } else if x >= 1e3 {
        format!("{:.0}k", x / 1e3)
    } else {
        format!("{x:.0}")
    }
}

// ---------------------------------------------------------------------------------------------
// Rust baselines (the "AOT compiled paint function" equivalent of bench.js)

const COLOR: u32 = 0xff8040ff;

fn rust_frame(n: usize, sink: &mut Sink) {
    for i in 0..n {
        sink.cmds.push(Cmd::Rect {
            x: ((i % 100) * 10) as f32,
            y: ((i / 100) * 10) as f32,
            w: 8.0,
            h: 8.0,
            rgba: COLOR,
        });
    }
}

fn rust_spark(data: &[f64], x0: f32, y0: f32, w: f32, h: f32, sink: &mut Sink) {
    let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in data {
        mn = mn.min(v);
        mx = mx.max(v);
    }
    let n = data.len();
    let sx = w as f64 / (n - 1) as f64;
    let sy = h as f64 / if mx - mn == 0.0 { 1.0 } else { mx - mn };
    let start = sink.pts.len() as u32;
    for (i, &v) in data.iter().enumerate() {
        sink.pts.push(pos2((x0 as f64 + i as f64 * sx) as f32, (y0 as f64 + h as f64 - (v - mn) * sy) as f32));
    }
    sink.cmds.push(Cmd::Line { start, count: n as u32, width: 1.5, rgba: 0x4080ffff });
}

fn rust_knob(cx: f32, cy: f32, r: f32, value: f32, sink: &mut Sink) {
    use std::f32::consts::PI;
    let (a0, sweep, segs) = (PI * 0.75, PI * 1.5, 48u32);
    let arc = |count: u32, span: f32, rgba: u32, sink: &mut Sink| {
        let start = sink.pts.len() as u32;
        for i in 0..=count {
            let a = a0 + span * i as f32 / count as f32;
            sink.pts.push(pos2(cx + a.cos() * r, cy + a.sin() * r));
        }
        sink.cmds.push(Cmd::Line { start, count: count + 1, width: 3.0, rgba });
    };
    arc(segs, sweep, 0x404040ff, sink);
    let vs = ((segs as f32 * value).round() as u32).max(1);
    arc(vs, sweep * value, 0x40a0ffff, sink);
    let a = a0 + sweep * value;
    sink.cmds.push(Cmd::Circle { cx: cx + a.cos() * r * 0.7, cy: cy + a.sin() * r * 0.7, r: r * 0.12, rgba: 0xffffffff });
}

// ---------------------------------------------------------------------------------------------
// Retained paint IR with late-bound slots: what JS would produce once, evaluated by Rust per frame.

#[derive(Clone, Copy)]
enum Node {
    Const(f32),
    Slot(u8),
    Add(u32, u32),
    MulAdd(u32, u32, u32), // a * b + c
}

struct RectOp {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    c0: Color32,
    c1: Color32,
    mix: u32,
}

struct Program {
    nodes: Vec<Node>,
    rects: Vec<RectOp>,
}

// Slots the host binds every frame without running JS.
const S_RECT_X: u8 = 0;
const S_RECT_Y: u8 = 1;
const S_HOVER: u8 = 2; // animated 0..1 (egui animate_bool)

/// The same scene as `rust_frame`, but positions are relative to the widget rect and each rect's
/// height and colour follow the hover animation: 4 nodes per rect plus 4 shared ones.
fn build_program(n: usize) -> Program {
    let mut nodes = vec![Node::Slot(S_RECT_X), Node::Slot(S_RECT_Y), Node::Slot(S_HOVER), Node::Const(8.0), Node::Const(4.0)];
    let (rx, ry, hover, eight, four) = (0, 1, 2, 3, 4);
    nodes.push(Node::MulAdd(hover, four, eight));
    let h = 5;
    let mut rects = Vec::with_capacity(n);
    for i in 0..n {
        let b = nodes.len() as u32;
        nodes.push(Node::Const(((i % 100) * 10) as f32));
        nodes.push(Node::Const(((i / 100) * 10) as f32));
        nodes.push(Node::Add(rx, b));
        nodes.push(Node::Add(ry, b + 1));
        rects.push(RectOp {
            x: b + 2,
            y: b + 3,
            w: eight,
            h,
            c0: color(COLOR),
            c1: Color32::WHITE,
            mix: hover,
        });
    }
    Program { nodes, rects }
}

fn eval_program(p: &Program, slots: &[f32], regs: &mut Vec<f32>, out: &mut Vec<Shape>) {
    regs.clear();
    for n in &p.nodes {
        let v = match *n {
            Node::Const(c) => c,
            Node::Slot(s) => slots[s as usize],
            Node::Add(a, b) => regs[a as usize] + regs[b as usize],
            Node::MulAdd(a, b, c) => regs[a as usize] * regs[b as usize] + regs[c as usize],
        };
        regs.push(v);
    }
    for r in &p.rects {
        let rect = Rect::from_min_size(pos2(regs[r.x as usize], regs[r.y as usize]), vec2(regs[r.w as usize], regs[r.h as usize]));
        out.push(Shape::Rect(RectShape::filled(rect, CornerRadius::ZERO, r.c0.lerp_to_gamma(r.c1, regs[r.mix as usize]))));
    }
}

/// The hand-written Rust equivalent of `eval_program`.
fn rust_slots(n: usize, slots: &[f32], out: &mut Vec<Shape>) {
    let h = slots[2] * 4.0 + 8.0;
    let c = color(COLOR).lerp_to_gamma(Color32::WHITE, slots[2]);
    for i in 0..n {
        let rect = Rect::from_min_size(
            pos2(slots[0] + ((i % 100) * 10) as f32, slots[1] + ((i / 100) * 10) as f32),
            vec2(8.0, h),
        );
        out.push(Shape::Rect(RectShape::filled(rect, CornerRadius::ZERO, c)));
    }
}

// ---------------------------------------------------------------------------------------------
// egui frame (headless: run_ui + tessellate, no GPU)

fn egui_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
        ..Default::default()
    }
}

fn egui_frame(ctx: &egui::Context, mut paint: impl FnMut(&egui::Painter)) -> usize {
    let out = ctx.run_ui(egui_input(), |ui| paint(ui.painter()));
    let ppp = out.pixels_per_point;
    let prims = ctx.tessellate(out.shapes, ppp);
    prims.len()
}

// ---------------------------------------------------------------------------------------------

fn install<'js>(ctx: &Ctx<'js>, sink: &Rc<RefCell<Sink>>) -> rquickjs::Result<()> {
    let g = ctx.globals();
    let s = sink.clone();
    let rect = Function::new(ctx.clone(), move |x: f64, y: f64, w: f64, h: f64, rgba: u32| {
        s.borrow_mut().cmds.push(Cmd::Rect { x: x as f32, y: y as f32, w: w as f32, h: h as f32, rgba });
    })?;
    g.set("rect", rect.clone())?;
    let painter = Object::new(ctx.clone())?;
    painter.set("rect", rect)?;
    g.set("painter", painter)?;

    let s = sink.clone();
    g.set(
        "flush",
        Function::new(ctx.clone(), move |ab: ArrayBuffer<'js>, words: u32| {
            // SAFETY: the buffer is not detached or resized while we read it (single-threaded, no JS runs).
            let bytes = unsafe { ab.as_bytes() }.expect("detached");
            decode(bytes, words as usize, &mut s.borrow_mut());
        })?,
    )?;

    let s = sink.clone();
    g.set(
        "flush_json",
        Function::new(ctx.clone(), move |json: String| {
            let ops: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
            let mut s = s.borrow_mut();
            for op in ops {
                let a = op.as_array().unwrap();
                let n = |i: usize| a[i].as_f64().unwrap() as f32;
                let hex = a[5].as_str().unwrap();
                let rgba = u32::from_str_radix(&hex[1..], 16).unwrap();
                debug_assert_eq!(a[0].as_str(), Some("fillRect"));
                s.cmds.push(Cmd::Rect { x: n(1), y: n(2), w: n(3), h: n(4), rgba });
            }
        })?,
    )?;

    let s = sink.clone();
    g.set(
        "flush_array",
        Function::new(ctx.clone(), move |a: Vec<f64>| {
            let mut s = s.borrow_mut();
            for c in a.chunks_exact(6) {
                s.cmds.push(Cmd::Rect { x: c[1] as f32, y: c[2] as f32, w: c[3] as f32, h: c[4] as f32, rgba: c[5] as u32 });
            }
        })?,
    )?;
    Ok(())
}

fn main() {
    let quick = std::env::args().any(|a| a == "quick");
    let only = std::env::args().find_map(|a| a.strip_prefix("only=").map(str::to_owned));
    // Engine first: the JIT guard must outlive the bench harness that polls it.
    let rt = Runtime::new().unwrap();
    // The host installs an interrupt handler (turn budget); PROBE_NO_INTERRUPT=1 measures without it.
    if std::env::var_os("PROBE_NO_INTERRUPT").is_none() {
        rt.set_interrupt_handler(Some(Box::new(|| false)));
    }
    #[cfg(feature = "jit")]
    let jit = quickjs_jit_runtime::Jit::attach(&rt, quickjs_jit_runtime::JitConfig::default()).expect("attach JIT");
    let poll: Option<&dyn Fn()> = None;
    let mut b = Bench { rows: Vec::new(), samples: if quick { 5 } else { 11 }, only, poll };
    let sink = Rc::new(RefCell::new(Sink::default()));
    let sizes = [100usize, 1_000, 10_000];

    // ---- Calibration: a serial add+rotate chain is 2 cycles/iteration on current x86/arm cores,
    // so this row gives the effective clock of this (shared) machine during the run.
    fn calibrate(b: &mut Bench<'_>, label: &'static str) -> f64 {
        let only = b.only.take();
        b.run("calib", label, 1_000_000, || {
            let mut s = 1u64;
            for i in 0..1_000_000u64 {
                s = s.wrapping_add(i).rotate_left(1);
            }
            black_box(s);
        });
        b.only = only;
        b.rows.last().unwrap().ns_frame / 1e6
    }
    let calib_start = calibrate(&mut b, "2-cycle dependent chain x1e6 (start)");

    // ---- Rust baselines -----------------------------------------------------------------------
    for &n in &sizes {
        b.run("rust", "push Cmd (AOT equivalent)", n, || {
            let mut s = sink.borrow_mut();
            s.clear();
            rust_frame(n, &mut s);
            black_box(s.cmds.len());
        });
    }
    let slots = [100.0f32, 50.0, 0.35];
    let mut shapes: Vec<Shape> = Vec::new();
    for &n in &sizes {
        b.run("rust", "push egui Shape (slot scene)", n, || {
            shapes.clear();
            rust_slots(n, &slots, &mut shapes);
            black_box(shapes.len());
        });
    }

    // ---- Retained IR --------------------------------------------------------------------------
    let mut regs = Vec::new();
    for &n in &sizes {
        let prog = build_program(n);
        b.run("ir", "eval slot IR -> egui Shape", n, || {
            shapes.clear();
            eval_program(&prog, &slots, &mut regs, &mut shapes);
            black_box(shapes.len());
        });
    }
    for &n in &sizes {
        let mut cached = Vec::new();
        rust_slots(n, &slots, &mut cached);
        b.run("ir", "replay cached Vec<Shape> (clone)", n, || {
            black_box(cached.clone());
        });
    }

    // ---- JS -----------------------------------------------------------------------------------
    let jsctx = Context::full(&rt).unwrap();
    let ectx = egui::Context::default();
    jsctx.with(|ctx| {
        install(&ctx, &sink).unwrap();
        ctx.eval::<(), _>(include_str!("bench.js")).unwrap();
    });
    // The JS section runs twice: a short warm-up pass whose rows are dropped, then the measured pass.
    // quickjs-jit compiles on worker threads and installs code from `Jit::poll`, which must run
    // outside `Context::with`; polling between the passes measures warmed-up, installed code.
    let js_section = |b: &mut Bench<'_>| jsctx.with(|ctx| {
        let g = ctx.globals();
        let f = |name: &str| -> Function { g.get(name).unwrap() };

        let cases: [(&str, &str); 8] = [
            ("frame_empty", "empty counted loop (s += i)"),
            ("frame_nocall", "loop+math only, no draw (floor)"),
            ("frame_call", "1 host fn call per op"),
            ("frame_method", "1 host method call per op"),
            ("frame_buf_inline", "inline typed-array writes + 1 flush"),
            ("frame_buf_api", "painter.rect() JS method -> buffer"),
            ("frame_array", "plain Array -> Vec<f64>"),
            ("frame_json", "JSON string (X8 today)"),
        ];
        for (fname, label) in cases {
            let func = f(fname);
            for &n in &sizes {
                b.run("js", label, n, || {
                    sink.borrow_mut().clear();
                    let _: rquickjs::Value = func.call((n as u32,)).unwrap();
                    black_box(sink.borrow().cmds.len());
                });
            }
        }
        // sanity: every path produced the same scene
        for fname in ["frame_call", "frame_buf_inline", "frame_buf_api", "frame_array", "frame_json"] {
            sink.borrow_mut().clear();
            let _: rquickjs::Value = f(fname).call((1000u32,)).unwrap();
            let s = sink.borrow();
            assert_eq!(s.cmds.len(), 1000, "{fname}");
            match s.cmds[999] {
                Cmd::Rect { x, y, rgba, .. } => assert!(x == 990.0 && y == 90.0 && rgba == COLOR, "{fname}: {:?}", s.cmds[999]),
                _ => panic!(),
            }
        }

        // ---- Rust -> JS invocation cost (one call per widget per frame) ----------------------
        let noop = f("noop");
        b.run("call", "Rust->JS noop() call", 1000, || {
            for _ in 0..1000 {
                let _: () = noop.call(()).unwrap();
            }
        });
        let small = f("paint_small");
        let flush_p: Function = ctx.eval("() => P.flush()").unwrap();
        b.run("call", "Rust->JS paint(x,y,w,h,t), 3 rects", 1000, || {
            sink.borrow_mut().clear();
            for i in 0..1000 {
                let _: () = small.call(((i % 40) as f64 * 30.0, (i / 40) as f64 * 20.0, 28.0, 18.0, 0.5)).unwrap();
            }
            let _: () = flush_p.call(()).unwrap();
            black_box(sink.borrow().cmds.len());
        });
        let small_obj = f("paint_small_obj");
        b.run("call", "Rust->JS paint({x,y,w,h,t}) object arg", 1000, || {
            sink.borrow_mut().clear();
            for i in 0..1000 {
                let o = Object::new(ctx.clone()).unwrap();
                o.set("x", (i % 40) as f64 * 30.0).unwrap();
                o.set("y", (i / 40) as f64 * 20.0).unwrap();
                o.set("w", 28.0).unwrap();
                o.set("h", 18.0).unwrap();
                o.set("t", 0.5).unwrap();
                let _: () = small_obj.call((o,)).unwrap();
            }
            let _: () = flush_p.call(()).unwrap();
            black_box(sink.borrow().cmds.len());
        });
        b.run("call", "Rust paint (3 rects) equivalent", 1000, || {
            let mut s = sink.borrow_mut();
            s.clear();
            for i in 0..1000 {
                let (x, y, w, h, t) = ((i % 40) as f32 * 30.0, (i / 40) as f32 * 20.0, 28.0f32, 18.0f32, 0.5f32);
                s.cmds.push(Cmd::Rect { x, y, w, h, rgba: 0x202020ff });
                s.cmds.push(Cmd::Rect { x: x + 1.0, y: y + 1.0, w: (w - 2.0) * t, h: h - 2.0, rgba: 0x40a0ffff });
                s.cmds.push(Cmd::Rect { x: x + w - 4.0, y, w: 4.0, h, rgba: 0xffffffff });
            }
            black_box(s.cmds.len());
        });

        // ---- Realistic widgets ---------------------------------------------------------------
        let spark = f("frame_spark");
        let data: Vec<f64> = (0..100_000).map(|i| (i as f64 * 0.05).sin() * 10.0 + (i as f64 * 0.31).sin() * 3.0).collect();
        for n in [200usize, 2_000] {
            b.run("widget", "sparkline JS (points)", n, || {
                sink.borrow_mut().clear();
                let _: () = spark.call((n as u32, 10.0, 10.0, 300.0, 40.0)).unwrap();
                black_box(sink.borrow().pts.len());
            });
            b.run("widget", "sparkline Rust (points)", n, || {
                let mut s = sink.borrow_mut();
                s.clear();
                rust_spark(&data[..n], 10.0, 10.0, 300.0, 40.0, &mut s);
                black_box(s.pts.len());
            });
        }
        let knob = f("knob");
        let knob_flush = f("knob_flush");
        for m in [1usize, 100] {
            b.run("widget", "knob JS, Rust calls knob() per widget", m, || {
                sink.borrow_mut().clear();
                for i in 0..m {
                    let _: () = knob.call(((i % 10) as f64 * 60.0 + 30.0, (i / 10) as f64 * 60.0 + 30.0, 24.0, 0.6)).unwrap();
                }
                let _: () = knob_flush.call(()).unwrap();
                black_box(sink.borrow().pts.len());
            });
            b.run("widget", "knob Rust", m, || {
                let mut s = sink.borrow_mut();
                s.clear();
                for i in 0..m {
                    rust_knob((i % 10) as f32 * 60.0 + 30.0, (i / 10) as f32 * 60.0 + 30.0, 24.0, 0.6, &mut s);
                }
                black_box(s.pts.len());
            });
        }
        // sanity: JS and Rust knobs agree
        {
            sink.borrow_mut().clear();
            let _: () = knob.call((30.0, 30.0, 24.0, 0.6)).unwrap();
            let _: () = knob_flush.call(()).unwrap();
            let js_pts = sink.borrow().pts.clone();
            let mut s = sink.borrow_mut();
            s.clear();
            rust_knob(30.0, 30.0, 24.0, 0.6, &mut s);
            assert_eq!(js_pts.len(), s.pts.len());
            for (a, b) in js_pts.iter().zip(&s.pts) {
                assert!((a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3);
            }
        }

        // ---- Whole egui frame (run_ui + tessellate) ------------------------------------------
        let _ = egui_frame(&ectx, |_| {}); // fonts, first pass
        b.run("egui", "empty frame", 1, || {
            black_box(egui_frame(&ectx, |_| {}));
        });
        let api = f("frame_buf_api");
        for &n in &[1_000usize, 10_000] {
            b.run("egui", "Rust rect_filled", n, || {
                black_box(egui_frame(&ectx, |p| {
                    let mut v = Vec::with_capacity(n);
                    rust_slots(n, &slots, &mut v);
                    p.extend(v);
                }));
            });
            let prog = build_program(n);
            let mut regs = Vec::new();
            b.run("egui", "slot IR eval -> painter", n, || {
                black_box(egui_frame(&ectx, |p| {
                    let mut v = Vec::with_capacity(n);
                    eval_program(&prog, &slots, &mut regs, &mut v);
                    p.extend(v);
                }));
            });
            let mut cached = Vec::new();
            rust_slots(n, &slots, &mut cached);
            b.run("egui", "cached shapes -> painter", n, || {
                black_box(egui_frame(&ectx, |p| p.extend(cached.iter().cloned())));
            });
            b.run("egui", "JS painter.rect() each frame -> painter", n, || {
                black_box(egui_frame(&ectx, |p| {
                    sink.borrow_mut().clear();
                    let _: () = api.call((n as u32,)).unwrap();
                    let mut v = Vec::with_capacity(n);
                    sink.borrow().to_shapes(&mut v);
                    p.extend(v);
                }));
            });
        }
        // stroked rounded rects cost more to tessellate than filled square ones; one data point
        b.run("egui", "Rust rounded+stroked rects", 1_000, || {
            black_box(egui_frame(&ectx, |p| {
                for i in 0..1000 {
                    let r = Rect::from_min_size(pos2(((i % 100) * 10) as f32, ((i / 100) * 10) as f32), vec2(8.0, 8.0));
                    p.add(RectShape::new(r, 3.0, color(COLOR), Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside));
                }
            }));
        });
    });
    {
        let (samples, rows) = (b.samples, b.rows.len());
        b.samples = 1;
        eprintln!("-- warm-up pass (dropped)");
        js_section(&mut b);
        b.rows.truncate(rows);
        b.samples = samples;
    }
    #[cfg(feature = "jit")]
    for _ in 0..100 {
        jit.poll();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    eprintln!("-- measured pass");
    js_section(&mut b);

    let calib_end = calibrate(&mut b, "2-cycle dependent chain x1e6 (end)");
    let calib = Some((calib_start + calib_end) / 2.0);
    println!();
    #[cfg(feature = "jit")]
    {
        let m = jit.metrics();
        println!(
            "engine: quickjs-jit 0.14.1 (automatic tiering): installed {} functions, {} native entries ({} tier 2), {} code bytes, {} unsupported-opcode rejections",
            m.installed, m.native_entries, m.tier2_entries, m.code_bytes, m.unsupported_opcode_failures
        );
    }
    #[cfg(not(feature = "jit"))]
    println!(
        "engine: rquickjs 0.14.0 (QuickJS-NG 0.16.2+), C asserts {}",
        if cfg!(feature = "ndebug") { "off (disable-assertions)" } else { "on (host default)" }
    );
    println!("egui 0.37.0, release profile, interrupt handler {}.", if std::env::var_os("PROBE_NO_INTERRUPT").is_none() { "on" } else { "off" });
    if let Some(c) = calib {
        println!(
            "calibration: {calib_start:.3} / {calib_end:.3} ns per 2-cycle iteration (start / end) => effective clock {:.2} GHz; cycles/op uses it",
            2.0 / c
        );
    }
    b.print(calib.map(|c| 2.0 / c).unwrap_or(f64::NAN));
}
