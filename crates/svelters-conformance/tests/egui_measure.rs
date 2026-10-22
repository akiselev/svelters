//! W4 "measure first": runs every pin of `conformance/pins/` on svelters-egui's `text-edit` (egui
//! 0.37 `TextEdit`) under kittest, as the egui-winit X11 path would deliver the input on Linux
//! (`OperatingSystem::Nix`; Ctrl sets `ctrl` and `command`), and compares each checkpoint with the
//! Linux goldens (`conformance/golden/linux/*.json`, GTK and Qt). Writes the egui capture, its
//! traces and the gap table (`target/na/linux/egui-gap.md`, and the CI job summary).
//!
//! FX1: runs every pin three times, once per `svelters_egui::native::Native`: `Egui` (egui's own
//! behaviour, against both goldens), `Gtk` (against GTK's) and `Qt` (against Qt's). The native runs
//! are gated by a ratchet ([`FLOOR`]): a pin matching fewer checkpoints than its floor fails the
//! test; raise the floor when a change matches more.
//!
//! Input is synthetic (egui `Event`s, not winit): no key repeat. Offsets are chars, as in the
//! goldens.
//!
//! Pins 4-6 (WG3b) follow egui-winit 0.37 on Linux: Ctrl+X/C/V (and only those: `command_only`)
//! become `Cut`/`Copy`/`Paste(clipboard)` instead of the key, the paste text with CRLF turned into
//! LF; `CopyText` and `TextSelectionSettled` output commands set the clipboard and PRIMARY; a middle
//! press pushes `MiddleClickPaste` with PRIMARY. Typed characters are `Text` events. Dead keys are
//! a model of winit's X11 path without an input-method server (the setup the native captures use):
//! a dead key composes with the next letter or gives the Compose table's accent on Space; a key
//! that does not compose cancels it and goes through alone [model, not measured on winit].
//! Ctrl+Shift+U reaches egui as a plain key (the hex entry is a GTK and IBus feature).
//!
//! WG3w: `egui_text_edit_against_windows_goldens` runs pins 1-3 and 7 as egui-winit delivers input on Windows
//! (`OperatingSystem::Windows`, egui's own behaviour; Ctrl sets `ctrl` and `command`, as on Linux) against the
//! Windows goldens (`conformance/golden/windows/*.json`, Win32 `EDIT` and WinUI `TextBox`) and writes
//! `target/na/windows/egui-gap.md`. A measurement: it fails only when the harness breaks. Pins 4-6 are left out
//! there: this driver models egui-winit's Linux clipboard and dead-key paths, not Windows'.

use egui::OutputCommand;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::fmt::Write as _;
use svelters_abi::*;
use svelters_conformance::golden::{Capture, PinGap, compare, finish, goldens};
use svelters_conformance::pins::{
    BUTTON, BUTTON_LABEL, CLIPBOARD, Chord, FIELDS, Keys, Pin, load_pins,
};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::{Driver, run_pin, selection_handle};
use svelters_egui::EguiBackend;
use svelters_egui::native::Native;

/// FX1's ratchet: per pin, the fewest checkpoints the `Gtk` run must match in GTK's golden and the
/// `Qt` run in Qt's (`(pin id, gtk, qt)`).
const FLOOR: &[(&str, usize, usize)] = &[
    ("word-move", 33, 33),
    ("word-delete-back", 16, 16),
    ("word-delete-fwd", 16, 16),
    ("word-delete-mid", 7, 7),
    ("select-extend", 14, 14),
    ("select-collapse", 17, 17),
    ("select-all", 11, 11),
    // GTK: double-click on `-` and on a space, one click after the CJK run; Qt: double-click
    // word rules and double-click-drag (FX1 follow-ups)
    ("multi-click", 11, 9),
    ("focus-tab", 9, 9),
    ("focus-click", 7, 7),
    // WG3b's pins 4-6 at their first measurement (egui's own undo, clipboard and IME; FX3 raises them)
    ("undo-typing", 6, 11),
    ("undo-edits", 12, 10),
    ("undo-paste", 6, 6),
    ("clipboard-keys", 12, 12),
    ("paste-multiline", 1, 1),
    ("primary", 5, 5),
    ("dead-keys", 8, 12),
    ("unicode-entry", 1, 1),
];

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
    /// What egui-winit's clipboard would hold: the system clipboard and PRIMARY.
    clipboard: String,
    primary: String,
    /// The pin's selection pseudo-anchors.
    selections: Vec<String>,
    /// A dead key waiting for the next key (`acute`, `diaeresis`, `grave`).
    dead: Option<String>,
}

