//! Styling for the AppKit backend: the `decl` dialect (ST2, `metadata/style/appkit.yaml`).
//!
//! AppKit has no stylesheet language. The runtime cascade engine (`@svelters/native-runtime/style`)
//! matches the component rules and sends every styled element its resolved declarations per state
//! with `style(SVL_STYLE_DECLS, h, text)`: blocks separated by `\x02`, each `state` then
//! `\x01prop:value` fields (the base block has an empty state). This module keeps the blocks per
//! handle, merges the base with the blocks of the states the element is in (hover, focus, checked,
//! active, disabled, in that order), and applies the result natively:
//!
//! - text (`color`, `font-*`, `text-decoration`): `textColor` / `font`, an attributed title on
//!   buttons, an attributed string on text fields with a decoration (re-applied after the text changes);
//! - box (`background-color`, `border-*`, `border-radius`, `box-shadow`): the view's layer; bezeled
//!   text fields draw their own background; a push button gets a tinted bezel (`bezelColor`), or a
//!   flat layer-backed look once a border or radius is styled too;
//! - `opacity` (`alphaValue`), `accent-color` (`bezelColor` / `contentTintColor`);
//! - layout: `width`, `height`, `min-*`, `max-*` as Auto Layout constraints, `flex-grow` as content
//!   hugging; `cursor` through the tracking area.
//!
//! What a declaration replaced is read once and written back when no declaration sets it any more.
//! States: hover from an `NSTrackingArea`, active from KVO on a button cell's `highlighted`, and
//! focus / disabled / checked re-read after every op on the handle, every action, and every window
//! update (`NSWindowDidUpdateNotification`, observed while some element has such a block).

use super::*;
use objc2_foundation::NSSize;
use std::collections::BTreeMap;
use std::ffi::c_void;

/// `CGColorRef` with its ObjC encoding (the layer's colour properties).
#[repr(transparent)]
#[derive(Clone, Copy)]
struct CGColorRef(*const c_void);
// SAFETY: a CGColorRef is a pointer to the opaque `struct CGColor`.
unsafe impl Encode for CGColorRef {
    const ENCODING: Encoding = Encoding::Pointer(&Encoding::Struct("CGColor", &[]));
}

const STATES: [&str; 5] = ["hover", "focus", "checked", "active", "disabled"];
const TEXT_PROPS: [&str; 6] = ["color", "font-size", "font-weight", "font-family", "font-style", "text-decoration"];
const BOX_PROPS: [&str; 6] = ["background-color", "border-width", "border-color", "border-radius", "box-shadow", "accent-color"];
const SIZE_PROPS: [&str; 6] = ["width", "height", "min-width", "min-height", "max-width", "max-height"];

const TRACK_ENTER_EXIT: usize = 0x01;
const TRACK_CURSOR: usize = 0x04;
const TRACK_ACTIVE_ALWAYS: usize = 0x80;
const TRACK_IN_VISIBLE_RECT: usize = 0x200;
const TRACK_DURING_DRAG: usize = 0x400;

pub struct TrackerIvars {
    token: u64,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; StyleTracker does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SveltersStyleTracker"]
    #[ivars = TrackerIvars]
    pub(crate) struct StyleTracker;

    impl StyleTracker {
        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _e: Option<&AnyObject>) {
            let t = self.ivars().token;
            with_current(|i| i.style_hover(t, true));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _e: Option<&AnyObject>) {
            let t = self.ivars().token;
            with_current(|i| i.style_hover(t, false));
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _e: Option<&AnyObject>) {
            let t = self.ivars().token;
            with_current(|i| i.style_cursor(t));
        }

        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(&self, _path: Option<&NSString>, _obj: Option<&AnyObject>, _change: Option<&AnyObject>, _ctx: *mut c_void) {
            let t = self.ivars().token;
            with_current(|i| i.style_refresh_states(t));
        }

        #[unsafe(method(windowUpdated:))]
        fn window_updated(&self, _note: Option<&AnyObject>) {
            with_current(|i| i.style_refresh_all());
        }
    }
);

impl StyleTracker {
    fn new(mtm: MainThreadMarker, token: u64) -> Retained<StyleTracker> {
        let this = Self::alloc(mtm).set_ivars(TrackerIvars { token });
        unsafe { msg_send![super(this), init] }
    }
}

/// What a declaration group replaced, read before the first override.
#[derive(Default)]
struct Orig {
    text_color: Option<Option<Retained<AnyObject>>>,
    font: Option<Option<Retained<AnyObject>>>,
    field_bg: Option<(Option<Retained<AnyObject>>, bool)>,
    bordered: Option<bool>,
    bezel: Option<Option<Retained<AnyObject>>>,
    tint: Option<Option<Retained<AnyObject>>>,
    alpha: Option<f64>,
    hug: Option<(f32, f32)>,
    wants_layer: Option<bool>,
}

