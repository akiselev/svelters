//! The user-path injector for the X11 toolkits (widget survey §4.1 gap 2): real X events through
//! the XTEST extension, on the capture's private Xvfb. The toolkit receives them from the X server
//! like keyboard and mouse input (XKB keymap, XI2 devices, server timestamps for multi-click), not
//! through a toolkit-internal synthesizer. GTK4 has no public key-synthesis API; Qt's `sendEvent`
//! path (`verify.cpp.in` `ut_key`) skips the platform plugin, which XTEST does not.
//!
//! The toolkit runs on this thread, so after every injected event the caller's `pump` runs its
//! loop; a round trip to the server first makes sure the events were delivered.
//!
//! WG3b adds what pins 4-6 need: typing text (keysyms from the server's keymap, Shift for the
//! second column), the middle button, dead keys (the Xvfb keymap has none: they are bound to unused
//! keycodes once per display, as `xdotool` does), and the X selections: reading CLIPBOARD and
//! PRIMARY with `ConvertSelection` like any other client (the toolkit answers while the caller's
//! loop is pumped), clearing them, and seeding CLIPBOARD from a separate thread with its own
//! connection (another client owns it, so a toolkit that blocks while it reads, as Qt's xcb
//! clipboard does, still gets its answer).

use crate::pins::Chord;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, InputFocus, PropMode,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
const BUTTON_PRESS: u8 = 4;
const BUTTON_RELEASE: u8 = 5;
const MOTION_NOTIFY: u8 = 6;

/// X keysyms of the names the key table uses (`/usr/include/X11/keysymdef.h`); a single character
/// is its own keysym ([`char_keysym`]).
pub fn keysym(name: &str) -> Option<u32> {
    Some(match name {
        "Left" => 0xff51,
        "Up" => 0xff52,
        "Right" => 0xff53,
        "Down" => 0xff54,
        "Home" => 0xff50,
        "End" => 0xff57,
        "BackSpace" => 0xff08,
        "Delete" => 0xffff,
        "Tab" => 0xff09,
        "Escape" => 0xff1b,
        "Return" => 0xff0d,
        "Insert" => 0xff63,
        "space" => 0x20,
        "Control_L" => 0xffe3,
        "Shift_L" => 0xffe1,
        "Alt_L" => 0xffe9,
        "Super_L" => 0xffeb,
        "dead_grave" => 0xfe50,
        "dead_acute" => 0xfe51,
        "dead_diaeresis" => 0xfe57,
        s if s.chars().count() == 1 => char_keysym(s.chars().next().unwrap()),
        _ => return None,
    })
}

/// The keysym that types `c`: Latin-1 keysyms equal the code point, others are 0x0100_0000 + it.
pub fn char_keysym(c: char) -> u32 {
    match c as u32 {
        n @ (0x20..=0x7e | 0xa0..=0xff) => n,
        n => 0x0100_0000 + n,
    }
}

/// Dead keys the pins use; bound to unused keycodes when the keymap has none.
const DEAD_KEYS: [u32; 3] = [0xfe50, 0xfe51, 0xfe57];

/// A CLIPBOARD owner on its own connection and thread (`XInput::seed_clipboard`).
struct Seed {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Seed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub struct XInput {
    conn: RustConnection,
    root: Window,
    win: Window,
    /// keysym -> (keycode, needs Shift).
    codes: HashMap<u32, (u8, bool)>,
    /// Keycodes without keysyms (for dead keys).
    spare: Vec<u8>,
    /// The requestor window of selection reads.
    req: Window,
    seed: Option<Seed>,
    /// Time to let pass before a click sequence, so it never joins the previous one (above every
    /// toolkit's double-click time).
    pub settle_ms: u64,
    /// Loop time after each injected event.
    pub pump_ms: u64,
}

fn e<T: std::fmt::Display>(x: T) -> String {
    x.to_string()
}

fn intern(conn: &RustConnection, name: &str) -> Result<u32, String> {
    Ok(conn
        .intern_atom(false, name.as_bytes())
        .map_err(e)?
        .reply()
        .map_err(e)?
        .atom)
}

fn input_only_window(conn: &RustConnection, root: Window) -> Result<Window, String> {
    let w = conn.generate_id().map_err(e)?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        w,
        root,
        -10,
        -10,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        COPY_FROM_PARENT,
        &CreateWindowAux::new(),
    )
    .map_err(e)?;
    Ok(w)
}

