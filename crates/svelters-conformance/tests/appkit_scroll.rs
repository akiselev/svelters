//! WG3s: the scroll pins (9, 10) on AppKit's `NSScrollView`
//! (`plans/milestones/WG3s-scroll-references.md`). The fixture (§1): an outer scroll view `s`
//! whose document is a 960 × 2400 `NSTextView` of rows, with a 280 × 120 scroll view `t` (a
//! 280 × 600 document) inside it at content y 40. Input goes through the session tap with the
//! pointer over the target: mouse-wheel line events, and trackpad events with their gesture phases
//! (`svelters_conformance::macos::scroll_post`). Both scroll views' offsets (their clip views'
//! bounds origin; text views are flipped, so y grows towards the content's end) are sampled every
//! 8 ms from the first input event until nothing moved for 300 ms.
//!
//! Momentum: macOS's trackpad momentum comes from the OS input stack, not from AppKit, and a
//! synthetic gesture gets none. `native` and `replay:<name>` both post the recorded momentum
//! stream of a real flick (`conformance/scroll/macos-flick.json`, made with
//! `examples/scroll_record.rs`) after the gesture's *ended*, with its own timing.
//!
//! Opt-in (`NA_MACOS=1`): it moves the pointer, needs Accessibility, and must not share the screen
//! with a user. Writes `target/na/macos/nsscrollview-<macOS>.json`, the per-pin traces and samples;
//! compares with `conformance/golden/macos/` within the scroll tolerances; `NA_UPDATE=1` rewrites.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(true, &[("appkit_scroll_references", mac::capture)]);
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::time::Instant;
    use svelters_conformance::golden::{Capture, checkpoint_json, finish, out_dir};
    use svelters_conformance::macos::{self, CGPoint, Input, scroll_post};
    use svelters_conformance::pins::{
        Chord, Keys, Momentum, Pin, conformance_dir, load_scroll_pins,
    };
    use svelters_conformance::scroll::{Sample, ScrollRecord};
    use svelters_conformance::trace::{Checkpoint, State, Trace};
    use svelters_conformance::{Driver, run_pin_scroll};

    type Obj = Retained<AnyObject>;

    const NOT_HERE: &str = "the scroll fixture has no text fields";
    /// Sampling period and the settle rule (§2: no change for 300 ms, at most 5 s).
    const SAMPLE_MS: u64 = 8;
    const SETTLE_MS: f64 = 300.0;
    const SETTLE_TIMEOUT_MS: f64 = 5000.0;

    fn ns(s: &str) -> Retained<NSString> {
        NSString::from_str(s)
    }

    fn app() -> Obj {
        unsafe { msg_send![class!(NSApplication), sharedApplication] }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    fn defaults_read(domain: &str, key: &str) -> String {
        std::process::Command::new("defaults")
            .args(["read", domain, key])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unset".into())
    }

    /// A text view of `rows` lines (`Row 1` …) with a fixed frame: the document of a scroll view.
    fn document(w: f64, h: f64, rows: usize) -> Obj {
        unsafe {
            let al: Allocated<AnyObject> = msg_send![class!(NSTextView), alloc];
            let tv: Obj = msg_send![al, initWithFrame: rect(0.0, 0.0, w, h)];
            let text: String = (1..=rows).map(|i| format!("Row {i}\n")).collect();
            let _: () = msg_send![&*tv, setString: &*ns(&text)];
            let _: () = msg_send![&*tv, setEditable: false];
            let _: () = msg_send![&*tv, setVerticallyResizable: false];
            let _: () = msg_send![&*tv, setHorizontallyResizable: false];
            let _: () = msg_send![&*tv, setMinSize: NSSize::new(w, h)];
            let _: () = msg_send![&*tv, setMaxSize: NSSize::new(w, h)];
            let _: () = msg_send![&*tv, setFrame: rect(0.0, 0.0, w, h)];
            tv
        }
    }

    fn scroll_view(frame: NSRect, doc: &AnyObject) -> Obj {
        unsafe {
            let al: Allocated<AnyObject> = msg_send![class!(NSScrollView), alloc];
            let sv: Obj = msg_send![al, initWithFrame: frame];
            let _: () = msg_send![&*sv, setHasVerticalScroller: true];
            let _: () = msg_send![&*sv, setHasHorizontalScroller: true];
            let _: () = msg_send![&*sv, setDocumentView: doc];
            sv
        }
    }

    /// The clip view's bounds origin: the offset from the content's start (documents are flipped).
    fn offset(sv: &AnyObject) -> (f64, f64) {
        unsafe {
            let clip: Obj = msg_send![sv, contentView];
            let b: NSRect = msg_send![&*clip, bounds];
            (b.origin.x, b.origin.y)
        }
    }

    fn scroll_to(sv: &AnyObject, x: f64, y: f64) {
        unsafe {
            let clip: Obj = msg_send![sv, contentView];
            let _: () = msg_send![&*clip, scrollToPoint: NSPoint::new(x, y)];
            let _: () = msg_send![sv, reflectScrolledClipView: &*clip];
        }
    }

    /// A view's rectangle in screen points, origin top left (as CGEvent takes them).
    fn screen_rect(v: &AnyObject) -> NSRect {
        unsafe {
            let b: NSRect = msg_send![v, bounds];
            let in_win: NSRect = msg_send![v, convertRect: b, toView: Option::<&AnyObject>::None];
            let win: Obj = msg_send![v, window];
            let on_screen: NSRect = msg_send![&*win, convertRectToScreen: in_win];
            let screen: Option<Obj> = msg_send![class!(NSScreen), mainScreen];
            let sf: NSRect = msg_send![&*screen.expect("a screen"), frame];
            rect(
                on_screen.origin.x,
                sf.size.height - on_screen.origin.y - on_screen.size.height,
                on_screen.size.width,
                on_screen.size.height,
            )
        }
    }

    struct Scroll {
        input: Input,
        trace: Trace,
        win: Option<Obj>,
        outer: Option<Obj>,
        inner: Option<Obj>,
    }

    impl Scroll {
        fn sv(&self, anchor: &str) -> Result<&AnyObject, String> {
            match anchor {
                "s" => self.outer.as_deref(),
                "t" => self.inner.as_deref(),
                _ => None,
            }
            .ok_or_else(|| format!("no scroll area {anchor}"))
        }

        /// Where the pointer rests for an action on `anchor`: `t`'s centre, or a point of `s`'s
        /// viewport outside `t`.
        fn point(&self, anchor: &str) -> Result<CGPoint, String> {
            let r = screen_rect(self.sv(anchor)?);
            if anchor == "t" {
                return Ok(CGPoint {
                    x: r.origin.x + r.size.width / 2.0,
                    y: r.origin.y + r.size.height / 2.0,
                });
            }
            let t = screen_rect(self.sv("t")?);
            for (fx, fy) in [(0.9, 0.1), (0.9, 0.9), (0.1, 0.1)] {
                let p = CGPoint {
                    x: r.origin.x + r.size.width * fx,
                    y: r.origin.y + r.size.height * fy,
                };
                let in_t = p.x >= t.origin.x
                    && p.x <= t.origin.x + t.size.width
                    && p.y >= t.origin.y
                    && p.y <= t.origin.y + t.size.height;
                if !in_t {
                    return Ok(p);
                }
            }
            Err("no point of s outside t".into())
        }

        fn sample(&self, t0: Instant, out: &mut Vec<Sample>) {
            let t_ms = t0.elapsed().as_secs_f64() * 1000.0;
            for (a, sv) in [("s", &self.outer), ("t", &self.inner)] {
                if let Some(sv) = sv {
                    let (x, y) = offset(sv);
                    out.push(Sample {
                        t_ms,
                        anchor: a.into(),
                        x,
                        y,
                    });
                }
            }
        }

        /// Pumps and samples for `ms`.
        fn sample_for(&mut self, ms: f64, t0: Instant, out: &mut Vec<Sample>) {
            let until = t0.elapsed().as_secs_f64() * 1000.0 + ms;
            while t0.elapsed().as_secs_f64() * 1000.0 < until {
                self.input.pump(SAMPLE_MS);
                self.sample(t0, out);
            }
        }

        /// Pumps and samples until nothing moved for 300 ms (or 5 s).
        fn settle(&mut self, t0: Instant, out: &mut Vec<Sample>) -> Result<(), String> {
            let start = t0.elapsed().as_secs_f64() * 1000.0;
            let mut last = (offset(self.sv("s")?), offset(self.sv("t")?));
            let mut still_since = start;
            loop {
                self.input.pump(SAMPLE_MS);
                self.sample(t0, out);
                let now = t0.elapsed().as_secs_f64() * 1000.0;
                let cur = (offset(self.sv("s")?), offset(self.sv("t")?));
                if cur != last {
                    last = cur;
                    still_since = now;
                }
                if now - still_since >= SETTLE_MS {
                    return Ok(());
                }
                if now - start > SETTLE_TIMEOUT_MS {
                    return Err("the scroll did not settle within 5 s".into());
                }
            }
        }

        /// Each area's largest offset: document size minus clip size.
        fn max(&self) -> Vec<(String, [f64; 2])> {
            [("s", &self.outer), ("t", &self.inner)]
                .iter()
                .filter_map(|(a, sv)| {
                    let sv = sv.as_deref()?;
                    unsafe {
                        let clip: Obj = msg_send![sv, contentView];
                        let doc: Obj = msg_send![sv, documentView];
                        let c: NSRect = msg_send![&*clip, bounds];
                        let d: NSRect = msg_send![&*doc, frame];
                        Some((
                            a.to_string(),
                            [
                                (d.size.width - c.size.width).max(0.0),
                                (d.size.height - c.size.height).max(0.0),
                            ],
                        ))
                    }
                })
                .collect()
        }

        fn begin(&mut self, anchor: &str) -> Result<(Instant, Vec<Sample>), String> {
            let p = self.point(anchor)?;
            scroll_post::move_to(p);
            self.input.pump(300);
            let t0 = Instant::now();
            let mut samples = Vec::new();
            self.sample(t0, &mut samples);
            Ok((t0, samples))
        }
    }

    impl Driver for Scroll {
        fn setup(&mut self, pin: &Pin) -> Result<(), String> {
            let title = format!("svelters-na {}", pin.id);
            unsafe {
                let screen: Option<Obj> = msg_send![class!(NSScreen), mainScreen];
                let sf: NSRect = msg_send![&*screen.ok_or("no screen")?, frame];
                let al: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
                let win: Obj = msg_send![al, initWithContentRect: rect(120.0, sf.size.height - 460.0, 400.0, 320.0), styleMask: 1usize, backing: 2usize, defer: false];
                let _: () = msg_send![&*win, setReleasedWhenClosed: false];
                let _: () = msg_send![&*win, setLevel: 3isize];
                let _: () = msg_send![&*win, setTitle: &*ns(&title)];
                let content: Obj = msg_send![&*win, contentView];
                let outer_doc = document(960.0, 2400.0, 100);
                let outer = scroll_view(rect(40.0, 40.0, 320.0, 240.0), &outer_doc);
                let inner_doc = document(280.0, 600.0, 25);
                // the document is flipped: y 40 from the content's top
                let inner = scroll_view(rect(20.0, 40.0, 280.0, 120.0), &inner_doc);
                let _: () = msg_send![&*outer_doc, addSubview: &*inner];
                let _: () = msg_send![&*content, addSubview: &*outer];
                let _: () = msg_send![&*win, makeKeyAndOrderFront: Option::<&AnyObject>::None];
                let _: () = msg_send![&*app(), activateIgnoringOtherApps: true];
                self.trace.create("NSWindow", 1);
                self.trace.create("NSScrollView", 2);
                self.trace.create("NSScrollView", 3);
                self.trace.insert(1, 2);
                self.trace.insert(2, 3);
                self.win = Some(win);
                self.outer = Some(outer);
                self.inner = Some(inner);
            }
            self.input.pump(300);
            for (a, [x, y]) in &pin.setup.scroll {
                scroll_to(self.sv(a)?, *x, *y);
                self.trace.call(
                    if a == "s" { 2 } else { 3 },
                    "scrollToPoint",
                    &format!("[{x},{y}]"),
                );
            }
            self.input.pump(200);
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
            let (t0, mut samples) = self.begin(target)?;
            for i in 0..burst.max(1) {
                // the OS's sign: a negative wheel delta scrolls towards the content's end
                scroll_post::wheel([-lines[0], -lines[1]], shift);
                if i + 1 < burst {
                    self.sample_for(interval_ms as f64, t0, &mut samples);
                }
            }
            let input_end_ms = t0.elapsed().as_secs_f64() * 1000.0;
            self.settle(t0, &mut samples)?;
            Ok(ScrollRecord {
                samples,
                input_end_ms,
                max: self.max(),
            })
        }

        fn scroll_gesture(
            &mut self,
            target: &str,
            deltas: &[[f64; 2]],
            interval_ms: u32,
            momentum: &Momentum,
        ) -> Result<ScrollRecord, String> {
            let (t0, mut samples) = self.begin(target)?;
            // every event carries its scripted time (the clock CGEvent timestamps use)
            let base = scroll_post::now_ns();
            let at = |ms: f64| Some(base + (ms * 1e6) as u64);
            let step = interval_ms as f64;
            for (i, d) in deltas.iter().enumerate() {
                scroll_post::continuous_at(
                    [-d[0], -d[1]],
                    if i == 0 { 1 } else { 2 },
                    0,
                    at(i as f64 * step),
                );
                self.sample_for(step, t0, &mut samples);
            }
            let ended = deltas.len() as f64 * step;
            scroll_post::continuous_at([0.0, 0.0], 4, 0, at(ended));
            let input_end_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let replay = match momentum {
                Momentum::None => None,
                Momentum::Native => Some("macos-flick".to_string()),
                Momentum::Replay(n) => Some(n.clone()),
            };
            if let Some(name) = replay {
                let path = conformance_dir()
                    .join("scroll")
                    .join(format!("{name}.json"));
                let stream = scroll_post::recorded_momentum(&path)?;
                let start = t0.elapsed().as_secs_f64() * 1000.0;
                for (t, d, phase) in stream {
                    let wait = start + t - t0.elapsed().as_secs_f64() * 1000.0;
                    if wait > 0.0 {
                        self.sample_for(wait, t0, &mut samples);
                    }
                    // recorded in the OS's sign already; stamped at the recording's own offset
                    scroll_post::continuous_at(d, 0, phase, at(ended + t));
                }
            }
            self.settle(t0, &mut samples)?;
            Ok(ScrollRecord {
                samples,
                input_end_ms,
                max: self.max(),
            })
        }

        fn observe(&mut self) -> Result<Checkpoint, String> {
            let mut cp = Vec::new();
            for (a, h) in [("s", 2), ("t", 3)] {
                let (x, y) = offset(self.sv(a)?);
                let st = State::offset(x, y);
                self.trace.state(h, &st);
                cp.push((a.to_string(), st));
            }
            Ok(cp)
        }

        fn teardown(&mut self) -> Result<String, String> {
            if let Some(w) = self.win.take() {
                unsafe {
                    let _: () = msg_send![&*w, orderOut: Option::<&AnyObject>::None];
                    let _: () = msg_send![&*w, close];
                }
            }
            self.outer = None;
            self.inner = None;
            self.trace.destroy(1);
            self.input.pump(100);
            Ok(std::mem::take(&mut self.trace).text())
        }
    }

    pub fn capture() {
        if std::env::var("NA_MACOS").ok().as_deref() != Some("1") {
            eprintln!(
                "appkit_scroll_references: skipped (opt-in with NA_MACOS=1: it moves the pointer and needs Accessibility)"
            );
            return;
        }
        assert!(
            macos::trusted(),
            "the capture needs Accessibility for the terminal"
        );
        let keys = Keys::load("macos").unwrap();
        let pins = load_scroll_pins().unwrap();
        let only: Option<Vec<String>> = std::env::var("NA_PINS")
            .ok()
            .map(|s| s.split(',').map(str::to_string).collect());
        unsafe {
            let _: bool = msg_send![&*app(), setActivationPolicy: 0isize];
            let _: () = msg_send![&*app(), finishLaunching];
        }
        let mut d = Scroll {
            input: Input::new(),
            trace: Trace::default(),
            win: None,
            outer: None,
            inner: None,
        };
        let (mut results, mut traces) = (vec![], vec![]);
        let version = macos::macos_version();
        let dir = out_dir("macos", &format!("nsscrollview-{version}"));
        std::fs::create_dir_all(&dir).unwrap();
        for pin in &pins {
            if only.as_ref().is_some_and(|o| !o.contains(&pin.id)) {
                continue;
            }
            let (r, t, records) =
                run_pin_scroll(&mut d, pin, &keys).unwrap_or_else(|e| panic!("nsscrollview: {e}"));
            let samples: Vec<String> = records
                .iter()
                .map(|(l, r)| format!("{}:{}", serde_json::to_string(l).unwrap(), r.json()))
                .collect();
            std::fs::write(
                dir.join(format!("{}.samples.json", pin.id)),
                format!("{{{}}}\n", samples.join(",\n")),
            )
            .unwrap();
            if only.is_some() {
                eprintln!("nsscrollview {}:", pin.id);
                for (after, cp) in &r.checkpoints {
                    eprintln!("  {}", checkpoint_json(after, cp));
                }
            }
            results.push(r);
            traces.push((pin.id.clone(), t));
        }
        if only.is_some() {
            return;
        }
        let q = |s: &str| format!("\"{s}\"");
        let settings = vec![
            ("macos".to_string(), q(&version)),
            ("appkit".to_string(), macos::appkit_version().to_string()),
            (
                "natural-scrolling".to_string(),
                q(&defaults_read("-g", "com.apple.swipescrolldirection")),
            ),
            (
                "scroll-wheel-scaling".to_string(),
                q(&defaults_read("-g", "com.apple.scrollwheel.scaling")),
            ),
            (
                "trackpad-scaling".to_string(),
                q(&defaults_read("-g", "com.apple.trackpad.scaling")),
            ),
            (
                "momentum".to_string(),
                q("conformance/scroll/macos-flick.json, replayed"),
            ),
        ];
        let cap = Capture {
            platform: "macos".into(),
            toolkit: "nsscrollview".into(),
            version,
            injector: "cgevent".into(),
            settings,
            observe: vec![],
            pins: results,
        };
        finish(&cap, &traces).unwrap_or_else(|e| panic!("{e}"));
    }
}
