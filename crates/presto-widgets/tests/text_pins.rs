//! The conformance pins 1–7 (`conformance/pins/`, the ones svelters-conformance's `egui_measure`
//! runs on egui's `TextEdit`) on presto's `text-edit` ([`TextField`] on quire-text), against the
//! Linux goldens (`conformance/golden/linux/`, GTK 4 and Qt 6): the `Gtk` policy against GTK's
//! golden, the `Qt` policy against Qt's. Writes the gap table to `target/na/linux/presto-gap.md`
//! and the CI job summary.
//!
//! The fixture is presto-core's job in the real toolkit and this test's here: two fields `a` and
//! `b` and a button `c` in tab order, focus on click, GTK's one selection owner, the clipboard and
//! PRIMARY served from a model of the X11 selections. Text is laid out with `MonoLayouter` (every
//! char one cell), so a pin's character offset is a cell. Input is presto `KeyEvent`s and
//! `PointerEvent`s as the platform layer delivers them; dead keys are a model of an input method
//! without a server (GTK's simple context): the dead key shows its accent as a preedit, a letter
//! that composes commits the composed letter, Space commits the Compose table's ASCII accent, a
//! letter that does not compose cancels it and goes through alone, Backspace cancels [model, not
//! measured].
//!
//! A measurement, like `egui_text_edit_against_windows_goldens`: it fails only when the harness
//! breaks. A floor (a ratchet per pin) follows the first CI run (`plans/milestones/PRESTO-text-input.md`).

use presto_base::event::{KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerPhase};
use presto_base::event::ImeEvent;
use presto_base::geom::{Point, Rect};
use presto_base::key::Key;
use presto_base::time::{Span, Time};
use presto_widgets::text::editor::{EditorConfig, FocusReason};
use presto_widgets::text::keys::KeyAction;
use presto_widgets::text::shim::{Clipboard, MonoLayouter};
use presto_widgets::text::field::FieldPointer;
use presto_widgets::text::{Platform, TextField, TextPolicy};
use std::fmt::Write as _;
use svelters_conformance::golden::{PinResult, compare, goldens, out_dir};
use svelters_conformance::pins::{BUTTON, CLIPBOARD, Chord, FIELDS, Keys, Pin, load_pins};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::{Driver, run_pin, selection_handle};

const CELL: f32 = 8.0;
const ROW: f32 = 16.0;

/// The X11 CLIPBOARD and PRIMARY as the fixture's only client sees them.
#[derive(Default)]
struct X11 {
    clipboard: String,
    primary: String,
}

impl Clipboard for X11 {
    fn get_text(&mut self) -> Option<String> {
        Some(self.clipboard.replace("\r\n", "\n"))
    }
    fn set_text(&mut self, t: &str) {
        self.clipboard = t.to_string();
    }
    fn get_primary(&mut self) -> Option<String> {
        Some(self.primary.clone())
    }
    fn set_primary(&mut self, t: &str) {
        self.primary = t.to_string();
    }
    // GTK gives PRIMARY up; the last text stays readable by nobody (the golden observes it empty)
    fn clear_primary(&mut self) {
        self.primary.clear();
    }
}

struct Presto {
    policy: TextPolicy,
    lay: MonoLayouter,
    fields: Vec<TextField>,
    /// 0, 1: the fields; 2: the button.
    focus: Option<usize>,
    /// GTK's one selection owner.
    owner: Option<usize>,
    x11: X11,
    now: Time,
    dead: Option<&'static str>,
    selections: Vec<String>,
    trace: Trace,
}

fn origin(i: usize) -> Point {
    Point::new(10.0, 10.0 + i as f32 * 30.0)
}

fn content() -> Rect {
    // wide enough that no pin text scrolls
    Rect::new(0.0, 0.0, 2000.0, ROW)
}

fn chars_of(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].chars().count()
}

fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

fn key_of(name: &str) -> Result<(Key, Option<&'static str>), String> {
    Ok(match name {
        "Left" => (Key::ArrowLeft, None),
        "Right" => (Key::ArrowRight, None),
        "Up" => (Key::ArrowUp, None),
        "Down" => (Key::ArrowDown, None),
        "Home" => (Key::Home, None),
        "End" => (Key::End, None),
        "BackSpace" => (Key::Backspace, None),
        "Delete" => (Key::Delete, None),
        "Tab" => (Key::Tab, Some("\t")),
        "Escape" => (Key::Escape, None),
        "Return" => (Key::Enter, Some("\r")),
        "Insert" => (Key::Insert, None),
        "space" => (Key::Space, Some(" ")),
        "a" => (Key::A, Some("a")),
        "c" => (Key::C, Some("c")),
        "u" => (Key::U, Some("u")),
        "v" => (Key::V, Some("v")),
        "x" => (Key::X, Some("x")),
        "y" => (Key::Y, Some("y")),
        "z" => (Key::Z, Some("z")),
        o => Err(format!("no presto key for {o}"))?,
    })
}

