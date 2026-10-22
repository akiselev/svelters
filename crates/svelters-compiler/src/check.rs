//! CX1 (cadatomic SV27): check one file in process for diagnostics only, with incremental state
//! per file, fix-its and the stable codes of [`crate::codes`].
//!
//! ```no_run
//! use svelters_compiler::check::{check, FileState};
//! use svelters_compiler::semantics::SpecBackend;
//!
//! let egui = SpecBackend::builtin("egui").unwrap();
//! let mut state = FileState::new();
//! let buffer = "<button label=\"Go\" onclickd={() => {}} />";
//! let result = check("ui/panels/Run.svelte", buffer, &egui, &mut state);
//! for d in &result.diagnostics {
//!     println!("{}:{}: {} {}", d.range.start_pos.line, d.range.start_pos.column, d.id, d.message);
//!     for f in &d.fixes {
//!         println!("  quick fix: {}", f.message);
//!     }
//! }
//! ```
//!
//! **What a check runs.** A `.svelte` file goes through the whole compiler pipeline (parse, scope
//! analysis, `<style>`, lowering against the backend's semantic spec, the rune transform), but code
//! generation runs as a traversal that writes nothing and builds no source map
//! ([`crate::compile::check_component`]); the diagnostics are exactly [`crate::compile::compile`]'s.
//! A `.svelte.js` module runs [`crate::compile::compile_module`]'s pipeline. Any other file has no
//! diagnostics.
//!
//! **Incremental state** ([`FileState`], one per file, owned by the caller):
//! - *Unchanged input*: the same source, path, options and backend (by
//!   [`PlatformSemantics::fingerprint`]) as the previous check return the previous result without
//!   running anything ([`Reuse::Unchanged`]). An editor asks again on every cursor move, focus change
//!   or save; the module loader checks a buffer the editor has already checked.
//! - *Syntax errors while typing*: when the buffer does not parse (or its scopes do not analyse),
//!   the pipeline stops at the first error, so the later diagnostics are missing, not fixed. The
//!   diagnostics of the last check that ran the whole pipeline are carried over, mapped through the
//!   edit since then (dropped where the edit touched them), and marked `stale`
//!   ([`Reuse::CarriedOver`]). The editor keeps showing them, dimmed, instead of flickering.
//!
//! Positions are given both as UTF-8 byte offsets and as 1-based line and column (the column in
//! UTF-8 bytes and in UTF-16 code units). A fix-it is a set of text edits in the same terms.

use crate::codes;
use crate::compile::{CompileOptions, CompileResult, ModuleOptions, Timings, check_component, compile_module};
use crate::diag::{Diagnostic, LineIndex, Severity};
use crate::semantics::PlatformSemantics;
use serde::Serialize;
use std::time::Instant;

/// Options of [`check_with`] beyond the backend.
#[derive(Clone, Default)]
pub struct CheckOptions<'b> {
    /// `--strict-native`: the observability warnings are errors.
    pub strict_native: bool,
    /// Svelte's `dev` option (`$inspect` is kept); it changes no diagnostic today.
    pub dev: bool,
    /// Portability mode: other platforms the component is checked against (`observability_differs`).
    pub portability: Vec<&'b dyn PlatformSemantics>,
    /// Run Q3w's CSS port over `<style>` (`CompileOptions::scoped_css`): its `css_*` errors and
    /// `css_unused_selector` warnings are reported too.
    pub scoped_css: bool,
}

/// What the file is, by its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    /// `.svelte`
    Component,
    /// `.svelte.js`
    Module,
    /// Anything else: no diagnostics.
    Other,
}

impl FileKind {
    pub fn of(path: &str) -> FileKind {
        if path.ends_with(".svelte") {
            FileKind::Component
        } else if path.ends_with(".svelte.js") {
            FileKind::Module
        } else {
            FileKind::Other
        }
    }
}

/// A position in the source. `line` and both columns are 1-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Position {
    pub line: u32,
    /// Column in UTF-8 bytes from the start of the line, plus 1.
    pub column: u32,
    /// Column in UTF-16 code units, plus 1 (LSP's default `character` is this minus 1).
    pub utf16_column: u32,
}

/// A source range: UTF-8 byte offsets `start..end` and their positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Range {
    pub start: u32,
    pub end: u32,
    pub start_pos: Position,
    pub end_pos: Position,
}

/// One text edit of a fix-it: replace `range` with `text`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Edit {
    pub range: Range,
    pub text: String,
}

/// A fix-it: a quick-fix title and its edits (sorted, not overlapping).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fix {
    pub message: String,
    pub edits: Vec<Edit>,
}

/// A diagnostic as tools see it (the JSON of `svelters-compile --check --json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CheckDiagnostic {
    /// The stable namespaced id (`svelters::bind::partially_observed`); see [`crate::codes`].
    pub id: &'static str,
    /// The short code (`bind_partially_observed`).
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// The backend whose semantic spec produced the diagnostic, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    pub range: Range,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
    /// Carried over from the last complete check while the source has a syntax error
    /// ([`Reuse::CarriedOver`]); its range is mapped through the edit since then.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
}

