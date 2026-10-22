//! W4 "measure first": runs every pin of `conformance/pins/` on svelters-egui's `text-edit` (egui
//! 0.37 `TextEdit`) under kittest, as the egui-winit X11 path would deliver the input on Linux
//! (`OperatingSystem::Nix`; Ctrl sets `ctrl` and `command`), and compares each checkpoint with the
//! Linux goldens (`conformance/golden/linux/*.json`, GTK and Qt). Writes the egui capture, its
//! traces and the gap table (`target/na/linux/egui-gap.md`). A measurement, not a gate: it fails
//! only when the harness itself breaks.
//!
//! Input is synthetic (egui `Event`s, not winit): no IME, no key repeat. Offsets are chars, as in
//! the goldens.

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::fmt::Write as _;
use svelters_abi::*;
use svelters_conformance::golden::{Capture, PinGap, compare, finish, goldens};
use svelters_conformance::pins::{BUTTON, BUTTON_LABEL, Chord, FIELDS, Keys, Pin, load_pins};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::{Driver, run_pin};
use svelters_egui::EguiBackend;

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

struct Egui {
    backend: EguiBackend,
    vt: &'static BackendVtable,
    harness: Harness<'static, EguiBackend>,
    trace: Trace,
    win: u64,
    fields: Vec<(String, u64)>,
    button: u64,
}

impl Egui {
    fn new() -> Egui {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec)));
        vt.backend_ref().start(hv).expect("start");
        let harness = Harness::builder()
            .with_size(egui::vec2(700.0, 240.0))
            .with_os(egui::os::OperatingSystem::Nix)
            .with_step_dt(STEP)
            .build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        // egui's bundled fonts have no CJK glyphs; kittest panics on a missing glyph, an app draws
        // tofu. The pins' offsets do not depend on the glyphs.
        harness
            .ctx
            .set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
        Egui {
            backend,
            vt,
            harness,
            trace: Trace::default(),
            win: 0,
            fields: vec![],
            button: 0,
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

    fn set(&mut self, h: u64, name: &str, v: Value) {
        let kind = self.br().handle_info(h).unwrap().kind;
        let id = self.br().resolve(NameKind::Prop, kind, name);
        assert_ne!(id, 0, "no prop {name}");
        self.br().set_prop(h, id, &v).unwrap();
        let json = match &v {
            Value::Str(s) => serde_json::to_string(s).unwrap(),
            Value::F64(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            o => format!("{o:?}"),
        };
        self.trace.set_raw(h, name, &json);
    }

    fn get_text(&self, h: u64) -> String {
        let kind = self.br().handle_info(h).unwrap().kind;
        match self
            .br()
            .get_prop(h, self.br().resolve(NameKind::Prop, kind, "text"))
            .unwrap()
        {
            Value::Str(s) => s,
            o => panic!("text {o:?}"),
        }
    }

    fn insert(&mut self, p: u64, c: u64) {
        self.br().insert(p, c, None, 0).unwrap();
        self.trace.insert(p, c);
    }

    fn handle(&self, anchor: &str) -> Result<u64, String> {
        if anchor == BUTTON {
            return Ok(self.button);
        }
        self.fields
            .iter()
            .find(|(a, _)| a == anchor)
            .map(|(_, h)| *h)
            .ok_or_else(|| format!("no anchor {anchor}"))
    }

    fn id(&self, h: u64) -> egui::Id {
        self.backend.with_scene(|s| s.n(h).id)
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.harness.step();
        }
    }

    /// 30% into the character at `at` (the GTK and Qt captures click there too), or the centre.
    fn point(&mut self, target: &str, at: Option<usize>) -> Result<egui::Pos2, String> {
        let h = self.handle(target)?;
        let rect = self.backend.with_scene(|s| s.n(h).drawn);
        let Some(at) = at else {
            return Ok(rect.center());
        };
        let text = self.get_text(h);
        let ctx = self.harness.ctx.clone();
        let font = egui::TextStyle::Body.resolve(&ctx.global_style());
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap(text.clone(), font, egui::Color32::WHITE));
        let len = text.chars().count();
        let x0 = galley.pos_from_cursor(CCursor::new(at.min(len))).min.x;
        let x1 = if at < len {
            galley.pos_from_cursor(CCursor::new(at + 1)).min.x
        } else {
            x0 + 6.0
        };
        // TextEdit's default margin is (4, 2); its text is not scrolled in these short fields
        Ok(egui::pos2(
            rect.left() + 4.0 + x0 + (x1 - x0) * 0.3,
            rect.center().y,
        ))
    }

    fn button_event(&mut self, pos: egui::Pos2, pressed: bool) {
        self.harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        self.frames(1);
    }

    fn settle(&mut self) {
        // let a previous click sequence expire (egui's double-click delay is 0.3 s)
        self.frames((0.7 / STEP) as usize);
    }
}

