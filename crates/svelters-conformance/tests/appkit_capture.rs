//! NA1 capture on AppKit (WG3m, `plans/milestones/WG3m-macos-references.md`): every pin of
//! `conformance/pins/` (an intent without a macOS chord is a step without input, `run_pin`; there is
//! no primary selection, so `primary` reads ""), on two `NSTextField`s and an `NSButton`, then on two
//! `NSTextView`s and the button, in a tiny AppKit fixture (a window and the standard Edit menu, as
//! every AppKit app has; without it Cmd+Z/X/C/V/A do nothing). Input is the user path through
//! `svelters_conformance::macos` (CGEvent: keys to this process, the mouse through the session
//! tap). Text, selection and focus are read through accessibility (`AXValue`,
//! `AXSelectedTextRange`, `AXFocused`) and cross-checked against AppKit at every checkpoint (the
//! capture fails on a difference). The selection's direction comes from AppKit's private
//! `selectionExtensionAnchor` (`macos::extension_anchor`): AX reports a range only.
//!
//! Opt-in: it moves the real pointer and needs Accessibility for the terminal, so it runs only with
//! `NA_MACOS=1` and must not share the screen with a user (see the milestone). Writes
//! `target/na/macos/<nstextfield|nstextview>-<macOS version>.json` and the per-pin traces; compares
//! with `conformance/golden/macos/` when the version has a golden; `NA_UPDATE=1` rewrites it.
//! `NA_PINS=a,b` runs only those pins and prints them instead (a debugging aid; writes nothing).

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(true, &[("appkit_native_references", mac::capture)]);
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, sel};
    use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
    use std::collections::BTreeMap;
    use svelters_conformance::golden::{Capture, checkpoint_json, finish};
    use svelters_conformance::macos::{self, Ax, CGPoint, Cf, Input};
    use svelters_conformance::pins::{
        BUTTON, BUTTON_LABEL, CLIPBOARD, Chord, FIELDS, Keys, Pin, load_pins,
    };
    use svelters_conformance::trace::{Checkpoint, State, Trace, scalar_to_utf16, utf16_to_scalar};
    use svelters_conformance::{Driver, run_pin, selection_handle};

    type Obj = Retained<AnyObject>;
    /// A field's `AXValue`, `AXSelectedTextRange` (UTF-16) and `AXFocused`.
    type AxField = (Option<String>, Option<(usize, usize)>, bool);

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Kind {
        TextField,
        TextView,
    }

    impl Kind {
        fn toolkit(self) -> &'static str {
            match self {
                Kind::TextField => "nstextfield",
                Kind::TextView => "nstextview",
            }
        }
        fn class(self) -> &'static str {
            match self {
                Kind::TextField => "NSTextField",
                Kind::TextView => "NSTextView",
            }
        }
    }

    // Trace handles of the fixture (the capture does not go through a backend, so it numbers them).
    const H_WIN: u64 = 1;
    fn h_of(anchor: &str) -> u64 {
        match anchor {
            "a" => 2,
            "b" => 3,
            _ => 4,
        }
    }

    fn ns(s: &str) -> Retained<NSString> {
        NSString::from_str(s)
    }

    fn app() -> Obj {
        unsafe { msg_send![class!(NSApplication), sharedApplication] }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    /// The application and its standard Edit menu (Undo, Redo, Cut, Copy, Paste, Select All with
    /// their usual key equivalents and first-responder actions, as Xcode's template has them).
    fn init_app() {
        unsafe {
            let app = app();
            let _: bool = msg_send![&*app, setActivationPolicy: 0isize];
            let main: Obj = msg_send![class!(NSMenu), new];
            let app_item: Obj = msg_send![class!(NSMenuItem), new];
            let _: () = msg_send![&*main, addItem: &*app_item];
            let app_menu: Obj = msg_send![class!(NSMenu), new];
            let _: Obj = msg_send![&*app_menu, addItemWithTitle: &*ns("Quit"), action: sel!(terminate:), keyEquivalent: &*ns("q")];
            let _: () = msg_send![&*app_item, setSubmenu: &*app_menu];
            let edit_item: Obj = msg_send![class!(NSMenuItem), new];
            let _: () = msg_send![&*main, addItem: &*edit_item];
            let edit_alloc: Allocated<AnyObject> = msg_send![class!(NSMenu), alloc];
            let edit: Obj = msg_send![edit_alloc, initWithTitle: &*ns("Edit")];
            for (title, action, key, shift) in [
                ("Undo", sel!(undo:), "z", false),
                ("Redo", sel!(redo:), "z", true),
                ("Cut", sel!(cut:), "x", false),
                ("Copy", sel!(copy:), "c", false),
                ("Paste", sel!(paste:), "v", false),
                ("Select All", sel!(selectAll:), "a", false),
            ] {
                let item: Obj = msg_send![&*edit, addItemWithTitle: &*ns(title), action: action, keyEquivalent: &*ns(key)];
                if shift {
                    // NSEventModifierFlagShift | NSEventModifierFlagCommand
                    let _: () = msg_send![&*item, setKeyEquivalentModifierMask: (1usize << 17) | (1usize << 20)];
                }
            }
            let _: () = msg_send![&*edit_item, setSubmenu: &*edit];
            let _: () = msg_send![&*app, setMainMenu: &*main];
            let _: () = msg_send![&*app, finishLaunching];
        }
    }

    fn pasteboard() -> Obj {
        unsafe { msg_send![class!(NSPasteboard), generalPasteboard] }
    }

    const PB_STRING: &str = "public.utf8-plain-text";

    fn clipboard_get() -> Option<String> {
        unsafe {
            let s: Option<Retained<NSString>> =
                msg_send![&*pasteboard(), stringForType: &*ns(PB_STRING)];
            s.map(|s| s.to_string())
        }
    }

    fn clipboard_set(text: Option<&str>) {
        unsafe {
            let pb = pasteboard();
            let _: isize = msg_send![&*pb, clearContents];
            if let Some(t) = text {
                let _: bool = msg_send![&*pb, setString: &*ns(t), forType: &*ns(PB_STRING)];
            }
        }
    }

    fn string_of(o: &AnyObject, kind: Kind) -> String {
        unsafe {
            let s: Retained<NSString> = match kind {
                Kind::TextField => msg_send![o, stringValue],
                Kind::TextView => msg_send![o, string],
            };
            s.to_string()
        }
    }

    #[derive(Default)]
    struct Evidence {
        /// Checkpoints of a field, and of those the ones whose AX range was checked against the
        /// focused editor's `selectedRange`.
        field_checkpoints: u32,
        range_checked: u32,
        /// Non-empty selections by `selectionExtensionAnchor` (0 none, 1 end, 2 start).
        direction: [u32; 3],
    }

    struct Mac {
        kind: Kind,
        input: Input,
        trace: Trace,
        win: Option<Obj>,
        fields: Vec<(String, Obj)>,
        button: Option<Obj>,
        /// AX elements of `a`, `b`, `c`.
        ax: Vec<Cf>,
        selections: Vec<String>,
        ev: Evidence,
        settings: BTreeMap<String, String>,
    }

    impl Mac {
        fn new(kind: Kind) -> Mac {
            Mac {
                kind,
                input: Input::new(),
                trace: Trace::default(),
                win: None,
                fields: vec![],
                button: None,
                ax: vec![],
                selections: vec![],
                ev: Evidence::default(),
                settings: BTreeMap::new(),
            }
        }

        fn win(&self) -> &AnyObject {
            self.win.as_deref().expect("fixture window")
        }

        fn field(&self, anchor: &str) -> Result<&AnyObject, String> {
            self.fields
                .iter()
                .find(|(a, _)| a == anchor)
                .map(|(_, o)| &**o)
                .ok_or_else(|| format!("no field {anchor}"))
        }

        fn ax_index(anchor: &str) -> usize {
            match anchor {
                "a" => 0,
                "b" => 1,
                _ => 2,
            }
        }

        /// The text view that edits `field` now: the field editor while the text field is being
        /// edited, the text view itself while it is the first responder.
        fn editor(&self, field: &AnyObject) -> Option<Obj> {
            unsafe {
                let fr: Option<Obj> = msg_send![self.win(), firstResponder];
                let fr = fr?;
                match self.kind {
                    Kind::TextView => std::ptr::eq(&*fr, field).then_some(fr),
                    Kind::TextField => {
                        let is_tv: bool = msg_send![&*fr, isKindOfClass: class!(NSTextView)];
                        if !is_tv {
                            return None;
                        }
                        let d: Option<Obj> = msg_send![&*fr, delegate];
                        d.is_some_and(|d| std::ptr::eq(&*d, field)).then_some(fr)
                    }
                }
            }
        }

        fn wait_key(&mut self) -> Result<(), String> {
            let t0 = std::time::Instant::now();
            loop {
                let (active, key): (bool, bool) = unsafe {
                    (
                        msg_send![&*app(), isActive],
                        msg_send![self.win(), isKeyWindow],
                    )
                };
                if active && key {
                    return Ok(());
                }
                if t0.elapsed().as_secs() > 10 {
                    return Err(format!(
                        "the fixture window did not become key (active {active}, key {key})"
                    ));
                }
                unsafe {
                    let _: () = msg_send![&*app(), activateIgnoringOtherApps: true];
                }
                self.input.pump(50);
            }
        }

        /// Screen point 30% into the character at `at` (so a single click puts the caret before
        /// it), past the end by 1.8 pt at the end, or the button's centre (AX geometry).
        fn point(&mut self, target: &str, at: Option<usize>) -> Result<CGPoint, String> {
            let el = &self.ax[Self::ax_index(target)];
            self.input.off_main(|| {
                let Some(at) = at else {
                    let f = Ax::frame(el).ok_or("no AX frame")?;
                    return Ok(CGPoint {
                        x: f.origin.x + f.size.x / 2.0,
                        y: f.origin.y + f.size.y / 2.0,
                    });
                };
                let text = Ax::value(el).unwrap_or_default();
                let len = text.chars().count();
                if len == 0 {
                    let f = Ax::frame(el).ok_or("no AX frame")?;
                    return Ok(CGPoint {
                        x: f.origin.x + 4.0,
                        y: f.origin.y + f.size.y / 2.0,
                    });
                }
                let (i, past_end) = if at < len {
                    (at, false)
                } else {
                    (len - 1, true)
                };
                let u0 = scalar_to_utf16(&text, i);
                let u1 = scalar_to_utf16(&text, i + 1);
                let r = Ax::bounds_for_range(el, u0, u1 - u0).ok_or("no AXBoundsForRange")?;
                let y = r.origin.y + r.size.y / 2.0;
                Ok(if past_end {
                    CGPoint {
                        x: r.origin.x + r.size.x + 1.8,
                        y,
                    }
                } else {
                    CGPoint {
                        x: r.origin.x + r.size.x * 0.3,
                        y,
                    }
                })
            })
        }

        /// What decides the behaviour: recorded in the golden's `settings`.
        fn record_settings(&mut self, editor: &AnyObject) {
            unsafe {
                let defaults: Obj = msg_send![class!(NSUserDefaults), standardUserDefaults];
                let ui: isize = msg_send![&*defaults, integerForKey: &*ns("AppleKeyboardUIMode")];
                let dbl: f64 = msg_send![class!(NSEvent), doubleClickInterval];
                let tlm: Option<Obj> = msg_send![editor, textLayoutManager];
                let flags: [(&str, bool); 5] = [
                    (
                        "automatic-spelling-correction",
                        msg_send![editor, isAutomaticSpellingCorrectionEnabled],
                    ),
                    (
                        "automatic-text-replacement",
                        msg_send![editor, isAutomaticTextReplacementEnabled],
                    ),
                    (
                        "automatic-quote-substitution",
                        msg_send![editor, isAutomaticQuoteSubstitutionEnabled],
                    ),
                    (
                        "automatic-dash-substitution",
                        msg_send![editor, isAutomaticDashSubstitutionEnabled],
                    ),
                    (
                        "smart-insert-delete",
                        msg_send![editor, smartInsertDeleteEnabled],
                    ),
                ];
                let s = &mut self.settings;
                s.insert("macos".into(), format!("\"{}\"", macos::macos_version()));
                s.insert("appkit".into(), macos::appkit_version().to_string());
                s.insert(
                    "text-kit".into(),
                    if tlm.is_some() { "2" } else { "1" }.into(),
                );
                s.insert("double-click-interval".into(), dbl.to_string());
                s.insert("keyboard-ui-mode".into(), ui.to_string());
                s.insert(
                    "input-source".into(),
                    format!("\"{}\"", macos::input_source()),
                );
                for (k, v) in flags {
                    s.insert(k.into(), v.to_string());
                }
            }
        }
    }

    impl Driver for Mac {
        fn setup(&mut self, pin: &Pin) -> Result<(), String> {
            let title = format!("svelters-na {}", pin.id);
            let t = &mut self.trace;
            unsafe {
                let screen: Option<Obj> = msg_send![class!(NSScreen), mainScreen];
                let sf: NSRect = msg_send![&*screen.ok_or("no screen")?, frame];
                let alloc: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
                // titled, buffered; content 520x200 near the top left of the main screen
                let win: Obj = msg_send![alloc, initWithContentRect: rect(120.0, sf.size.height - 360.0, 520.0, 200.0), styleMask: 1usize, backing: 2usize, defer: false];
                let _: () = msg_send![&*win, setReleasedWhenClosed: false];
                // floating: above other apps' windows, so the session-tap clicks land on it
                let _: () = msg_send![&*win, setLevel: 3isize];
                let _: () = msg_send![&*win, setTitle: &*ns(&title)];
                let _: () = msg_send![&*win, setAutorecalculatesKeyViewLoop: true];
                t.create("NSWindow", H_WIN);
                t.set_str(H_WIN, "title", &title);
                let content: Obj = msg_send![&*win, contentView];
                self.fields.clear();
                for (i, a) in FIELDS.iter().enumerate() {
                    let f = pin.setup.field(a);
                    let frame = rect(20.0, 150.0 - 34.0 * i as f64, 440.0, 24.0);
                    let o: Obj = match self.kind {
                        Kind::TextField => {
                            let al: Allocated<AnyObject> = msg_send![class!(NSTextField), alloc];
                            let o: Obj = msg_send![al, initWithFrame: frame];
                            let _: () = msg_send![&*o, setStringValue: &*ns(&f.text)];
                            o
                        }
                        Kind::TextView => {
                            let al: Allocated<AnyObject> = msg_send![class!(NSTextView), alloc];
                            let o: Obj = msg_send![al, initWithFrame: frame];
                            let _: () = msg_send![&*o, setRichText: false];
                            // Interface Builder's default for a text view (a text view made in code
                            // has no undo; the field editor of a text field has)
                            let _: () = msg_send![&*o, setAllowsUndo: true];
                            let _: () = msg_send![&*o, setString: &*ns(&f.text)];
                            o
                        }
                    };
                    let _: () = msg_send![&*o, setIdentifier: &*ns(&format!("na-{a}"))];
                    let _: () = msg_send![&*content, addSubview: &*o];
                    t.create(self.kind.class(), h_of(a));
                    t.set_str(h_of(a), "text", &f.text);
                    t.insert(H_WIN, h_of(a));
                    self.fields.push((a.to_string(), o));
                }
                let al: Allocated<AnyObject> = msg_send![class!(NSButton), alloc];
                let b: Obj = msg_send![al, initWithFrame: rect(20.0, 70.0, 100.0, 32.0)];
                let _: () = msg_send![&*b, setTitle: &*ns(BUTTON_LABEL)];
                // NSBezelStyleRounded, the push button
                let _: () = msg_send![&*b, setBezelStyle: 1usize];
                let _: () = msg_send![&*b, setIdentifier: &*ns("na-c")];
                let _: () = msg_send![&*content, addSubview: &*b];
                t.create("NSButton", h_of(BUTTON));
                t.set_str(h_of(BUTTON), "title", BUTTON_LABEL);
                t.insert(H_WIN, h_of(BUTTON));
                let _: () = msg_send![&*win, makeKeyAndOrderFront: Option::<&AnyObject>::None];
                let _: () = msg_send![&*app(), activateIgnoringOtherApps: true];
                t.call(H_WIN, "makeKeyAndOrderFront", "[]");
                self.win = Some(win);
                self.button = Some(b);
            }
            self.wait_key()?;
            clipboard_set(pin.setup.clipboard.as_deref());
            self.selections = pin.observe.clone();
            // Programmatic setup (the user path starts after it): focus, then the selections. An
            // NSTextField has a selection only while it is edited, so only the focused one gets it.
            if let Some(f) = &pin.setup.focus {
                let o = self.field(f)?;
                let ok: bool = unsafe { msg_send![self.win(), makeFirstResponder: o] };
                if !ok {
                    return Err(format!("{f} refused first responder"));
                }
                self.trace.call(h_of(f), "makeFirstResponder", "[]");
            }
            for (a, o) in self.fields.clone() {
                let f = pin.setup.field(&a);
                let [anchor, caret] = f.sel();
                let (s, e) = (anchor.min(caret), anchor.max(caret));
                let target: Option<Obj> = match self.kind {
                    Kind::TextView => Some(o.clone()),
                    Kind::TextField => self.editor(&o),
                };
                if let Some(tv) = target {
                    let (u0, u1) = (scalar_to_utf16(&f.text, s), scalar_to_utf16(&f.text, e));
                    unsafe {
                        let _: () = msg_send![&*tv, setSelectedRange: NSRange::new(u0, u1 - u0)];
                    }
                    self.trace
                        .call(h_of(&a), "setSelectedRange", &format!("[{s},{e}]"));
                }
            }
            self.input.pump(100);
            if self.settings.is_empty()
                && let Some(tv) = pin
                    .setup
                    .focus
                    .as_deref()
                    .and_then(|f| self.editor(self.field(f).ok()?))
            {
                self.record_settings(&tv);
            }
            let ax = self.input.off_main(|| {
                let own = Ax::own();
                ["na-a", "na-b", "na-c"]
                    .iter()
                    .map(|id| own.find(id))
                    .collect::<Result<Vec<_>, _>>()
            })?;
            self.ax = ax;
            Ok(())
        }

        fn key(&mut self, chord: &Chord) -> Result<(), String> {
            self.input.key(chord)
        }

        fn click(
            &mut self,
            target: &str,
            at: Option<usize>,
            count: u32,
            button: u32,
        ) -> Result<(), String> {
            let p = self.point(target, at)?;
            self.input.click(p, count, button)
        }

        fn type_text(&mut self, text: &str) -> Result<(), String> {
            self.input.type_text(text)
        }

        fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
            let (a, b) = (
                self.point(target, Some(from))?,
                self.point(target, Some(to))?,
            );
            self.input.drag(a, b, count)
        }

        fn observe(&mut self) -> Result<Checkpoint, String> {
            let (active, key): (bool, bool) = unsafe {
                (
                    msg_send![&*app(), isActive],
                    msg_send![self.win(), isKeyWindow],
                )
            };
            if !(active && key) {
                return Err(format!(
                    "the fixture lost key status (active {active}, key {key}); re-run without touching the Mac"
                ));
            }
            let els = &self.ax;
            // AX: value, selected range and focus of a, b; focus of c
            let read = self.input.off_main(|| {
                let fields: Vec<AxField> = els[..2]
                    .iter()
                    .map(|e| (Ax::value(e), Ax::selected_range(e), Ax::focused(e)))
                    .collect();
                (fields, Ax::focused(&els[2]), Ax::role(&els[0]))
            });
            let (fields, button_focus, role) = read;
            self.settings
                .entry("ax-role".into())
                .or_insert_with(|| format!("\"{}\"", role.unwrap_or_default()));
            let fr: Option<Obj> = unsafe { msg_send![self.win(), firstResponder] };
            let mut cp = Vec::new();
            for ((a, o), (value, range, ax_focus)) in self.fields.clone().into_iter().zip(fields) {
                let native = string_of(&o, self.kind);
                let text = value.ok_or_else(|| format!("{a}: no AXValue"))?;
                if text != native {
                    return Err(format!("{a}: AXValue {text:?}, AppKit {native:?}"));
                }
                let (loc, len) = range.ok_or_else(|| format!("{a}: no AXSelectedTextRange"))?;
                let editor = self.editor(&o);
                self.ev.field_checkpoints += 1;
                let mut ext = 0;
                if let Some(tv) = &editor {
                    let r: NSRange = unsafe { msg_send![&**tv, selectedRange] };
                    if (r.location, r.length) != (loc, len) {
                        return Err(format!(
                            "{a}: AX range ({loc},{len}), AppKit ({},{})",
                            r.location, r.length
                        ));
                    }
                    self.ev.range_checked += 1;
                    ext = macos::extension_anchor(tv)?;
                }
                let (s, e) = (
                    utf16_to_scalar(&text, loc),
                    utf16_to_scalar(&text, loc + len),
                );
                if e > s {
                    self.ev.direction[ext.min(2) as usize] += 1;
                }
                let sel = if ext == 1 { [e, s] } else { [s, e] };
                if ax_focus != editor.is_some() {
                    return Err(format!(
                        "{a}: AXFocused {ax_focus}, AppKit editing {}",
                        editor.is_some()
                    ));
                }
                let st = State {
                    text: Some(text),
                    sel: Some(sel),
                    focus: ax_focus,
                };
                self.trace.state(h_of(&a), &st);
                cp.push((a, st));
            }
            let native_button = fr
                .as_deref()
                .is_some_and(|f| std::ptr::eq(f, &**self.button.as_ref().unwrap()));
            if button_focus != native_button {
                return Err(format!(
                    "button: AXFocused {button_focus}, first responder {native_button}"
                ));
            }
            let st = State {
                text: None,
                sel: None,
                focus: button_focus,
            };
            self.trace.state(h_of(BUTTON), &st);
            cp.push((BUTTON.to_string(), st));
            for name in self.selections.clone() {
                // macOS has no primary selection: `primary` is always "" (as on Windows, WG3w)
                let text = if name == CLIPBOARD {
                    clipboard_get().unwrap_or_default()
                } else {
                    String::new()
                };
                let st = State::selection(text);
                self.trace.state(selection_handle(&name), &st);
                cp.push((name, st));
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
            self.trace.destroy(H_WIN);
            self.fields.clear();
            self.button = None;
            self.ax.clear();
            self.input.pump(100);
            Ok(std::mem::take(&mut self.trace).text())
        }
    }

    fn run_kind(kind: Kind, pins: &[Pin], keys: &Keys, only: &Option<Vec<String>>) {
        let mut d = Mac::new(kind);
        let (mut results, mut traces) = (vec![], vec![]);
        for pin in pins {
            if only.as_ref().is_some_and(|o| !o.contains(&pin.id)) {
                continue;
            }
            let (r, t) =
                run_pin(&mut d, pin, keys).unwrap_or_else(|e| panic!("{}: {e}", kind.toolkit()));
            if only.is_some() {
                eprintln!("{} {}:", kind.toolkit(), pin.id);
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
        let e = &d.ev;
        let observe = vec![
            (
                "ax-cross-check".to_string(),
                format!(
                    "{{\"field_checkpoints\":{},\"range_equals_editor\":{}}}",
                    e.field_checkpoints, e.range_checked
                ),
            ),
            (
                "selection-direction".to_string(),
                format!(
                    "{{\"none\":{},\"anchor_end\":{},\"anchor_start\":{}}}",
                    e.direction[0], e.direction[1], e.direction[2]
                ),
            ),
        ];
        let cap = Capture {
            platform: "macos".into(),
            toolkit: kind.toolkit().into(),
            version: macos::macos_version(),
            injector: "cgevent".into(),
            settings: d.settings.clone().into_iter().collect(),
            observe,
            pins: results,
        };
        finish(&cap, &traces).unwrap_or_else(|e| panic!("{e}"));
    }

    pub fn capture() {
        if std::env::var("NA_MACOS").ok().as_deref() != Some("1") {
            eprintln!(
                "appkit_native_references: skipped (opt-in with NA_MACOS=1: it moves the pointer and needs Accessibility)"
            );
            return;
        }
        assert!(
            macos::trusted(),
            "the capture needs Accessibility for the process that runs it (System Settings → Privacy & Security → Accessibility: the terminal)"
        );
        let keys = Keys::load("macos").unwrap();
        let pins = load_pins().unwrap();
        let only: Option<Vec<String>> = std::env::var("NA_PINS")
            .ok()
            .map(|s| s.split(',').map(str::to_string).collect());
        let kinds: Vec<Kind> = match std::env::var("NA_MACOS_KIND").ok().as_deref() {
            Some("nstextfield") => vec![Kind::TextField],
            Some("nstextview") => vec![Kind::TextView],
            _ => vec![Kind::TextField, Kind::TextView],
        };
        init_app();
        // the capture seeds and reads the general pasteboard: keep the user's text and put it back
        let saved = clipboard_get();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for k in kinds {
                run_kind(k, &pins, &keys, &only);
            }
        }));
        clipboard_set(saved.as_deref());
        if let Err(e) = r {
            std::panic::resume_unwind(e);
        }
    }
}
