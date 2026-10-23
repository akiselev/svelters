//! The macOS user path and observation of the AppKit captures (WG3m,
//! `plans/milestones/WG3m-macos-references.md`): CGEvent input, accessibility (AX) reads, and the
//! main-thread run loop the fixture lives in. Everything here runs in the capture's own process,
//! whose fixture window is AppKit's: the calling thread must be the main thread.
//!
//! - Keys go to this process (`CGEventPostToPid`), so a capture never types into another app.
//! - Clicks and drags go through the session event tap (`CGEventPost`): the window server routes
//!   them to the window under the pointer, as for a mouse. Events posted to a process skip that
//!   routing and AppKit drops them (no window). The real pointer moves, so a capture checks after
//!   every mouse action that the pointer is still where it put it.
//! - Every action ends with a barrier: an F20 key event, posted the same way as the action. The run
//!   loop dispatches every event before it and swallows the marker, so the checkpoint reads the
//!   state after the action and never a stale one (AX answers from the run loop too).
//! - AX is read from a helper thread while the main thread keeps dispatching (an AX request to the
//!   own process is served by its main run loop).

use crate::pins::Chord;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use std::ffi::{CStr, c_char, c_void};
use std::time::{Duration, Instant};

type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    /// `(width, height)`.
    pub size: CGPoint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFBooleanGetValue(b: CFTypeRef) -> bool;
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: bool,
    ) -> CFTypeRef;
    fn CFStringGetLength(s: CFTypeRef) -> isize;
    fn CFStringGetCharacters(s: CFTypeRef, r: CFRange, buf: *mut u16);
    fn CFArrayGetCount(a: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFTypeRef, i: isize) -> CFTypeRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(e: CFTypeRef, attr: CFTypeRef, out: *mut CFTypeRef) -> i32;
    fn AXUIElementCopyParameterizedAttributeValue(
        e: CFTypeRef,
        attr: CFTypeRef,
        param: CFTypeRef,
        out: *mut CFTypeRef,
    ) -> i32;
    fn AXValueCreate(ty: u32, ptr: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(v: CFTypeRef, ty: u32, ptr: *mut c_void) -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> CFTypeRef;
    fn CGEventCreate(src: CFTypeRef) -> CFTypeRef;
    fn CGEventGetLocation(e: CFTypeRef) -> CGPoint;
    fn CGEventCreateKeyboardEvent(src: CFTypeRef, key: u16, down: bool) -> CFTypeRef;
    fn CGEventCreateMouseEvent(src: CFTypeRef, ty: u32, at: CGPoint, button: u32) -> CFTypeRef;
    fn CGEventSetFlags(e: CFTypeRef, flags: u64);
    fn CGEventSetIntegerValueField(e: CFTypeRef, field: u32, v: i64);
    fn CGEventKeyboardSetUnicodeString(e: CFTypeRef, len: usize, s: *const u16);
    fn CGEventPostToPid(pid: i32, e: CFTypeRef);
    fn CGEventPost(tap: u32, e: CFTypeRef);
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> CFTypeRef;
    fn TISGetInputSourceProperty(src: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    static kTISPropertyInputSourceID: CFTypeRef;
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSAppKitVersionNumber: f64;
}

unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> i32;
    fn getpid() -> i32;
}

/// An owned Core Foundation reference (released on drop). CF objects may be retained and released
/// on any thread; AX elements may be queried from any thread.
pub struct Cf(CFTypeRef);
unsafe impl Send for Cf {}
unsafe impl Sync for Cf {}

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

impl Cf {
    fn new(r: CFTypeRef) -> Option<Cf> {
        (!r.is_null()).then_some(Cf(r))
    }
}

fn cfstr(s: &str) -> Cf {
    // kCFStringEncodingUTF8
    Cf(unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            s.as_ptr(),
            s.len() as isize,
            0x0800_0100,
            false,
        )
    })
}

/// A CFString's text (`None` for another type).
fn string_of(r: CFTypeRef) -> Option<String> {
    unsafe {
        if r.is_null() || CFGetTypeID(r) != CFStringGetTypeID() {
            return None;
        }
        let n = CFStringGetLength(r);
        let mut buf = vec![0u16; n as usize];
        CFStringGetCharacters(
            r,
            CFRange {
                location: 0,
                length: n,
            },
            buf.as_mut_ptr(),
        );
        Some(String::from_utf16_lossy(&buf))
    }
}

