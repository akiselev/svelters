//! NA1 capture on GTK4 (WG3): every pin of `conformance/pins/` on real `GtkEntry`s and a
//! `GtkButton`, created through the svelters GTK backend's C ABI, with input injected through XTEST
//! on a private Xvfb. Text, caret and selection are read through the backend (`text`,
//! `cursor-position`, `selection-bound`; since WG3b the backend reads the last two on the entry's
//! GtkText delegate, and the capture fails when they differ from the GtkText's), keyboard focus
//! through the window's `focus-widget` (the
//! entry's own `has-focus` stays false: GTK4 focuses the entry's inner `GtkText`), and every one of
//! those props is observed (ABI v2.6) so the capture also records whether the backend announced
//! each change the user path made.
//!
//! Writes `target/na/linux/gtk-<version>.json` and the per-pin traces; compares with
//! `conformance/golden/linux/gtk-<version>.json` when it exists; `NA_UPDATE=1` rewrites it.

#![cfg(target_os = "linux")]

use gtk::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use svelters_conformance::golden::{Capture, finish};
use svelters_conformance::pins::{BUTTON, BUTTON_LABEL, Chord, FIELDS, Keys, Pin, load_pins};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::xtest::XInput;
use svelters_conformance::{
    Driver, observe_selections, prepare_selections, rerun_on_private_display, run_pin,
};
use svelters_gtk::testing::{Harness, V, on_gtk, pump};

/// Observation evidence: per prop, (changes seen at checkpoints, changes announced).
#[derive(Default)]
struct Evidence {
    last: HashMap<(u64, String), String>,
    counts: BTreeMap<String, (u32, u32)>,
    /// Checkpoints, and those where the entry's `cursor-position`/`selection-bound` equal the GtkText's.
    agree: (u32, u32),
    unsupported: BTreeMap<String, String>,
}

struct Gtk<'h> {
    h: &'h Harness,
    trace: Trace,
    win: u64,
    fields: Vec<(String, u64)>,
    button: u64,
    x: Option<XInput>,
    ev: Evidence,
    observed: Vec<(u64, String, u32)>,
    /// The pin's selection pseudo-anchors.
    selections: Vec<String>,
}

fn p(ms: u64) {
    pump(ms)
}

impl Gtk<'_> {
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

    fn text_widget(&self, h: u64) -> gtk::Text {
        let e: gtk::Editable = self.h.widget(h).downcast().expect("entry is editable");
        e.delegate()
            .expect("entry delegate")
            .downcast()
            .expect("GtkText")
    }

    /// Window coordinates of a point: 30% into the character at `at` (so a single click puts the
    /// caret before it), or the widget's centre.
    fn point(&self, target: &str, at: Option<usize>) -> Result<(f64, f64), String> {
        let h = self.handle(target)?;
        let win: gtk::Window = self.h.widget(self.win).downcast().unwrap();
        let (w, x, y): (gtk::Widget, f64, f64) = match at {
            Some(at) => {
                let t = self.text_widget(h);
                let len = t.text().chars().count();
                let (s0, _) = t.compute_cursor_extents(at.min(len));
                let x1 = if at < len {
                    t.compute_cursor_extents(at + 1).0.x() as f64
                } else {
                    s0.x() as f64 + 6.0
                };
                let x0 = s0.x() as f64;
                (
                    t.upcast(),
                    x0 + (x1 - x0) * 0.3,
                    s0.y() as f64 + s0.height() as f64 / 2.0,
                )
            }
            None => {
                let w = self.h.widget(h);
                let (x, y) = (w.width() as f64 / 2.0, w.height() as f64 / 2.0);
                (w, x, y)
            }
        };
        #[allow(deprecated)]
        let (wx, wy) = w
            .translate_coordinates(&win, x, y)
            .ok_or("not in the window")?;
        let (sx, sy) = win.surface_transform();
        Ok((wx + sx, wy + sy))
    }

    fn observe_on(&mut self, h: u64, kind: &str, prop: &str) {
        let id = self.h.resolve_prop(kind, prop);
        match self.h.observe(h, prop, true) {
            Ok(()) => self.observed.push((h, prop.to_string(), id)),
            Err((rc, m)) => {
                self.ev
                    .unsupported
                    .insert(prop.to_string(), format!("rc {rc}: {m}"));
            }
        }
    }

    fn value_text(&self, h: u64, prop: &str) -> String {
        match self.h.get(h, prop) {
            V::Handle(_) => {
                // a borrowed handle is minted per read: compare the object, not the number
                let win: gtk::Window = self.h.widget(self.win).downcast().unwrap();
                format!(
                    "{:?}",
                    gtk::prelude::GtkWindowExt::focus(&win).map(|w| w.as_ptr() as usize)
                )
            }
            v => format!("{v:?}"),
        }
    }
}