/// What a check reused from the file's previous state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reuse {
    /// The pipeline ran.
    None,
    /// Same input as the previous check: its result, nothing ran.
    Unchanged,
    /// The pipeline stopped at a syntax or scope error; the last complete check's other
    /// diagnostics are carried over as `stale`.
    CarriedOver,
}

/// Microseconds per phase (`generate_us`: the diagnostics-only traversal of code generation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CheckTimings {
    pub parse_us: u64,
    pub analyze_us: u64,
    pub lower_us: u64,
    pub generate_us: u64,
    /// The whole check, including position mapping and state bookkeeping.
    pub total_us: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    pub path: String,
    pub kind: FileKind,
    /// Sorted by position; stale ones are mixed in at their mapped positions.
    pub diagnostics: Vec<CheckDiagnostic>,
    pub reuse: Reuse,
    pub timings: CheckTimings,
}

impl CheckResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error && !d.stale)
    }
}

/// Counters of a [`FileState`] (for measurements and tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CheckStats {
    pub checks: u64,
    /// Checks answered from the previous result ([`Reuse::Unchanged`]).
    pub unchanged: u64,
    /// Checks that ran the pipeline.
    pub runs: u64,
    /// Checks that carried stale diagnostics over ([`Reuse::CarriedOver`]).
    pub carried_over: u64,
}

/// What identifies a check's input besides the source.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    path: String,
    backend: String,
    /// `None`: a backend without a fingerprint, which is never reused.
    fingerprint: Option<u64>,
    portability: Vec<(String, Option<u64>)>,
    strict_native: bool,
    dev: bool,
    scoped_css: bool,
}

impl Key {
    fn reusable(&self) -> bool {
        self.fingerprint.is_some() && self.portability.iter().all(|p| p.1.is_some())
    }
}

/// A finished run of the pipeline.
#[derive(Clone)]
struct Run {
    key: Key,
    source: String,
    result: CheckResult,
    /// The raw diagnostics (byte spans) of a complete run, for carrying over.
    raw: Vec<Diagnostic>,
}

/// The incremental state of one file (one editor buffer or one loader module): create it once per
/// file and pass it to every [`check`] of that file. It holds the last result and the last
/// complete one (two copies of the source at most).
#[derive(Clone, Default)]
pub struct FileState {
    last: Option<Run>,
    /// The last run that went through the whole pipeline.
    last_complete: Option<Run>,
    stats: CheckStats,
}

impl FileState {
    pub fn new() -> FileState {
        FileState::default()
    }

    pub fn stats(&self) -> CheckStats {
        self.stats
    }

    /// Forgets the previous results (e.g. after the backend's spec changed in a way its
    /// fingerprint does not show).
    pub fn reset(&mut self) {
        self.last = None;
        self.last_complete = None;
    }
}

/// Checks `source` (the text of `path`, e.g. an unsaved editor buffer) against `backend`.
pub fn check(path: &str, source: &str, backend: &dyn PlatformSemantics, state: &mut FileState) -> CheckResult {
    check_with(path, source, backend, &CheckOptions::default(), state)
}

/// [`check`] with options.
pub fn check_with(path: &str, source: &str, backend: &dyn PlatformSemantics, opts: &CheckOptions, state: &mut FileState) -> CheckResult {
    let t = Instant::now();
    state.stats.checks += 1;
    let kind = FileKind::of(path);
    let key = Key {
        path: path.to_string(),
        backend: backend.backend().to_string(),
        fingerprint: backend.fingerprint(),
        portability: opts.portability.iter().map(|p| (p.backend().to_string(), p.fingerprint())).collect(),
        strict_native: opts.strict_native,
        dev: opts.dev,
        scoped_css: opts.scoped_css,
    };
    if let Some(last) = &state.last
        && key.reusable()
        && last.key == key
        && last.source == source
    {
        state.stats.unchanged += 1;
        let mut r = last.result.clone();
        r.reuse = Reuse::Unchanged;
        r.timings = CheckTimings { total_us: t.elapsed().as_micros() as u64, ..Default::default() };
        return r;
    }

    state.stats.runs += 1;
    let res: Option<CompileResult> = match kind {
        FileKind::Component => {
            let mut co = CompileOptions::new(path, backend);
            co.dev = opts.dev;
            co.strict_native = opts.strict_native;
            co.portability = opts.portability.clone();
            co.scoped_css = opts.scoped_css;
            Some(check_component(source, &co))
        }
        FileKind::Module => Some(compile_module(source, &ModuleOptions { filename: path.to_string(), dev: opts.dev })),
        FileKind::Other => None,
    };
    let (raw, timings) = match res {
        Some(r) => (r.diagnostics, r.timings),
        None => (Vec::new(), Timings::default()),
    };
    let lines = LineIndex::new(source);
    let mut diagnostics: Vec<CheckDiagnostic> = raw.iter().map(|d| convert(d, &lines, false)).collect();
    let mut reuse = Reuse::None;
    if timings.stopped {
        if let Some(prev) = state.last_complete.as_ref().filter(|p| p.key == key) {
            let carried = carry_over(&prev.source, source, &prev.raw, &raw);
            if !carried.is_empty() {
                reuse = Reuse::CarriedOver;
                state.stats.carried_over += 1;
                diagnostics.extend(carried.iter().map(|d| convert(d, &lines, true)));
                diagnostics.sort_by_key(|d| (d.range.start, d.range.end));
            }
        }
    }
    let result = CheckResult {
        path: path.to_string(),
        kind,
        diagnostics,
        reuse,
        timings: CheckTimings {
            parse_us: timings.parse_us,
            analyze_us: timings.analyze_us,
            lower_us: timings.lower_us,
            generate_us: timings.generate_us,
            total_us: t.elapsed().as_micros() as u64,
        },
    };
    let run = Run { key, source: source.to_string(), result: result.clone(), raw };
    if !timings.stopped {
        state.last_complete = Some(run.clone());
    }
    state.last = Some(run);
    result
}