/// Whether this process may post events and read other elements (Privacy & Security →
/// Accessibility, granted to the terminal that runs the capture).
pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

fn pid() -> i32 {
    unsafe { getpid() }
}

// AXValueType
const AX_POINT: u32 = 1;
const AX_SIZE: u32 = 2;
const AX_RECT: u32 = 3;
const AX_RANGE: u32 = 4;

/// AX reads of this process's own elements. Call from a helper thread ([`Input::off_main`]).
pub struct Ax {
    app: Cf,
}

impl Ax {
    pub fn own() -> Ax {
        Ax {
            app: Cf(unsafe { AXUIElementCreateApplication(pid()) }),
        }
    }

    fn attr(el: &Cf, name: &str) -> Option<Cf> {
        let mut out: CFTypeRef = std::ptr::null();
        let rc = unsafe { AXUIElementCopyAttributeValue(el.0, cfstr(name).0, &mut out) };
        if rc == 0 { Cf::new(out) } else { None }
    }

    fn children(el: &Cf, name: &str) -> Vec<Cf> {
        let Some(a) = Self::attr(el, name) else {
            return vec![];
        };
        unsafe {
            if CFGetTypeID(a.0) != CFArrayGetTypeID() {
                return vec![];
            }
            (0..CFArrayGetCount(a.0))
                .map(|i| {
                    let r = CFArrayGetValueAtIndex(a.0, i);
                    objc_retain_cf(r)
                })
                .collect()
        }
    }

    /// The element whose `AXIdentifier` is `id` (an NSView `identifier`), in this app's windows.
    pub fn find(&self, id: &str) -> Result<Cf, String> {
        let mut stack = Self::children(&self.app, "AXWindows");
        while let Some(e) = stack.pop() {
            if Self::attr(&e, "AXIdentifier")
                .and_then(|s| string_of(s.0))
                .as_deref()
                == Some(id)
            {
                return Ok(e);
            }
            stack.extend(Self::children(&e, "AXChildren"));
        }
        Err(format!("no accessibility element with identifier {id}"))
    }

    pub fn value(el: &Cf) -> Option<String> {
        Self::attr(el, "AXValue").and_then(|v| string_of(v.0))
    }

    pub fn role(el: &Cf) -> Option<String> {
        Self::attr(el, "AXRole").and_then(|v| string_of(v.0))
    }

    /// `AXSelectedTextRange` as `(location, length)` in UTF-16 units.
    pub fn selected_range(el: &Cf) -> Option<(usize, usize)> {
        let v = Self::attr(el, "AXSelectedTextRange")?;
        let mut r = CFRange::default();
        unsafe { AXValueGetValue(v.0, AX_RANGE, &mut r as *mut _ as *mut c_void) }
            .then_some((r.location as usize, r.length as usize))
    }

    pub fn focused(el: &Cf) -> bool {
        Self::attr(el, "AXFocused").is_some_and(|v| unsafe {
            CFGetTypeID(v.0) == CFBooleanGetTypeID() && CFBooleanGetValue(v.0)
        })
    }

    /// `AXBoundsForRange` (UTF-16 range): screen coordinates, origin top left, as CGEvent takes them.
    pub fn bounds_for_range(el: &Cf, location: usize, length: usize) -> Option<CGRect> {
        let r = CFRange {
            location: location as isize,
            length: length as isize,
        };
        let param = Cf(unsafe { AXValueCreate(AX_RANGE, &r as *const _ as *const c_void) });
        let mut out: CFTypeRef = std::ptr::null();
        let rc = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                el.0,
                cfstr("AXBoundsForRange").0,
                param.0,
                &mut out,
            )
        };
        let out = if rc == 0 { Cf::new(out)? } else { return None };
        let mut rect = CGRect::default();
        unsafe { AXValueGetValue(out.0, AX_RECT, &mut rect as *mut _ as *mut c_void) }
            .then_some(rect)
    }

    /// `AXPosition` and `AXSize`: the element's frame in screen coordinates.
    pub fn frame(el: &Cf) -> Option<CGRect> {
        let (p, s) = (Self::attr(el, "AXPosition")?, Self::attr(el, "AXSize")?);
        let mut r = CGRect::default();
        unsafe {
            (AXValueGetValue(p.0, AX_POINT, &mut r.origin as *mut _ as *mut c_void)
                && AXValueGetValue(s.0, AX_SIZE, &mut r.size as *mut _ as *mut c_void))
            .then_some(r)
        }
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
}