/// The composition of a dead key and the next character (libX11's en_US.UTF-8 Compose table; Space
/// gives the ASCII accent).
fn compose(dead: &str, c: char) -> Option<char> {
    let (from, to, spacing): (&str, &str, char) = match dead {
        "acute" => ("aeiou", "áéíóú", '\''),
        "diaeresis" => ("aeiou", "äëïöü", '"'),
        _ => return None,
    };
    if c == ' ' {
        return Some(spacing);
    }
    from.chars().position(|f| f == c).and_then(|i| to.chars().nth(i))
}

impl Presto {
    fn new(platform: Platform) -> Self {
        Presto {
            policy: TextPolicy::of(platform),
            lay: MonoLayouter { advance: CELL, line_height: ROW },
            fields: Vec::new(),
            focus: None,
            owner: None,
            x11: X11::default(),
            now: Time(1_000_000_000),
            dead: None,
            selections: Vec::new(),
            trace: Trace::default(),
        }
    }

    fn tick(&mut self, ms: i64) {
        self.now = self.now + Span::from_millis(ms);
    }

    fn prepare(&mut self) {
        for (i, f) in self.fields.iter_mut().enumerate() {
            f.prepare(&mut self.lay, content(), origin(i));
        }
    }

    /// After an operation on field `i`: serve its requests, keep one selection owner.
    fn settle(&mut self, i: usize) {
        self.fields[i].serve(&mut self.x11);
        self.fields[i].take_signals();
        if self.policy.one_selection_owner && !self.fields[i].editor.selection().range().is_empty() {
            if let Some(o) = self.owner.filter(|&o| o != i) {
                self.fields[o].editor.collapse_to_start();
            }
            self.owner = Some(i);
        }
        self.prepare();
    }

    fn set_focus(&mut self, to: Option<usize>, reason: FocusReason) {
        if to == self.focus {
            return;
        }
        if let Some(f) = self.focus.filter(|&f| f < 2) {
            self.fields[f].focus_out();
            self.settle(f);
        }
        self.focus = to;
        if let Some(t) = to.filter(|&t| t < 2) {
            self.fields[t].focus_in(reason);
            self.settle(t);
        }
    }