/// Owns CLIPBOARD with `text` until `stop`, or until another client takes it.
fn serve_clipboard(
    text: &str,
    stop: &AtomicBool,
    ready: &std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let (conn, screen) = RustConnection::connect(None).map_err(e)?;
    let root = conn.setup().roots[screen].root;
    let win = input_only_window(&conn, root)?;
    let clipboard = intern(&conn, "CLIPBOARD")?;
    let targets = intern(&conn, "TARGETS")?;
    let utf8 = intern(&conn, "UTF8_STRING")?;
    let text_atom = intern(&conn, "TEXT")?;
    let string: u32 = AtomEnum::STRING.into();
    conn.set_selection_owner(win, clipboard, CURRENT_TIME)
        .map_err(e)?;
    let owner = conn
        .get_selection_owner(clipboard)
        .map_err(e)?
        .reply()
        .map_err(e)?
        .owner;
    if owner != win {
        return Err("could not own CLIPBOARD".into());
    }
    let _ = ready.send(Ok(()));
    let answer = |r: &SelectionRequestEvent| -> Result<(), String> {
        let prop = if r.property == NONE {
            r.target
        } else {
            r.property
        };
        let ok = if r.target == targets {
            conn.change_property32(
                PropMode::REPLACE,
                r.requestor,
                prop,
                AtomEnum::ATOM,
                &[targets, utf8, text_atom, string],
            )
            .map_err(e)?;
            true
        } else if [utf8, text_atom, string].contains(&r.target) {
            let ty = if r.target == string { string } else { utf8 };
            conn.change_property8(PropMode::REPLACE, r.requestor, prop, ty, text.as_bytes())
                .map_err(e)?;
            true
        } else {
            false
        };
        let ev = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: r.time,
            requestor: r.requestor,
            selection: r.selection,
            target: r.target,
            property: if ok { prop } else { NONE },
        };
        conn.send_event(false, r.requestor, EventMask::NO_EVENT, ev)
            .map_err(e)?;
        conn.flush().map_err(e)?;
        Ok(())
    };
    while !stop.load(Ordering::SeqCst) {
        match conn.poll_for_event().map_err(e)? {
            Some(Event::SelectionRequest(r)) => answer(&r)?,
            Some(Event::SelectionClear(_)) => break,
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    Ok(())
}

impl XInput {
    /// Connects to `$DISPLAY`, waits (pumping the toolkit) until a top-level window titled `title`
    /// is mapped, binds the dead keys and gives the window the X input focus.
    pub fn attach(title: &str, pump: &mut dyn FnMut(u64)) -> Result<XInput, String> {
        let (conn, screen) = RustConnection::connect(None).map_err(e)?;
        let root = conn.setup().roots[screen].root;
        conn.xtest_get_version(2, 2)
            .map_err(e)?
            .reply()
            .map_err(|err| format!("no XTEST extension: {err}"))?;
        let (min, max) = (conn.setup().min_keycode, conn.setup().max_keycode);
        let map = conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(e)?
            .reply()
            .map_err(e)?;
        let per = map.keysyms_per_keycode as usize;
        let mut codes = HashMap::new();
        // First the unshifted column, so `a` maps to the key that types it without Shift.
        for col in 0..per.min(2) {
            for (i, chunk) in map.keysyms.chunks(per).enumerate() {
                if let Some(&sym) = chunk.get(col) {
                    if sym != 0 {
                        codes.entry(sym).or_insert((min + i as u8, col == 1));
                    }
                }
            }
        }
        let spare: Vec<u8> = map
            .keysyms
            .chunks(per)
            .enumerate()
            .filter(|(_, c)| c.iter().all(|&s| s == 0))
            .map(|(i, _)| min + i as u8)
            .collect();
        let req = input_only_window(&conn, root)?;
        let mut x = XInput {
            conn,
            root,
            win: 0,
            codes,
            spare,
            req,
            seed: None,
            settle_ms: 700,
            pump_ms: 40,
        };
        let pump_ms = std::env::var("NA_PUMP_MS")
            .ok()
            .and_then(|v| v.parse().ok());
        if let Some(ms) = pump_ms {
            x.pump_ms = ms;
        }
        for _ in 0..200 {
            if let Some(w) = x.find_window(title)? {
                x.win = w;
                break;
            }
            pump(25);
        }
        if x.win == 0 {
            return Err(format!("no mapped top-level window titled {title:?}"));
        }
        x.bind_dead_keys(per, pump)?;
        x.focus(pump)?;
        Ok(x)
    }

    /// Binds the dead keys the pins use to unused keycodes, once per display (the next attach
    /// finds them in the keymap). Toolkits reload their keymap on the MappingNotify that follows.
    fn bind_dead_keys(&mut self, per: usize, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        let mut bound = false;
        for sym in DEAD_KEYS {
            if self.codes.contains_key(&sym) {
                continue;
            }
            let code = self.spare.pop().ok_or("no unused keycode for a dead key")?;
            let mut syms = vec![0u32; per];
            syms[0] = sym;
            if per > 1 {
                syms[1] = sym;
            }
            self.conn
                .change_keyboard_mapping(1, code, per as u8, &syms)
                .map_err(e)?;
            self.codes.insert(sym, (code, false));
            bound = true;
        }
        if bound {
            self.sync()?;
            pump(200);
        }
        Ok(())
    }

    fn atom(&self, name: &str) -> Result<u32, String> {
        intern(&self.conn, name)
    }

    fn title_of(&self, w: Window) -> Result<Option<String>, String> {
        let utf8 = self.atom("UTF8_STRING")?;
        for (prop, ty) in [
            (self.atom("_NET_WM_NAME")?, utf8),
            (AtomEnum::WM_NAME.into(), AtomEnum::ANY.into()),
        ] {
            let r = self
                .conn
                .get_property(false, w, prop, ty, 0, 1024)
                .map_err(e)?
                .reply()
                .map_err(e)?;
            if !r.value.is_empty() {
                return Ok(Some(String::from_utf8_lossy(&r.value).into_owned()));
            }
        }
        Ok(None)
    }

    /// A mapped child of the root window with this title.
    pub fn find_window(&self, title: &str) -> Result<Option<Window>, String> {
        let tree = self
            .conn
            .query_tree(self.root)
            .map_err(e)?
            .reply()
            .map_err(e)?;
        for &w in tree.children.iter().rev() {
            let Ok(attrs) = self.conn.get_window_attributes(w).map_err(e)?.reply() else {
                continue;
            };
            if attrs.map_state != x11rb::protocol::xproto::MapState::VIEWABLE {
                continue;
            }
            if self.title_of(w)?.as_deref() == Some(title) {
                return Ok(Some(w));
            }
        }
        Ok(None)
    }

    pub fn window(&self) -> Window {
        self.win
    }

    /// Gives the window the X input focus (there is no window manager on the private display).
    pub fn focus(&self, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        self.conn
            .set_input_focus(InputFocus::PARENT, self.win, CURRENT_TIME)
            .map_err(e)?;
        self.sync()?;
        pump(100);
        Ok(())
    }

    /// The window's origin on the root window.
    pub fn origin(&self) -> Result<(i32, i32), String> {
        let r = self
            .conn
            .translate_coordinates(self.win, self.root, 0, 0)
            .map_err(e)?
            .reply()
            .map_err(e)?;
        Ok((r.dst_x as i32, r.dst_y as i32))
    }

    /// Flush and wait for the server: every injected event has been delivered to the clients.
    pub fn sync(&self) -> Result<(), String> {
        self.conn.flush().map_err(e)?;
        self.conn.get_input_focus().map_err(e)?.reply().map_err(e)?;
        Ok(())
    }

    fn fake(&self, ty: u8, detail: u8, x: i16, y: i16) -> Result<(), String> {
        self.conn
            .xtest_fake_input(ty, detail, CURRENT_TIME, self.root, x, y, 0)
            .map_err(e)?;
        Ok(())
    }

    fn code(&self, name: &str) -> Result<u8, String> {
        Ok(self.code_shift(name)?.0)
    }

    fn code_shift(&self, name: &str) -> Result<(u8, bool), String> {
        let sym = keysym(name).ok_or_else(|| format!("no keysym for {name}"))?;
        self.codes
            .get(&sym)
            .copied()
            .ok_or_else(|| format!("the X keymap has no key for {name}"))
    }

    /// Presses and releases the chord: modifiers down, key down, key up, modifiers up.
    pub fn key(&self, c: &Chord, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        let mut mods = Vec::new();
        for (on, n) in [
            (c.ctrl, "Control_L"),
            (c.alt, "Alt_L"),
            (c.cmd, "Super_L"),
            (c.shift, "Shift_L"),
        ] {
            if on {
                mods.push(self.code(n)?);
            }
        }
        let (k, sh) = self.code_shift(&c.key)?;
        if sh && !c.shift {
            mods.push(self.code("Shift_L")?);
        }
        for &m in &mods {
            self.fake(KEY_PRESS, m, 0, 0)?;
        }
        self.fake(KEY_PRESS, k, 0, 0)?;
        self.fake(KEY_RELEASE, k, 0, 0)?;
        for &m in mods.iter().rev() {
            self.fake(KEY_RELEASE, m, 0, 0)?;
        }
        self.sync()?;
        pump(self.pump_ms);
        Ok(())
    }

    /// Types `text`: one key press per character, with Shift where the keymap puts the character
    /// in the second column.
    pub fn type_text(&self, text: &str, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        let shift = self.code("Shift_L")?;
        for c in text.chars() {
            let (k, sh) = self
                .codes
                .get(&char_keysym(c))
                .copied()
                .ok_or_else(|| format!("the X keymap has no key for {c:?}"))?;
            if sh {
                self.fake(KEY_PRESS, shift, 0, 0)?;
            }
            self.fake(KEY_PRESS, k, 0, 0)?;
            self.fake(KEY_RELEASE, k, 0, 0)?;
            if sh {
                self.fake(KEY_RELEASE, shift, 0, 0)?;
            }
            self.sync()?;
            pump(self.pump_ms);
        }
        Ok(())
    }

    fn move_to(&self, (x, y): (f64, f64), pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        let (ox, oy) = self.origin()?;
        self.fake(
            MOTION_NOTIFY,
            0,
            (ox as f64 + x).round() as i16,
            (oy as f64 + y).round() as i16,
        )?;
        self.sync()?;
        pump(self.pump_ms / 2);
        Ok(())
    }

    fn button(&self, n: u8, down: bool, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        self.fake(if down { BUTTON_PRESS } else { BUTTON_RELEASE }, n, 0, 0)?;
        self.sync()?;
        pump(self.pump_ms / 2);
        Ok(())
    }

    /// `count` primary clicks at `at` (window coordinates), after letting any previous click
    /// sequence expire.
    pub fn click(
        &self,
        at: (f64, f64),
        count: u32,
        pump: &mut dyn FnMut(u64),
    ) -> Result<(), String> {
        self.click_button(at, count, 1, pump)
    }

    /// [`XInput::click`] with button `n` (2: middle).
    pub fn click_button(
        &self,
        at: (f64, f64),
        count: u32,
        n: u8,
        pump: &mut dyn FnMut(u64),
    ) -> Result<(), String> {
        pump(self.settle_ms);
        self.move_to(at, pump)?;
        for _ in 0..count {
            self.button(n, true, pump)?;
            self.button(n, false, pump)?;
        }
        pump(self.pump_ms);
        Ok(())
    }

    /// `count` presses at `from` (all but the last released), then a drag to `to` in a few motion
    /// steps, then the release.
    pub fn drag(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        count: u32,
        pump: &mut dyn FnMut(u64),
    ) -> Result<(), String> {
        pump(self.settle_ms);
        self.move_to(from, pump)?;
        for i in 0..count {
            self.button(1, true, pump)?;
            if i + 1 < count {
                self.button(1, false, pump)?;
            }
        }
        let steps = 6;
        for s in 1..=steps {
            let t = s as f64 / steps as f64;
            self.move_to(
                (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t),
                pump,
            )?;
        }
        self.button(1, false, pump)?;
        pump(self.pump_ms);
        Ok(())
    }

    /// Drops the owners of CLIPBOARD and PRIMARY (and stops a seeded owner), so a pin starts with
    /// empty selections.
    pub fn clear_selections(&mut self, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        self.seed = None;
        for name in ["CLIPBOARD", "PRIMARY"] {
            let a = self.atom(name)?;
            self.conn
                .set_selection_owner(NONE, a, CURRENT_TIME)
                .map_err(e)?;
        }
        self.sync()?;
        pump(self.pump_ms);
        Ok(())
    }

    /// Another client (a thread with its own connection) owns CLIPBOARD with `text`, until a
    /// toolkit takes it or this injector is dropped.
    pub fn seed_clipboard(&mut self, text: &str, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let (st, text) = (stop.clone(), text.to_string());
        let thread = std::thread::spawn(move || {
            if let Err(m) = serve_clipboard(&text, &st, &tx) {
                let _ = tx.send(Err(m));
            }
        });
        self.seed = Some(Seed {
            stop,
            thread: Some(thread),
        });
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "the clipboard owner did not start".to_string())??;
        // let the toolkits see the new owner (XFixes selection notify)
        pump(100);
        Ok(())
    }

    /// The text a selection (`CLIPBOARD`, `PRIMARY`) holds, read as UTF8_STRING like any client;
    /// "" without an owner or when the owner refuses. Pumps the toolkit while it answers.
    pub fn read_selection(&self, name: &str, pump: &mut dyn FnMut(u64)) -> Result<String, String> {
        let sel = self.atom(name)?;
        let utf8 = self.atom("UTF8_STRING")?;
        let incr = self.atom("INCR")?;
        let prop = self.atom("SVELTERS_NA_SELECTION")?;
        self.conn
            .convert_selection(self.req, sel, utf8, prop, CURRENT_TIME)
            .map_err(e)?;
        self.conn.flush().map_err(e)?;
        let start = Instant::now();
        loop {
            while let Some(ev) = self.conn.poll_for_event().map_err(e)? {
                let Event::SelectionNotify(n) = ev else {
                    continue;
                };
                if n.requestor != self.req || n.selection != sel {
                    continue;
                }
                if n.property == NONE {
                    return Ok(String::new());
                }
                let r = self
                    .conn
                    .get_property(true, self.req, n.property, AtomEnum::ANY, 0, 1 << 20)
                    .map_err(e)?
                    .reply()
                    .map_err(e)?;
                if r.type_ == incr {
                    return Err(format!("{name}: INCR transfers are not supported"));
                }
                return Ok(String::from_utf8_lossy(&r.value).into_owned());
            }
            if start.elapsed() > Duration::from_secs(3) {
                return Err(format!("{name}: the owner did not answer"));
            }
            pump(5);
        }
    }
}