unsafe fn objc_retain_cf(r: CFTypeRef) -> Cf {
    Cf(unsafe { CFRetain(r) })
}

/// macOS virtual key code (`kVK_*`, ANSI layout) of an X keysym name as `keys.json` writes them.
pub fn keycode(name: &str) -> Result<u16, String> {
    Ok(match name {
        "Left" => 0x7B,
        "Right" => 0x7C,
        "Down" => 0x7D,
        "Up" => 0x7E,
        "Home" => 0x73,
        "End" => 0x77,
        "BackSpace" => 0x33,
        "Delete" => 0x75,
        "Tab" => 0x30,
        "Escape" => 0x35,
        "Return" => 0x24,
        "space" => 0x31,
        s if s.chars().count() == 1 => {
            let c = s.chars().next().unwrap();
            match char_key(c) {
                Some((k, false)) => k,
                _ => return Err(format!("no key code for {name}")),
            }
        }
        o => return Err(format!("no key code for {o}")),
    })
}

/// The US (ANSI) key of a printable ASCII character: `(key code, shift)`.
pub fn char_key(c: char) -> Option<(u16, bool)> {
    const PLAIN: &str = "asdfhgzxcv\0bqweryt123465=97-80]ou[ip\0lj'k;\\,/nm.\0 `";
    const SHIFTED: &str = "ASDFHGZXCV\0BQWERYT!@#$^%+(&_*)}OU{IP\0LJ\"K:|<?NM>\0 ~";
    if c == '\0' {
        return None;
    }
    if let Some(i) = PLAIN.chars().position(|p| p == c) {
        return Some((i as u16, false));
    }
    SHIFTED
        .chars()
        .position(|p| p == c)
        .map(|i| (i as u16, true))
}

// CGEventFlags
const SHIFT: u64 = 0x0002_0000;
const CONTROL: u64 = 0x0004_0000;
const ALTERNATE: u64 = 0x0008_0000;
const COMMAND: u64 = 0x0010_0000;
// CGEventType
const LEFT_DOWN: u32 = 1;
const LEFT_UP: u32 = 2;
const MOUSE_MOVED: u32 = 5;
const LEFT_DRAGGED: u32 = 6;
const OTHER_DOWN: u32 = 25;
const OTHER_UP: u32 = 26;
// CGEventField kCGMouseEventClickState
const CLICK_STATE: u32 = 1;
/// kCGSessionEventTap
const SESSION_TAP: u32 = 1;
/// kVK_F20: the barrier marker (no app binds it).
const MARKER: u16 = 0x5A;
// NSEventType
const NS_KEY_DOWN: usize = 10;
const NS_KEY_UP: usize = 11;
const NS_LEFT_MOUSE_DOWN: usize = 1;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_msgSend();
}

/// `-[NSEvent type]` (`type` is a Rust keyword, so `msg_send!` cannot name it).
fn event_type(e: &AnyObject) -> usize {
    let sel = objc2::runtime::Sel::register(c"type");
    unsafe {
        let f: unsafe extern "C" fn(*const AnyObject, objc2::runtime::Sel) -> usize =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(e, sel)
    }
}

/// The input side of a capture: posts events, dispatches the run loop, waits on barriers.
pub struct Input {
    src: Cf,
    markers: u32,
    /// A dispatched mouse press that carried modifiers (it must not: [`Input::mouse`] posts none).
    modified_press: Option<String>,
    /// The pointer position of the last mouse action.
    pointer: Option<CGPoint>,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn new() -> Input {
        // kCGEventSourceStatePrivate: the events carry only the modifiers we give them
        Input {
            src: Cf(unsafe { CGEventSourceCreate(-1) }),
            markers: 0,
            modified_press: None,
            pointer: None,
        }
    }

