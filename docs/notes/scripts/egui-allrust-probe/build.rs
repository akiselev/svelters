//! Probe: run svelters-compiler from build.rs. Diagnostics surface as cargo warnings/errors with
//! .svelte file:line:col; the output is compiled to QuickJS bytecode at build time and embedded.
//! The gtk spec stands in for the (not yet written) egui spec.
use rquickjs::module::WriteOptions;
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Context, Ctx, Module, Runtime};

/// The runtime modules are not part of this probe: resolve them to empty stubs so `declare`
/// (which links imports in QuickJS-NG) succeeds. A real build walks the whole graph
/// (docs/notes/scripts/qjs-loader-probe) and writes every module.
struct Stub;
impl Resolver for Stub {
    fn resolve<'js>(&mut self, _: &Ctx<'js>, _base: &str, name: &str, _: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        Ok(name.to_string())
    }
}
impl Loader for Stub {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _: Option<ImportAttributes<'js>>) -> rquickjs::Result<Module<'js, Declared>> {
        Module::declare(ctx.clone(), name, "export {}")
    }
}
use std::time::Instant;
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn main() {
    let file = if std::env::var("PROBE_BAD").is_ok_and(|v| v == "1") { "ui/Bad.svelte" } else { "ui/App.svelte" };
    println!("cargo::rerun-if-changed=ui");
    println!("cargo::rerun-if-env-changed=PROBE_BAD");
    let src = std::fs::read_to_string(file).unwrap();
    let t = Instant::now();
    let backend = SpecBackend::builtin("gtk").unwrap();
    let t_spec = t.elapsed();
    let t = Instant::now();
    let r = compile(&src, &CompileOptions::new(file, &backend));
    let t_compile = t.elapsed();
    let abs = std::fs::canonicalize(file).unwrap();
    let mut errors = 0;
    for d in &r.diagnostics {
        // one line per message: cargo prints each `cargo::` line separately
        let text = d.render(&abs.display().to_string()).replace('\n', " | ");
        match d.severity {
            Severity::Error => {
                errors += 1;
                println!("cargo::error={text}");
            }
            Severity::Warning => println!("cargo::warning={text}"),
        }
    }
    if errors > 0 {
        return; // cargo::error fails the build after the script exits
    }
    let js = r.js.unwrap();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("App.js"), &js).unwrap();
    std::fs::write(out.join("App.js.map"), r.map.unwrap()).unwrap();

    let t = Instant::now();
    let rt = Runtime::new().unwrap();
    rt.set_loader(Stub, Stub);
    let ctx = Context::full(&rt).unwrap();
    let bc = ctx.with(|ctx| {
        // declare parses and compiles without resolving imports
        let m = match Module::declare(ctx.clone(), "App.svelte", js.clone()) { Ok(m) => m, Err(e) => panic!("{e}: {:?}\n{js}", ctx.catch()) };
        m.write(WriteOptions::default()).unwrap()
    });
    let t_bc = t.elapsed();
    std::fs::write(out.join("App.qbc"), &bc).unwrap();
    std::fs::write(
        out.join("stats.txt"),
        format!("spec load {t_spec:?}, compile {t_compile:?}, bytecode {t_bc:?}, js {} B, qbc {} B", js.len(), bc.len()),
    )
    .unwrap();
}
