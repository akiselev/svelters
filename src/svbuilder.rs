use std::path::Path;

use rquickjs::{BuiltinResolver, Loader, ModuleDef, ModuleLoader, Resolver};

use crate::{
    js::*,
    js_modules::{console, loader::SvResolver, timing},
    Svelters,
};

type Result<T> = JsResult<T>;

pub type Initializers = Vec<Box<dyn FnOnce(&Svelters) -> Result<()>>>;

pub struct SveltersBuilder {
    resolver: BuiltinResolver,
    loader: BuiltinLoader,
    module_loader: ModuleLoader,
    initializers: Initializers,
}

impl SveltersBuilder {
    pub fn new() -> SveltersBuilder {
        let (resolver, loader) = get_dev_dependencies();
        SveltersBuilder {
            resolver,
            loader,
            module_loader: ModuleLoader::default(),
            initializers: vec![],
        }
    }

    pub fn with_initializer<F: 'static + FnOnce(&Svelters) -> Result<()>>(
        mut self,
        func: F,
    ) -> Self {
        self.initializers.push(Box::new(func));
        self
    }

    pub fn with_module<T: ModuleDef>(self, name: &str, module: T) -> Self {
        SveltersBuilder {
            resolver: self.resolver.with_module(name),
            loader: self.loader,
            module_loader: self.module_loader.with_module(name, module),
            ..self
        }
    }

    pub fn build(self) -> Result<Svelters> {
        let rt = rquickjs::Runtime::new()?;
        rt.set_loader(
            (
                self.resolver,
                FileResolver::default()
                    .with_path("./")
                    .with_path("./node_modules/svelte/"),
                SvResolver::new(),
            ),
            (
                self.loader,
                self.module_loader,
                ScriptLoader::default(),
                NativeLoader::default(),
            ),
        );

        let ctx = rquickjs::Context::full(&rt)?;
        initialize_context(&ctx)?;

        Svelters::build(rt, ctx, self.initializers)
    }

    pub fn build_with<R, L>(self, resolver: R, loader: L) -> Result<Svelters>
    where
        R: 'static + rquickjs::Resolver,
        L: 'static + rquickjs::Loader,
    {
        let rt = rquickjs::Runtime::new()?;
        rt.set_loader(
            (
                self.resolver,
                FileResolver::default()
                    .with_path("./")
                    .with_path("./node_modules/svelte/"),
                resolver,
            ),
            (
                self.loader,
                self.module_loader,
                ScriptLoader::default(),
                NativeLoader::default(),
                loader,
            ),
        );

        let ctx = rquickjs::Context::full(&rt)?;
        initialize_context(&ctx)?;

        Svelters::build(rt, ctx, self.initializers)
    }
}

#[tracing::instrument(skip(ctx))]
fn initialize_context(ctx: &Context) -> Result<()> {
    ctx.with(move |ctx| {
        console::init(ctx)?;
        timing::init(ctx)?;
        let obj = Object::new(ctx)?;
        ctx.globals().set("window", obj)?;
        Result::Ok(())
    })?;
    Ok(())
}

pub(in crate) fn get_dev_dependencies() -> (BuiltinResolver, BuiltinLoader) {
    let resolver = BuiltinResolver::default();
    // .with_module("svelte/internal")
    // .with_module("./internal/index.mjs")
    // .with_module("svelte");
    let loader = BuiltinLoader::default();
    // .with_module(
    //     "svelte/internal",
    //     std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
    // )
    // .with_module(
    //     Path::new("./node_modules/svelte/internal/index.mjs")
    //         .canonicalize()
    //         .unwrap()
    //         .to_string_lossy(),
    //     std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
    // )
    // .with_module(
    //     "./internal/index.mjs",
    //     std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
    // )
    // .with_module(
    //     "svelte",
    //     std::fs::read_to_string("./node_modules/svelte/index.mjs").unwrap(),
    // );

    (resolver, loader)
}

// pub(in crate) fn get_prod_dependencies() -> (impl Resolver, impl Loader) {
//     let resolver = (
//         BuiltinResolver::default()
//             .with_module("svelte/internal")
//             .with_module("./internal/index.mjs")
//             .with_module("svelte"),
//         FileResolver::default()
//             .with_path("./")
//             .with_path("./node_modules/svelte/"),
//     );
//     let loader = (
//         BuiltinLoader::default()
//             .with_module(
//                 "svelte/internal",
//                 include_str!("../../../node_modules/svelte/internal/index.mjs"),
//             )
//             .with_module(
//                 "./internal/index.mjs",
//                 include_str!("../../../node_modules/svelte/internal/index.mjs"),
//             )
//             .with_module(
//                 "svelte",
//                 include_str!("../../../node_modules/svelte/index.mjs"),
//             ),
//         ScriptLoader::default(),
//         NativeLoader::default(),
//     );

//     (resolver, loader)
// }
