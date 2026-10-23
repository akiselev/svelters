//! Pin scripts (`conformance/pins/*.json`) and the key-role table (`conformance/keys.json`).
//!
//! A pin runs on one fixture: a window holding text fields `a` and `b` and a button `c`, in that
//! order. `setup` gives each field its text and selection (`[anchor, caret]` in Unicode scalar
//! offsets; a negative offset counts from the end, -1 being the end) and which anchor has keyboard
//! focus. Every step is one action followed by one checkpoint; `repeat: n` runs the pair n times.
//! Actions: `{"key": intent, "shift": bool}`, `{"click": anchor, "at": offset, "count": n}` (on a
//! button `at` is omitted: the click lands at its centre), `{"drag": anchor, "from", "to", "count"}`
//! (`count` presses at `from`, the last one held while the pointer moves to `to`). A click takes
//! `"button": 2` for the middle button (X11 primary-selection paste). `{"type": text}` types the
//! text one key per character (WG3b, pins 4-6).
//!
//! Optional (WG3b): `setup.clipboard` puts that text on the system clipboard before the user path,
//! owned by another client (the capture's X connection; egui's driver keeps it as the text
//! egui-winit would read); `observe: ["clipboard", "primary"]` adds those two pseudo-anchors to
//! every checkpoint: the text the CLIPBOARD and PRIMARY selections hold ("" without an owner).

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The fixture's anchors in window order: two text fields and a button.
pub const FIELDS: [&str; 2] = ["a", "b"];
pub const BUTTON: &str = "c";
pub const BUTTON_LABEL: &str = "Done";
/// The pseudo-anchors a pin can observe besides the fixture's nodes.
pub const CLIPBOARD: &str = "clipboard";
pub const PRIMARY: &str = "primary";

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub text: String,
    /// `[anchor, caret]`, negative counting from the end.
    pub sel: [i64; 2],
}