fn egui_key(name: &str) -> Result<egui::Key, String> {
    use egui::Key::*;
    Ok(match name {
        "Left" => ArrowLeft,
        "Right" => ArrowRight,
        "Up" => ArrowUp,
        "Down" => ArrowDown,
        "Home" => Home,
        "End" => End,
        "BackSpace" => Backspace,
        "Delete" => Delete,
        "Tab" => Tab,
        "Escape" => Escape,
        "Return" => Enter,
        "a" => A,
        o => return Err(format!("no egui key for {o}")),
    })
}

impl Driver for Egui {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let win = self.create("window");
        self.set(win, "title", Value::Str(format!("svelters-na {}", pin.id)));
        let bx = self.create("box");
        self.set(bx, "gap", Value::F64(6.0));
        self.fields.clear();
        for a in FIELDS {
            let e = self.create("text-edit");
            self.set(e, "text", Value::Str(pin.setup.field(a).text));
            self.set(e, "accessible-name", Value::Str(format!("field {a}")));
            self.insert(bx, e);
            self.fields.push((a.to_string(), e));
        }
        let b = self.create("button");
        self.set(b, "text", Value::Str(BUTTON_LABEL.into()));
        self.insert(bx, b);
        self.insert(win, bx);
        let m = self.br().resolve(
            NameKind::Method,
            self.br().handle_info(win).unwrap().kind,
            "present",
        );
        self.br().call(win, m, &[]).unwrap();
        self.trace.call(win, "present", "[]");
        self.win = win;
        self.button = b;
        self.frames(3);
        let ctx = self.harness.ctx.clone();
        if let Some(f) = &pin.setup.focus {
            let id = self.id(self.handle(f)?);
            ctx.memory_mut(|m| m.request_focus(id));
            self.trace.call(self.handle(f)?, "grabFocus", "[]");
            self.frames(2);
        }
        for (a, e) in self.fields.clone() {
            let [anchor, caret] = pin.setup.field(&a).sel();
            let id = self.id(e);
            let mut st = TextEditState::load(&ctx, id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(anchor),
                CCursor::new(caret),
            )));
            st.store(&ctx, id);
        }
        self.frames(2);
        Ok(())
    }

    fn key(&mut self, c: &Chord) -> Result<(), String> {
        // egui-winit on Linux: Ctrl is `ctrl` and `command`; Super is `mac_cmd` only on macOS
        let mods = egui::Modifiers {
            alt: c.alt,
            ctrl: c.ctrl,
            shift: c.shift,
            mac_cmd: false,
            command: c.ctrl,
        };
        self.harness.key_press_modifiers(mods, egui_key(&c.key)?);
        self.frames(3);
        Ok(())
    }

    fn click(&mut self, target: &str, at: Option<usize>, count: u32) -> Result<(), String> {
        self.settle();
        let pos = self.point(target, at)?;
        self.harness.event(egui::Event::PointerMoved(pos));
        self.frames(1);
        for _ in 0..count {
            self.button_event(pos, true);
            self.button_event(pos, false);
        }
        self.frames(3);
        Ok(())
    }

    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
        self.settle();
        let (a, b) = (
            self.point(target, Some(from))?,
            self.point(target, Some(to))?,
        );
        self.harness.event(egui::Event::PointerMoved(a));
        self.frames(1);
        for i in 0..count {
            self.button_event(a, true);
            if i + 1 < count {
                self.button_event(a, false);
            }
        }
        for s in 1..=6 {
            self.harness
                .event(egui::Event::PointerMoved(a + (b - a) * (s as f32 / 6.0)));
            self.frames(1);
        }
        self.button_event(b, false);
        self.frames(3);
        Ok(())
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        let ctx = self.harness.ctx.clone();
        let mut cp = Vec::new();
        for (a, e) in self.fields.clone() {
            let id = self.id(e);
            let sel = TextEditState::load(&ctx, id)
                .and_then(|s| s.cursor.char_range())
                .map(|r| [r.secondary.index.0, r.primary.index.0]);
            let s = State {
                text: Some(self.get_text(e)),
                sel,
                focus: ctx.memory(|m| m.has_focus(id)),
            };
            self.trace.state(e, &s);
            cp.push((a, s));
        }
        let focus = self
            .harness
            .query_by_label(BUTTON_LABEL)
            .is_some_and(|n| n.is_focused());
        let s = State {
            text: None,
            sel: None,
            focus,
        };
        self.trace.state(self.button, &s);
        cp.push((BUTTON.to_string(), s));
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        self.br().destroy(self.win);
        self.trace.destroy(self.win);
        self.frames(2);
        Ok(std::mem::take(&mut self.trace).text())
    }
}

