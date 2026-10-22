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
}

impl State {
    /// The content of a system selection (CLIPBOARD, PRIMARY).
    pub fn selection(text: String) -> State {
        State {
            text: Some(text),
            sel: None,
            focus: false,
        }
    }

    fn is_selection(&self) -> bool {
        self.text.is_some() && self.sel.is_none() && !self.focus
    }

    /// Compact JSON with a fixed key order: `{"text":…,"sel":[a,f],"caret":f,"focus":b}`; a
    /// selection pseudo-anchor is `{"text":…}`.
    pub fn json(&self) -> String {
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
