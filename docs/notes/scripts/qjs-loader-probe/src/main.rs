//! Probe for plans/research/QUICKJS-RUNTIME.md: can the host load an app straight from source,
//! with no Vite in the loop?
//!
//!   qjs-loader-probe <entry.svelte|entry.js> <modules-base-dir>
//!
//! - Resolver: oxc_resolver (Node ESM resolution: package.json `exports`/`imports`, conditions),
//!   plus the alias table packages/native-runtime/vite.js applies (Svelte's hidden internal
//!   modules, @svelters/native-runtime). Bare specifiers that the importer's directory cannot
//!   resolve fall back to <modules-base-dir> (stand-in for "the app's package root").
//! - Loader: `.svelte` -> svelters_compiler::compile (backend gtk), `.svelte.js` ->
//!   compile_module, everything else read as is.
//! - Pass 1 loads and evaluates from source, recording every module's bytecode with
//!   `Module::write`. Pass 2 evaluates a fresh runtime from the bytecode table only (the release
//!   shape: an embedded module table). Both are timed.

use oxc_resolver::{AliasValue, ResolveOptions, Resolver as OxcResolver};
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::{Declared, WriteOptions};
use rquickjs::{CatchResultExt, Context, Ctx, Module, Runtime, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_compiler::compile::{CompileOptions, ModuleOptions, compile, compile_module};
use svelters_compiler::semantics::SpecBackend;

#[derive(Default)]
struct Stats {
    modules: usize,
    compiled: usize,
    resolve: Duration,
    read: Duration,
    compile: Duration,
    declare: Duration,
    write: Duration,
    max_depth: usize,
    bytes_src: usize,
    bytecode: BTreeMap<String, Vec<u8>>,
}

thread_local!(static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) });

struct SrcResolver {
    oxc: OxcResolver,
    fallback: PathBuf,
    stats: Rc<RefCell<Stats>>,
}

impl Resolver for SrcResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str, _a: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        let t = Instant::now();
        let dir = Path::new(base).parent().unwrap_or(Path::new("/"));
        let res = self.oxc.resolve(dir, name).or_else(|_| self.oxc.resolve(&self.fallback, name));
        self.stats.borrow_mut().resolve += t.elapsed();
        match res {
            Ok(r) => Ok(r.full_path().to_string_lossy().into_owned()),
            Err(e) => Err(rquickjs::Error::new_resolving_message(base, name, e.to_string())),
        }
    }
}

struct SrcLoader {
    backend: SpecBackend,
    stats: Rc<RefCell<Stats>>,
}

impl SrcLoader {
    fn source(&self, path: &str) -> Result<String, String> {
        let t = Instant::now();
        let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut st = self.stats.borrow_mut();
        st.read += t.elapsed();
        st.bytes_src += src.len();
        let t = Instant::now();
        let out = if path.ends_with(".svelte") {
            st.compiled += 1;
            let r = compile(&src, &CompileOptions::new(path, &self.backend));
            r.js.ok_or_else(|| format!("{path}: {:?}", r.diagnostics.iter().map(|d| d.render(path)).collect::<Vec<_>>()))?
        } else if path.ends_with(".svelte.js") {
            st.compiled += 1;
            let r = compile_module(&src, &ModuleOptions { filename: path.into(), dev: false });
            r.js.ok_or_else(|| format!("{path}: module compile failed"))?
        } else {
            src
        };
        st.compile += t.elapsed();
        Ok(out)
    }
}

impl Loader for SrcLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _a: Option<ImportAttributes<'js>>) -> rquickjs::Result<Module<'js, Declared>> {
        let src = self.source(name).map_err(|m| rquickjs::Error::new_loading_message(name, m))?;
        let depth = DEPTH.with(|d| { d.set(d.get() + 1); d.get() });
        let t = Instant::now();
        let m = Module::declare(ctx.clone(), name, src)?;
        let td = t.elapsed();
        let t = Instant::now();
        let bc = m.write(write_opts())?;
        let tw = t.elapsed();
        DEPTH.with(|d| d.set(d.get() - 1));
        let mut st = self.stats.borrow_mut();
        st.max_depth = st.max_depth.max(depth);
        st.declare += td;
        st.write += tw;
        st.modules += 1;
        st.bytecode.insert(name.to_string(), bc);
        Ok(m)
    }
}

/// Release shape: specifiers were resolved at build time; here we only look names up.
struct TableResolver(Rc<BTreeMap<String, String>>);
impl Resolver for TableResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str, _a: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        self.0
            .get(&format!("{base}\0{name}"))
            .cloned()
            .ok_or_else(|| rquickjs::Error::new_resolving_message(base, name, "not in table"))
    }
}
struct TableLoader(Rc<BTreeMap<String, Vec<u8>>>);
impl Loader for TableLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _a: Option<ImportAttributes<'js>>) -> rquickjs::Result<Module<'js, Declared>> {
        let bc = self.0.get(name).ok_or_else(|| rquickjs::Error::new_loading_message(name, "not in table"))?;
        // Safety: bytecode written by this process from the same QuickJS build.
        unsafe { Module::load(ctx.clone(), bc) }
    }
}

