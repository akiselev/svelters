//! `formatToParts`: collects ICU4X's writeable parts into ECMA-402 `{ type, value }` pieces.

use std::fmt;
use writeable::{PartsWrite, Writeable};

/// One piece of formatted text, `{ type, value }` in `formatToParts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    /// The ECMA-402 part type (`integer`, `group`, `decimal`, `month`, `literal`, ...).
    pub kind: String,
    pub value: String,
}

#[derive(Default)]
struct Collect {
    stack: Vec<writeable::Part>,
    out: Vec<Part>,
}

impl Collect {
    /// The type of text written now: the outermost non-decimal part (a date field formatted with
    /// decimal digits is still the field), else the innermost decimal part, else literal.
    fn kind(&self) -> &'static str {
        if let Some(p) = self.stack.iter().find(|p| p.category != "decimal") {
            return p.value;
        }
        self.stack.last().map_or("literal", |p| p.value)
    }
}

impl fmt::Write for Collect {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.is_empty() {
            return Ok(());
        }
        let kind = self.kind();
        match self.out.last_mut() {
            Some(last) if last.kind == kind => last.value.push_str(s),
            _ => self.out.push(Part { kind: kind.to_string(), value: s.to_string() }),
        }
        Ok(())
    }
}

impl PartsWrite for Collect {
    type SubPartsWrite = Collect;

    fn with_part(&mut self, part: writeable::Part, mut f: impl FnMut(&mut Self::SubPartsWrite) -> fmt::Result) -> fmt::Result {
        self.stack.push(part);
        let r = f(self);
        self.stack.pop();
        r
    }
}

/// The parts of `w`, adjacent pieces of one type merged.
pub(crate) fn collect(w: &impl Writeable) -> Vec<Part> {
    let mut c = Collect::default();
    // Writing to a Vec cannot fail.
    let _ = w.write_to_parts(&mut c);
    c.out
}

/// Splits literal pieces into whitespace (`literal`) and the rest (`kind`): the percent sign,
/// currency symbol or compact suffix around a number that its formatter writes unmarked.
pub(crate) fn split_literals(parts: Vec<Part>, kind: &str) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::with_capacity(parts.len());
    let mut push = |k: &str, s: &str| match out.last_mut() {
        Some(last) if last.kind == k => last.value.push_str(s),
        _ => out.push(Part { kind: k.to_string(), value: s.to_string() }),
    };
    for p in parts {
        if p.kind != "literal" {
            push(&p.kind, &p.value);
            continue;
        }
        for ch in p.value.chars() {
            let mut buf = [0u8; 4];
            let k = if ch.is_whitespace() || ch == '\u{200e}' || ch == '\u{200f}' { "literal" } else { kind };
            push(k, ch.encode_utf8(&mut buf));
        }
    }
    out
}

/// Concatenated part values.
pub(crate) fn join(parts: &[Part]) -> String {
    parts.iter().map(|p| p.value.as_str()).collect()
}