    /// Dispatches every event for about `ms` milliseconds; swallows (and counts) barrier markers.
    pub fn pump(&mut self, ms: u64) {
        unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let until: Retained<AnyObject> =
                msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: ms as f64 / 1000.0];
            let mode = NSString::from_str("kCFRunLoopDefaultMode");
            loop {
                let ev: Option<Retained<AnyObject>> = msg_send![&*app, nextEventMatchingMask: usize::MAX, untilDate: &*until, inMode: &*mode, dequeue: true];
                let Some(e) = ev else { break };
                let ty = event_type(&e);
                if ty == NS_KEY_DOWN || ty == NS_KEY_UP {
                    let code: u16 = msg_send![&*e, keyCode];
                    if code == MARKER {
                        if ty == NS_KEY_UP {
                            self.markers += 1;
                        }
                        continue;
                    }
                }
                if ty == NS_LEFT_MOUSE_DOWN {
                    let flags: usize = msg_send![&*e, modifierFlags];
                    // device-independent modifiers (Caps Lock and up), not the device bits below
                    if flags & 0xffff_0000 != 0 && self.modified_press.is_none() {
                        self.modified_press = Some(format!("{flags:#x}"));
                    }
                }
                let _: () = msg_send![&*app, sendEvent: &*e];
            }
        }
    }

    /// Runs `f` on a helper thread while this (main) thread keeps dispatching: AX requests to the
    /// own process are answered by its main run loop.
    pub fn off_main<T: Send>(&mut self, f: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|s| {
            let h = s.spawn(f);
            while !h.is_finished() {
                self.pump(2);
            }
            h.join().expect("AX thread")
        })
    }

    /// Posts the marker (to this process, or through the session tap after mouse input, so it
    /// queues behind it) and dispatches until it arrives.
    fn barrier(&mut self, tap: bool) -> Result<(), String> {
        let want = self.markers + 1;
        for down in [true, false] {
            let e = Cf(unsafe { CGEventCreateKeyboardEvent(self.src.0, MARKER, down) });
            unsafe {
                if tap {
                    CGEventPost(SESSION_TAP, e.0)
                } else {
                    CGEventPostToPid(pid(), e.0)
                }
            }
        }
        let t0 = Instant::now();
        while self.markers < want {
            if t0.elapsed() > Duration::from_secs(20) {
                return Err(
                    "barrier: the marker never arrived (did the fixture window lose key status?)"
                        .into(),
                );
            }
            self.pump(5);
        }
        Ok(())
    }

    fn post_key(&self, code: u16, flags: u64, text: Option<&[u16]>) {
        for down in [true, false] {
            let e = Cf(unsafe { CGEventCreateKeyboardEvent(self.src.0, code, down) });
            unsafe {
                CGEventSetFlags(e.0, flags);
                if let Some(t) = text {
                    CGEventKeyboardSetUnicodeString(e.0, t.len(), t.as_ptr());
                }
                CGEventPostToPid(pid(), e.0);
            }
        }
    }

    /// One chord, then the barrier.
    pub fn key(&mut self, c: &Chord) -> Result<(), String> {
        let mut flags = 0;
        for (on, f) in [
            (c.shift, SHIFT),
            (c.ctrl, CONTROL),
            (c.alt, ALTERNATE),
            (c.cmd, COMMAND),
        ] {
            if on {
                flags |= f;
            }
        }
        self.post_key(keycode(&c.key)?, flags, None);
        self.barrier(false)
    }

    /// One key per character (US layout; another character goes as its text on key code 0).
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        for ch in text.chars() {
            match char_key(ch) {
                Some((k, shift)) => self.post_key(k, if shift { SHIFT } else { 0 }, None),
                None => {
                    let u: Vec<u16> = ch.encode_utf16(&mut [0; 2]).to_vec();
                    self.post_key(0, 0, Some(&u));
                }
            }
        }
        self.barrier(false)
    }

    fn mouse(&mut self, ty: u32, at: CGPoint, clicks: u32) {
        // kCGMouseButtonLeft, or kCGMouseButtonCenter for the "other" button events
        let button = if matches!(ty, OTHER_DOWN | OTHER_UP) {
            2
        } else {
            0
        };
        let e = Cf(unsafe { CGEventCreateMouseEvent(self.src.0, ty, at, button) });
        unsafe {
            CGEventSetIntegerValueField(e.0, CLICK_STATE, clicks as i64);
            // no modifiers: a mouse event otherwise takes the source's, and a click that carries a
            // Shift left over from a pin's keys extends the selection (seen on the first capture)
            CGEventSetFlags(e.0, 0);
            CGEventPost(SESSION_TAP, e.0);
        }
        self.pointer = Some(at);
    }

    /// Fails when a press arrived with modifiers or the pointer is not where the last mouse action
    /// left it (someone used the keyboard or the mouse during the capture).
    fn check_mouse(&mut self) -> Result<(), String> {
        if let Some(f) = self.modified_press.take() {
            return Err(format!(
                "a click arrived with modifier flags {f}; re-run without touching the keyboard"
            ));
        }
        let Some(want) = self.pointer else {
            return Ok(());
        };
        let e = Cf(unsafe { CGEventCreate(std::ptr::null()) });
        let now = unsafe { CGEventGetLocation(e.0) };
        if (now.x - want.x).abs() > 1.0 || (now.y - want.y).abs() > 1.0 {
            return Err(format!(
                "the pointer moved during the capture ({now:?}, expected {want:?}); re-run without touching the mouse"
            ));
        }
        Ok(())
    }

    /// `count` clicks of `button` (1 left, 2 middle) at `at` (screen coordinates, origin top left).
    /// Waits 0.7 s first, so a sequence never joins the previous one (the GTK and Qt captures do too).
    pub fn click(&mut self, at: CGPoint, count: u32, button: u32) -> Result<(), String> {
        let (down, up) = if button == 2 {
            (OTHER_DOWN, OTHER_UP)
        } else {
            (LEFT_DOWN, LEFT_UP)
        };
        self.pump(700);
        self.mouse(MOUSE_MOVED, at, 0);
        for i in 1..=count {
            self.mouse(down, at, i);
            self.mouse(up, at, i);
            // each press and release is dispatched before the next press (the click count is
            // explicit, so this does not break a multi-click)
            self.barrier(true)?;
        }
        self.check_mouse()
    }

    /// `count` presses at `from`, the last one held while the pointer moves to `to` in six steps.
    pub fn drag(&mut self, from: CGPoint, to: CGPoint, count: u32) -> Result<(), String> {
        self.pump(700);
        self.mouse(MOUSE_MOVED, from, 0);
        for i in 1..=count {
            self.mouse(LEFT_DOWN, from, i);
            if i < count {
                self.mouse(LEFT_UP, from, i);
                // as in `click`: an earlier press still pending under load turned the held press
                // into NSTextView's drag of the selected text (a modal loop) on one capture
                self.barrier(true)?;
            }
        }
        for s in 1..=6 {
            let t = s as f64 / 6.0;
            let p = CGPoint {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
            self.mouse(LEFT_DRAGGED, p, count);
        }
        self.mouse(LEFT_UP, to, count);
        self.barrier(true)?;
        self.check_mouse()
    }
}

