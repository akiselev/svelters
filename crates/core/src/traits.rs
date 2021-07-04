use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub trait Framework: Sized + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);

    fn module<MOD>(module: MOD) -> JsModuleLoader<Self, MOD>
    where
        MOD: FrameworkModule<Self>,
    {
        JsModuleLoader::new(module)
    }
}

pub trait Cast<T> {
    type Output<'a>: Deref<Target = T>;

    fn cast<'a>(&'a self) -> Self::Output<'a>;
}

pub type ClsBox<FW, CLS> = <FW as Container<CLS>>::Type;

pub trait Container<CLS> {
    type Type;

    // fn into_container_type(data: CLS) -> Self::Type;
}

pub trait IntoContainer<FW, CLS>
where
    FW: Container<CLS>,
{
    type Type;

    fn into_framework_type(self) -> Self::Type;
}

pub trait FrameworkModule<FW>: Sized {
    fn define<P>(module: JsModule<FW, Self, P>) -> Result<()>
    where
        FW: Framework;
}

pub trait FrameworkClass<CLS>: Sized {
    const CLASS_NAME: &'static str;

    unsafe fn get_id() -> &'static mut ClassId;

    fn define<P>(cls: JsClass<Self, CLS, P>) -> Result<()>;
    fn get_name() -> &'static str {
        Self::CLASS_NAME
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

pub trait GetRef<'a, T> {
    type Pointer;
    fn get_ref(&'a self) -> &'a T;
    fn get_ptr(&'a self) -> Self::Pointer;
}

pub trait Implements<P> {
    fn build(self, input: &P) -> Result<()>;
}

impl<T, P> Implements<P> for T {
    default fn build(self, phase: &P) -> Result<()> {
        Ok(())
    }
}