/// One element's style: the blocks from the runtime and what was applied.
#[derive(Default)]
pub(crate) struct ElemStyle {
    base: Vec<(String, String)>,
    states: Vec<(&'static str, Vec<(String, String)>)>,
    /// Bits of STATES the element is in.
    active: u8,
    applied: BTreeMap<String, String>,
    orig: Orig,
    constraints: HashMap<&'static str, Retained<AnyObject>>,
    tracker: Option<Retained<StyleTracker>>,
    area: Option<Retained<AnyObject>>,
    /// The button cell observed for `highlighted` (:active).
    kvo_cell: Option<Retained<AnyObject>>,
    /// The text needs the attributed form re-applied after the string changes.
    attributed: bool,
}

impl ElemStyle {
    fn has_state(&self, s: &str) -> bool {
        self.states.iter().any(|(n, _)| *n == s)
    }

    /// The declarations in effect: the base, then the blocks of the active states.
    fn merged(&self) -> BTreeMap<String, String> {
        let mut m: BTreeMap<String, String> = self.base.iter().cloned().collect();
        for (i, st) in STATES.iter().enumerate() {
            if self.active & (1 << i) == 0 {
                continue;
            }
            if let Some((_, decls)) = self.states.iter().find(|(n, _)| n == st) {
                for (p, v) in decls {
                    m.insert(p.clone(), v.clone());
                }
            }
        }
        m
    }
}

/// Parses the DECLS payload into the base and the state blocks.
fn parse_blocks(text: &str) -> (Vec<(String, String)>, Vec<(&'static str, Vec<(String, String)>)>) {
    let mut base = Vec::new();
    let mut states = Vec::new();
    for block in text.split('\u{2}') {
        let mut fields = block.split('\u{1}');
        let state = fields.next().unwrap_or("");
        let decls: Vec<(String, String)> = fields
            .filter_map(|f| f.split_once(':').map(|(p, v)| (p.trim().to_string(), v.trim().to_string())))
            .collect();
        if state.is_empty() {
            base = decls;
        } else if let Some(s) = STATES.iter().find(|s| **s == state) {
            states.push((*s, decls));
        }
    }
    (base, states)
}

// ---- values ----

/// A CSS colour as sRGB components 0..1 (hex, rgb[a](), hsl[a](), a few names, transparent).
pub(crate) fn parse_color(v: &str) -> Option<[f64; 4]> {
    let v = v.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        let d = |s: &str| u8::from_str_radix(s, 16).ok().map(|x| x as f64 / 255.0);
        let dd = |c: char| u8::from_str_radix(&c.to_string(), 16).ok().map(|x| (x * 17) as f64 / 255.0);
        let c: Vec<char> = hex.chars().collect();
        return match c.len() {
            3 => Some([dd(c[0])?, dd(c[1])?, dd(c[2])?, 1.0]),
            4 => Some([dd(c[0])?, dd(c[1])?, dd(c[2])?, dd(c[3])?]),
            6 => Some([d(&hex[0..2])?, d(&hex[2..4])?, d(&hex[4..6])?, 1.0]),
            8 => Some([d(&hex[0..2])?, d(&hex[2..4])?, d(&hex[4..6])?, d(&hex[6..8])?]),
            _ => None,
        };
    }
    if let Some((f, rest)) = v.split_once('(') {
        let args: Vec<&str> = rest.trim_end_matches(')').split(|c| c == ',' || c == ' ' || c == '/').filter(|s| !s.is_empty()).collect();
        let num = |s: &str, scale: f64| -> Option<f64> {
            if let Some(p) = s.strip_suffix('%') {
                p.parse::<f64>().ok().map(|x| x / 100.0)
            } else {
                s.trim_end_matches("deg").parse::<f64>().ok().map(|x| x / scale)
            }
        };
        let alpha = args.get(3).and_then(|a| num(a, 1.0)).unwrap_or(1.0);
        return match f {
            "rgb" | "rgba" if args.len() >= 3 => Some([num(args[0], 255.0)?, num(args[1], 255.0)?, num(args[2], 255.0)?, alpha]),
            "hsl" | "hsla" if args.len() >= 3 => {
                let h = args[0].trim_end_matches("deg").parse::<f64>().ok()? / 360.0;
                let (s, l) = (num(args[1], 100.0)?, num(args[2], 100.0)?);
                let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
                let p = 2.0 * l - q;
                let hue = |mut t: f64| {
                    if t < 0.0 {
                        t += 1.0;
                    }
                    if t > 1.0 {
                        t -= 1.0;
                    }
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
                Some([hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0), alpha])
            }
            _ => None,
        };
    }
    Some(match v.as_str() {
        "transparent" => [0.0, 0.0, 0.0, 0.0],
        "black" => [0.0, 0.0, 0.0, 1.0],
        "white" => [1.0, 1.0, 1.0, 1.0],
        "red" => [1.0, 0.0, 0.0, 1.0],
        "green" => [0.0, 0.5, 0.0, 1.0],
        "blue" => [0.0, 0.0, 1.0, 1.0],
        "gray" | "grey" => [0.5, 0.5, 0.5, 1.0],
        "orange" => [1.0, 0.647, 0.0, 1.0],
        "yellow" => [1.0, 1.0, 0.0, 1.0],
        "purple" => [0.5, 0.0, 0.5, 1.0],
        "pink" => [1.0, 0.753, 0.796, 1.0],
        "brown" => [0.647, 0.165, 0.165, 1.0],
        "cyan" => [0.0, 1.0, 1.0, 1.0],
        "magenta" => [1.0, 0.0, 1.0, 1.0],
        _ => return None,
    })
}

/// A length in points (`px`, `pt` or unitless); `auto` / `none` are None.
pub(crate) fn parse_len(v: &str) -> Option<f64> {
    let v = v.trim();
    if v == "auto" || v == "none" {
        return None;
    }
    v.trim_end_matches("px").trim_end_matches("pt").trim().parse().ok()
}

/// NSFontWeight of a CSS weight.
fn font_weight(v: &str) -> f64 {
    match v.trim() {
        "100" => -0.8,
        "200" => -0.6,
        "300" | "lighter" => -0.4,
        "400" | "normal" => 0.0,
        "500" => 0.23,
        "600" => 0.3,
        "700" | "bold" | "bolder" => 0.4,
        "800" => 0.56,
        "900" => 0.62,
        _ => 0.0,
    }
}

fn nscolor(v: &str) -> Option<Retained<AnyObject>> {
    let [r, g, b, a] = parse_color(v)?;
    unsafe { msg_send![class!(NSColor), colorWithSRGBRed: r, green: g, blue: b, alpha: a] }
}

fn cg(color: Option<&AnyObject>) -> CGColorRef {
    match color {
        Some(c) => unsafe { msg_send![c, CGColor] },
        None => CGColorRef(std::ptr::null()),
    }
}

fn is(obj: &AnyObject, cls: &str) -> bool {
    bridge::class_named(cls).is_some_and(|c| bridge::is_kind(obj, c))
}

fn responds(obj: &AnyObject, s: Sel) -> bool {
    bridge::responds(obj, s)
}

/// A push-style button (title in a bezel), not a checkbox, radio or switch.
fn push_button(obj: &AnyObject) -> bool {
    if !is(obj, "NSButton") || is(obj, "NSPopUpButton") {
        return false;
    }
    let cell: Option<Retained<AnyObject>> = unsafe { msg_send![obj, cell] };
    let Some(cell) = cell else { return false };
    // checkbox / radio cells have an image position other than "no image" with a non-bordered look;
    // NSButtonCell's `_buttonType` is private, so ask whether it draws a bezel.
    let bordered: bool = unsafe { msg_send![obj, isBordered] };
    let image: Option<Retained<AnyObject>> = unsafe { msg_send![&*cell, image] };
    bordered && image.is_none()
}

fn layer_of(view: &AnyObject) -> Option<Retained<AnyObject>> {
    unsafe {
        let _: () = msg_send![view, setWantsLayer: true];
        msg_send![view, layer]
    }
}

/// `SVELTERS_APPKIT_TRACE_STYLE=1` prints every DECLS op and state change.
fn trace() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var_os("SVELTERS_APPKIT_TRACE_STYLE").is_some())
}