/// AppKit's own record of which end of an NSTextView's selection is the anchor: the 2-bit
/// `selectionExtensionAnchor` of `NSTextViewSharedData._sdFlags` (private; 0 none yet, 1 the end,
/// 2 the start). Neither AX nor a public API exposes the direction; this is read for the
/// checkpoints only, never written. The bit position comes from the ivar's type encoding.
pub fn extension_anchor(text_view: &AnyObject) -> Result<u8, String> {
    unsafe {
        let tv_class: &AnyClass = class!(NSTextView);
        let shared_iv = tv_class
            .instance_variable(c"_sharedData")
            .ok_or("NSTextView has no _sharedData")?;
        let base = text_view as *const AnyObject as *const u8;
        let shared = *(base.offset(shared_iv.offset()) as *const *const AnyObject);
        if shared.is_null() {
            return Err("NSTextView without shared data".into());
        }
        let shared = &*shared;
        let flags_iv = shared
            .class()
            .instance_variable(c"_sdFlags")
            .ok_or("NSTextViewSharedData has no _sdFlags")?;
        let enc = flags_iv.type_encoding().to_string_lossy().into_owned();
        let (bit, width, total) = bitfield(&enc, "selectionExtensionAnchor")
            .ok_or_else(|| format!("no selectionExtensionAnchor in {enc}"))?;
        let p = (shared as *const AnyObject as *const u8).offset(flags_iv.offset());
        let mut word: u64 = 0;
        for i in 0..total.div_ceil(8).min(8) {
            word |= (*p.add(i) as u64) << (8 * i);
        }
        Ok(((word >> bit) & ((1 << width) - 1)) as u8)
    }
}

/// `(first bit, width, total bits)` of a field in a bitfield struct encoding
/// (`{name="a"b1"b"b2…}`); clang allocates bitfields from the low bit on x86_64 and arm64.
pub fn bitfield(enc: &str, field: &str) -> Option<(usize, usize, usize)> {
    let parts: Vec<&str> = enc.split('"').collect();
    let mut bit = 0;
    let mut found = None;
    let mut i = 1;
    while i + 1 < parts.len() {
        let width: usize = parts[i + 1]
            .trim_start_matches('b')
            .trim_end_matches('}')
            .parse()
            .ok()?;
        if parts[i] == field {
            found = Some((bit, width));
        }
        bit += width;
        i += 2;
    }
    found.map(|(b, w)| (b, w, bit))
}