fn position(lines: &LineIndex, src_len: u32, offset: u32) -> Position {
    let offset = offset.min(src_len);
    let (line, utf16) = lines.line_col(offset);
    Position { line: line + 1, column: offset - lines.line_start(line) + 1, utf16_column: utf16 + 1 }
}

fn range(lines: &LineIndex, src_len: u32, start: u32, end: u32) -> Range {
    Range { start, end, start_pos: position(lines, src_len, start), end_pos: position(lines, src_len, end) }
}

fn convert(d: &Diagnostic, lines: &LineIndex, stale: bool) -> CheckDiagnostic {
    let len = lines.len();
    CheckDiagnostic {
        id: codes::id_of(d.code),
        code: d.code,
        severity: d.severity,
        message: d.message.clone(),
        help: d.help.clone(),
        backend: d.backend.clone(),
        range: range(lines, len, d.start, d.end),
        // a stale diagnostic's fix-its were computed for the old text: they are dropped
        fixes: if stale {
            Vec::new()
        } else {
            d.fixes
                .iter()
                .map(|f| Fix {
                    message: f.message.clone(),
                    edits: f.edits.iter().map(|e| Edit { range: range(lines, len, e.start, e.end), text: e.text.clone() }).collect(),
                })
                .collect()
        },
        stale,
    }
}

/// The diagnostics of the last complete check (`prev`, over `old`) that still apply to `new`:
/// those outside the edited region, shifted by the edit. The ones the stopped run reported itself
/// (`now`, the syntax error) are not duplicated.
fn carry_over(old: &str, new: &str, prev: &[Diagnostic], now: &[Diagnostic]) -> Vec<Diagnostic> {
    let (prefix, old_end, new_end) = changed_region(old, new);
    let delta = new_end as i64 - old_end as i64;
    let mut out = Vec::new();
    for d in prev {
        let (s, e) = if d.end as usize <= prefix {
            (d.start, d.end)
        } else if d.start >= old_end as u32 {
            ((d.start as i64 + delta) as u32, (d.end as i64 + delta) as u32)
        } else {
            continue;
        };
        if now.iter().any(|n| n.code == d.code && n.start == s) {
            continue;
        }
        out.push(Diagnostic { start: s, end: e, ..d.clone() });
    }
    out
}

/// The region an edit changed: `(prefix, old_end, new_end)`, so `old[prefix..old_end]` became
/// `new[prefix..new_end]` (the longest common prefix and suffix, on character boundaries).
pub fn changed_region(old: &str, new: &str) -> (usize, usize, usize) {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let mut p = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    while !old.is_char_boundary(p) || !new.is_char_boundary(p) {
        p -= 1;
    }
    let max_suffix = a.len().min(b.len()) - p;
    let mut s = a.iter().rev().zip(b.iter().rev()).take(max_suffix).take_while(|(x, y)| x == y).count();
    while !old.is_char_boundary(a.len() - s) || !new.is_char_boundary(b.len() - s) {
        s -= 1;
    }
    (p, a.len() - s, b.len() - s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_region_finds_the_edit() {
        assert_eq!(changed_region("abcdef", "abXdef"), (2, 3, 3));
        assert_eq!(changed_region("abcdef", "abcdef"), (6, 6, 6));
        assert_eq!(changed_region("abc", "abXYc"), (2, 2, 4));
        assert_eq!(changed_region("aaaa", "aa"), (2, 4, 2));
        // multi-byte: never splits a character
        assert_eq!(changed_region("aé", "aè"), (1, 3, 3));
    }
}