#[test]
fn egui_text_edit_against_native_goldens() {
    let keys = Keys::load("linux").unwrap();
    let pins = load_pins().unwrap();
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        // a fresh context per pin, as the native captures use a fresh window
        let mut d = Egui::new();
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let cap = Capture {
        platform: "linux".into(),
        toolkit: "egui".into(),
        version: "0.37.0".into(),
        injector: "kittest".into(),
        settings: vec![],
        observe: vec![],
        pins: results.clone(),
    };
    finish(&cap, &traces).unwrap();

    let golds = goldens("linux").unwrap();
    let mut md = String::from("| Pin | Behaviour | Checkpoints |");
    for g in &golds {
        let _ = write!(md, " = {} {} |", g.toolkit, g.version);
    }
    md.push_str(" = either |\n|---|---|---|");
    md.push_str(&"---|".repeat(golds.len() + 1));
    md.push('\n');
    let mut details = String::new();
    for r in &results {
        let gaps: Vec<(String, Option<PinGap>)> = golds
            .iter()
            .map(|g| {
                (
                    format!("{} {}", g.toolkit, g.version),
                    g.pin(&r.id)
                        .map(|p| compare(&r.checkpoints, &p.checkpoints)),
                )
            })
            .collect();
        let either = (0..r.checkpoints.len())
            .filter(|&i| {
                golds.iter().any(|g| {
                    g.pin(&r.id)
                        .and_then(|p| p.checkpoints.get(i))
                        .is_some_and(|c| c.1 == r.checkpoints[i].1)
                })
            })
            .count();
        let _ = write!(md, "| {} | {} | {} |", r.pin, r.id, r.checkpoints.len());
        for (_, g) in &gaps {
            let _ = write!(
                md,
                " {} |",
                g.as_ref()
                    .map_or("-".to_string(), |g| match g.diffs.first() {
                        // the first differing step: later checkpoints of a pin build on earlier ones
                        Some((i, after, ..)) => format!("{} (first: #{i} `{after}`)", g.equal),
                        None => g.equal.to_string(),
                    })
            );
        }
        let _ = writeln!(md, " {either} |");
        for (name, g) in &gaps {
            for (i, after, ours, theirs) in g.iter().flat_map(|g| g.diffs.iter()) {
                let _ = writeln!(
                    details,
                    "- {} #{i} after `{after}` vs {name}:\n  - egui: `{ours}`\n  - native: `{theirs}`",
                    r.id
                );
            }
        }
    }
    let report = format!("{md}\n## Differing checkpoints\n\n{details}");
    let out = svelters_conformance::golden::out_dir("linux", "egui-gap.md");
    std::fs::write(&out, &report).unwrap();
    eprintln!("{md}\nW4 gap report: {}", out.display());
}
