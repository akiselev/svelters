//! The compiler pipeline: parse (Svelte port, native mode) -> scope analysis (Svelte port) ->
//! native style sheet -> native lowering (backend semantics) -> JS generation; and
//! [`compile_module`] for `.svelte.js` modules (parse -> module scope analysis -> rune transform).

use crate::codegen::component::{Features, IdSource, generate};
use crate::semantics::capability;
use crate::diag::{Diagnostic, Diagnostics};
use crate::lower::lower;
use crate::semantics::PlatformSemantics;
use crate::target::{QuickJsHost, RuntimeTarget};
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
    pub backend: &'b dyn PlatformSemantics,
    /// Svelte's `dev` option: `$inspect` / `$inspect.trace` are kept and sources are labelled
    /// (`$.tag`); otherwise they are stripped.
    pub dev: bool,
    /// The scope class of the component's X6 sheets (the Vite plugin compiles the `<style>` with
    /// Svelte and this `cssHash`): every native element gets it, so the scoped selectors match.
    pub css_scope: Option<String>,
    /// What the emitted module runs on (runtime imports, specifiers, dev hooks).
    pub target: &'b dyn RuntimeTarget,
    /// `--strict-native`: the observability warnings (`codes::STRICT_NATIVE`) are errors (decision 13 #5).
    pub strict_native: bool,
    /// Portability mode (`--backend gtk,qt,...`): the other platforms the same component is checked
    /// against; their answers only add `observability_differs` warnings (output stays per platform).
    pub portability: Vec<&'b dyn PlatformSemantics>,
    /// `--hmr` (dev only): the component is wrapped for in-place replacement
    /// (`if (import.meta.hot) { C = $n.hmr(C, id); ... }`); `filename` is the component id.
    pub hmr: bool,
    /// Q3w (`--css`): scope the `<style>` with the Rust port of Svelte's CSS pipeline
    /// ([`crate::scoped_css`]) and return it as [`CompileResult::css`], hashed with `css_scope`
    /// (else Svelte's default hash of `filename`, which then also becomes the elements' scope class).
    pub scoped_css: bool,
    /// U4: check utility classes (UTILITY-CSS; opt-in like the runtime's `EmbedConfig::utilities`)
    /// on a backend that runs the style engine (egui). `@apply` in `<style>` is expanded there
    /// either way (with this prefix when set).
    pub utilities: Option<crate::utilities::UtilityCheck>,
}

impl<'b> CompileOptions<'b> {
    pub fn new(filename: impl Into<String>, backend: &'b dyn PlatformSemantics) -> Self {
        CompileOptions { filename: filename.into(), backend, dev: false, css_scope: None, target: &QuickJsHost, strict_native: false, portability: Vec::new(), scoped_css: false, hmr: false, utilities: None }
    }
}

/// Options of [`compile_module`].
#[derive(Clone, Debug, Default)]
pub struct ModuleOptions {
    pub filename: String,
    pub dev: bool,
    /// `--hmr`: top-level `$state`, `writable(..)` and `createContext()` keep their values or objects across
    /// a re-evaluation of the module (`import.meta.hot.data`).
    pub hmr: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct Timings {
    pub parse_us: u64,
    pub analyze_us: u64,
    pub lower_us: u64,
    pub generate_us: u64,
    /// The pipeline stopped before lowering (a syntax or scope error): the later phases' diagnostics
    /// are missing, not absent (CX1's check carries the last complete ones over).
    #[serde(skip)]
    pub stopped: bool,
}

#[derive(Debug, Serialize)]
pub struct CompileResult {
    /// None when there are errors.
    pub js: Option<String>,
    pub map: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub backend: String,
    pub timings: Timings,
    /// With `CompileOptions::scoped_css`: the scoped `<style>` (None without one, or when the CSS
    /// step failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css: Option<crate::scoped_css::ScopedCss>,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == crate::diag::Severity::Error)
    }
}