/// Records base\0name -> resolved, so pass 2 can resolve without a filesystem.
struct RecordingResolver(SrcResolver, Rc<RefCell<BTreeMap<String, String>>>);
impl Resolver for RecordingResolver {
    fn resolve<'js>(&mut self, ctx: &Ctx<'js>, base: &str, name: &str, a: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        let r = self.0.resolve(ctx, base, name, a)?;
        self.1.borrow_mut().insert(format!("{base}\0{name}"), r.clone());
        Ok(r)
    }
}

fn eval_entry(ctx: &Ctx<'_>, name: &str, src: Option<String>, bc: Option<&[u8]>) -> Result<(), String> {
    let m = match (src, bc) {
        (Some(s), _) => Module::declare(ctx.clone(), name, s),
        (None, Some(b)) => unsafe { Module::load(ctx.clone(), b) },
        _ => unreachable!(),
    }
    .catch(ctx)
    .map_err(|e| e.to_string())?;
    let (_m, p) = m.eval().catch(ctx).map_err(|e| e.to_string())?;
    while ctx.execute_pending_job() {}
    match p.result::<Value>() {
        Some(Err(e)) => Err(format!("{e:?}: {:?}", ctx.catch())),
        _ => Ok(()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entry = std::fs::canonicalize(&args[0]).expect("entry");
    let base = std::fs::canonicalize(&args[1]).expect("modules base");
    let rt_dir = base.join("src");
    let svelte_client = std::fs::canonicalize(base.join("node_modules/svelte/src/internal/client")).unwrap();
    let alias = |k: &str, v: PathBuf| (k.to_string(), vec![AliasValue::Path(v.to_string_lossy().into_owned())]);
    let opts = ResolveOptions {
        condition_names: vec!["custom-renderer".into(), "browser".into(), "import".into(), "module".into(), "production".into(), "default".into()],
        extensions: vec![".js".into(), ".mjs".into(), ".svelte".into(), ".json".into()],
        alias: vec![
            alias("@svelters/native-runtime$", rt_dir.join("index.js")),
            alias("svelte/internal/client/reactivity/effects", svelte_client.join("reactivity/effects.js")),
            alias("svelte/internal/client/dom/task", svelte_client.join("dom/task.js")),
            alias("svelte/internal/client/context", svelte_client.join("context.js")),
        ],
        ..ResolveOptions::default()
    };

    let stats = Rc::new(RefCell::new(Stats::default()));
    let table = Rc::new(RefCell::new(BTreeMap::new()));
    let name = entry.to_string_lossy().into_owned();

    // Pass 1: from source.
    let t0 = Instant::now();
    let rt = Runtime::new().unwrap();
    rt.set_max_stack_size(4 << 20);
    rt.set_loader(
        RecordingResolver(SrcResolver { oxc: OxcResolver::new(opts), fallback: base.clone(), stats: stats.clone() }, table.clone()),
        SrcLoader { backend: SpecBackend::builtin("gtk").unwrap(), stats: stats.clone() },
    );
    let ctx = Context::full(&rt).unwrap();
    let entry_src = SrcLoader { backend: SpecBackend::builtin("gtk").unwrap(), stats: stats.clone() }.source(&name).unwrap();
    let entry_bc = ctx.with(|ctx| {
        let bc = Module::declare(ctx.clone(), name.as_str(), entry_src.clone()).unwrap().write(write_opts()).unwrap();
        let r = eval_entry(&ctx, &format!("{name}#1"), Some(entry_src.clone()), None);
        println!("pass 1 (source) eval: {r:?}");
        bc
    });
    let pass1 = t0.elapsed();
    let st = stats.borrow();
    let bc_total: usize = st.bytecode.values().map(Vec::len).sum::<usize>() + entry_bc.len();
    println!(
        "pass 1: {:?} total; {} modules ({} compiled by svelters-compiler), {} KiB source; resolve {:?}, read {:?}, compile {:?}, declare {:?} (max nesting {}), write {:?}",
        pass1, st.modules, st.compiled, st.bytes_src / 1024, st.resolve, st.read, st.compile, st.declare, st.max_depth, st.write
    );
    println!("bytecode table: {} KiB (strip={})", bc_total / 1024, std::env::var("QJS_PROBE_STRIP").is_ok_and(|v| v == "1"));
    let bytecode = Rc::new(st.bytecode.clone());
    drop(st);
    drop(ctx);
    drop(rt);

    // Pass 2: from the bytecode table only.
    let t0 = Instant::now();
    let rt = Runtime::new().unwrap();
    rt.set_max_stack_size(4 << 20);
    let mut tbl = table.borrow().clone();
    // The entry was declared as "<name>#1" in pass 1; its imports resolve the same from "<name>".
    for (k, v) in table.borrow().iter() {
        if let Some(rest) = k.strip_prefix(&format!("{name}#1\0")) {
            tbl.insert(format!("{name}\0{rest}"), v.clone());
        }
    }
    rt.set_loader(TableResolver(Rc::new(tbl)), TableLoader(bytecode));
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        let r = eval_entry(&ctx, &name, None, Some(&entry_bc));
        println!("pass 2 (bytecode) eval: {r:?}");
    });
    println!("pass 2: {:?} total", t0.elapsed());
}

/// `QJS_PROBE_STRIP=1` strips source and debug info (no line numbers in stacks).
fn write_opts() -> WriteOptions {
    let strip = std::env::var("QJS_PROBE_STRIP").is_ok_and(|v| v == "1");
    WriteOptions { strip_source: strip, strip_debug: strip, ..WriteOptions::default() }
}