impl Field {
    /// The selection in Unicode scalar offsets.
    pub fn sel(&self) -> [usize; 2] {
        let len = self.text.chars().count() as i64;
        let fix = |v: i64| {
            if v < 0 {
                (len + 1 + v).max(0) as usize
            } else {
                v.min(len) as usize
            }
        };
        [fix(self.sel[0]), fix(self.sel[1])]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// By anchor (`a`, `b`); a field the pin does not list is empty.
    pub fields: BTreeMap<String, Field>,
    pub focus: Option<String>,
    /// Text on the system clipboard when the user path starts (owned by another client).
    pub clipboard: Option<String>,
    /// WG3s: the scroll fixture (scroll areas `s` and `t` instead of fields and the button), with
    /// each area's initial offset in points from the content's start.
    pub scroll: BTreeMap<String, [f64; 2]>,
}

impl Setup {
    pub fn field(&self, anchor: &str) -> Field {
        self.fields.get(anchor).cloned().unwrap_or(Field {
            text: String::new(),
            sel: [0, 0],
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Key {
        intent: String,
        shift: bool,
    },
    Click {
        target: String,
        at: Option<usize>,
        count: u32,
        /// 1 primary, 2 middle.
        button: u32,
    },
    /// Types the text, one key per character.
    Type {
        text: String,
    },
    Drag {
        target: String,
        from: usize,
        to: usize,
        count: u32,
    },
    /// WG3s: mouse-wheel notches, `lines` per event (positive towards the content's end),
    /// `burst` events `interval_ms` apart.
    Wheel {
        target: String,
        lines: [i32; 2],
        shift: bool,
        burst: u32,
        interval_ms: u32,
    },
    /// WG3s: a trackpad two-finger scroll: `deltas` (points per event, positive towards the end)
    /// with phases began/changed/ended, then `momentum`.
    Gesture {
        target: String,
        /// The profile name, or `inline` (the label).
        profile: String,
        deltas: Vec<[f64; 2]>,
        interval_ms: u32,
        momentum: Momentum,
    },
}

/// WG3s: what follows a gesture's *ended* phase.
#[derive(Clone, Debug, PartialEq)]
pub enum Momentum {
    /// The fingers stop before lifting.
    None,
    /// The platform's own momentum.
    Native,
    /// A recorded momentum stream (`conformance/scroll/<name>.json`).
    Replay(String),
}

/// The scroll fixture's anchors: the outer area and the one inside it.
pub const SCROLL_ANCHORS: [&str; 2] = ["s", "t"];

/// WG3s delta profiles (`plans/milestones/WG3s-scroll-references.md` §2.1): dy per 16 ms event.
pub fn profile(name: &str) -> Option<Vec<[f64; 2]>> {
    let ys: Vec<f64> = match name {
        "drag-slow" => vec![2.0; 20],
        "flick-down" => vec![3.0, 8.0, 16.0, 28.0, 40.0, 44.0, 40.0],
        "flick-up" => vec![-3.0, -8.0, -16.0, -28.0, -40.0, -44.0, -40.0],
        "to-end" => vec![40.0; 60],
        _ => return None,
    };
    Some(ys.into_iter().map(|y| [0.0, y]).collect())
}

impl Action {
    /// The short label a checkpoint carries (`after`).
    pub fn label(&self) -> String {
        match self {
            Action::Key { intent, shift } => {
                format!("{}{intent}", if *shift { "shift+" } else { "" })
            }
            Action::Click {
                target,
                at,
                count,
                button,
            } => {
                let b = if *button == 2 {
                    "middle-click"
                } else {
                    "click"
                };
                match at {
                    Some(at) => format!("{b} {target}@{at} x{count}"),
                    None => format!("{b} {target} x{count}"),
                }
            }
            Action::Type { text } => format!("type {}", serde_json::to_string(text).unwrap()),
            Action::Drag {
                target,
                from,
                to,
                count,
            } => format!("drag {target} {from}->{to} x{count}"),
            Action::Wheel {
                target,
                lines,
                shift,
                burst,
                interval_ms,
            } => {
                let mut l = format!(
                    "{}wheel {target} {},{}",
                    if *shift { "shift+" } else { "" },
                    lines[0],
                    lines[1]
                );
                if *burst > 1 {
                    l += &format!(" x{burst}/{interval_ms}ms");
                }
                l
            }
            Action::Gesture {
                target,
                profile,
                momentum,
                ..
            } => {
                let m = match momentum {
                    Momentum::None => "no momentum".to_string(),
                    Momentum::Native => "native momentum".to_string(),
                    Momentum::Replay(n) => format!("replay {n}"),
                };
                format!("gesture {target} {profile}, {m}")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub id: String,
    /// The widget survey §5.3 behaviour number.
    pub pin: u32,
    pub title: String,
    pub setup: Setup,
    /// Expanded: one checkpoint follows each action.
    pub actions: Vec<Action>,
    /// Pseudo-anchors observed at every checkpoint besides the nodes ([`CLIPBOARD`], [`PRIMARY`]).
    pub observe: Vec<String>,
    /// The file's text (fingerprinted).
    pub source: String,
}

/// The repository root (two levels above this crate).
pub fn repo_root() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    p.canonicalize().unwrap_or(p)
}

pub fn conformance_dir() -> PathBuf {
    repo_root().join("conformance")
}

fn as_u(v: &Value, what: &str) -> Result<usize, String> {
    v.as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| format!("{what}: expected a non-negative integer, got {v}"))
}

pub fn parse_pin(source: &str) -> Result<Pin, String> {
    let v: Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let id = v["id"].as_str().ok_or("pin without id")?.to_string();
    let ctx = |m: String| format!("pin {id}: {m}");
    let pin = v["pin"]
        .as_u64()
        .ok_or_else(|| ctx("no behaviour number `pin`".into()))? as u32;
    let title = v["title"].as_str().unwrap_or_default().to_string();
    let mut fields = BTreeMap::new();
    if let Some(m) = v["setup"]["fields"].as_object() {
        for (anchor, f) in m {
            if !FIELDS.contains(&anchor.as_str()) {
                return Err(ctx(format!("unknown field {anchor}")));
            }
            let text = f["text"].as_str().unwrap_or_default().to_string();
            let sel = match f.get("sel") {
                Some(Value::Array(a)) if a.len() == 2 => {
                    let n = |x: &Value| x.as_i64().ok_or_else(|| ctx(format!("bad sel {x}")));
                    [n(&a[0])?, n(&a[1])?]
                }
                None => [0, 0],
                Some(o) => return Err(ctx(format!("bad sel {o}"))),
            };
            fields.insert(anchor.clone(), Field { text, sel });
        }
    }
    let focus = v["setup"]["focus"].as_str().map(str::to_string);
    let clipboard = match v["setup"].get("clipboard") {
        None => None,
        Some(Value::String(t)) => Some(t.clone()),
        Some(o) => return Err(ctx(format!("bad clipboard {o}"))),
    };
    let mut scroll = BTreeMap::new();
    if let Some(m) = v["setup"]["scroll"].as_object() {
        for (anchor, o) in m {
            if !SCROLL_ANCHORS.contains(&anchor.as_str()) {
                return Err(ctx(format!("unknown scroll area {anchor}")));
            }
            let xy = o
                .as_array()
                .filter(|a| a.len() == 2)
                .and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?]))
                .ok_or_else(|| ctx(format!("bad scroll offset {o}")))?;
            scroll.insert(anchor.clone(), xy);
        }
    }
    let mut observe = Vec::new();
    for o in v["observe"].as_array().into_iter().flatten() {
        match o.as_str() {
            Some(n @ (CLIPBOARD | PRIMARY)) => observe.push(n.to_string()),
            _ => return Err(ctx(format!("unknown observable {o}"))),
        }
    }
    let mut actions = Vec::new();
    for s in v["steps"]
        .as_array()
        .ok_or_else(|| ctx("no steps".into()))?
    {
        let repeat = s
            .get("repeat")
            .map(|r| as_u(r, "repeat"))
            .transpose()
            .map_err(ctx)?
            .unwrap_or(1);
        let count = s
            .get("count")
            .map(|r| as_u(r, "count"))
            .transpose()
            .map_err(ctx)?
            .unwrap_or(1) as u32;
        let a = if let Some(k) = s["key"].as_str() {
            Action::Key {
                intent: k.to_string(),
                shift: s["shift"].as_bool().unwrap_or(false),
            }
        } else if let Some(t) = s["click"].as_str() {
            let at = s
                .get("at")
                .map(|x| as_u(x, "at"))
                .transpose()
                .map_err(ctx)?;
            let button = s
                .get("button")
                .map(|x| as_u(x, "button"))
                .transpose()
                .map_err(ctx)?
                .unwrap_or(1) as u32;
            if !matches!(button, 1 | 2) {
                return Err(ctx(format!("button {button}: 1 or 2")));
            }
            Action::Click {
                target: t.to_string(),
                at,
                count,
                button,
            }
        } else if let Some(t) = s["type"].as_str() {
            Action::Type {
                text: t.to_string(),
            }
        } else if let Some(t) = s["drag"].as_str() {
            Action::Drag {
                target: t.to_string(),
                from: as_u(&s["from"], "from").map_err(ctx)?,
                to: as_u(&s["to"], "to").map_err(ctx)?,
                count,
            }
        } else if let Some(t) = s["wheel"].as_str() {
            let lines = s["lines"]
                .as_array()
                .filter(|a| a.len() == 2)
                .and_then(|a| Some([a[0].as_i64()? as i32, a[1].as_i64()? as i32]))
                .ok_or_else(|| ctx(format!("wheel: bad lines {}", s["lines"])))?;
            Action::Wheel {
                target: t.to_string(),
                lines,
                shift: s["shift"].as_bool().unwrap_or(false),
                burst: s
                    .get("burst")
                    .map(|x| as_u(x, "burst"))
                    .transpose()
                    .map_err(ctx)?
                    .unwrap_or(1) as u32,
                interval_ms: s
                    .get("interval_ms")
                    .map(|x| as_u(x, "interval_ms"))
                    .transpose()
                    .map_err(ctx)?
                    .unwrap_or(0) as u32,
            }
        } else if let Some(t) = s["gesture"].as_str() {
            let (profile_name, deltas) = match &s["deltas"] {
                Value::String(n) => (
                    n.clone(),
                    profile(n).ok_or_else(|| ctx(format!("unknown profile {n}")))?,
                ),
                Value::Array(a) => {
                    let mut d = Vec::new();
                    for p in a {
                        let xy = p
                            .as_array()
                            .filter(|q| q.len() == 2)
                            .and_then(|q| Some([q[0].as_f64()?, q[1].as_f64()?]))
                            .ok_or_else(|| ctx(format!("gesture: bad delta {p}")))?;
                        d.push(xy);
                    }
                    ("inline".to_string(), d)
                }
                o => return Err(ctx(format!("gesture: bad deltas {o}"))),
            };
            let momentum = match s["momentum"].as_str().unwrap_or("none") {
                "none" => Momentum::None,
                "native" => Momentum::Native,
                m if m.starts_with("replay:") => Momentum::Replay(m["replay:".len()..].to_string()),
                m => return Err(ctx(format!("gesture: bad momentum {m}"))),
            };
            Action::Gesture {
                target: t.to_string(),
                profile: profile_name,
                deltas,
                interval_ms: s
                    .get("interval_ms")
                    .map(|x| as_u(x, "interval_ms"))
                    .transpose()
                    .map_err(ctx)?
                    .unwrap_or(16) as u32,
                momentum,
            }
        } else {
            return Err(ctx(format!("unknown step {s}")));
        };
        for _ in 0..repeat {
            actions.push(a.clone());
        }
    }
    Ok(Pin {
        id,
        pin,
        title,
        setup: Setup {
            fields,
            focus,
            clipboard,
            scroll,
        },
        actions,
        observe,
        source: source.replace("\r\n", "\n"),
    })
}

/// Every pin, in file-name order.
pub fn load_pins() -> Result<Vec<Pin>, String> {
    Ok(load_all_pins()?
        .into_iter()
        .filter(|p| !p.is_scroll())
        .collect())
}

/// WG3s: the scroll pins (9, 10), the ones with a scroll fixture (`setup.scroll`). They form their
/// own family: a toolkit's scroll references are their own golden (`nsscrollview-…`), next to its
/// text references, and [`load_pins`] leaves them out.
pub fn load_scroll_pins() -> Result<Vec<Pin>, String> {
    Ok(load_all_pins()?
        .into_iter()
        .filter(Pin::is_scroll)
        .collect())
}

impl Pin {
    /// A scroll pin (WG3s): its fixture is the scroll areas, not the text fields.
    pub fn is_scroll(&self) -> bool {
        !self.setup.scroll.is_empty()
    }
}

/// Every pin of either family, in file-name order.
pub fn load_all_pins() -> Result<Vec<Pin>, String> {
    let dir = conformance_dir().join("pins");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut out: Vec<Pin> = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let p = parse_pin(&text).map_err(|e| format!("{}: {e}", f.display()))?;
        if out.iter().any(|q| q.id == p.id) {
            return Err(format!("duplicate pin id {}", p.id));
        }
        out.push(p);
    }
    Ok(out)
}

/// A key chord: modifiers plus an X keysym name (`Right`, `BackSpace`, `a`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// The macOS command key (Super on X11).
    pub cmd: bool,
    pub key: String,
}