    fn key_event(c: &Chord, key: Key, text: Option<&str>) -> KeyEvent {
        let mut m = Modifiers::default();
        for (on, x) in [(c.ctrl, Modifiers::CTRL), (c.shift, Modifiers::SHIFT), (c.alt, Modifiers::ALT), (c.cmd, Modifiers::SUPER)] {
            if on {
                m = m.union(x);
            }
        }
        let plain = !(c.ctrl || c.alt || c.cmd);
        let text = text.filter(|_| plain).map(|t| if c.shift { t.to_uppercase() } else { t.to_string() });
        KeyEvent { key: Some(key), physical: 0, text, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

    fn ime(&mut self, i: usize, ev: ImeEvent) {
        self.fields[i].ime(&ev, self.now, &mut self.lay);
        self.settle(i);
    }

    /// One printable character from the keyboard (through a waiting dead key).
    fn char_key(&mut self, ch: char) {
        self.tick(30);
        let Some(i) = self.focus.filter(|&f| f < 2) else { return };
        if let Some(d) = self.dead.take() {
            match compose(d, ch) {
                Some(x) => return self.ime(i, ImeEvent::Commit(x.to_string())),
                None => self.ime(i, ImeEvent::Preedit { text: String::new(), cursor: None }),
            }
        }
        let key = if ch == ' ' { Key::Space } else { Key::from_name(&ch.to_ascii_uppercase().to_string()).unwrap_or(Key::A) };
        let ev = KeyEvent { key: Some(key), physical: 0, text: Some(ch.to_string()), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() };
        self.fields[i].key(&ev, self.now, &mut self.lay);
        self.settle(i);
    }

    fn handle(&self, target: &str) -> Result<usize, String> {
        match target {
            "a" => Ok(0),
            "b" => Ok(1),
            t if t == BUTTON => Ok(2),
            t => Err(format!("unknown anchor {t}")),
        }
    }

    /// Character offset `at` of a field, in the field's node coordinates.
    fn point(at: usize) -> Point {
        Point::new(at as f32 * CELL, ROW / 2.0)
    }

    fn ptr(phase: PointerPhase, button: Option<MouseButton>, buttons: u32, local: Point) -> FieldPointer {
        FieldPointer::new(phase, button, buttons, local)
    }

    fn press(&mut self, i: usize, p: Point, button: MouseButton) {
        let bits = if button == MouseButton::Middle { 4 } else { 1 };
        self.fields[i].pointer(&Self::ptr(PointerPhase::Down, Some(button), bits, p), self.now, &mut self.lay);
        self.settle(i);
    }

    fn release(&mut self, i: usize, p: Point, button: MouseButton) {
        self.fields[i].pointer(&Self::ptr(PointerPhase::Up, Some(button), 0, p), self.now, &mut self.lay);
        self.settle(i);
    }
}

impl Driver for Presto {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let cfg = EditorConfig::default();
        self.fields = FIELDS.iter().map(|a| TextField::new(&pin.setup.field(a).text, cfg, self.policy)).collect();
        for (h, a) in FIELDS.iter().enumerate() {
            self.trace.create("text-edit", h as u64 + 1);
            self.trace.set_str(h as u64 + 1, "text", &pin.setup.field(a).text);
        }
        self.trace.create("button", 3);
        self.focus = None;
        self.owner = None;
        self.x11 = X11 { clipboard: pin.setup.clipboard.clone().unwrap_or_default(), primary: String::new() };
        self.selections = pin.observe.clone();
        self.dead = None;
        self.prepare();
        if let Some(f) = &pin.setup.focus {
            let i = self.handle(f)?;
            self.set_focus(Some(i), FocusReason::Programmatic);
            self.trace.call(i as u64 + 1, "grabFocus", "[]");
        }
        for (i, a) in FIELDS.iter().enumerate() {
            let fl = pin.setup.field(a);
            let [anchor, caret] = fl.sel();
            let t = &fl.text;
            self.fields[i].editor.set_selection(byte_of(t, anchor), byte_of(t, caret));
            self.fields[i].take_signals();
        }
        // the pin's selection is the starting state, not a selection the user made
        self.x11.primary.clear();
        self.prepare();
        Ok(())
    }

    fn key(&mut self, c: &Chord) -> Result<(), String> {
        self.tick(100);
        if let Some(d) = c.key.strip_prefix("dead_") {
            let accent = match d {
                "acute" => "\u{b4}",
                "diaeresis" => "\u{a8}",
                _ => return Err(format!("no model for dead key {d}")),
            };
            self.dead = Some(if d == "acute" { "acute" } else { "diaeresis" });
            if let Some(i) = self.focus.filter(|&f| f < 2) {
                self.ime(i, ImeEvent::Preedit { text: accent.into(), cursor: Some((accent.len(), accent.len())) });
            }
            return Ok(());
        }
        let plain = !(c.ctrl || c.alt || c.cmd || c.shift);
        if plain && c.key == "space" && self.dead.is_some() {
            self.char_key(' ');
            return Ok(());
        }
        if self.dead.take().is_some()
            && let Some(i) = self.focus.filter(|&f| f < 2)
        {
            // Backspace and other keys cancel the dead key; Backspace goes no further
            self.ime(i, ImeEvent::Preedit { text: String::new(), cursor: None });
            if c.key == "BackSpace" && plain {
                return Ok(());
            }
        }
        let (key, text) = key_of(&c.key)?;
        let ev = Self::key_event(c, key, text);
        match self.focus {
            Some(i) if i < 2 => {
                let action = self.fields[i].key(&ev, self.now, &mut self.lay);
                self.settle(i);
                if action == KeyAction::Ignore && key == Key::Tab {
                    let n = if c.shift { (i + 2) % 3 } else { (i + 1) % 3 };
                    self.set_focus(Some(n), FocusReason::Keyboard);
                }
            }
            Some(2) if key == Key::Tab => {
                let n = if c.shift { 1 } else { 0 };
                self.set_focus(Some(n), FocusReason::Keyboard);
            }
            _ => {}
        }
        Ok(())
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        for ch in text.chars() {
            self.char_key(ch);
        }
        Ok(())
    }

    fn click(&mut self, target: &str, at: Option<usize>, count: u32, button: u32) -> Result<(), String> {
        self.tick(600);
        let i = self.handle(target)?;
        let b = if button == 2 { MouseButton::Middle } else { MouseButton::Primary };
        if i == 2 {
            if b == MouseButton::Primary && self.policy.platform != Platform::Mac {
                self.set_focus(Some(2), FocusReason::Pointer);
            }
            return Ok(());
        }
        if b == MouseButton::Primary {
            self.set_focus(Some(i), FocusReason::Pointer);
        }
        let p = Self::point(at.unwrap_or(0));
        for _ in 0..count {
            self.press(i, p, b);
            self.tick(20);
            self.release(i, p, b);
            self.tick(30);
        }
        Ok(())
    }

    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
        self.tick(600);
        let i = self.handle(target)?;
        self.set_focus(Some(i), FocusReason::Pointer);
        let (a, b) = (Self::point(from), Self::point(to));
        for k in 0..count {
            self.press(i, a, MouseButton::Primary);
            self.tick(20);
            if k + 1 < count {
                self.release(i, a, MouseButton::Primary);
                self.tick(30);
            }
        }
        for s in 1..=6 {
            let p = a + (b - a) * (s as f32 / 6.0);
            self.tick(16);
            self.fields[i].pointer(&Self::ptr(PointerPhase::Move, None, 1, p), self.now, &mut self.lay);
            self.settle(i);
        }
        self.release(i, b, MouseButton::Primary);
        Ok(())
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        let mut cp = Vec::new();
        for (i, a) in FIELDS.iter().enumerate() {
            let f = &self.fields[i];
            let t = f.text();
            let s = f.editor.selection();
            let st = State { text: Some(t.clone()), sel: Some([chars_of(&t, s.anchor), chars_of(&t, s.head)]), focus: self.focus == Some(i), scroll: None };
            self.trace.state(i as u64 + 1, &st);
            cp.push((a.to_string(), st));
        }
        let st = State { text: None, sel: None, focus: self.focus == Some(2), scroll: None };
        self.trace.state(3, &st);
        cp.push((BUTTON.to_string(), st));
        for name in self.selections.clone() {
            let t = if name == CLIPBOARD { self.x11.clipboard.clone() } else { self.x11.primary.clone() };
            let st = State::selection(t);
            self.trace.state(selection_handle(&name), &st);
            cp.push((name, st));
        }
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        self.fields.clear();
        Ok(std::mem::take(&mut self.trace).text())
    }
}

