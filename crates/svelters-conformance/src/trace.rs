//! The trace writer: an op log (plans/architecture/backend-abi.md "Op-log format") of what the
//! capture did through the backend, with a `state #h {...}` line per fixture node at every
//! checkpoint. `tools/oplog-compare.mjs` compares two traces by state; `--states` compares only the
//! `state` lines, so traces from different toolkits (GTK, Qt, egui) can be compared.

/// What a checkpoint observes on one node. Fields have `text` and `sel`; the button only `focus`.
/// A selection pseudo-anchor (`clipboard`, `primary`) has only `text` ([`State::selection`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub text: Option<String>,
    /// `[anchor, caret]` in Unicode scalar offsets; the caret is `sel[1]`.
    pub sel: Option<[usize; 2]>,
    /// Has keyboard focus (the window's focus is this node or inside it, and the window is active).
    pub focus: bool,
    /// WG3s: a scroll area's offset, or (the `motion` pseudo-anchor) an action's motion metrics.
    pub scroll: Option<ScrollState>,
}

/// WG3s (`plans/milestones/WG3s-scroll-references.md` §3). Integers so states stay `Eq`: offsets
/// and distances in half points, times in milliseconds, velocities in points per second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollState {
    /// `[x, y]` in half points from the content's start, positive towards its end.
    Offset([i64; 2]),
    Motion(Motion),
}

/// An action's motion on its main axis (§3 metrics).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    /// Half points.
    pub distance: i64,
    /// `wheel` only: tenths of a point per notch.
    pub per_notch: Option<i64>,
    pub peak_v: i64,
    pub tau_ms: Option<i64>,
    pub settle_ms: i64,
    /// Half points past the content's start or end.
    pub overshoot: i64,
    /// Half points the outer area moved (an action on `t`).
    pub chained: Option<i64>,
}

/// Points to half points, rounded.
pub fn half_points(v: f64) -> i64 {
    (v * 2.0).round() as i64
}

fn pts(h: i64) -> String {
    if h % 2 == 0 {
        format!("{}", h / 2)
    } else {
        format!("{:.1}", h as f64 / 2.0)
    }
}

fn within(a: i64, b: i64, abs: i64, rel: f64) -> bool {
    let tol = (abs as f64).max(rel * a.abs().max(b.abs()) as f64);
    ((a - b).abs() as f64) <= tol
}

impl Motion {
    fn json(&self) -> String {
        let mut p = vec![format!("\"distance\":{}", pts(self.distance))];
        if let Some(n) = self.per_notch {
            p.push(format!("\"per_notch\":{:.1}", n as f64 / 10.0));
        }
        p.push(format!("\"peak_v\":{}", self.peak_v));
        p.push(format!(
            "\"tau_ms\":{}",
            self.tau_ms.map_or("null".to_string(), |t| t.to_string())
        ));
        p.push(format!("\"settle_ms\":{}", self.settle_ms));
        p.push(format!("\"overshoot\":{}", pts(self.overshoot)));
        if let Some(c) = self.chained {
            p.push(format!("\"chained\":{}", pts(c)));
        }
        format!("{{{}}}", p.join(","))
    }

    fn from_json(v: &serde_json::Value) -> Result<Motion, String> {
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64());
        Ok(Motion {
            distance: half_points(f("distance").ok_or("motion without distance")?),
            per_notch: f("per_notch").map(|n| (n * 10.0).round() as i64),
            peak_v: f("peak_v").unwrap_or(0.0).round() as i64,
            tau_ms: f("tau_ms").map(|t| t.round() as i64),
            settle_ms: f("settle_ms").unwrap_or(0.0).round() as i64,
            overshoot: half_points(f("overshoot").unwrap_or(0.0)),
            chained: f("chained").map(half_points),
        })
    }

    /// Within the §3 tolerances.
    pub fn matches(&self, o: &Motion) -> bool {
        let opt = |a: Option<i64>, b: Option<i64>, abs: i64, rel: f64| match (a, b) {
            (Some(a), Some(b)) => within(a, b, abs, rel),
            (None, None) => true,
            _ => false,
        };
        // positions are strict; the timing metrics come from sampling the view every frame, so
        // they carry the sampling's noise (WG3s §3): wide tolerances, τ only when both fit one
        let tau = match (self.tau_ms, o.tau_ms) {
            (Some(a), Some(b)) => within(a, b, 0, 0.60),
            _ => true,
        };
        within(self.distance, o.distance, 4, 0.05)
            && opt(self.per_notch, o.per_notch, 10, 0.05)
            && within(self.peak_v, o.peak_v, 0, 0.60)
            && tau
            && within(self.settle_ms, o.settle_ms, 150, 0.50)
            && within(self.overshoot, o.overshoot, 6, 0.25)
            && opt(self.chained, o.chained, 4, 0.05)
    }
}

impl State {
    /// The content of a system selection (CLIPBOARD, PRIMARY).
    pub fn selection(text: String) -> State {
        State {
            text: Some(text),
            sel: None,
            focus: false,
            scroll: None,
        }
    }

