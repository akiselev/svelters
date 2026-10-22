//! Diagnostics: errors, warnings and notes with byte spans into the `.svelte` source, their
//! registered codes (`crate::codes`) and fix-its (text edits; `crate::fixits`).

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    /// Information about how the source was lowered; never fails a build and is not a warning
    /// (FW1's `legacy_forwarding_lowered`).
    Note,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `unknown_element`, `bind_no_plan`.
    pub code: &'static str,
    /// The code's stable namespaced id (`svelters::element::unknown`, `svelte::expected_token`),
    /// from the registry (`crate::codes`).
    pub id: &'static str,
    pub message: String,
    /// Byte offsets into the component source.
    pub start: u32,
    pub end: u32,
    /// The backend whose semantic specification produced the diagnostic, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// 1-based line and column of `start`, filled in by [`Diagnostics::finish`].
    pub line: u32,
    pub column: u32,
    /// Fix-its: edits that resolve the diagnostic (CX1). Set where the diagnostic is reported, or
    /// derived from the source by [`Diagnostics::finish`] (`crate::fixits`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<FixIt>,
    /// What [`Diagnostics::finish`] needs to derive a fix-it from the source text.
    #[serde(skip)]
    pub hint: FixHint,
}

/// A fix-it: a labelled set of non-overlapping edits that resolve one diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FixIt {
    /// What the fix does, as a quick-fix title ("replace with `onclicked`").
    pub message: String,
    /// Edits in byte offsets of the source, sorted, not overlapping.
    pub edits: Vec<TextEdit>,
}

/// Replace `source[start..end]` with `text` (an insertion when `start == end`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextEdit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

impl FixIt {
    pub fn new(message: impl Into<String>, edits: Vec<TextEdit>) -> FixIt {
        FixIt { message: message.into(), edits }
    }

    /// `source` with the edits applied.
    pub fn apply(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut cur = 0usize;
        for e in &self.edits {
            out.push_str(&source[cur..e.start as usize]);
            out.push_str(&e.text);
            cur = e.end as usize;
        }
        out.push_str(&source[cur..]);
        out
    }
}

/// Facts the reporting site knows and [`Diagnostics::finish`] turns into fix-its once it has the
/// source text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FixHint {
    #[default]
    None,
    /// "did you mean": names that can replace the one at the span.
    Suggest(Vec<String>),
    /// Text inside the span to delete (`|once` of a directive).
    Remove(String),
}

#[derive(Default)]
pub struct Diagnostics {
    pub list: Vec<Diagnostic>,
    backend: Option<String>,
}

impl Diagnostics {
    pub fn for_backend(name: &str) -> Diagnostics {
        Diagnostics { list: Vec::new(), backend: Some(name.to_string()) }
    }

    pub fn error(&mut self, code: &'static str, span: (u32, u32), message: impl Into<String>) -> &mut Diagnostic {
        self.push(Severity::Error, code, span, message.into(), false)
    }

    /// An error that comes from the backend specification (tagged with the backend name).
    pub fn native_error(&mut self, code: &'static str, span: (u32, u32), message: impl Into<String>) -> &mut Diagnostic {
        self.push(Severity::Error, code, span, message.into(), true)
    }

    pub fn native_warning(&mut self, code: &'static str, span: (u32, u32), message: impl Into<String>) -> &mut Diagnostic {
        self.push(Severity::Warning, code, span, message.into(), true)
    }

    pub fn warning(&mut self, code: &'static str, span: (u32, u32), message: impl Into<String>) -> &mut Diagnostic {
        self.push(Severity::Warning, code, span, message.into(), false)
    }

    pub fn note(&mut self, code: &'static str, span: (u32, u32), message: impl Into<String>) -> &mut Diagnostic {
        self.push(Severity::Note, code, span, message.into(), false)
    }

    fn push(&mut self, severity: Severity, code: &'static str, span: (u32, u32), message: String, native: bool) -> &mut Diagnostic {
        self.list.push(Diagnostic {
            severity,
            code,
            id: crate::codes::id_of(code),
            message,
            start: span.0,
            end: span.1,
            backend: if native { self.backend.clone() } else { None },
            help: None,
            line: 0,
            column: 0,
            fixes: Vec::new(),
            hint: FixHint::None,
        });
        self.list.last_mut().unwrap()
    }

    /// Turns the warnings with one of `codes` into errors (`--strict-native`).
    pub fn promote(&mut self, codes: &[&str]) {
        for d in &mut self.list {
            if d.severity == Severity::Warning && codes.contains(&d.code) {
                d.severity = Severity::Error;
            }
        }
    }

    pub fn has_errors(&self) -> bool {
        self.list.iter().any(|d| d.severity == Severity::Error)
    }

    /// Sorts by position, fills in line/column and derives the fix-its that need the source.
    pub fn finish(mut self, source: &str) -> Vec<Diagnostic> {
        let lines = LineIndex::new(source);
        for d in &mut self.list {
            let (l, c) = lines.line_col(d.start);
            d.line = l + 1;
            d.column = c + 1;
            crate::fixits::derive(source, d);
        }
        self.list.sort_by_key(|d| (d.start, d.end));
        self.list
    }
}

impl Diagnostic {
    pub fn help(&mut self, h: impl Into<String>) -> &mut Self {
        self.help = Some(h.into());
        self
    }

    /// Adds a fix-it.
    pub fn fix(&mut self, f: FixIt) -> &mut Self {
        self.fixes.push(f);
        self
    }

    /// Records what [`Diagnostics::finish`] needs to derive a fix-it from the source.
    pub fn hint(&mut self, h: FixHint) -> &mut Self {
        self.hint = h;
        self
    }

    /// `App.svelte:3:5: error[unknown_element] (gtk): ...`
    pub fn render(&self, filename: &str) -> String {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        let be = self.backend.as_deref().map(|b| format!(" ({b})")).unwrap_or_default();
        let mut s = format!("{filename}:{}:{}: {sev}[{}]{be}: {}", self.line, self.column, self.code, self.message);
        if let Some(h) = &self.help {
            s.push_str(&format!("\n  help: {h}"));
        }
        s
    }
}

/// Byte offset -> (0-based line, 0-based UTF-16 column), the convention of source maps.
pub struct LineIndex<'s> {
    src: &'s str,
    starts: Vec<u32>,
}

impl<'s> LineIndex<'s> {
    pub fn new(src: &'s str) -> Self {
        let mut starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
            }
        }
        LineIndex { src, starts }
    }

    /// Length of the source in bytes.
    pub fn len(&self) -> u32 {
        self.src.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
    }

    /// Byte offset of the start of 0-based `line`.
    pub fn line_start(&self, line: u32) -> u32 {
        self.starts[line as usize]
    }

    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let start = self.starts[line] as usize;
        let end = (offset as usize).min(self.src.len());
        let col = self.src.get(start..end).map_or(0, |s| s.encode_utf16().count());
        (line as u32, col as u32)
    }
}