fn run(platform: Platform, pins: &[Pin], keys: &Keys) -> Vec<PinResult> {
    pins.iter()
        .map(|pin| {
            let mut d = Presto::new(platform);
            run_pin(&mut d, pin, keys).unwrap_or_else(|e| panic!("{e}")).0
        })
        .collect()
}

#[test]
fn presto_text_edit_against_linux_goldens() {
    let keys = Keys::load("linux").unwrap();
    let mut pins = load_pins().unwrap();
    pins.retain(|p| (1..=7).contains(&p.pin));
    let gtk = run(Platform::Gtk, &pins, &keys);
    let qt = run(Platform::Qt, &pins, &keys);
    let golds = goldens("linux").unwrap();
    let find = |t: &str| golds.iter().filter(|g| g.toolkit == t).max_by(|a, b| a.version.cmp(&b.version));
    let (Some(g_gtk), Some(g_qt)) = (find("gtk"), find("qt")) else {
        eprintln!("no Linux goldens (conformance/golden/linux/): nothing to measure");
        return;
    };
    let mut md = format!(
        "| Pin | Behaviour | Checkpoints | presto-gtk = {} {} | presto-qt = {} {} |\n|---|---|---|---|---|\n",
        g_gtk.toolkit, g_gtk.version, g_qt.toolkit, g_qt.version
    );
    let mut details = String::new();
    let mut totals = [0usize; 3];
    for (g, q) in gtk.iter().zip(&qt) {
        let gg = g_gtk.pin(&g.id).map(|p| compare(&g.checkpoints, &p.checkpoints)).unwrap_or_default();
        let qq = g_qt.pin(&q.id).map(|p| compare(&q.checkpoints, &p.checkpoints)).unwrap_or_default();
        let _ = writeln!(md, "| {} | {} | {} | {} | {} |", g.pin, g.id, g.checkpoints.len(), gg.equal, qq.equal);
        totals[0] += g.checkpoints.len();
        totals[1] += gg.equal;
        totals[2] += qq.equal;
        for (name, c) in [("presto-gtk vs gtk", &gg), ("presto-qt vs qt", &qq)] {
            for (i, after, ours, theirs) in &c.diffs {
                let _ = writeln!(details, "- {} #{i} after `{after}`, {name}:\n  - presto: `{ours}`\n  - native: `{theirs}`", g.id);
            }
        }
    }
    let _ = writeln!(md, "| | **total** | **{}** | **{}** | **{}** |", totals[0], totals[1], totals[2]);
    let report = format!("{md}\n## Differing checkpoints\n\n{details}");
    let out = out_dir("linux", "presto-gap.md");
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&out, &report);
    eprintln!("{md}\npresto text-edit gap report: {}", out.display());
    if let Ok(summary) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(summary) {
            let _ = writeln!(f, "### presto text-edit against the Linux references (presto-widgets text_pins)\n\n{md}");
        }
    }
}