/// Svelte's `get_component_name` (2-analyze/index.js): the file's base name without `.svelte`
/// (`index.svelte` takes its directory's name, except under `src`), first letter upper-cased, so a
/// file named like a keyword (`await.svelte`) still gives a valid function name. The caller makes
/// it a valid, unique identifier (`scope.generate`).
fn component_name(filename: &str) -> String {
    let mut parts: Vec<&str> = filename.split(['/', '\\']).collect();
    let base = parts.pop().unwrap_or(filename);
    let last_dir = parts.last().copied().filter(|d| !d.is_empty());
    let mut stem = base.replacen(".svelte", "", 1);
    if stem == "index"
        && let Some(d) = last_dir
        && d != "src"
    {
        stem = d.to_string();
    }
    let mut cs = stem.chars();
    let mut name: String = match cs.next() {
        Some(c) => c.to_uppercase().chain(cs).collect(),
        None => String::new(),
    };
    name = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn from_compile_error(diags: &mut Diagnostics, e: &CompileError) {
    diags.error(e.code, (e.start, e.end), e.message.clone());
}

pub fn compile(src: &str, opts: &CompileOptions) -> CompileResult {
    run_component(src, opts, false)
}

/// The diagnostics of [`compile`] without its output (CX1, `crate::check`): the same pipeline, but
/// code generation runs as a traversal that writes nothing ([`crate::codegen::component::traverse`]),
/// and `js`/`map` are `None`. The diagnostics are the same as [`compile`]'s (`cx1_check` tests).
pub fn check_component(src: &str, opts: &CompileOptions) -> CompileResult {
    run_component(src, opts, true)
}

fn run_component(src: &str, opts: &CompileOptions, check: bool) -> CompileResult {
    let mut timings = Timings::default();
    let backend = opts.backend.backend().to_string();
    let mut diags = Diagnostics::for_backend(&backend);
    let done = |js, map, diags: Diagnostics, timings| CompileResult { js, map, diagnostics: diags.finish(src), backend: backend.clone(), timings, css: None };

    let t = Instant::now();
    let mut root = match parse(src, &ParseOptions { loose: false, native: !opts.backend.html_syntax() }) {
        Ok(r) => r,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, Timings { stopped: true, ..timings });
        }
    };
    // `lang="ts"`: Svelte's `remove_typescript_nodes`, plus the source ranges to strip
    let ts_strip = match crate::svelte::ts::strip(&mut root, src) {
        Ok(s) => s,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, Timings { stopped: true, ..timings });
        }
    };
    timings.parse_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let analysis = match analyze_component_scopes(&root, Some(true)) {
        Ok(a) => a,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, Timings { stopped: true, ..timings });
        }
    };
    timings.analyze_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    // U4: `@apply` expanded before scoping and the engine's check
    let no_utilities = crate::utilities::UtilityCheck::default();
    let apply = crate::utilities::apply::expand(src, &root, opts.backend, opts.utilities.as_ref().unwrap_or(&no_utilities), &mut diags);
    // presto `css_precompiled`: release builds get the sheet as bytes too (dev keeps the text path
    // for HMR CSS edits)
    let utility_tokens = opts.utilities.as_ref().map(|u| u.utility_tokens());
    let precompile = (opts.scoped_css && !opts.dev && opts.backend.has_capability(capability::CSS_PRECOMPILED)).then(|| crate::scoped_css::Precompile {
        utilities: opts.utilities.as_ref().zip(utility_tokens.as_ref()).zip(svelters_style::Table::for_toolkit(opts.backend.backend())).map(|((u, tokens), table)| {
            let mut r = svelters_style::Resolver::new(table, tokens);
            r.prefix = u.prefix.as_deref();
            r
        }),
    });
    let css = if opts.scoped_css {
        crate::scoped_css::scoped_css(src, &root, &analysis, &opts.filename, opts.css_scope.as_deref(), opts.dev, apply.as_ref(), precompile.as_ref(), &mut diags)
    } else {
        None
    };
    let css_scope = opts.css_scope.as_deref().or(css.as_ref().map(|c| c.hash.as_str()));
    let is_setter = |name: &str| opts.backend.is_setter_property(name);
    let sheet = root.css.as_ref().map(|c| parse_style(&c.content.styles, c.content.start, &mut diags, &is_setter));
    let mut ir = lower(&root, opts.backend, sheet.as_ref(), css_scope, apply.as_ref(), &mut diags);
    // presto (`plans/research/presto/compiler-and-js.md` §3): change classes (W-C4), the
    // accessibility checks (W-C9), then templates (W-C2) for a backend that takes them
    crate::lower::change::tag(&mut ir.root, opts.backend, src, &root, &mut diags, opts.dev);
    crate::lower::a11y::check(&ir.root, opts.backend, &mut diags);
    if opts.backend.has_capability(capability::TEMPLATES) {
        crate::lower::templates::wrap(&mut ir.root, &mut 0);
    }
    if let Some(u) = &opts.utilities {
        crate::utilities::check(&root, src, opts.backend, u, &mut diags);
    }
    if analysis.template.has_await {
        crate::lower::template_await::check(&root, &mut diags);
    }
    crate::lower::native_reads::check(&root, &analysis, opts.backend, &mut diags);
    if !opts.portability.is_empty() {
        let mut all: Vec<&dyn PlatformSemantics> = vec![opts.backend];
        all.extend(opts.portability.iter().copied());
        crate::lower::portability::check(&root, &all, &mut diags);
    }
    if opts.strict_native {
        diags.promote(crate::semantics::codes::STRICT_NATIVE);
    }
    timings.lower_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let mut rw = ScopeRewriter::with_options(src, &root, &analysis, diags, opts.dev, &opts.filename, opts.hmr);
    rw.strip_typescript(ts_strip);
    rw.set_native_motion(opts.backend.has_capability(capability::NATIVE_TRANSITIONS), opts.backend.has_capability(capability::NATIVE_MOTION));
    let name = rw.component_name(&component_name(&opts.filename));
    let out = if check {
        crate::codegen::component::traverse(src, &ir, &rw, &name, &opts.target.runtime_imports());
        None
    } else {
        {
        // DZ0: in dev, the kinds that take the backend's `source-loc` prop (memoized per tag)
        let cache = std::cell::RefCell::new(std::collections::HashMap::<String, bool>::new());
        let has_loc = |tag: &str| {
            *cache.borrow_mut().entry(tag.to_string()).or_insert_with(|| opts.backend.resolve_element(tag).is_ok_and(|el| opts.backend.source_loc(&el)))
        };
        let locs: Option<&dyn Fn(&str) -> bool> = (opts.dev && opts.backend.source_locs()).then_some(&has_loc);
        let ids = SpecIds::new(opts.backend);
        let features = Features {
            ids: ids.as_ref().map(|i| i as &dyn IdSource),
            change_classes: opts.backend.has_capability(capability::CHANGE_CLASSES),
        };
        Some(generate(src, &ir, &rw, &name, &opts.filename, &opts.target.runtime_imports(), opts.hmr, locs, &features))
    }
    };
    let diags = rw.into_diagnostics();
    timings.generate_us = t.elapsed().as_micros() as u64;
    let res = match out {
        Some((code, map)) if !diags.has_errors() => done(Some(code), Some(map), diags, timings),
        _ => done(None, None, diags, timings),
    };
    CompileResult { css, ..res }
}

