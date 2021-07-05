use std::{future::Future, time::Duration};

use super::*;

impl Framework for Fw {
    type Loader = impl Loader;
    type Resolver = impl Resolver;
    type AppTask = impl Future<Output = anyhow::Result<()>>;

    fn init_app(_ctx: Context) -> Result<Self::AppTask> {
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
            .with_module("Core", Fw::module(FwCore))
            .with_module("Widgets", Fw::module(FwWidgets));
        (resolver, loader)
    }

    fn name() -> &'static str {
        "Fw"
    }
}

impl<CLS> Wraps<CLS> for Fw
where
    Fw: TypeDef<CLS>,
    CLS: StaticUpcast<Object>,
{
    type Type = FwWrapper<CLS>;
}
