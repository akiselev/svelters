//! WG3s: the scroll pins (9, 10) on svelters-egui's `scroll-area` (OW2's `Scrollable`,
//! `interact/scroll.rs`) under kittest, as egui-winit delivers macOS input, against the AppKit
//! references (`conformance/golden/macos/nsscrollview-*.json`): a report, not gated
//! (`target/na/macos/egui-scroll-gap.md` and the CI job summary).
//!
//! The fixture is §1's: `s` (320 × 240) over a 960 × 2400 box whose column holds a 40 pt spacer,
//! `t` (280 × 120 over a 280 × 600 box) and the rest. Offsets are the areas' `scroll-x`/`scroll-y`.
//! Input [model of egui-winit on macOS, not measured on winit]: a wheel notch is
//! `MouseWheel { unit: Line, source: Wheel }`; a trackpad gesture is `unit: Point,
//! source: Trackpad` with phases Start, Move…, End; momentum (`native`, `replay:`) is the recorded
//! macOS stream (`conformance/scroll/macos-flick.json`) as `Move` events after `End`, which is how
//! the OS momentum reaches egui (`Kinetic::Os`: svelters adds none of its own). egui's delta sign:
//! positive moves the content down (towards its start), the OS's sign, so recorded deltas go in as
//! they are. One kittest step is 1/60 s; offsets are sampled every step.

use egui_kittest::Harness;
use std::fmt::Write as _;
use svelters_abi::*;
use svelters_conformance::golden::{Capture, compare, finish, goldens};
use svelters_conformance::pins::{Chord, Keys, Momentum, Pin, conformance_dir, load_scroll_pins};
use svelters_conformance::scroll::{Sample, ScrollRecord};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::{Driver, run_pin_scroll};
use svelters_egui::EguiBackend;
use svelters_egui::native::Native;

struct Rec;
impl HostCallbacks for Rec {
    fn post_event(&self, _: svl_handle, _: svl_id, _: &[Value]) {}
    fn checkpoint(&self) {}
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, SyncError> {
        Err(SyncError::Failed("no answer".into()))
    }
    fn on_destroyed(&self, _: svl_handle, _: u32) {}
    fn frame(&self) {}
    fn prop_changed(&self, _: svl_handle, _: svl_id) {}
}

const STEP: f32 = 1.0 / 60.0;
const STEP_MS: f64 = 1000.0 / 60.0;
const NOT_HERE: &str = "the scroll fixture has no text fields";

struct EguiScroll {
    backend: EguiBackend,
    vt: &'static BackendVtable,
    harness: Harness<'static, EguiBackend>,
    trace: Trace,
    win: u64,
    s: u64,
    t: u64,
    /// Steps since the action's first event.
    step: u64,
}