impl Inner {
    /// `style(op, h, text)` (ABI v2.3). AppKit speaks the decl dialect; the CSS-dialect ops are
    /// accepted and ignored, so a shared runtime can send them.
    pub(super) fn style_op(&self, op: u32, h: u64, text: Option<&str>) -> Result<i32, String> {
        match op {
            SVL_STYLE_TARGET => Ok(SVL_STYLE_TARGET_APPKIT),
            SVL_STYLE_SCHEME => Ok(self.style_scheme(text)),
            SVL_STYLE_DECLS => {
                self.live(h)?;
                self.style_set(h, text.unwrap_or(""));
                Ok(0)
            }
            _ => Ok(0),
        }
    }

    /// `SVL_STYLE_SCHEME`: `light` / `dark` sets the app's appearance (unstyled controls follow the
    /// runtime's scheme); no text answers the system's (1 dark, 0 light).
    fn style_scheme(&self, text: Option<&str>) -> i32 {
        let app = NSApplication::sharedApplication(self.mtm);
        let scheme = text.and_then(|t| t.split(SVL_STYLE_SEP).next()).unwrap_or("");
        unsafe {
            match scheme {
                "dark" | "light" => {
                    let name = NSString::from_str(if scheme == "dark" { "NSAppearanceNameDarkAqua" } else { "NSAppearanceNameAqua" });
                    let ap: Option<Retained<AnyObject>> = msg_send![class!(NSAppearance), appearanceNamed: &*name];
                    let _: () = msg_send![&*app, setAppearance: ap.as_deref()];
                    0
                }
                _ => {
                    let ap: Retained<AnyObject> = msg_send![&*app, effectiveAppearance];
                    let best: Option<Retained<NSString>> = msg_send![&*ap, name];
                    i32::from(best.is_some_and(|b| b.to_string().contains("Dark")))
                }
            }
        }
    }

