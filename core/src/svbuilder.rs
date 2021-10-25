use rquickjs::{BuiltinResolver, ModuleLoader};

use crate::{init::initialize_svelters, inner::*};

pub type Initializers = Vec<Box<dyn FnOnce(&Svelters) -> Result<()>>>;

pub struct SveltersBuilder {
    resolver: BuiltinResolver,
    loader: ModuleLoader,
    initializers: Initializers,
}

impl SveltersBuilder {
    pub fn new() -> SveltersBuilder {
        SveltersBuilder {
            resolver: BuiltinResolver::default(),
            loader: ModuleLoader::default(),
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
            loader: self.loader.with_module(name, module),
            ..self
        }
    }

    pub fn with_types<F: 'static + for<'js> FnOnce(TypeBuilder<'js>)>(mut self, func: F) -> Self {
        self.initializers.push(Box::new(|sv: &Svelters| {
            sv.with_js(|ctx| Ok(func(TypeBuilder::from_ctx(ctx)?)))
        }));
        self
    }

    pub fn build(self) -> Result<Svelters> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        rt.set_loader(self.resolver, self.loader);
        let runtime = initialize_svelters(&ctx)?;
        Svelters::build(runtime, rt, ctx, self.initializers)
    }

    pub fn build_with<R, L>(self, resolver: R, loader: L) -> Result<Svelters>
    where
        R: 'static + rquickjs::Resolver,
        L: 'static + rquickjs::Loader,
    {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        rt.set_loader((self.resolver, resolver), (self.loader, loader));
        let runtime = initialize_svelters(&ctx)?;
        Svelters::build(runtime, rt, ctx, self.initializers)
    }
}