impl EguiScroll {
    fn new(native: Native) -> EguiScroll {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec)));
        vt.backend_ref().start(hv).expect("start");
        let harness = Harness::builder()
            .with_size(egui::vec2(400.0, 320.0))
            .with_os(egui::os::OperatingSystem::Mac)
            .with_step_dt(STEP)
            .build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        svelters_egui::native::set(&harness.ctx, native);
        EguiScroll {
            backend,
            vt,
            harness,
            trace: Trace::default(),
            win: 0,
            s: 0,
            t: 0,
            step: 0,
        }
    }

    fn br(&self) -> BackendRef<'static> {
        self.vt.backend_ref()
    }

    fn create(&mut self, tag: &str) -> u64 {
        let k = self.br().resolve(NameKind::Kind, 0, tag);
        assert_ne!(k, 0, "unknown tag {tag}");
        let h = self.br().create(k).unwrap();
        self.trace.create(tag, h);
        h
    }

    fn prop(&self, h: u64, name: &str) -> u32 {
        let kind = self.br().handle_info(h).unwrap().kind;
        let id = self.br().resolve(NameKind::Prop, kind, name);
        assert_ne!(id, 0, "no prop {name}");
        id
    }

    fn set(&mut self, h: u64, name: &str, v: f64) {
        let id = self.prop(h, name);
        self.br().set_prop(h, id, &Value::F64(v)).unwrap();
        self.trace.set_raw(h, name, &v.to_string());
    }

    fn get(&self, h: u64, name: &str) -> f64 {
        match self.br().get_prop(h, self.prop(h, name)).unwrap() {
            Value::F64(f) => f,
            Value::I64(i) => i as f64,
            o => panic!("{name}: {o:?}"),
        }
    }

    fn insert(&mut self, p: u64, c: u64) {
        self.br().insert(p, c, None, 0).unwrap();
        self.trace.insert(p, c);
    }

    fn sized_box(&mut self, w: f64, h: f64) -> u64 {
        let b = self.create("box");
        self.set(b, "width", w);
        self.set(b, "height", h);
        b
    }

    fn area(&self, anchor: &str) -> Result<u64, String> {
        match anchor {
            "s" => Ok(self.s),
            "t" => Ok(self.t),
            o => Err(format!("no scroll area {o}")),
        }
    }

    fn offset(&self, h: u64) -> (f64, f64) {
        (self.get(h, "scroll-x"), self.get(h, "scroll-y"))
    }

    fn rect(&self, h: u64) -> egui::Rect {
        self.backend.with_scene(|s| s.n(h).drawn)
    }

    /// `t`'s centre, or a point of `s` outside `t` (as the AppKit driver picks it).
    fn point(&self, anchor: &str) -> Result<egui::Pos2, String> {
        let r = self.rect(self.area(anchor)?);
        if anchor == "t" {
            return Ok(r.center());
        }
        let t = self.rect(self.t);
        for (fx, fy) in [(0.9, 0.1), (0.9, 0.9), (0.1, 0.1)] {
            let p = egui::pos2(r.left() + r.width() * fx, r.top() + r.height() * fy);
            if !t.contains(p) {
                return Ok(p);
            }
        }
        Err("no point of s outside t".into())
    }

    fn sample(&self, out: &mut Vec<Sample>) {
        let t_ms = self.step as f64 * STEP_MS;
        for (a, h) in [("s", self.s), ("t", self.t)] {
            let (x, y) = self.offset(h);
            out.push(Sample {
                t_ms,
                anchor: a.into(),
                x,
                y,
            });
        }
    }

    fn step(&mut self, out: &mut Vec<Sample>) {
        self.harness.step();
        self.step += 1;
        self.sample(out);
    }

    fn steps_for(&mut self, ms: f64, out: &mut Vec<Sample>) {
        let until = self.step as f64 * STEP_MS + ms;
        while (self.step as f64 * STEP_MS) < until {
            self.step(out);
        }
    }

    /// Steps until nothing moved for 300 ms (at most 5 s).
    fn settle(&mut self, out: &mut Vec<Sample>) -> Result<(), String> {
        let start = self.step;
        let mut last = (self.offset(self.s), self.offset(self.t));
        let mut still = self.step;
        loop {
            self.step(out);
            let cur = (self.offset(self.s), self.offset(self.t));
            if cur != last {
                last = cur;
                still = self.step;
            }
            if (self.step - still) as f64 * STEP_MS >= 300.0 {
                return Ok(());
            }
            if (self.step - start) as f64 * STEP_MS > 5000.0 {
                return Err("the scroll did not settle within 5 s".into());
            }
        }
    }

    fn begin(&mut self, anchor: &str) -> Result<Vec<Sample>, String> {
        let p = self.point(anchor)?;
        self.harness.event(egui::Event::PointerMoved(p));
        for _ in 0..3 {
            self.harness.step();
        }
        self.step = 0;
        let mut samples = Vec::new();
        self.sample(&mut samples);
        Ok(samples)
    }

    fn wheel_event(
        &mut self,
        unit: egui::MouseWheelUnit,
        delta: egui::Vec2,
        phase: egui::TouchPhase,
        source: egui::MouseWheelSource,
        shift: bool,
    ) {
        let modifiers = if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        };
        self.harness.event(egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            phase,
            source,
        });
    }
}