    fn is_selection(&self) -> bool {
        self.text.is_some() && self.sel.is_none() && !self.focus
    }

    /// WG3s: a scroll area's state.
    pub fn offset(x: f64, y: f64) -> State {
        State {
            text: None,
            sel: None,
            focus: false,
            scroll: Some(ScrollState::Offset([half_points(x), half_points(y)])),
        }
    }

    /// WG3s: the `motion` pseudo-anchor.
    pub fn motion(m: Motion) -> State {
        State {
            text: None,
            sel: None,
            focus: false,
            scroll: Some(ScrollState::Motion(m)),
        }
    }

    /// Equal, with WG3s's tolerances for scroll state: an offset within max(2 pt, 5% of the larger
    /// one), motion metrics per [`Motion::matches`]. Everything else exactly.
    pub fn matches(&self, o: &State) -> bool {
        match (&self.scroll, &o.scroll) {
            (Some(ScrollState::Offset(a)), Some(ScrollState::Offset(b))) => {
                (0..2).all(|i| within(a[i], b[i], 4, 0.05))
            }
            (Some(ScrollState::Motion(a)), Some(ScrollState::Motion(b))) => a.matches(b),
            _ => self == o,
        }
    }

    /// Compact JSON with a fixed key order: `{"text":…,"sel":[a,f],"caret":f,"focus":b}`; a
    /// selection pseudo-anchor is `{"text":…}`.
    pub fn json(&self) -> String {
        match &self.scroll {
            Some(ScrollState::Offset([x, y])) => {
                return format!("{{\"offset\":[{},{}]}}", pts(*x), pts(*y));
            }
            Some(ScrollState::Motion(m)) => return m.json(),
            None => {}
        }
        let mut parts = Vec::new();
        if let Some(t) = &self.text {
            parts.push(format!("\"text\":{}", serde_json::to_string(t).unwrap()));
        }
        if let Some([a, f]) = self.sel {
            parts.push(format!("\"sel\":[{a},{f}],\"caret\":{f}"));
        }
        if !self.is_selection() {
            parts.push(format!("\"focus\":{}", self.focus));
        }
        format!("{{{}}}", parts.join(","))
    }

    pub fn from_json(v: &serde_json::Value) -> Result<State, String> {
        if let Some(o) = v.get("offset").and_then(|o| o.as_array()) {
            let n = |i: usize| o.get(i).and_then(|x| x.as_f64()).ok_or("bad offset");
            return Ok(State::offset(n(0)?, n(1)?));
        }
        if v.get("distance").is_some() {
            return Ok(State::motion(Motion::from_json(v)?));
        }
        let text = v
            .get("text")
            .map(|t| t.as_str().map(str::to_string).ok_or("text is not a string"))
            .transpose()?;
        let sel = match v.get("sel") {
            Some(serde_json::Value::Array(a)) if a.len() == 2 => {
                let n = |x: &serde_json::Value| x.as_u64().map(|n| n as usize).ok_or("bad sel");
                Some([n(&a[0])?, n(&a[1])?])
            }
            None | Some(serde_json::Value::Null) => None,
            Some(o) => return Err(format!("bad sel {o}")),
        };
        Ok(State {
            text,
            sel,
            focus: v.get("focus").and_then(|f| f.as_bool()).unwrap_or(false),
            scroll: None,
        })
    }
}

/// One checkpoint: the state of every anchor (`a`, `b`, `c`), in anchor order.
pub type Checkpoint = Vec<(String, State)>;

/// Offsets: UTF-16 code units to Unicode scalar values (Qt reports UTF-16).
pub fn utf16_to_scalar(text: &str, u16_offset: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.chars().enumerate() {
        if units >= u16_offset {
            return i;
        }
        units += c.len_utf16();
    }
    text.chars().count()
}

pub fn scalar_to_utf16(text: &str, offset: usize) -> usize {
    text.chars().take(offset).map(char::len_utf16).sum()
}

#[derive(Default, Debug, Clone)]
pub struct Trace {
    lines: Vec<String>,
}

fn j(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

impl Trace {
    pub fn op(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }
    pub fn create(&mut self, kind: &str, h: u64) {
        self.op(format!("create {kind} #{h}"));
    }
    pub fn set_str(&mut self, h: u64, prop: &str, v: &str) {
        self.op(format!("set #{h} {prop} {}", j(v)));
    }
    pub fn set_raw(&mut self, h: u64, prop: &str, json: &str) {
        self.op(format!("set #{h} {prop} {json}"));
    }
    pub fn insert(&mut self, parent: u64, child: u64) {
        self.op(format!("insert #{parent} #{child} before=end slot=default"));
    }
    pub fn call(&mut self, h: u64, method: &str, args_json: &str) {
        self.op(format!("call #{h} {method} {args_json}"));
    }
    pub fn destroy(&mut self, h: u64) {
        self.op(format!("destroy #{h}"));
    }
    /// The trace writer's line (WG3, additive to the op-log format).
    pub fn state(&mut self, h: u64, s: &State) {
        self.op(format!("state #{h} {}", s.json()));
    }
    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}