    fn style_set(&self, h: u64, text: &str) {
        if trace() {
            eprintln!("[appkit style] decls {h}: {}", text.replace('\u{1}', " ; ").replace('\u{2}', " || "));
        }
        let mut st = self.styles.borrow_mut().remove(&h).unwrap_or_default();
        let (base, states) = parse_blocks(text);
        st.base = base;
        st.states = states;
        let Ok(obj) = self.live(h) else { return };
        self.style_install_states(h, &obj, &mut st);
        st.active = self.style_read_states(&obj, &st);
        self.style_apply(&obj, &mut st, false);
        if text.is_empty() && st.applied.is_empty() {
            self.style_teardown(&obj, &mut st);
        } else {
            self.styles.borrow_mut().insert(h, st);
        }
    }

    /// Tracking area for :hover / cursor, KVO for :active, the window-update observer for the rest.
    fn style_install_states(&self, h: u64, obj: &AnyObject, st: &mut ElemStyle) {
        let cursor = st.base.iter().any(|(p, _)| p == "cursor");
        let want_area = st.has_state("hover") || cursor;
        if want_area && st.area.is_none() && is(obj, "NSView") {
            let tracker = st.tracker.get_or_insert_with(|| StyleTracker::new(self.mtm, h)).clone();
            let opts = TRACK_ENTER_EXIT | TRACK_CURSOR | TRACK_ACTIVE_ALWAYS | TRACK_IN_VISIBLE_RECT | TRACK_DURING_DRAG;
            let rect = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };
            unsafe {
                let a: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSTrackingArea), alloc];
                let area: Retained<AnyObject> = msg_send![a, initWithRect: rect, options: opts, owner: &*tracker, userInfo: std::ptr::null::<AnyObject>()];
                let _: () = msg_send![obj, addTrackingArea: &*area];
                st.area = Some(area);
            }
        }
        if st.has_state("active") && st.kvo_cell.is_none() && is(obj, "NSButton") {
            let tracker = st.tracker.get_or_insert_with(|| StyleTracker::new(self.mtm, h)).clone();
            let cell: Option<Retained<AnyObject>> = unsafe { msg_send![obj, cell] };
            if let Some(cell) = cell {
                let key = NSString::from_str("highlighted");
                unsafe {
                    let _: () = msg_send![&*cell, addObserver: &*tracker, forKeyPath: &*key, options: 0usize, context: std::ptr::null_mut::<c_void>()];
                }
                st.kvo_cell = Some(cell);
            }
        }
        if (st.has_state("focus") || st.has_state("disabled") || st.has_state("checked")) && self.style_window_observer.borrow().is_none() {
            let tracker = StyleTracker::new(self.mtm, 0);
            let name = NSString::from_str("NSWindowDidUpdateNotification");
            unsafe {
                let nc: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
                let _: () = msg_send![&*nc, addObserver: &*tracker, selector: sel!(windowUpdated:), name: &*name, object: std::ptr::null::<AnyObject>()];
            }
            *self.style_window_observer.borrow_mut() = Some(tracker);
        }
    }

    /// The bits of the states read from the control (hover is kept: only the tracking area knows it).
    fn style_read_states(&self, obj: &AnyObject, st: &ElemStyle) -> u8 {
        let mut bits = st.active & 1; // hover
        unsafe {
            if st.has_state("focus") {
                let win: Option<Retained<AnyObject>> = msg_send![obj, window];
                if let Some(win) = win {
                    let fr: Option<Retained<AnyObject>> = msg_send![&*win, firstResponder];
                    if let Some(fr) = fr {
                        let mine = std::ptr::eq(&*fr, obj) || (is(&fr, "NSText") && {
                            let d: Option<Retained<AnyObject>> = msg_send![&*fr, delegate];
                            d.is_some_and(|d| std::ptr::eq(&*d, obj))
                        });
                        if mine {
                            bits |= 1 << 1;
                        }
                    }
                }
            }
            if st.has_state("checked") && is(obj, "NSButton") {
                let s: isize = msg_send![obj, state];
                if s == 1 {
                    bits |= 1 << 2;
                }
            }
            if st.has_state("active") {
                if let Some(cell) = &st.kvo_cell {
                    let hl: bool = msg_send![&**cell, isHighlighted];
                    if hl {
                        bits |= 1 << 3;
                    }
                }
            }
            if st.has_state("disabled") && responds(obj, sel!(isEnabled)) {
                let en: bool = msg_send![obj, isEnabled];
                if !en {
                    bits |= 1 << 4;
                }
            }
        }
        bits
    }

    pub(super) fn style_hover(&self, h: u64, on: bool) {
        if trace() {
            eprintln!("[appkit style] hover {h} {on}");
        }
        let Some(mut st) = self.styles.borrow_mut().remove(&h) else { return };
        if let Ok(obj) = self.live(h) {
            st.active = if on { st.active | 1 } else { st.active & !1 };
            self.style_apply(&obj, &mut st, false);
        }
        self.styles.borrow_mut().insert(h, st);
    }

    pub(super) fn style_cursor(&self, h: u64) {
        let cursor = self.styles.borrow().get(&h).and_then(|s| s.merged().get("cursor").cloned());
        let sel = match cursor.as_deref() {
            Some("pointer") => sel!(pointingHandCursor),
            Some("text") => sel!(IBeamCursor),
            _ => sel!(arrowCursor),
        };
        unsafe {
            let c: Retained<AnyObject> = msg_send![class!(NSCursor), performSelector: sel];
            let _: () = msg_send![&*c, set];
        }
    }

    /// Re-reads the states of `h` and re-applies when they changed (after ops, actions, KVO).
    pub(super) fn style_refresh_states(&self, h: u64) {
        let Some(mut st) = self.styles.borrow_mut().remove(&h) else { return };
        if let Ok(obj) = self.live(h) {
            let bits = self.style_read_states(&obj, &st);
            if bits != st.active {
                st.active = bits;
                self.style_apply(&obj, &mut st, false);
            }
        }
        self.styles.borrow_mut().insert(h, st);
    }

    pub(super) fn style_refresh_all(&self) {
        let hs: Vec<u64> = self
            .styles
            .borrow()
            .iter()
            .filter(|(_, s)| s.has_state("focus") || s.has_state("disabled") || s.has_state("checked"))
            .map(|(h, _)| *h)
            .collect();
        for h in hs {
            self.style_refresh_states(h);
        }
    }

    /// After `set_prop` on a styled handle: states may have changed, and an attributed text must be
    /// rebuilt from the new string.
    pub(super) fn style_after_set(&self, h: u64) {
        let Some(mut st) = self.styles.borrow_mut().remove(&h) else { return };
        if let Ok(obj) = self.live(h) {
            st.active = self.style_read_states(&obj, &st);
            let force = st.attributed;
            self.style_apply(&obj, &mut st, force);
        }
        self.styles.borrow_mut().insert(h, st);
    }

    /// After `insert`: layout declarations that depend on the parent are applied again.
    pub(super) fn style_after_insert(&self, h: u64) {
        let Some(mut st) = self.styles.borrow_mut().remove(&h) else { return };
        if let Ok(obj) = self.live(h) {
            if st.applied.contains_key("flex-grow") || st.applied.contains_key("align-self") {
                st.applied.remove("flex-grow");
                st.applied.remove("align-self");
                self.style_apply(&obj, &mut st, false);
            }
        }
        self.styles.borrow_mut().insert(h, st);
    }

    pub(super) fn style_forget(&self, h: u64) {
        let Some(mut st) = self.styles.borrow_mut().remove(&h) else { return };
        if let Ok(obj) = self.live(h) {
            self.style_teardown(&obj, &mut st);
        } else if let (Some(cell), Some(t)) = (st.kvo_cell.take(), st.tracker.as_ref()) {
            let key = NSString::from_str("highlighted");
            unsafe {
                let _: () = msg_send![&*cell, removeObserver: &**t, forKeyPath: &*key];
            }
        }
    }

    fn style_teardown(&self, obj: &AnyObject, st: &mut ElemStyle) {
        unsafe {
            if let Some(a) = st.area.take() {
                let _: () = msg_send![obj, removeTrackingArea: &*a];
            }
            if let (Some(cell), Some(t)) = (st.kvo_cell.take(), st.tracker.as_ref()) {
                let key = NSString::from_str("highlighted");
                let _: () = msg_send![&*cell, removeObserver: &**t, forKeyPath: &*key];
            }
            for (_, c) in st.constraints.drain() {
                let _: () = msg_send![&*c, setActive: false];
            }
        }
    }

    /// Applies the merged declarations of `st`, group by group, where they changed. `force_text`
    /// rebuilds the text group anyway (its string changed).
    fn style_apply(&self, obj: &AnyObject, st: &mut ElemStyle, force_text: bool) {
        let want = st.merged();
        let changed = |props: &[&str]| props.iter().any(|p| want.get(*p) != st.applied.get(*p));
        let text = force_text || changed(&TEXT_PROPS);
        let boxed = changed(&BOX_PROPS);
        let opacity = changed(&["opacity"]);
        let grow = changed(&["flex-grow", "align-self"]);
        let sizes: Vec<&'static str> = SIZE_PROPS.iter().copied().filter(|p| want.get(*p) != st.applied.get(*p)).collect();
        let r = bridge::catch("style", || unsafe {
            if boxed {
                self.style_box(obj, &want, &mut st.orig);
            }
            if text {
                st.attributed = self.style_text(obj, &want, &mut st.orig);
            }
            if opacity && is(obj, "NSView") {
                let a = st.orig.alpha.get_or_insert_with(|| msg_send![obj, alphaValue]);
                let v = want.get("opacity").and_then(|v| {
                    let v = v.trim();
                    v.strip_suffix('%').map_or_else(|| v.parse::<f64>().ok(), |p| p.parse::<f64>().ok().map(|x| x / 100.0))
                });
                let _: () = msg_send![obj, setAlphaValue: v.unwrap_or(*a)];
            }
            if grow && is(obj, "NSView") {
                let orig = *st.orig.hug.get_or_insert_with(|| {
                    let x: f32 = msg_send![obj, contentHuggingPriorityForOrientation: 0isize];
                    let y: f32 = msg_send![obj, contentHuggingPriorityForOrientation: 1isize];
                    (x, y)
                });
                let g = want.get("flex-grow").and_then(|v| v.trim().parse::<f64>().ok());
                // along the parent stack view's axis (horizontal when there is none)
                let sup: Option<Retained<AnyObject>> = msg_send![obj, superview];
                let axis: isize = match sup {
                    Some(s) if is(&s, "NSStackView") => msg_send![&*s, orientation],
                    _ => 0,
                };
                // > 0 stretches along the axis (hugging 1); 0 keeps the content size there (hugging 750,
                // as a CSS flex item does not grow); none restores the native priorities
                let along = match g {
                    Some(g) if g > 0.0 => Some(1.0f32),
                    Some(_) => Some(750.0f32),
                    None => None,
                };
                let (mut x, mut y) = match along {
                    Some(a) if axis == 0 => (a, orig.1),
                    Some(a) => (orig.0, a),
                    None => orig,
                };
                // align-self: stretch fills the cross axis
                if want.get("align-self").is_some_and(|v| v == "stretch") {
                    if axis == 0 { y = 1.0 } else { x = 1.0 }
                }
                let _: () = msg_send![obj, setContentHuggingPriority: x, forOrientation: 0isize];
                let _: () = msg_send![obj, setContentHuggingPriority: y, forOrientation: 1isize];
            }
            for p in sizes {
                if let Some(c) = st.constraints.remove(p) {
                    let _: () = msg_send![&*c, setActive: false];
                }
                let Some(v) = want.get(p).and_then(|v| parse_len(v)) else { continue };
                if !is(obj, "NSView") {
                    continue;
                }
                let anchor: Retained<AnyObject> = if p.ends_with("width") { msg_send![obj, widthAnchor] } else { msg_send![obj, heightAnchor] };
                let c: Retained<AnyObject> = if p.starts_with("min-") {
                    msg_send![&*anchor, constraintGreaterThanOrEqualToConstant: v]
                } else if p.starts_with("max-") {
                    msg_send![&*anchor, constraintLessThanOrEqualToConstant: v]
                } else {
                    msg_send![&*anchor, constraintEqualToConstant: v]
                };
                // below required: a stack view's own constraints win a conflict instead of breaking
                let _: () = msg_send![&*c, setPriority: 999.0f32];
                let _: () = msg_send![&*c, setActive: true];
                st.constraints.insert(p, c);
            }
        });
        if let Err(e) = r {
            self.report_style_error(&e);
        }
        st.applied = want;
    }

    fn report_style_error(&self, e: &str) {
        eprintln!("[svelters-appkit] style: {e}");
    }

    /// Background, border, radius, shadow and accent colour.
    unsafe fn style_box(&self, obj: &AnyObject, want: &BTreeMap<String, String>, orig: &mut Orig) {
        unsafe {
            if !is(obj, "NSView") {
                return;
            }
            let bg = want.get("background-color").and_then(|v| nscolor(v));
            let has_frame = ["border-width", "border-color", "border-radius"].iter().any(|p| want.contains_key(*p));
            let push = push_button(obj) || (orig.bordered.is_some() && is(obj, "NSButton"));
            let bezeled_field = is(obj, "NSTextField") && {
                let b: bool = msg_send![obj, isBezeled];
                b
            };
            // the background: where it is drawn depends on the control
            let mut layer_bg: Option<Retained<AnyObject>> = None;
            if bezeled_field {
                let o = orig.field_bg.get_or_insert_with(|| (msg_send![obj, backgroundColor], msg_send![obj, drawsBackground]));
                match &bg {
                    Some(c) => {
                        let _: () = msg_send![obj, setBackgroundColor: &**c];
                        let _: () = msg_send![obj, setDrawsBackground: true];
                    }
                    None => {
                        let _: () = msg_send![obj, setBackgroundColor: o.0.as_deref()];
                        let _: () = msg_send![obj, setDrawsBackground: o.1];
                    }
                }
            } else if push {
                let bordered0 = *orig.bordered.get_or_insert_with(|| msg_send![obj, isBordered]);
                let bezel0 = orig.bezel.get_or_insert_with(|| msg_send![obj, bezelColor]).clone();
                if bg.is_some() && has_frame {
                    // flat: no bezel, the layer draws background, border and corners
                    let _: () = msg_send![obj, setBordered: false];
                    let _: () = msg_send![obj, setBezelColor: bezel0.as_deref()];
                    layer_bg = bg.clone();
                } else {
                    let _: () = msg_send![obj, setBordered: bordered0];
                    let tint = bg.clone().or_else(|| want.get("accent-color").and_then(|v| nscolor(v)));
                    let _: () = msg_send![obj, setBezelColor: tint.as_deref().or(bezel0.as_deref())];
                }
            } else {
                layer_bg = bg.clone();
            }
            if !push && responds(obj, sel!(setContentTintColor:)) {
                let t0 = orig.tint.get_or_insert_with(|| msg_send![obj, contentTintColor]).clone();
                let t = want.get("accent-color").and_then(|v| nscolor(v));
                let _: () = msg_send![obj, setContentTintColor: t.as_deref().or(t0.as_deref())];
            }
            let shadow = want.get("box-shadow").filter(|v| v.trim() != "none");
            let needs_layer = layer_bg.is_some() || has_frame || shadow.is_some();
            if !needs_layer && orig.wants_layer.is_none() {
                return;
            }
            orig.wants_layer.get_or_insert_with(|| msg_send![obj, wantsLayer]);
            let Some(layer) = layer_of(obj) else { return };
            let _: () = msg_send![&*layer, setBackgroundColor: cg(layer_bg.as_deref())];
            let bw = want.get("border-width").and_then(|v| parse_len(v)).unwrap_or(if want.contains_key("border-color") { 1.0 } else { 0.0 });
            let _: () = msg_send![&*layer, setBorderWidth: bw];
            let bc = want.get("border-color").and_then(|v| nscolor(v));
            let _: () = msg_send![&*layer, setBorderColor: cg(bc.as_deref())];
            let r = want.get("border-radius").and_then(|v| parse_len(v)).unwrap_or(0.0);
            let _: () = msg_send![&*layer, setCornerRadius: r];
            match shadow {
                Some(v) => {
                    let parts: Vec<&str> = v.split_whitespace().collect();
                    let lens: Vec<f64> = parts.iter().filter_map(|p| parse_len(p)).collect();
                    let color = parts.iter().find(|p| parse_len(p).is_none()).and_then(|p| nscolor(p)).or_else(|| nscolor("rgba(0,0,0,0.3)"));
                    let (x, y, blur) = (lens.first().copied().unwrap_or(0.0), lens.get(1).copied().unwrap_or(0.0), lens.get(2).copied().unwrap_or(0.0));
                    let _: () = msg_send![&*layer, setShadowColor: cg(color.as_deref())];
                    let _: () = msg_send![&*layer, setShadowOpacity: 1.0f32];
                    // CSS y grows down; a non-flipped layer's y grows up
                    let _: () = msg_send![&*layer, setShadowOffset: NSSize::new(x, -y)];
                    let _: () = msg_send![&*layer, setShadowRadius: blur / 2.0];
                    let _: () = msg_send![&*layer, setMasksToBounds: false];
                }
                None => {
                    let _: () = msg_send![&*layer, setShadowOpacity: 0.0f32];
                }
            }
            if !needs_layer {
                let w = orig.wants_layer.unwrap_or(false);
                let _: () = msg_send![obj, setWantsLayer: w];
            }
        }
    }

    /// Text colour, font and decoration. Returns whether the text is attributed (to rebuild when it changes).
    unsafe fn style_text(&self, obj: &AnyObject, want: &BTreeMap<String, String>, orig: &mut Orig) -> bool {
        unsafe {
            // font
            let has_font = ["font-size", "font-weight", "font-family", "font-style"].iter().any(|p| want.contains_key(*p));
            let mut font: Option<Retained<AnyObject>> = None;
            if responds(obj, sel!(setFont:)) {
                let f0 = orig.font.get_or_insert_with(|| msg_send![obj, font]).clone();
                if has_font {
                    let size0: f64 = f0.as_deref().map(|f| msg_send![f, pointSize]).unwrap_or(13.0);
                    let size = want.get("font-size").and_then(|v| parse_len(v)).unwrap_or(size0);
                    let weight = want.get("font-weight").map(|v| font_weight(v)).unwrap_or(0.0);
                    let family = want.get("font-family").map(|v| v.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
                    let mut f: Option<Retained<AnyObject>> = match family.as_deref() {
                        Some("monospace") | Some("ui-monospace") => msg_send![class!(NSFont), monospacedSystemFontOfSize: size, weight: weight],
                        Some(name) if !matches!(name, "system-ui" | "sans-serif" | "-apple-system") => {
                            let n = NSString::from_str(name.split(',').next().unwrap_or(name).trim());
                            let f: Option<Retained<AnyObject>> = msg_send![class!(NSFont), fontWithName: &*n, size: size];
                            match f {
                                Some(f) if weight >= 0.3 => {
                                    let fm: Retained<AnyObject> = msg_send![class!(NSFontManager), sharedFontManager];
                                    msg_send![&*fm, convertFont: &*f, toHaveTrait: 2usize] // NSBoldFontMask
                                }
                                f => f,
                            }
                        }
                        _ => msg_send![class!(NSFont), systemFontOfSize: size, weight: weight],
                    };
                    if want.get("font-style").is_some_and(|s| s == "italic") {
                        if let Some(base) = f.take() {
                            let fm: Retained<AnyObject> = msg_send![class!(NSFontManager), sharedFontManager];
                            f = msg_send![&*fm, convertFont: &*base, toHaveTrait: 1usize]; // NSItalicFontMask
                        }
                    }
                    if let Some(f) = &f {
                        let _: () = msg_send![obj, setFont: &**f];
                    }
                    font = f;
                } else {
                    let _: () = msg_send![obj, setFont: f0.as_deref()];
                    font = f0;
                }
            }
            let color = want.get("color").and_then(|v| nscolor(v));
            let deco = want.get("text-decoration").map(|s| s.as_str()).filter(|d| *d != "none");
            if is(obj, "NSButton") && !is(obj, "NSPopUpButton") {
                let title: Retained<NSString> = msg_send![obj, title];
                if color.is_some() || deco.is_some() {
                    let s = self.attributed(&title, font.as_deref(), color.as_deref(), deco, Some(msg_send![obj, alignment]));
                    let _: () = msg_send![obj, setAttributedTitle: &*s];
                    return true;
                }
                let _: () = msg_send![obj, setTitle: &*title];
                return false;
            }
            if responds(obj, sel!(setTextColor:)) {
                let c0 = orig.text_color.get_or_insert_with(|| msg_send![obj, textColor]).clone();
                let _: () = msg_send![obj, setTextColor: color.as_deref().or(c0.as_deref())];
            }
            if is(obj, "NSTextField") {
                let s: Retained<NSString> = msg_send![obj, stringValue];
                if let Some(d) = deco {
                    let c = color.or_else(|| msg_send![obj, textColor]);
                    let a = self.attributed(&s, font.as_deref(), c.as_deref(), Some(d), Some(msg_send![obj, alignment]));
                    let _: () = msg_send![obj, setAttributedStringValue: &*a];
                    return true;
                }
                // drop attributes left by an earlier decoration
                let _: () = msg_send![obj, setStringValue: &*s];
            }
            false
        }
    }

    unsafe fn attributed(&self, s: &NSString, font: Option<&AnyObject>, color: Option<&AnyObject>, deco: Option<&str>, align: Option<isize>) -> Retained<AnyObject> {
        unsafe {
            let attrs: Retained<AnyObject> = msg_send![class!(NSMutableDictionary), new];
            let put = |k: &str, v: &AnyObject| {
                let k = NSString::from_str(k);
                let _: () = msg_send![&*attrs, setObject: v, forKey: &*k];
            };
            if let Some(f) = font {
                put("NSFont", f);
            }
            if let Some(c) = color {
                put("NSColor", c);
            }
            match deco {
                Some("line-through") => {
                    let one: Retained<AnyObject> = msg_send![class!(NSNumber), numberWithInteger: 1isize];
                    put("NSStrikethrough", &one);
                }
                Some("underline") => {
                    let one: Retained<AnyObject> = msg_send![class!(NSNumber), numberWithInteger: 1isize];
                    put("NSUnderline", &one);
                }
                _ => {}
            }
            if let Some(a) = align {
                let ps: Retained<AnyObject> = msg_send![class!(NSMutableParagraphStyle), new];
                let _: () = msg_send![&*ps, setAlignment: a];
                put("NSParagraphStyle", &ps);
            }
            let a: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSAttributedString), alloc];
            msg_send![a, initWithString: s, attributes: &*attrs]
        }
    }
}
