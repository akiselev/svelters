//! The compiler pipeline: parse (Svelte port, native mode) -> scope analysis (Svelte port) ->
//! native style sheet -> native lowering (backend semantics) -> JS generation.

use crate::codegen::component::generate;
use crate::diag::{Diagnostic, Diagnostics};
use crate::lower::lower;
use crate::semantics::BackendSemantics;
use crate::style::parse_style;
use crate::svelte::errors::CompileError;
use crate::svelte::parse::{ParseOptions, parse};
use crate::svelte::scope::analyze::analyze_component_scopes;
use crate::transform::ScopeRewriter;
use serde::Serialize;
use std::time::Instant;

pub struct CompileOptions<'b> {
    /// Used in diagnostics and the source map; also names the component (`App.svelte` -> `App`).
    pub filename: String,
    pub backend: &'b dyn BackendSemantics,
}

#[derive(Debug, Default, Serialize)]
pub struct Timings {
    pub parse_us: u64,
    pub analyze_us: u64,
    pub lower_us: u64,
    pub generate_us: u64,
}

#[derive(Debug, Serialize)]
pub struct CompileResult {
    /// None when there are errors.
    pub js: Option<String>,
    pub map: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub backend: String,
    pub timings: Timings,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == crate::diag::Severity::Error)
    }
}

fn component_name(filename: &str) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let stem = base.strip_suffix(".svelte").unwrap_or(base);
    let mut name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn from_compile_error(diags: &mut Diagnostics, e: &CompileError) {
    diags.error(e.code, (e.start, e.end), e.message.clone());
}

pub fn compile(src: &str, opts: &CompileOptions) -> CompileResult {
    let mut timings = Timings::default();
    let backend = opts.backend.backend().to_string();
    let mut diags = Diagnostics::for_backend(&backend);
    let done = |js, map, diags: Diagnostics, timings| CompileResult { js, map, diagnostics: diags.finish(src), backend: backend.clone(), timings };

    let t = Instant::now();
    let mut root = match parse(src, &ParseOptions { loose: false, native: true }) {
        Ok(r) => r,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, timings);
        }
    };
    // `lang="ts"`: Svelte's `remove_typescript_nodes`, plus the source ranges to strip
    let ts_strip = match crate::svelte::ts::strip(&mut root, src) {
        Ok(s) => s,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, timings);
        }
    };
    if root.module.is_some() {
        let m = root.module.as_ref().unwrap();
        diags.error("unsupported_feature", (m.start, m.end), "<script module> is not supported by the native compiler yet");
    }
    timings.parse_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let analysis = match analyze_component_scopes(&root, Some(true)) {
        Ok(a) => a,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, timings);
        }
    };
    timings.analyze_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let sheet = root.css.as_ref().map(|c| parse_style(&c.content.styles, c.content.start, &mut diags));
    let ir = lower(&root, opts.backend, sheet.as_ref(), &mut diags);
    timings.lower_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let mut rw = ScopeRewriter::new(src, &root, &analysis, diags);
    rw.strip_typescript(ts_strip);
    let (code, map) = generate(src, &ir, &rw, &component_name(&opts.filename), &opts.filename);
    let diags = rw.into_diagnostics();
    timings.generate_us = t.elapsed().as_micros() as u64;
    if diags.has_errors() {
        return done(None, None, diags, timings);
    }
    done(Some(code), Some(map), diags, timings)
}