/// W-C1: the stable ids of a backend with the `ids` capability and a spec hash, as codegen asks
/// for them.
struct SpecIds<'b> {
    backend: &'b dyn PlatformSemantics,
    hash: &'b str,
}

impl<'b> SpecIds<'b> {
    fn new(backend: &'b dyn PlatformSemantics) -> Option<SpecIds<'b>> {
        let hash = backend.spec_hash().filter(|_| backend.has_capability(capability::IDS))?;
        Some(SpecIds { backend, hash })
    }
}

impl IdSource for SpecIds<'_> {
    fn backend(&self) -> &str {
        self.backend.backend()
    }
    fn spec_hash(&self) -> &str {
        self.hash
    }
    fn kind(&self, tag: &str) -> Option<(u32, String)> {
        let el = self.backend.resolve_element(tag).ok()?;
        Some((self.backend.kind_id(&el)?, el.kind))
    }
    fn prop(&self, tag: &str, prop: &str) -> Option<(u32, String, bool)> {
        use crate::semantics::PropType as T;
        let el = self.backend.resolve_element(tag).ok()?;
        let p = self.backend.property(&el, prop).ok()?;
        let scalar = matches!(p.ty, T::Bool | T::I32 | T::U32 | T::I64 | T::F64 | T::String | T::Enum);
        Some((p.id?, p.owner.clone(), scalar))
    }
}