/// `kern.osproductversion` (`15.7.5`).
pub fn macos_version() -> String {
    let mut buf = [0u8; 64];
    let mut len = buf.len();
    let rc = unsafe {
        sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return "unknown".into();
    }
    CStr::from_bytes_until_nul(&buf)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `NSAppKitVersionNumber` (`2575.7` on macOS 15.7).
pub fn appkit_version() -> f64 {
    unsafe { NSAppKitVersionNumber }
}

/// The current keyboard input source (`com.apple.keylayout.ABC`): the key codes above assume a US
/// (ANSI) layout.
pub fn input_source() -> String {
    unsafe {
        let Some(src) = Cf::new(TISCopyCurrentKeyboardInputSource()) else {
            return String::new();
        };
        string_of(TISGetInputSourceProperty(src.0, kTISPropertyInputSourceID)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitfield_positions() {
        let enc = r#"{__sdFlags="selectable"b1"editable"b1"selectionAffinity"b2"selectionExtensionAnchor"b2"last"b1}"#;
        assert_eq!(bitfield(enc, "selectionExtensionAnchor"), Some((4, 2, 7)));
        assert_eq!(bitfield(enc, "missing"), None);
    }

    #[test]
    fn us_keys() {
        assert_eq!(char_key('a'), Some((0x00, false)));
        assert_eq!(char_key('Z'), Some((0x06, true)));
        assert_eq!(char_key(' '), Some((0x31, false)));
        assert_eq!(char_key('b'), Some((0x0B, false)));
        assert_eq!(char_key('é'), None);
        assert_eq!(keycode("e"), Ok(0x0E));
        assert_eq!(keycode("BackSpace"), Ok(0x33));
    }
}

/// WG3s: recording real scroll input (a listen-only event tap) and posting scroll events.
pub mod scroll_events {
    use super::{CFTypeRef, Cf};
    use std::ffi::c_void;

    type TapCallback = extern "C" fn(
        proxy: *const c_void,
        ty: u32,
        event: CFTypeRef,
        user: *mut c_void,
    ) -> CFTypeRef;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            mask: u64,
            cb: TapCallback,
            user: *mut c_void,
        ) -> CFTypeRef;
        fn CGEventGetIntegerValueField(e: CFTypeRef, field: u32) -> i64;
        fn CGEventGetDoubleValueField(e: CFTypeRef, field: u32) -> f64;
        fn CGEventGetTimestamp(e: CFTypeRef) -> u64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFMachPortCreateRunLoopSource(
            alloc: CFTypeRef,
            port: CFTypeRef,
            order: isize,
        ) -> CFTypeRef;
        fn CFRunLoopGetCurrent() -> CFTypeRef;
        fn CFRunLoopAddSource(rl: CFTypeRef, src: CFTypeRef, mode: CFTypeRef);
        fn CFRunLoopRunInMode(mode: CFTypeRef, seconds: f64, return_after_source: bool) -> i32;
        static kCFRunLoopDefaultMode: CFTypeRef;
    }

    // CGEventField
    pub const DELTA_AXIS_1: u32 = 11;
    pub const DELTA_AXIS_2: u32 = 12;
    pub const POINT_DELTA_AXIS_1: u32 = 96;
    pub const POINT_DELTA_AXIS_2: u32 = 97;
    pub const IS_CONTINUOUS: u32 = 88;
    pub const SCROLL_PHASE: u32 = 99;
    pub const MOMENTUM_PHASE: u32 = 123;
    const SCROLL_WHEEL: u32 = 22;

    /// One scroll event as the OS delivered it (device sign: positive axis 1 scrolls the content
    /// down, i.e. towards its start, with natural scrolling's inversion already applied).
    #[derive(Clone, Debug, PartialEq)]
    pub struct Recorded {
        /// Milliseconds from the first recorded event.
        pub t_ms: f64,
        /// `kCGScrollWheelEventScrollPhase`: 1 began, 2 changed, 4 ended, 8 cancelled, 128 may begin; 0 none.
        pub phase: i64,
        /// `kCGScrollWheelEventMomentumPhase`: 1 begin, 2 continue, 3 end; 0 none.
        pub momentum: i64,
        pub continuous: bool,
        /// Point deltas (axis 1 vertical, axis 2 horizontal).
        pub dy: f64,
        pub dx: f64,
        /// Line deltas.
        pub lines_y: i64,
        pub lines_x: i64,
    }

    struct Tap {
        events: Vec<Recorded>,
        t0: Option<u64>,
    }

    extern "C" fn on_event(
        _p: *const c_void,
        ty: u32,
        e: CFTypeRef,
        user: *mut c_void,
    ) -> CFTypeRef {
        if ty == SCROLL_WHEEL {
            let tap = unsafe { &mut *(user as *mut Tap) };
            let ts = unsafe { CGEventGetTimestamp(e) };
            let t0 = *tap.t0.get_or_insert(ts);
            unsafe {
                tap.events.push(Recorded {
                    t_ms: (ts.saturating_sub(t0)) as f64 / 1e6,
                    phase: CGEventGetIntegerValueField(e, SCROLL_PHASE),
                    momentum: CGEventGetIntegerValueField(e, MOMENTUM_PHASE),
                    continuous: CGEventGetIntegerValueField(e, IS_CONTINUOUS) != 0,
                    dy: CGEventGetDoubleValueField(e, POINT_DELTA_AXIS_1),
                    dx: CGEventGetDoubleValueField(e, POINT_DELTA_AXIS_2),
                    lines_y: CGEventGetIntegerValueField(e, DELTA_AXIS_1),
                    lines_x: CGEventGetIntegerValueField(e, DELTA_AXIS_2),
                });
            }
        }
        e
    }

    /// Records scroll events (a listen-only session tap: needs Input Monitoring for the terminal)
    /// until a momentum stream ends, or 1.5 s pass without an event after the first one, or
    /// `timeout_s` without any. Call on a thread with no other run-loop work.
    pub fn record(timeout_s: f64) -> Result<Vec<Recorded>, String> {
        let mut tap = Box::new(Tap {
            events: Vec::new(),
            t0: None,
        });
        let user = &mut *tap as *mut Tap as *mut c_void;
        // kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionListenOnly
        let port = unsafe { CGEventTapCreate(1, 0, 1, 1u64 << SCROLL_WHEEL, on_event, user) };
        if port.is_null() {
            return Err("CGEventTapCreate failed: Input Monitoring is missing for the terminal (System Settings > Privacy & Security > Input Monitoring)".into());
        }
        let port = Cf(port);
        let src = Cf(unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), port.0, 0) });
        unsafe { CFRunLoopAddSource(CFRunLoopGetCurrent(), src.0, kCFRunLoopDefaultMode) };
        let start = std::time::Instant::now();
        let mut last_count = 0;
        let mut last_change = std::time::Instant::now();
        loop {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false) };
            let n = tap.events.len();
            if n != last_count {
                last_count = n;
                last_change = std::time::Instant::now();
            }
            let momentum_ended = tap.events.last().is_some_and(|e| e.momentum == 3);
            if momentum_ended || (n > 0 && last_change.elapsed().as_secs_f64() > 1.5) {
                break;
            }
            if n == 0 && start.elapsed().as_secs_f64() > timeout_s {
                return Err(format!("no scroll event in {timeout_s} s"));
            }
        }
        Ok(std::mem::take(&mut tap.events))
    }

    /// The recording as the fixture JSON (`conformance/scroll/<name>.json`).
    pub fn to_json(events: &[Recorded], note: &str) -> String {
        let rows: Vec<String> = events
            .iter()
            .map(|e| {
                format!(
                    "  {{\"t_ms\":{:.3},\"phase\":{},\"momentum\":{},\"continuous\":{},\"dy\":{},\"dx\":{},\"lines_y\":{},\"lines_x\":{}}}",
                    e.t_ms, e.phase, e.momentum, e.continuous, e.dy, e.dx, e.lines_y, e.lines_x
                )
            })
            .collect();
        format!(
            "{{\n \"schema\": \"svelters-scroll-recording/v1\",\n \"note\": {},\n \"events\": [\n{}\n ]\n}}\n",
            serde_json::to_string(note).unwrap(),
            rows.join(",\n")
        )
    }
}