impl Driver for EguiScroll {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let win = self.create("window");
        let s = self.create("scroll-area");
        self.set(s, "width", 320.0);
        self.set(s, "height", 240.0);
        let content = self.sized_box(960.0, 2400.0);
        let spacer = self.sized_box(280.0, 40.0);
        let t = self.create("scroll-area");
        self.set(t, "width", 280.0);
        self.set(t, "height", 120.0);
        let inner = self.sized_box(280.0, 600.0);
        let rest = self.sized_box(280.0, 2240.0);
        self.insert(t, inner);
        self.insert(content, spacer);
        self.insert(content, t);
        self.insert(content, rest);
        self.insert(s, content);
        self.insert(win, s);
        let m = self.br().resolve(
            NameKind::Method,
            self.br().handle_info(win).unwrap().kind,
            "present",
        );
        self.br().call(win, m, &[]).unwrap();
        self.trace.call(win, "present", "[]");
        (self.win, self.s, self.t) = (win, s, t);
        for _ in 0..3 {
            self.harness.step();
        }
        for (a, [x, y]) in &pin.setup.scroll {
            let h = self.area(a)?;
            self.set(h, "scroll-x", *x);
            self.set(h, "scroll-y", *y);
        }
        for _ in 0..3 {
            self.harness.step();
        }
        Ok(())
    }

    fn key(&mut self, _: &Chord) -> Result<(), String> {
        Err(NOT_HERE.into())
    }
    fn click(&mut self, _: &str, _: Option<usize>, _: u32, _: u32) -> Result<(), String> {
        Err(NOT_HERE.into())
    }
    fn type_text(&mut self, _: &str) -> Result<(), String> {
        Err(NOT_HERE.into())
    }
    fn drag(&mut self, _: &str, _: usize, _: usize, _: u32) -> Result<(), String> {
        Err(NOT_HERE.into())
    }

    fn scroll_wheel(
        &mut self,
        target: &str,
        lines: [i32; 2],
        shift: bool,
        burst: u32,
        interval_ms: u32,
    ) -> Result<ScrollRecord, String> {
        let mut samples = self.begin(target)?;
        for i in 0..burst.max(1) {
            // towards the content's end is a negative delta
            let d = egui::vec2(-lines[0] as f32, -lines[1] as f32);
            self.wheel_event(
                egui::MouseWheelUnit::Line,
                d,
                egui::TouchPhase::Move,
                egui::MouseWheelSource::Wheel,
                shift,
            );
            if i + 1 < burst {
                self.steps_for(interval_ms as f64, &mut samples);
            }
        }
        let input_end_ms = self.step as f64 * STEP_MS;
        self.settle(&mut samples)?;
        Ok(ScrollRecord {
            samples,
            input_end_ms,
            max: vec![],
        })
    }

    fn scroll_gesture(
        &mut self,
        target: &str,
        deltas: &[[f64; 2]],
        interval_ms: u32,
        momentum: &Momentum,
    ) -> Result<ScrollRecord, String> {
        use egui::{MouseWheelSource::Trackpad, MouseWheelUnit::Point, TouchPhase};
        let mut samples = self.begin(target)?;
        for (i, d) in deltas.iter().enumerate() {
            let phase = if i == 0 {
                TouchPhase::Start
            } else {
                TouchPhase::Move
            };
            self.wheel_event(
                Point,
                egui::vec2(-d[0] as f32, -d[1] as f32),
                phase,
                Trackpad,
                false,
            );
            self.steps_for(interval_ms as f64, &mut samples);
        }
        self.wheel_event(Point, egui::Vec2::ZERO, TouchPhase::End, Trackpad, false);
        let input_end_ms = self.step as f64 * STEP_MS;
        let replay = match momentum {
            Momentum::None => None,
            Momentum::Native => Some("macos-flick".to_string()),
            Momentum::Replay(n) => Some(n.clone()),
        };
        if let Some(name) = replay {
            let path = conformance_dir()
                .join("scroll")
                .join(format!("{name}.json"));
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let ev: Vec<&serde_json::Value> = v["events"]
                .as_array()
                .ok_or("no events")?
                .iter()
                .filter(|e| e["momentum"].as_i64().unwrap_or(0) != 0)
                .collect();
            let t0 = ev.first().and_then(|e| e["t_ms"].as_f64()).unwrap_or(0.0);
            let start = self.step as f64 * STEP_MS;
            for e in ev {
                let at = start + e["t_ms"].as_f64().unwrap_or(0.0) - t0;
                let now = self.step as f64 * STEP_MS;
                if at > now {
                    self.steps_for(at - now, &mut samples);
                }
                let d = egui::vec2(
                    e["dx"].as_f64().unwrap_or(0.0) as f32,
                    e["dy"].as_f64().unwrap_or(0.0) as f32,
                );
                self.wheel_event(Point, d, TouchPhase::Move, Trackpad, false);
            }
        }
        self.settle(&mut samples)?;
        Ok(ScrollRecord {
            samples,
            input_end_ms,
            max: vec![],
        })
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        let mut cp = Vec::new();
        for (a, h) in [("s", self.s), ("t", self.t)] {
            let (x, y) = self.offset(h);
            let st = State::offset(x, y);
            self.trace.state(h, &st);
            cp.push((a.to_string(), st));
        }
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        self.br().destroy(self.win);
        self.trace.destroy(self.win);
        self.harness.step();
        Ok(std::mem::take(&mut self.trace).text())
    }
}