/// The composition a dead key and the next character give (`None`: no composition, the character
/// goes through alone). Space gives the ASCII accent of libX11's and xkbcommon's en_US.UTF-8 Compose
/// table (`<dead_acute> <space> : "'"`), which winit's X11 path composes with too.
fn compose(dead: &str, c: char) -> Option<char> {
    let (from, to, spacing): (&str, &str, char) = match dead {
        "acute" => ("aeiou", "áéíóú", '\''),
        "diaeresis" => ("aeiou", "äëïöü", '"'),
        "grave" => ("aeiou", "àèìòù", '`'),
        _ => return None,
    };
    if c == ' ' {
        return Some(spacing);
    }
    from.chars()
        .position(|f| f == c)
        .and_then(|i| to.chars().nth(i))
}

impl Egui {
    fn new(native: Native, os: egui::os::OperatingSystem) -> Egui {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec)));
        vt.backend_ref().start(hv).expect("start");
        let harness = Harness::builder()
            .with_size(egui::vec2(700.0, 240.0))
            .with_os(os)
            .with_step_dt(STEP)
            .build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        // egui's bundled fonts have no CJK glyphs; kittest panics on a missing glyph, an app draws
        // tofu. The pins' offsets do not depend on the glyphs.
        harness
            .ctx
            .set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
        svelters_egui::native::set(&harness.ctx, native);
        Egui {
            backend,
            vt,
            harness,
            trace: Trace::default(),
            win: 0,
            fields: vec![],
            button: 0,
            clipboard: String::new(),
            primary: String::new(),
            selections: vec![],
            dead: None,
        }
    }

    /// The clipboard and PRIMARY writes of the last frame (egui-winit's `handle_platform_output`).
    fn collect(&mut self) {
        for c in &self.harness.output().platform_output.commands {
            match c {
                OutputCommand::CopyText(t) => self.clipboard = t.clone(),
                OutputCommand::TextSelectionSettled(t) => self.primary = t.clone(),
                _ => {}
            }
        }
    }

    fn text_event(&mut self, c: char) {
        let c = match self.dead.take() {
            Some(d) => compose(&d, c).unwrap_or(c),
            None => c,
        };
        self.harness.event(egui::Event::Text(c.to_string()));
        self.frames(3);
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
            self.collect();
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
        self.button_n(pos, egui::PointerButton::Primary, pressed);
    }

    fn button_n(&mut self, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) {
        self.harness.event(egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        // egui-winit: a middle press pastes PRIMARY (X11, Wayland)
        if pressed && button == egui::PointerButton::Middle && !self.primary.is_empty() {
            self.harness.event(egui::Event::MiddleClickPaste {
                pos,
                text: self.primary.clone(),
            });
        }
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
        "Insert" => Insert,
        "space" => Space,
        "a" => A,
        "c" => C,
        "u" => U,
        "v" => V,
        "x" => X,
        "y" => Y,
        "z" => Z,
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
        self.clipboard = pin.setup.clipboard.clone().unwrap_or_default();
        self.primary = String::new();
        self.selections = pin.observe.clone();
        self.dead = None;
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
        if let Some(d) = c.key.strip_prefix("dead_") {
            self.dead = Some(d.to_string());
            self.frames(3);
            return Ok(());
        }
        let k = egui_key(&c.key)?;
        let plain = !(c.ctrl || c.alt || c.cmd);
        if plain && !c.shift && k == egui::Key::Space {
            // a key with text: the key and its `Text` (composed when a dead key waits)
            self.harness.key_press_modifiers(mods, k);
            self.text_event(' ');
            return Ok(());
        }
        // any other key cancels a waiting dead key and goes through alone
        self.dead = None;
        let command_only = c.ctrl && !c.shift && !c.alt && !c.cmd;
        match k {
            egui::Key::X if command_only => self.harness.event(egui::Event::Cut),
            egui::Key::C if command_only => self.harness.event(egui::Event::Copy),
            egui::Key::V if command_only => {
                let t = self.clipboard.replace("\r\n", "\n");
                if !t.is_empty() {
                    self.harness.event(egui::Event::Paste(t));
                }
            }
            _ => self.harness.key_press_modifiers(mods, k),
        }
        self.frames(3);
        Ok(())
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        for c in text.chars() {
            self.text_event(c);
        }
        Ok(())
    }

    fn click(
        &mut self,
        target: &str,
        at: Option<usize>,
        count: u32,
        button: u32,
    ) -> Result<(), String> {
        self.settle();
        let pos = self.point(target, at)?;
        self.harness.event(egui::Event::PointerMoved(pos));
        self.frames(1);
        let b = if button == 2 {
            egui::PointerButton::Middle
        } else {
            egui::PointerButton::Primary
        };
        for _ in 0..count {
            self.button_n(pos, b, true);
            self.button_n(pos, b, false);
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
        for name in self.selections.clone() {
            let t = if name == CLIPBOARD {
                self.clipboard.clone()
            } else {
                self.primary.clone()
            };
            let s = State::selection(t);
            self.trace.state(selection_handle(&name), &s);
            cp.push((name, s));
        }
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        self.br().destroy(self.win);
        self.trace.destroy(self.win);
        self.frames(2);
        Ok(std::mem::take(&mut self.trace).text())
    }
}

fn run(native: Native, pins: &[Pin], keys: &Keys) -> Vec<svelters_conformance::golden::PinResult> {
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in pins {
        // a fresh context per pin, as the native captures use a fresh window
        let mut d = Egui::new(native, egui::os::OperatingSystem::Nix);
        let (r, t) = run_pin(&mut d, pin, keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let toolkit = match native {
        Native::Egui => "egui",
        Native::Gtk => "egui-gtk",
        Native::Qt => "egui-qt",
    };
    let cap = Capture {
        platform: "linux".into(),
        toolkit: toolkit.into(),
        version: "0.37.0".into(),
        injector: "kittest".into(),
        settings: vec![],
        observe: vec![],
        pins: results.clone(),
    };
    finish(&cap, &traces).unwrap();
    results
}

#[test]
fn egui_text_edit_against_native_goldens() {
    let keys = Keys::load("linux").unwrap();
    let pins = load_pins().unwrap();
    let stock = run(Native::Egui, &pins, &keys);
    let gtk = run(Native::Gtk, &pins, &keys);
    let qt = run(Native::Qt, &pins, &keys);

    let golds = goldens("linux").unwrap();
    let toolkit = |t: &str| golds.iter().find(|g| g.toolkit == t).expect("a golden");
    let (g_gtk, g_qt) = (toolkit("gtk"), toolkit("qt"));
    let gap = |r: &svelters_conformance::golden::PinResult, g: &svelters_conformance::golden::Golden| {
        g.pin(&r.id).map(|p| compare(&r.checkpoints, &p.checkpoints)).unwrap_or_default()
    };
    let cell = |g: &PinGap| match g.diffs.first() {
        // the first differing step: later checkpoints of a pin build on earlier ones
        Some((i, after, ..)) => format!("{} (first: #{i} `{after}`)", g.equal),
        None => g.equal.to_string(),
    };
    let mut md = format!(
        "| Pin | Behaviour | Checkpoints | egui = {gv} | egui = {qv} | native=gtk = {gv} | native=qt = {qv} |\n|---|---|---|---|---|---|---|\n",
        gv = format!("{} {}", g_gtk.toolkit, g_gtk.version),
        qv = format!("{} {}", g_qt.toolkit, g_qt.version),
    );
    let mut totals = [0usize; 5];
    let mut details = String::new();
    let mut below = Vec::new();
    for ((s, g), q) in stock.iter().zip(&gtk).zip(&qt) {
        let row = [gap(s, g_gtk), gap(s, g_qt), gap(g, g_gtk), gap(q, g_qt)];
        let _ = write!(md, "| {} | {} | {} |", s.pin, s.id, s.checkpoints.len());
        for c in &row {
            let _ = write!(md, " {} |", cell(c));
        }
        md.push('\n');
        totals[0] += s.checkpoints.len();
        for (t, c) in totals[1..].iter_mut().zip(&row) {
            *t += c.equal;
        }
        for (name, c) in [("native=gtk vs gtk", &row[2]), ("native=qt vs qt", &row[3])] {
            for (i, after, ours, theirs) in &c.diffs {
                let _ = writeln!(details, "- {} #{i} after `{after}`, {name}:\n  - egui: `{ours}`\n  - native: `{theirs}`", s.id);
            }
        }
        if let Some(&(_, fg, fq)) = FLOOR.iter().find(|f| f.0 == s.id) {
            if row[2].equal < fg || row[3].equal < fq {
                below.push(format!("{}: gtk {} (floor {fg}), qt {} (floor {fq})", s.id, row[2].equal, row[3].equal));
            }
        } else {
            below.push(format!("{}: no floor (gtk {}, qt {})", s.id, row[2].equal, row[3].equal));
        }
    }
    let _ = writeln!(md, "| | **total** | **{}** | **{}** | **{}** | **{}** | **{}** |", totals[0], totals[1], totals[2], totals[3], totals[4]);
    let report = format!("{md}\n## Differing checkpoints of the native runs\n\n{details}");
    let out = svelters_conformance::golden::out_dir("linux", "egui-gap.md");
    std::fs::write(&out, &report).unwrap();
    eprintln!("{md}\nW4/FX1 gap report: {}", out.display());
    // CI: the table in the job summary
    if let Ok(summary) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(summary) {
            let _ = writeln!(f, "### egui against the native references (svelters-conformance egui_measure)\n\n{md}");
        }
    }
    assert!(below.is_empty(), "FX1 ratchet: {below:#?}");
}

/// WG3w: the pins on egui `TextEdit` as egui-winit feeds it on Windows, against the Windows goldens.
#[test]
fn egui_text_edit_against_windows_goldens() {
    let golds = goldens("windows").unwrap();
    if golds.is_empty() {
        eprintln!("no Windows goldens (conformance/golden/windows/): nothing to measure");
        return;
    }
    let keys = Keys::load("windows").unwrap();
    // pins 1-3 and 7: the driver models egui-winit's Linux clipboard and dead-key paths (pins 4-6)
    let mut pins = load_pins().unwrap();
    pins.retain(|p| matches!(p.pin, 1 | 2 | 3 | 7));
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let mut d = Egui::new(Native::Egui, egui::os::OperatingSystem::Windows);
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let cap = Capture {
        platform: "windows".into(),
        toolkit: "egui".into(),
        version: "0.37.0".into(),
        injector: "kittest".into(),
        settings: vec![],
        observe: vec![],
        pins: results.clone(),
    };
    finish(&cap, &traces).unwrap();
    let mut md = String::from("| Pin | Behaviour | Checkpoints |");
    for g in &golds {
        let _ = write!(md, " = {} {} |", g.toolkit, g.version);
    }
    md.push_str(" = either |\n|---|---|---|");
    md.push_str(&"---|".repeat(golds.len() + 1));
    md.push('\n');
    let mut totals = vec![0usize; golds.len() + 2];
    let mut details = String::new();
    for r in &results {
        let gaps: Vec<PinGap> = golds.iter().map(|g| g.pin(&r.id).map(|p| compare(&r.checkpoints, &p.checkpoints)).unwrap_or_default()).collect();
        let either = (0..r.checkpoints.len())
            .filter(|&i| golds.iter().any(|g| g.pin(&r.id).and_then(|p| p.checkpoints.get(i)).is_some_and(|c| c.1 == r.checkpoints[i].1)))
            .count();
        let _ = write!(md, "| {} | {} | {} |", r.pin, r.id, r.checkpoints.len());
        totals[0] += r.checkpoints.len();
        for (k, g) in gaps.iter().enumerate() {
            totals[k + 1] += g.equal;
            let _ = write!(
                md,
                " {} |",
                match g.diffs.first() {
                    Some((i, after, ..)) => format!("{} (first: #{i} `{after}`)", g.equal),
                    None => g.equal.to_string(),
                }
            );
        }
        totals[golds.len() + 1] += either;
        let _ = writeln!(md, " {either} |");
        for (g, gap) in golds.iter().zip(&gaps) {
            for (i, after, ours, theirs) in &gap.diffs {
                let _ = writeln!(details, "- {} #{i} after `{after}` vs {} {}:\n  - egui: `{ours}`\n  - native: `{theirs}`", r.id, g.toolkit, g.version);
            }
        }
    }
    let _ = write!(md, "| | **total** |");
    for t in &totals {
        let _ = write!(md, " **{t}** |");
    }
    md.push('\n');
    let report = format!("{md}\n## Differing checkpoints\n\n{details}");
    let out = svelters_conformance::golden::out_dir("windows", "egui-gap.md");
    std::fs::write(&out, &report).unwrap();
    eprintln!("{md}\nWG3w egui gap report: {}", out.display());
}
