use std::{
    collections::HashMap,
    convert::TryInto,
    fs::{canonicalize, read_to_string},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use anyhow::Result;
use rquickjs::{
    bind, generic_loader, BuiltinResolver, Context, Created, Ctx, Func, Loaded, Loader as JsLoader,
    Module, ModuleDef, ModuleLoader, Native, Script,
};
use swc_common::{
    errors::{ColorConfig, Handler},
    sync::Lrc,
    FileLoader, FilePathMapping, SourceMap,
};
use swc_ecma_codegen::{self, text_writer::JsWriter, Emitter};
use swc_ecma_parser::{lexer::Lexer, Parser as JsParser, StringInput, Syntax, TsConfig};

pub struct Loader {
    cm: Lrc<SourceMap>,
    ctx: rquickjs::Context,
    _rt: rquickjs::Runtime,
}

impl Loader {
    pub fn new() -> Loader {
        let path_mapping = FilePathMapping::empty();
        let cm = Lrc::new(SourceMap::with_file_loader(
            Box::new(FsLoader),
            path_mapping,
        ));
        let _rt = rquickjs::Runtime::new().unwrap();
        _rt.set_max_stack_size(100 * 1024 * 1024);
        let ctx = Context::full(&_rt).unwrap();
        {
            let cm = cm.clone();
            ctx.with(|ctx| {
                let _: () = ctx
                    .eval_global_file("./src/js/polyfills/node.URL.js")
                    .unwrap();
                let _: () = ctx.eval_global_file("./src/js/polyfills/self.js").unwrap();
                let _: () = ctx.eval_global_file("./src/js/compiler.js").unwrap();
                let _: () = ctx
                    .eval_global_file("./node_modules/svelte/compiler.js")
                    .unwrap();

                let globals = ctx.globals();
                globals.init_def::<Transpiler>().unwrap();
                globals
                    .set("transpiler", transpiler::Transpiler::new(cm))
                    .unwrap();
            });
        }
        Loader { cm, _rt, ctx }
    }

    fn compile_component(&self, src: String) -> Result<String> {
        let res = self.ctx.with(move |ctx| {
            let global = ctx.globals();
            global.set("source", src).unwrap();
            ctx.eval_file::<String, _>("./src/js/compile.js")
        });
        Ok(res?)
    }

    fn transpile<T: AsRef<Path>>(&mut self, path: T) -> Result<String> {
        let src = self.cm.load_file(path.as_ref())?;
        transpile(&self.cm, (&*src).into())
    }
}

impl JsLoader<Script> for Loader {
    fn load<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: &str,
    ) -> Result<Module<'js, Loaded<Script>>, rquickjs::Error> {
        let src = if name.ends_with(".svelte") {
            let src = read_to_string(name)?;
            self.compile_component(src)
                .map_err(|err| rquickjs::Error::Loading {
                    name: name.to_string(),
                    message: Some(format!("Failed to compile Svelte component: {}", err)),
                })?
        } else {
            self.transpile(name)
                .map_err(|err| rquickjs::Error::Loading {
                    name: name.to_string(),
                    message: Some(err.to_string()),
                })?
        };
        let module = Module::new(ctx, name, src).map_err(|err| rquickjs::Error::Loading {
            name: name.to_string(),
            message: Some(err.to_string()),
        })?;
        Ok(module)
    }
}

struct FsLoader;

impl FileLoader for FsLoader {
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
        std::fs::read_to_string(path)
    }
}

fn transpile(cm: &Lrc<SourceMap>, input: StringInput) -> Result<String> {
    let mut parser: JsParser<Lexer<StringInput>> = JsParser::new(
        Syntax::Typescript(TsConfig {
            tsx: false,
            decorators: true,
            dynamic_import: true,
            dts: false,
            no_early_errors: false,
            import_assertions: false,
        }),
        input,
        None,
    );

    let parser_result = parser.parse_typescript_module();

    match parser_result {
        Ok(module) => {
            let mut output = Vec::new();
            {
                let mut emitter = Emitter {
                    cfg: swc_ecma_codegen::Config { minify: false },
                    cm: cm.clone(),
                    comments: None,
                    wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut output, None)),
                };
                emitter.emit_module(&module)?;
            }
            // Safe because Javascript can be invalid utf8
            Ok(unsafe { String::from_utf8_unchecked(output) })
        }
        Err(err) => {
            let handler = Arc::new(Handler::with_tty_emitter(
                ColorConfig::Auto,
                true,
                false,
                Some(cm.clone()),
            ));
            let mut diag = err.into_diagnostic(&handler);
            diag.emit();
            Err(rquickjs::Error::Unknown.into())
        }
    }
}

#[bind(object, public)]
#[quickjs(bare)]
mod transpiler {
    use super::*;
    use rquickjs::bind;

    pub struct Transpiler {
        // field properties
        pub(crate) cm: Lrc<SourceMap>,
    }

    impl Transpiler {
        pub fn transpile(&self, code: String) -> Result<String, rquickjs::Error> {
            Ok("ttest".to_string())
        }
    }
}

impl transpiler::Transpiler {
    pub fn new(cm: Lrc<SourceMap>) -> Self {
        Self { cm }
    }
}

generic_loader! {
    // Without bounds and metas
    // The `Loader<Script>` trait should be implemented for `MyScriptLoader`
    Loader: Script,
}
