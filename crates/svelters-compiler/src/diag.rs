//! Diagnostics: errors and warnings with byte spans into the `.svelte` source.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `unknown_element`, `bind_no_plan`.
    pub code: &'static str,
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

    fn push(&mut self, severity: Severity, code: &'static str, span: (u32, u32), message: String, native: bool) -> &mut Diagnostic {
        self.list.push(Diagnostic {
            severity,
            code,
            message,
            start: span.0,
            end: span.1,
            backend: if native { self.backend.clone() } else { None },
            help: None,
            line: 0,
            column: 0,
        });
        self.list.last_mut().unwrap()
    }

    pub fn has_errors(&self) -> bool {
        self.list.iter().any(|d| d.severity == Severity::Error)
    }

    /// Sorts by position and fills in line/column.
    pub fn finish(mut self, source: &str) -> Vec<Diagnostic> {
        let lines = LineIndex::new(source);
        for d in &mut self.list {
            let (l, c) = lines.line_col(d.start);
            d.line = l + 1;
            d.column = c + 1;
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

    /// `App.svelte:3:5: error[unknown_element] (gtk): ...`
    pub fn render(&self, filename: &str) -> String {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
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
