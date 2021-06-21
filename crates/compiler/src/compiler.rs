use std::{
    convert::TryInto,
    fs::{canonicalize, read_to_string},
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{mpsc, Arc},
};

use crate::inner::*;
use crate::{task::task_loop, typescript};

#[derive(Clone)]
pub struct SvelteCompiler {
    tx: Sender<(String, String, Sender<Result<String>>)>,
    pub(in crate) resolver: NodeResolver,
}

impl SvelteCompiler {
    pub fn new() -> SvelteCompiler {
        let (tx, rx) = flume::bounded::<(String, String, Sender<Result<String>>)>(0);
        let resolver = NodeResolver::new()
            .with_extensions(&["svelte", "ts", "mjs", "js"])
            .with_main_fields(&["svelte", "module"]);

        std::thread::spawn(move || {
            use tokio::runtime;

            let rt = runtime::Builder::new_current_thread().build().unwrap();
            let local = tokio::task::LocalSet::new();

            rt.block_on(local.run_until(task_loop(rx)));
        });
        SvelteCompiler { tx, resolver }
    }

    #[tracing::instrument(skip(self))]
    pub async fn compile(&self, path: &str, source: String) -> Result<String> {
        let (tx, rx) = flume::bounded(0);
        self.tx
            .send_async((path.to_string(), source, tx))
            .await
            .unwrap();
        tracing::info!("Awaiting compiler response (async)...");
        let res = rx.recv_async().await.unwrap()?;
        tracing::debug!(res=%res, "Compiled");
        Ok(res)
    }

    #[tracing::instrument(skip(self))]
    pub fn compile_sync(&self, path: &str, source: String) -> Result<String> {
        let (tx, rx) = flume::bounded(0);
        self.tx
            .send((path.to_string(), source, tx))
            .map_err(|_| Error::Unknown)?;
        tracing::info!("Awaiting compiler response (sync)...");
        rx.recv().map_err(|_| Error::Unknown)?
    }
}

#[tracing::instrument(skip(cm, input))]
pub fn transpile_typescript(cm: &Arc<SourceMap>, path: &str, input: StringInput) -> Result<String> {
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
    let mut config = typescript::Config::default();
    config.import_not_used_as_values = typescript::ImportsNotUsedAsValues::Preserve;
    let mut strip = typescript::strip_with_config(config);

    let parser_result = swc_common::GLOBALS.set(&swc_common::Globals::new(), || {
        parser.parse_typescript_module().map(
            |p| p.fold_with(&mut strip),
            //  .fold_with(&mut fixer(None))
        )
    });

    // let parser_result = ;

    match parser_result {
        Ok(module) => {
            let mut wr = Buf(Arc::new(RwLock::new(vec![])));
            {
                let mut emitter = Emitter {
                    cfg: Config { minify: false },
                    cm: cm.clone(),
                    comments: None,
                    wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut wr, None)),
                };
                emitter.emit_module(&module)?;
            }
            let lock = wr.0.write();
            // Safe because Javascript can be invalid utf8
            let res = unsafe { String::from_utf8_unchecked(lock.to_vec()) };
            tracing::debug!(res=%res, "Transpiled");
            Ok(res)
        }
        Err(err) => {
            let handler = Arc::new(Handler::with_tty_emitter(
                ColorConfig::Auto,
                true,
                false,
                Some(cm.clone()),
            ));
            tracing::error!(res=?err, "Transpile failed");
            let exception = Error::Exception {
                message: format!("Transpile error: {:?}", err),
                file: path.to_string(),
                line: 0,
                stack: "".to_string(),
            };
            let mut diag = err.into_diagnostic(&handler);
            diag.emit();
            Err(exception)
        }
    }
}

pub(in crate) struct FsLoader;

impl FileLoader for FsLoader {
    #[tracing::instrument(skip(self))]
    fn file_exists(&self, path: &Path) -> bool {
        tracing::info!("Checking if path exists");
        if let Ok(path) = canonicalize(path) {
            tracing::info!(path=?path, "Path found");
            true
        } else {
            tracing::error!("Path not found");
            false
        }
    }

    #[tracing::instrument(skip(self))]
    fn abs_path(&self, path: &Path) -> Option<PathBuf> {
        tracing::info!("Canonicalizing path");
        if let Ok(path) = canonicalize(path) {
            tracing::info!(path=?path, "Path found");
            Some(path)
        } else {
            tracing::error!("Path not found");
            None
        }
    }

    #[tracing::instrument(skip(self))]
    fn read_file(&self, path: &Path) -> std::io::Result<String> {
        tracing::debug!("Reading file");
        std::fs::read_to_string(path)
    }
}

#[derive(Debug, Clone)]
struct Buf(Arc<RwLock<Vec<u8>>>);
impl Write for Buf {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let src = String::from_utf8(Vec::from(data)).unwrap();
        self.0.write().write(data)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.write().flush()
    }
}
