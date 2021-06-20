use std::convert::TryInto;

use futures::FutureExt;
use rquickjs::TypedMapKey;

use crate::{inner::*, transpile_typescript, FsLoader};

pub(in crate) async fn task_loop(rx: Receiver<(String, String, Sender<Result<String>>)>) {
    let span = tracing::debug_span!("Svelte Compiler");
    let _ = span.enter();

    tracing::info!("Starting compiler task");
    let rt = Runtime::new().unwrap();
    let executor = rt.spawn_executor(rquickjs::Tokio);
    // let local = tokio::task::LocalSet::new();
    // local
    //     .spawn_local({
    //         let rt = rt.clone();
    //         async move {  }
    //     })
    //     .await
    //     .unwrap();

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
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
            )
            .with_module(
                "./internal/index.mjs",
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
            )
            .with_module(
                "svelte",
                std::fs::read_to_string("./node_modules/svelte/index.mjs").unwrap(),
            ),
        ScriptLoader::default(),
        NativeLoader::default(),
    );
    rt.set_loader(resolver, loader);

    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        ctx.eval_global_file::<(), _>("./crates/compiler/js/self.js")
            .unwrap();
        ctx.eval_global_file::<(), _>("./crates/compiler/js/compiler.js")
            .unwrap();
        ctx.eval_global_file::<(), _>("./crates/compiler/js/polyfill/index.js")
            .unwrap();
        ctx.eval_global_file::<(), _>("./node_modules/svelte/compiler.js")
            .unwrap();

        ctx.globals().init_def::<Transpile>().unwrap();
    });

    while let Ok((path, src, tx)) = rx.recv_async().await {
        tracing::info!("Received component");
        ctx.with(|ctx| {
            ctx.spawn(
                call_compiler(ctx, path, src)
                    .unwrap()
                    .then(|res| async move {
                        tracing::info!("Sending response");
                        tx.send_async(res.unwrap()).await.unwrap();
                    }),
            )
        });
        tracing::info!("Compiling component");
    }

    executor.await.unwrap();
    tracing::info!("Task finished");
}

fn call_compiler<'js>(
    ctx: Ctx<'js>,
    path: String,
    source: String,
) -> Result<Promise<Result<String>>> {
    let compile_fn: Function<'js> = ctx.globals().get("compile")?;
    let res = compile_fn.call((path, source));
    tracing::info!("Compile called");
    res
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
