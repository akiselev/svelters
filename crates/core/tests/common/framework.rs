use std::{future::Future, rc::Rc};

use super::*;

impl Framework for FW {
    type Loader = impl Loader;
    type Resolver = impl Resolver;
    type AppTask = impl Future<Output = anyhow::Result<()>>;

    fn init_app(_ctx: rquickjs::Context) -> Result<Self::AppTask> {
        Ok(Box::pin(async move {
            // let mut interval = tokio::time::interval(Duration::from_millis(10));
            // loop {
            //     interval.tick().await;
            // }
            Ok(())
        }))
    }

    fn get_loader() -> (Self::Resolver, Self::Loader) {
        let resolver = BuiltinResolver::default()
            .with_module("Core")
            .with_module("Widgets");
        let loader = ModuleLoader::default()
            .with_module("Core", FW::module(FwCore))
            .with_module("Widgets", FW::module(FwWidgets));
        (resolver, loader)
    }

    fn name() -> &'static str {
        "Fw"
    }
}

impl<T> Default for Fw<T>
where
    T: Default,
{
    fn default() -> Self {
        Fw(Default::default())
    }
}

impl<CLS> Repr<CLS> for FW
where
    CLS: Type<FW>,
    CLS: StaticUpcast<Fw<Rc<Object>>>,
{
    type Type = FwWrapper<CLS>;
}
