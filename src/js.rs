use std::{
    collections::HashMap,
    fs::{canonicalize, read_to_string},
    io::{stdout, BufWriter},
    path::{Path, PathBuf},
    sync::mpsc,
    sync::Arc,
    thread,
};

use anyhow::Result;
use parking_lot::FairMutex;
use rquickjs::{intrinsic::Date, Context, Object};
use swc_bundler::{BundleKind, Bundler, Config, Hook, Load, ModuleData, ModuleRecord, Resolve};
use swc_common::{
    errors::{ColorConfig, Handler},
    sync::Lrc,
    FileLoader, FileName, FilePathMapping, Globals, SourceMap, Span,
};
use swc_ecma_ast::KeyValueProp;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{lexer::Lexer, EsConfig, Parser, StringInput, Syntax, TsConfig};

type Request = (String, mpsc::SyncSender<Response>);
type Response = Result<String>;

struct Loader {
    tx: mpsc::SyncSender<Request>,
}

impl Loader {
    pub fn new() -> Result<Loader> {
        let (tx, rx) = mpsc::sync_channel::<Request>(0);
        thread::spawn(move || {
            let rt = rquickjs::Runtime::new().unwrap();
            rt.set_max_stack_size(100 * 1024 * 1024);
            let ctx = Context::full(&rt).unwrap();
            ctx.with(|ctx| {
                let _: () = ctx
                    .eval_global_file("./src/js/polyfills/node.URL.js")
                    .unwrap();
                let _: () = ctx.eval_global_file("./src/js/polyfills/self.js").unwrap();
                let _: () = ctx
                    .eval_global_file("./node_modules/svelte/compiler.js")
                    .unwrap();
            });
            while let Ok((path, tx)) = rx.recv() {
                tracing::info!("Loading {}", path);
                let response = if path.ends_with(".svelte") {
                    Loader::compile_component(&ctx, path)
                } else {
                    if let Ok(data) = read_to_string(path) {
                        Ok(data)
                    } else {
                        Err(anyhow::anyhow!("..."))
                    }
                };
                tx.send(response).unwrap();
            }
        });
        Ok(Loader { tx })
    }

    fn compile_component(ctx: &Context, path: String) -> Result<String> {
        let contents = read_to_string(path)?;
        let res = ctx.with(move |ctx| {
            let global = ctx.globals();
            global.set("source", contents).unwrap();
            ctx.eval_file::<String, _>("./src/js/compile.js")
        });
        Ok(res?)
    }
}

impl FileLoader for Loader {
    fn file_exists(&self, path: &Path) -> bool {
        if let Ok(_) = canonicalize(path) {
            true
        } else {
            false
        }
    }

    fn abs_path(&self, path: &Path) -> Option<PathBuf> {
        if let Ok(path) = canonicalize(path) {
            Some(path)
        } else {
            None
        }
    }

    fn read_file(&self, path: &Path) -> std::io::Result<String> {
        let (tx, rx) = mpsc::sync_channel::<Response>(0);
        self.tx
            .send((path.to_string_lossy().to_string(), tx))
            .unwrap();
        match rx.recv() {
            Ok(Ok(contents)) => Ok(contents),
            _ => Err(std::io::ErrorKind::InvalidData.into()),
        }
    }
}

pub fn compile_svelte(path: &str) -> Result<String> {
    let file_loader: Box<dyn FileLoader + Send + Sync + 'static> = Box::new(Loader::new()?);
    let path_mapping = FilePathMapping::empty();
    let globals = Globals::new();
    let cm = Lrc::new(SourceMap::with_file_loader(file_loader, path_mapping));
    // This example does not use core modules.
    let external_modules = vec![];
    // let external_modules = vec!["svelte/internal".into()];
    // let external_modules = vec!["svelte".into(), "svelte/internal".into()];
    let bundler = Bundler::new(
        &globals,
        cm.clone(),
        PathLoader { cm: cm.clone() },
        PathResolver,
        Config {
            require: true,
            external_modules,
            ..Default::default()
        },
        Box::new(Noop),
    );
    let mut entries = HashMap::default();
    entries.insert("main".to_string(), FileName::Real(path.into()));

    let mut bundles = bundler.bundle(entries).expect("failed to bundle");
    assert_eq!(
        bundles.len(),
        1,
        "There's no conditional / dynamic imports and we provided only one entry"
    );
    let bundle = bundles.pop().ok_or(anyhow::anyhow!("..."))?;
    assert_eq!(
        bundle.kind,
        BundleKind::Named {
            name: "main".into()
        },
        "We provided it"
    );
    let mut wr = Vec::new();
    {
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config { minify: false },
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut wr, None)),
        };

        emitter.emit_module(&bundle.module)?;
    }
    // Safe because Javascript can be invalid utf8
    Ok(unsafe { String::from_utf8_unchecked(wr) })
}

struct PathLoader {
    cm: Lrc<SourceMap>,
}

impl Load for PathLoader {
    fn load(&self, file: &FileName) -> Result<ModuleData> {
        println!("{:?}", file);
        let file = match file {
            FileName::Real(v) => canonicalize(v).unwrap(),
            FileName::Custom(name) if name == "svelte/internal" => {
                canonicalize("./node_modules/svelte/internal/index.mjs").unwrap()
            }
            FileName::Custom(name) if name == "svelte" => {
                canonicalize("./node_modules/svelte/index.mjs").unwrap()
            }
            _ => unreachable!(),
        };

        let fm = self.cm.load_file(&file)?;
        let lexer = Lexer::new(
            Syntax::Typescript(TsConfig {
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        match parser.parse_module() {
            Ok(module) => Ok(ModuleData {
                fm,
                module,
                helpers: Default::default(),
            }),
            Err(err) => {
                let handler = Arc::new(Handler::with_tty_emitter(
                    ColorConfig::Auto,
                    true,
                    false,
                    Some(self.cm.clone()),
                ));
                let mut diag = err.into_diagnostic(&handler);
                diag.emit();
                Err(anyhow::anyhow!("Parser Error"))
            }
        }
    }
}

struct PathResolver;

impl Resolve for PathResolver {
    fn resolve(&self, base: &FileName, module_specifier: &str) -> Result<FileName> {
        println!("{:?}", module_specifier);
        if module_specifier.starts_with(".") {
            let base = match base {
                FileName::Real(v) => v,
                _ => unreachable!(),
            };
            if module_specifier.starts_with("./internal/index") {
                return Ok(FileName::Real(
                    canonicalize("./node_modules/svelte/internal/index.mjs").unwrap(),
                ));
            }
            let path = base
                .parent()
                .unwrap()
                .join(module_specifier.replace("./", ""));
            let path = if let Ok(path) = canonicalize(&path.with_extension("svelte")) {
                path
            } else if let Ok(path) = canonicalize(&path.with_extension("ts")) {
                path
            } else {
                path.with_extension("js")
            };
            Ok(FileName::Real(path))
        } else {
            if module_specifier == "svelte" {
                Ok(FileName::Real(
                    canonicalize("./node_modules/svelte/index.mjs").unwrap(),
                ))
            } else if module_specifier == "svelte/internal" {
                Ok(FileName::Real(
                    canonicalize("./node_modules/svelte/internal/index.mjs").unwrap(),
                ))
            } else {
                Err(anyhow::format_err!(
                    "Module {} not implemented",
                    module_specifier
                ))
            }
        }
    }
}

struct Noop;

impl Hook for Noop {
    fn get_import_meta_props(&self, _: Span, _: &ModuleRecord) -> Result<Vec<KeyValueProp>> {
        unimplemented!()
    }
}