/// WG3s: posting scroll events through the session tap (the window server routes them to the
/// window under the pointer, as for a real wheel or trackpad).
pub mod scroll_post {
    use super::{CFTypeRef, CGPoint, Cf};

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateScrollWheelEvent2(
            src: CFTypeRef,
            units: u32,
            count: u32,
            w1: i32,
            w2: i32,
            w3: i32,
        ) -> CFTypeRef;
        fn CGEventCreateMouseEvent(src: CFTypeRef, ty: u32, at: CGPoint, button: u32) -> CFTypeRef;
        fn CGEventSetIntegerValueField(e: CFTypeRef, field: u32, v: i64);
        fn CGEventSetFlags(e: CFTypeRef, flags: u64);
        fn CGEventPost(tap: u32, e: CFTypeRef);
        fn CGEventSetTimestamp(e: CFTypeRef, ns: u64);
    }

    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock: u32) -> u64;
    }

    /// Now on the clock CGEvent timestamps use (nanoseconds since boot, CLOCK_UPTIME_RAW).
    pub fn now_ns() -> u64 {
        unsafe { clock_gettime_nsec_np(8) }
    }

    const SESSION_TAP: u32 = 1;
    const UNIT_PIXEL: u32 = 0;
    const UNIT_LINE: u32 = 1;
    const SHIFT: u64 = 0x0002_0000;

    /// Moves the pointer (no button) so the scroll goes to the view under it.
    pub fn move_to(at: CGPoint) {
        let e = Cf(unsafe { CGEventCreateMouseEvent(std::ptr::null(), 5, at, 0) });
        unsafe {
            CGEventSetFlags(e.0, 0);
            CGEventPost(SESSION_TAP, e.0);
        }
    }

    /// One mouse-wheel event of `lines` notches (`[x, y]` in the OS's sign), a discrete wheel.
    pub fn wheel(lines: [i32; 2], shift: bool) {
        let e = Cf(unsafe {
            CGEventCreateScrollWheelEvent2(std::ptr::null(), UNIT_LINE, 2, lines[1], lines[0], 0)
        });
        unsafe {
            CGEventSetIntegerValueField(e.0, super::scroll_events::IS_CONTINUOUS, 0);
            CGEventSetFlags(e.0, if shift { SHIFT } else { 0 });
            CGEventPost(SESSION_TAP, e.0);
        }
    }

    /// One continuous (trackpad) event: point deltas `[x, y]` in the OS's sign, with its gesture
    /// phase (`kCGScrollWheelEventScrollPhase`) and momentum phase.
    pub fn continuous(delta: [f64; 2], phase: i64, momentum: i64) {
        continuous_at(delta, phase, momentum, None)
    }

    /// [`continuous`] stamped with `at_ns` (the scripted time on [`now_ns`]'s clock), so AppKit's
    /// velocity (computed from event timestamps) does not depend on when the event was delivered.
    pub fn continuous_at(delta: [f64; 2], phase: i64, momentum: i64, at_ns: Option<u64>) {
        let (dx, dy) = (delta[0].round() as i32, delta[1].round() as i32);
        let e = Cf(unsafe {
            CGEventCreateScrollWheelEvent2(std::ptr::null(), UNIT_PIXEL, 2, dy, dx, 0)
        });
        unsafe {
            CGEventSetIntegerValueField(e.0, super::scroll_events::IS_CONTINUOUS, 1);
            CGEventSetIntegerValueField(e.0, super::scroll_events::SCROLL_PHASE, phase);
            CGEventSetIntegerValueField(e.0, super::scroll_events::MOMENTUM_PHASE, momentum);
            CGEventSetFlags(e.0, 0);
            if let Some(t) = at_ns {
                CGEventSetTimestamp(e.0, t);
            }
            CGEventPost(SESSION_TAP, e.0);
        }
    }

    /// A recorded momentum stream (`conformance/scroll/<name>.json`): `(t_ms from its first
    /// momentum event, [dx, dy], momentum phase)`, in the OS's sign.
    pub fn recorded_momentum(path: &std::path::Path) -> Result<Vec<(f64, [f64; 2], i64)>, String> {
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let ev = v["events"].as_array().ok_or("no events")?;
        let m: Vec<&serde_json::Value> = ev
            .iter()
            .filter(|e| e["momentum"].as_i64().unwrap_or(0) != 0)
            .collect();
        let t0 = m.first().and_then(|e| e["t_ms"].as_f64()).unwrap_or(0.0);
        Ok(m.iter()
            .map(|e| {
                (
                    e["t_ms"].as_f64().unwrap_or(0.0) - t0,
                    [
                        e["dx"].as_f64().unwrap_or(0.0),
                        e["dy"].as_f64().unwrap_or(0.0),
                    ],
                    e["momentum"].as_i64().unwrap_or(0),
                )
            })
            .collect())
    }
}