impl Chord {
    pub fn parse(s: &str) -> Result<Chord, String> {
        let mut c = Chord::default();
        let parts: Vec<&str> = s.split('+').collect();
        let (key, mods) = parts
            .split_last()
            .ok_or_else(|| format!("empty chord {s:?}"))?;
        for m in mods {
            match *m {
                "Ctrl" => c.ctrl = true,
                "Shift" => c.shift = true,
                "Alt" => c.alt = true,
                "Cmd" => c.cmd = true,
                o => return Err(format!("unknown modifier {o} in {s:?}")),
            }
        }
        c.key = key.to_string();
        Ok(c)
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for (on, n) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.cmd, "Cmd+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                s.push_str(n);
            }
        }
        s + &self.key
    }
}

/// The key-role table of one platform.
#[derive(Clone, Debug)]
pub struct Keys {
    pub platform: String,
    pub map: BTreeMap<String, String>,
}

impl Keys {
    pub fn load(platform: &str) -> Result<Keys, String> {
        let p = conformance_dir().join("keys.json");
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let m = v["platforms"][platform]
            .as_object()
            .ok_or_else(|| format!("keys.json: no platform {platform}"))?;
        let map = m
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        Ok(Keys {
            platform: platform.to_string(),
            map,
        })
    }

    pub fn chord(&self, intent: &str, shift: bool) -> Result<Chord, String> {
        let s = self
            .map
            .get(intent)
            .ok_or_else(|| format!("no key for intent {intent} on {}", self.platform))?;
        let mut c = Chord::parse(s)?;
        c.shift |= shift;
        Ok(c)
    }
}

/// Version of the capture procedure: bump it when a change to the harness (not to a pin) changes
/// what a capture records, so every golden reads as stale.
pub const HARNESS_VERSION: &str = "na1-v1";

pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The pin's fingerprint on a platform: the harness version, the pin file and the chords its
/// intents resolve to. A golden whose fingerprint differs is stale (NA5).
pub fn fingerprint(pin: &Pin, keys: &Keys) -> String {
    let mut s = format!("{HARNESS_VERSION}\n{}\n", pin.source);
    let mut intents: Vec<String> = pin
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::Key { intent, shift } => Some(
                keys.chord(intent, *shift)
                    .map(|c| format!("{intent}={}", c.text()))
                    .unwrap_or_else(|e| e),
            ),
            _ => None,
        })
        .collect();
    intents.sort();
    intents.dedup();
    s.push_str(&intents.join("\n"));
    // WG3s: a named gesture profile is code, not pin text: its deltas are part of the fingerprint
    for a in &pin.actions {
        if let Action::Gesture {
            profile, deltas, ..
        } = a
            && profile != "inline"
        {
            s.push_str(&format!("\n{profile}={deltas:?}"));
        }
    }
    format!("{:016x}", fnv64(s.as_bytes()))
}
