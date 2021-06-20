use std::convert::TryInto;

use rquickjs::TypedMapKey;

use crate::{inner::*, transpile_typescript, FsLoader};

pub(in crate) fn task_loop(rx: Receiver<(String, String, SyncSender<Result<String>>)>) {
    let span = tracing::debug_span!("Svelte Compiler");
    let _ = span.enter();

    let rt = Runtime::new().unwrap();
    rt.set_max_stack_size(10 * 1024 * 1024);
    let resolver = (
        BuiltinResolver::default()
            .with_module("svelte/internal")
            .with_module("./internal/index.mjs")
            .with_module("svelte"),
        FileResolver::default()
            .with_path("./")
            .with_path("./node_modules/svelte/"),
    );
    let loader = (
        BuiltinLoader::default()
            .with_module(
                "svelte/internal",
                std::fs::read_to_string(include_str!(
                    "../../../node_modules/svelte/internal/index.mjs"
                ))
                .unwrap(),
            )
            .with_module(
                "./internal/index.mjs",
                std::fs::read_to_string(include_str!(
                    "../../../node_modules/svelte/internal/index.mjs"
                ))
                .unwrap(),
            )
            .with_module(
                "svelte",
                std::fs::read_to_string(include_str!("../../../node_modules/svelte/index.mjs"))
                    .unwrap(),
            ),
        ScriptLoader::default(),
        NativeLoader::default(),
    );
    rt.set_loader(resolver, loader);

    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        ctx.eval_global::<(), _>(include_str!("../js/self.js"))
            .unwrap();
        ctx.eval_global::<(), _>(include_str!("../js/compiler.js"))
            .unwrap();
        ctx.eval_global::<(), _>(include_str!("../js/polyfill/index.js"))
            .unwrap();
        ctx.eval_global::<(), _>(include_str!("../../../node_modules/svelte/compiler.js"))
            .unwrap();

        ctx.globals().init_def::<Transpile>().unwrap();

        while let Ok((path, src, tx)) = rx.recv() {
            tracing::info!("Compiling component");
            let res = call_compiler(ctx, &path, src);
            tx.send(res).unwrap();
        }
    });
}

fn call_compiler<'js>(ctx: Ctx<'js>, path: &str, source: String) -> Result<String> {
    let compile_fn: Function<'js> = ctx.globals().get("compile")?;
    compile_fn.call((path.to_string(), source))
}

#[bind(object)]
fn transpile<'js>(ctx: Ctx<'js>, path: String, src: String) -> Result<String> {
    let path_mapping = FilePathMapping::empty();
    let cm = Arc::new(SourceMap::with_file_loader(
        Box::new(FsLoader),
        path_mapping,
    ));
    ctx.insert_typed(SourceMapKey(path.clone()), cm.clone());
    // TODO: Get offsets from svelte compiler for source maps
    let input = cm.load_file(Path::new(&path)).unwrap();
    let start = input.src.find(&src).unwrap().try_into().unwrap();
    let len: u32 = src.len().try_into().unwrap();
    let input = StringInput::new(&src, BytePos(start), BytePos(start + len));

    Ok(transpile_typescript(&cm, &path, input)?)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourceMapKey(String);

impl TypedMapKey for SourceMapKey {
    type Value = Arc<SourceMap>;
}
