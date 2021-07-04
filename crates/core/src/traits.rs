use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub trait Framework: Copy + Clone + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);

    fn module<const MOD: &'static str>() -> JsModuleLoader<Self, MOD>
    where
        Self: FrameworkModule<MOD>,
    {
        JsModuleLoader::new()
    }
}

pub trait Cast<T> {
    type Output<'a>: Deref<Target = T>;

    fn cast<'a>(&'a self) -> Self::Output<'a>;
}

pub type ClsBox<FW, const CLS: &'static str> = <FW as Container<CLS>>::Type;

pub trait Container<const CLS: &'static str>: Framework + FrameworkClass<CLS> {
    type Type;
}

pub trait IntoContainer<FW, const CLS: &'static str>
where
    FW: Container<CLS>,
{
    type Type;

    fn into_framework_type(self) -> Self::Type;
}

pub trait FrameworkModule<const N: &'static str>: Framework {
    fn define<P: Phase>(module: JsModule<Self, N, P>) -> Result<()>;
}

pub type ClsTy<FW, const CLS: &'static str> = <FW as FrameworkClass<CLS>>::Type;

pub trait FrameworkClass<const N: &'static str>: Framework {
    type Type: 'static;

    unsafe fn get_id() -> &'static mut ClassId;

    fn define<P: Phase>(cls: JsClass<Self, N, P>) -> Result<()>;
    fn get_name() -> &'static str {
        N
    }
}

pub trait JsModuleDef: Sized + 'static {
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()>;

    fn eval<'js>(
        &'static self,
        _ctx: Ctx<'js>,
        _module: &Module<'js, Loaded<Native>>,
    ) -> Result<()>;
}

pub trait TypeDef: Sized {
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    fn eval<'js>(
        &'static self,
        _ctx: Ctx<'js>,
        _module: &Module<'js, Loaded<Native>>,
    ) -> Result<()> {
        Ok(())
    }

    fn init_proto<'js>(&'static self, _ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn init_static<'js>(&'static self, _ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

pub trait GetRef<'a, T> {
    type Pointer;
    fn get_ref(&'a self) -> &'a T;
    fn get_ptr(&'a self) -> Self::Pointer;
}

pub trait Phase {
    type Input<'a, 'b: 'a>: Copy;
    type Error: std::fmt::Debug;
}

pub trait Implements<P: 'static + Phase>: 'static {
    fn build<'a, 'b: 'a>(self, input: P::Input<'a, 'b>) -> Result<()>;
}

impl<T: 'static, P: 'static + Phase> Implements<P> for T {
    default fn build(self, phase: &P) -> Result<()> {
        Ok(())
    }
}

// default impl<FW, const MOD: &'static str, P: Phase> Implements<P> for JsModuleLoader<FW, MOD>
// where
//     FW: FrameworkModule<MOD>,
// {
//     fn build(self, phase: &P) -> Result<()> {
//         Ok(())
//     }
// }