impl Driver for Gtk<'_> {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let h = self.h;
        let title = format!("svelters-na {}", pin.id);
        let t = &mut self.trace;
        let win = h.create("window");
        t.create("window", win);
        h.set_str(win, "title", &title)?;
        t.set_str(win, "title", &title);
        h.set_bool(win, "decorated", false)?;
        t.set_raw(win, "decorated", "false");
        h.set_i64(win, "default-width", 640)?;
        t.set_raw(win, "default-width", "640");
        let bx = h.create("box");
        t.create("box", bx);
        h.set_str(bx, "orientation", "vertical")?;
        t.set_str(bx, "orientation", "vertical");
        h.set_i64(bx, "spacing", 6)?;
        t.set_raw(bx, "spacing", "6");
        self.fields.clear();
        for a in FIELDS {
            let e = h.create("entry");
            t.create("entry", e);
            let f = pin.setup.field(a);
            h.set_str(e, "text", &f.text)?;
            t.set_str(e, "text", &f.text);
            h.insert(bx, e, None, None)?;
            t.insert(bx, e);
            self.fields.push((a.to_string(), e));
        }
        let b = h.create("button");
        t.create("button", b);
        h.set_str(b, "label", BUTTON_LABEL)?;
        t.set_str(b, "label", BUTTON_LABEL);
        h.insert(bx, b, None, None)?;
        t.insert(bx, b);
        h.insert(win, bx, None, None)?;
        t.insert(win, bx);
        h.call(win, "present")?;
        t.call(win, "present", "[]");
        self.win = win;
        self.button = b;
        p(200);
        let mut x = XInput::attach(&title, &mut p)?;
        prepare_selections(&mut x, pin, &mut p)?;
        self.x = Some(x);
        self.selections = pin.observe.clone();
        // Programmatic setup (the user path starts after it): focus, then each field's selection
        // (grab_focus selects the whole entry, gtk-entry-select-on-focus).
        if let Some(f) = &pin.setup.focus {
            let fh = self.handle(f)?;
            self.h.widget(fh).grab_focus();
            t_call(&mut self.trace, fh, "grabFocus");
        }
        for (a, e) in self.fields.clone() {
            let [anchor, caret] = pin.setup.field(&a).sel();
            self.text_widget(e)
                .upcast::<gtk::Editable>()
                .select_region(anchor as i32, caret as i32);
        }
        p(100);
        self.observed.clear();
        for (_, e) in self.fields.clone() {
            for prop in ["text", "cursor-position", "selection-bound"] {
                self.observe_on(e, "entry", prop);
            }
        }
        self.observe_on(win, "window", "focus-widget");
        self.ev.last.clear();
        Ok(())
    }

    fn key(&mut self, chord: &Chord) -> Result<(), String> {
        self.x.as_ref().unwrap().key(chord, &mut p)
    }

    fn click(
        &mut self,
        target: &str,
        at: Option<usize>,
        count: u32,
        button: u32,
    ) -> Result<(), String> {
        let pt = self.point(target, at)?;
        self.x
            .as_ref()
            .unwrap()
            .click_button(pt, count, button as u8, &mut p)
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.x.as_ref().unwrap().type_text(text, &mut p)
    }

    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
        let (a, b) = (
            self.point(target, Some(from))?,
            self.point(target, Some(to))?,
        );
        self.x.as_ref().unwrap().drag(a, b, count, &mut p)
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        // evidence first: which observed values changed since the last checkpoint, and were they announced
        let announced: HashSet<(u64, u32)> = self.h.take_changed().into_iter().collect();
        for (h, prop, id) in self.observed.clone() {
            let now = self.value_text(h, &prop);
            let key = (h, prop.clone());
            if let Some(old) = self.ev.last.get(&key) {
                if *old != now {
                    let c = self.ev.counts.entry(prop.clone()).or_default();
                    c.0 += 1;
                    if announced.contains(&(h, id)) {
                        c.1 += 1;
                    }
                }
            }
            self.ev.last.insert(key, now);
        }
        let win: gtk::Window = self.h.widget(self.win).downcast().unwrap();
        let fw = match self.h.get(self.win, "focus-widget") {
            V::Handle(bh) => Some(self.h.widget(bh)),
            _ => None,
        };
        let focused = |w: &gtk::Widget| {
            fw.as_ref()
                .is_some_and(|f| f.has_focus() && win.is_active() && (f == w || f.is_ancestor(w)))
        };
        let mut cp = Vec::new();
        for (a, e) in self.fields.clone() {
            let text = match self.h.get(e, "text") {
                V::Str(s) => s,
                o => return Err(format!("text of {a}: {o:?}")),
            };
            let n = |v: V| match v {
                V::I64(n) => Ok(n as usize),
                o => Err(format!("{o:?}")),
            };
            // The backend reads both on the entry's GtkText delegate (WG3b): the real anchor
            // (`selection-bound`) and moving end (`cursor-position`). GtkEditable's own delegate
            // forwarding would report (start, end) and lose the direction (WG3 finding 1).
            let caret = n(self.h.get(e, "cursor-position"))?;
            let anchor = n(self.h.get(e, "selection-bound"))?;
            let t = self.text_widget(e);
            let (tc, tb) = (
                t.property::<i32>("cursor-position") as usize,
                t.property::<i32>("selection-bound") as usize,
            );
            let c = &mut self.ev.agree;
            c.0 += 1;
            if (caret, anchor) != (tc, tb) {
                return Err(format!(
                    "{a}: backend [{anchor},{caret}], GtkText [{tb},{tc}]"
                ));
            }
            c.1 += 1;
            let s = State {
                text: Some(text),
                sel: Some([anchor, caret]),
                focus: focused(&self.h.widget(e)),
            };
            self.trace.state(e, &s);
            cp.push((a, s));
        }
        let s = State {
            text: None,
            sel: None,
            focus: focused(&self.h.widget(self.button)),
        };
        self.trace.state(self.button, &s);
        cp.push((BUTTON.to_string(), s));
        observe_selections(
            self.x.as_ref().unwrap(),
            &self.selections,
            &mut p,
            &mut self.trace,
            &mut cp,
        )?;
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        for (h, prop, _) in std::mem::take(&mut self.observed) {
            let _ = self.h.observe(h, &prop, false);
        }
        self.h.destroy(self.win);
        self.trace.destroy(self.win);
        self.x = None;
        p(100);
        self.h.take_changed();
        Ok(std::mem::take(&mut self.trace).text())
    }
}