/// Parses a module, with TypeScript on and stripped when `filename` is TypeScript
/// ([`crate::svelte::ts::is_ts_module`]).
fn parse_module(src: &str, filename: &str) -> Result<(crate::estree::Program, crate::svelte::ts::TsStrip), CompileError> {
    let ts = crate::svelte::ts::is_ts_module(filename);
    let mut comments = Vec::new();
    let mut program = crate::svelte::parse::acorn::parse(src, &mut comments, ts, false)?;
    let strip = if ts { crate::svelte::ts::strip_program(&mut program, src)? } else { Default::default() };
    Ok((program, strip))
}

/// Compiles a `.svelte.js` module (Svelte's `compileModule`, client output): runes in module
/// scope become runtime calls against the core (`@svelters/native-runtime/core`).
///
/// SVELTERS: a `.svelte.ts` filename parses with TypeScript and strips it first (upstream needs a
/// preprocessor for that), with the same `typescript_invalid_feature` errors as `<script lang="ts">`.
pub fn compile_module(src: &str, opts: &ModuleOptions) -> CompileResult {
    let mut timings = Timings::default();
    let mut diags = Diagnostics::default();
    let done = |js, map, diags: Diagnostics, timings| CompileResult { js, map, diagnostics: diags.finish(src), backend: String::new(), timings, css: None };

    let t = Instant::now();
    let (program, ts) = match parse_module(src, &opts.filename) {
        Ok(p) => p,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, Timings { stopped: true, ..timings });
        }
    };
    timings.parse_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let analysis = match crate::svelte::scope::analyze::analyze_module_scopes(&program) {
        Ok(a) => a,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            return done(None, None, diags, Timings { stopped: true, ..timings });
        }
    };
    timings.analyze_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let (code, map, diags) = crate::transform::transform_module(src, &program, &analysis, opts.dev, &opts.filename, opts.hmr, &ts, diags);
    timings.generate_us = t.elapsed().as_micros() as u64;
    if diags.has_errors() {
        return done(None, None, diags, timings);
    }
    done(Some(code), Some(map), diags, timings)
}

/// Strips the types from a plain TypeScript module (`.ts`, `.mts`): no runes, no runtime imports,
/// the JavaScript left as written (with a source map). Errors are `typescript_invalid_feature`
/// (enums, namespaces with values, decorators, ...) and syntax errors, as in `<script lang="ts">`.
pub fn strip_typescript_module(src: &str, filename: &str) -> CompileResult {
    let mut diags = Diagnostics::default();
    let t = Instant::now();
    let mut comments = Vec::new();
    let res = crate::svelte::parse::acorn::parse(src, &mut comments, true, false).and_then(|mut p| crate::svelte::ts::strip_program(&mut p, src));
    let strip = match res {
        Ok(s) => s,
        Err(e) => {
            from_compile_error(&mut diags, &e);
            let timings = Timings { stopped: true, parse_us: t.elapsed().as_micros() as u64, ..Timings::default() };
            return CompileResult { js: None, map: None, diagnostics: diags.finish(src), backend: String::new(), timings, css: None };
        }
    };
    let parse_us = t.elapsed().as_micros() as u64;
    let t = Instant::now();
    let mut edits = crate::codegen::edit::Edits::new();
    strip.apply(&mut edits, None);
    let mut w = crate::codegen::writer::Writer::new(src);
    edits.emit(&mut w, src, 0, src.len() as u32);
    let base = filename.rsplit('/').next().unwrap_or(filename);
    let (code, map) = w.finish(filename, src, base);
    let timings = Timings { parse_us, generate_us: t.elapsed().as_micros() as u64, ..Timings::default() };
    CompileResult { js: Some(code), map: Some(map), diagnostics: diags.finish(src), backend: String::new(), timings, css: None }
}
