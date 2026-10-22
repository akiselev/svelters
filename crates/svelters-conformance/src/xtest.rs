//! The user-path injector for the X11 toolkits (widget survey §4.1 gap 2): real X events through
//! the XTEST extension, on the capture's private Xvfb. The toolkit receives them from the X server
//! like keyboard and mouse input (XKB keymap, XI2 devices, server timestamps for multi-click), not
//! through a toolkit-internal synthesizer. GTK4 has no public key-synthesis API; Qt's `sendEvent`
//! path (`verify.cpp.in` `ut_key`) skips the platform plugin, which XTEST does not.
//!
//! The toolkit runs on this thread, so after every injected event the caller's `pump` runs its
//! loop; a round trip to the server first makes sure the events were delivered.

use crate::pins::Chord;
use std::collections::HashMap;
use x11rb::CURRENT_TIME;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, InputFocus, Window};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
const BUTTON_PRESS: u8 = 4;
const BUTTON_RELEASE: u8 = 5;
const MOTION_NOTIFY: u8 = 6;

/// X keysyms of the names the key table uses (`/usr/include/X11/keysymdef.h`).
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
        "Control_L" => 0xffe3,
        "Shift_L" => 0xffe1,
        "Alt_L" => 0xffe9,
        "Super_L" => 0xffeb,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_lowercase() => s.as_bytes()[0] as u32,
        _ => return None,
    })
}

pub struct XInput {
    conn: RustConnection,
    root: Window,
    win: Window,
    codes: HashMap<u32, u8>,
    /// Time to let pass before a click sequence, so it never joins the previous one (above every
    /// toolkit's double-click time).
    pub settle_ms: u64,
    /// Loop time after each injected event.
    pub pump_ms: u64,
}

fn e<T: std::fmt::Display>(x: T) -> String {
    x.to_string()
}

impl XInput {
    /// Connects to `$DISPLAY`, waits (pumping the toolkit) until a top-level window titled `title`
    /// is mapped, and gives it the X input focus.
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
                        codes.entry(sym).or_insert(min + i as u8);
                    }
                }
            }
        }
        let mut x = XInput {
            conn,
            root,
            win: 0,
            codes,
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
        x.focus(pump)?;
        Ok(x)
    }

    fn atom(&self, name: &str) -> Result<u32, String> {
        Ok(self
            .conn
            .intern_atom(false, name.as_bytes())
            .map_err(e)?
            .reply()
            .map_err(e)?
            .atom)
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
        let k = self.code(&c.key)?;
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

    fn button(&self, down: bool, pump: &mut dyn FnMut(u64)) -> Result<(), String> {
        self.fake(if down { BUTTON_PRESS } else { BUTTON_RELEASE }, 1, 0, 0)?;
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
        pump(self.settle_ms);
        self.move_to(at, pump)?;
        for _ in 0..count {
            self.button(true, pump)?;
            self.button(false, pump)?;
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
            self.button(true, pump)?;
            if i + 1 < count {
                self.button(false, pump)?;
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
        self.button(false, pump)?;
        pump(self.pump_ms);
        Ok(())
    }
}