fn t_call(t: &mut Trace, h: u64, m: &str) {
    t.call(h, m, "[]");
}

fn capture() {
    let h = Harness::new();
    let keys = Keys::load("linux").unwrap();
    let pins = load_pins().unwrap();
    let mut d = Gtk {
        h: &h,
        trace: Trace::default(),
        win: 0,
        fields: vec![],
        button: 0,
        x: None,
        ev: Evidence::default(),
        observed: vec![],
        selections: vec![],
    };
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let s = gtk::Settings::default().expect("GTK settings");
    let int = |n: &str| s.property::<i32>(n).to_string();
    let settings = vec![
        (
            "gtk-double-click-time".to_string(),
            int("gtk-double-click-time"),
        ),
        (
            "gtk-double-click-distance".to_string(),
            int("gtk-double-click-distance"),
        ),
        (
            "gtk-dnd-drag-threshold".to_string(),
            int("gtk-dnd-drag-threshold"),
        ),
        (
            "gtk-entry-select-on-focus".to_string(),
            s.property::<bool>("gtk-entry-select-on-focus").to_string(),
        ),
        (
            "gtk-keynav-use-caret".to_string(),
            s.property::<bool>("gtk-keynav-use-caret").to_string(),
        ),
        (
            "gtk-im-module".to_string(),
            format!("{:?}", std::env::var("GTK_IM_MODULE").unwrap_or_default()),
        ),
    ];
    let mut observe: Vec<(String, String)> =
        d.ev.counts
            .iter()
            .map(|(k, (c, a))| (k.clone(), format!("{{\"changes\":{c},\"announced\":{a}}}")))
            .collect();
    observe.extend(d.ev.unsupported.iter().map(|(k, m)| {
        (
            k.clone(),
            format!("{{\"unsupported\":{}}}", serde_json_str(m)),
        )
    }));
    observe.push((
        "entry-selection-direction".into(),
        format!(
            "{{\"checkpoints\":{},\"entry_equals_text\":{}}}",
            d.ev.agree.0, d.ev.agree.1
        ),
    ));
    let cap = Capture {
        platform: "linux".into(),
        toolkit: "gtk".into(),
        version: format!(
            "{}.{}.{}",
            gtk::major_version(),
            gtk::minor_version(),
            gtk::micro_version()
        ),
        injector: "xtest".into(),
        settings,
        observe,
        pins: results,
    };
    finish(&cap, &traces).unwrap_or_else(|e| panic!("{e}"));
}

fn serde_json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn gtk_native_references() {
    if let Some(ok) = rerun_on_private_display(&[
        "--exact",
        "gtk_native_references",
        "--nocapture",
        "--test-threads",
        "1",
    ]) {
        assert!(
            ok,
            "the capture on the private display failed (output above)"
        );
        return;
    }
    on_gtk(capture);
}
