use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub trait Framework: Copy + Clone + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);

    fn module<const CLS: &'static str>() -> JsModule<Self, CLS>
    where
        Self: FrameworkModule<CLS>,
    {
        JsModule(PhantomData)
    }
}

pub const fn include<FW, const CLS: &'static str>() -> JsClass<FW, CLS>
where
    FW: FrameworkClass<CLS>,
{
    JsClass(PhantomData)
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
    type Output: JsModuleDef;

    fn get_module<'a>() -> &'static Self::Output;
}

pub type ClsTy<FW, const CLS: &'static str> = <FW as FrameworkClass<CLS>>::Type;

pub trait FrameworkClass<const N: &'static str>: Framework {
    type Type: 'static;
    type Proto: ClassProto;
    type Static: ClassStatic;
    type Constructor: JsModuleDef;

    unsafe fn get_id() -> &'static mut ClassId;

    fn get_proto() -> &'static Self::Proto;
    fn get_static() -> &'static Self::Static;
    fn get_constructor<'a>() -> &'static Self::Constructor;

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

pub trait ClassProto: Sized {
    fn init_proto<'js>(&'static self, _ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()>;
}

pub trait ClassStatic: Sized {
    fn init_static<'js>(&'static self, _ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()>;
}

pub trait GetRef<'a, T> {
    type Pointer;
    fn get_ref(&'a self) -> &'a T;
    fn get_ptr(&'a self) -> Self::Pointer;
}