/// WG3s: egui's scroll-area on macOS input against the NSScrollView references. A report.
#[test]
fn egui_scroll_against_macos_goldens() {
    let golds: Vec<_> = goldens("macos")
        .unwrap()
        .into_iter()
        .filter(|g| g.toolkit == "nsscrollview")
        .collect();
    let keys = Keys::load("macos").unwrap();
    let pins = load_scroll_pins().unwrap();
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let mut d = EguiScroll::new(Native::Egui);
        let (r, t, _) = run_pin_scroll(&mut d, pin, &keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let cap = Capture {
        platform: "macos".into(),
        toolkit: "egui-scroll".into(),
        version: "0.37.0".into(),
        injector: "kittest".into(),
        settings: vec![],
        observe: vec![],
        pins: results.clone(),
    };
    finish(&cap, &traces).unwrap();
    if golds.is_empty() {
        eprintln!(
            "no NSScrollView golden (conformance/golden/macos/nsscrollview-*.json): nothing to compare"
        );
        return;
    }
    let mut md = String::from("| Pin | Script | Checkpoints |");
    for g in &golds {
        let _ = write!(md, " = {} {} |", g.toolkit, g.version);
    }
    md.push_str("\n|---|---|---|");
    md.push_str(&"---|".repeat(golds.len()));
    md.push('\n');
    let mut details = String::new();
    for r in &results {
        let _ = write!(md, "| {} | {} | {} |", r.pin, r.id, r.checkpoints.len());
        for g in &golds {
            let gap = g
                .pin(&r.id)
                .map(|p| compare(&r.checkpoints, &p.checkpoints))
                .unwrap_or_default();
            let _ = write!(md, " {} |", gap.equal);
            for (i, after, ours, theirs) in &gap.diffs {
                let _ = writeln!(
                    details,
                    "- {} #{i} after `{after}`:\n  - egui: `{ours}`\n  - AppKit: `{theirs}`",
                    r.id
                );
            }
        }
        md.push('\n');
    }
    let out = svelters_conformance::golden::out_dir("macos", "egui-scroll-gap.md");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, format!("{md}\n## Differing checkpoints\n\n{details}")).unwrap();
    eprintln!("{md}\nWG3s egui scroll gap report: {}", out.display());
    if let Ok(summary) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)
        {
            let _ = writeln!(
                f,
                "### egui scroll against NSScrollView (svelters-conformance egui_scroll)\n\n{md}"
            );
        }
    }
}
