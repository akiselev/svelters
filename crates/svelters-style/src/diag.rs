//! [`Diagnostic`]: a problem in a sheet, with its source position.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The rule or declaration was dropped (CSS error recovery).
    Error,
    /// Kept, but probably not what was meant (an unknown property on this backend).
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// 1-based line and column in the sheet text.
    pub line: u32,
    pub column: u32,
}

impl Diagnostic {
    pub(crate) fn error(loc: cssparser::SourceLocation, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Error, message: message.into(), line: loc.line + 1, column: loc.column }
    }

    pub(crate) fn warning(loc: cssparser::SourceLocation, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, message: message.into(), line: loc.line + 1, column: loc.column }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{}:{}: {sev}: {}", self.line, self.column, self.message)
    }
}
