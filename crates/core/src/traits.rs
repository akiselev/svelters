use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub trait Framework: Sized + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);
    fn name() -> &'static str;

    fn module<MOD>(module: MOD) -> JsModuleLoader<Self, MOD>
    where
        MOD: Module<Self>,
    {
        JsModuleLoader::new(module)
    }
}

pub trait Cast<T> {
    type Output<'a>: Deref<Target = T>;

    fn cast<'a>(&'a self) -> Self::Output<'a>;
}

pub trait CastMut<T> {
    type Output<'a>: DerefMut<Target = T>;

    fn cast_mut<'a>(&'a self) -> Self::Output<'a>;
}

pub type Wrapped<FW, CLS> = <FW as Wraps<CLS>>::Type;
pub type Casted<'a, FW, CLS, T> = <<FW as Wraps<CLS>>::Type as Cast<T>>::Output<'a>;
pub type CastedMut<'a, FW, CLS, T> = <<FW as Wraps<CLS>>::Type as CastMut<T>>::Output<'a>;

pub trait Wraps<CLS> {
    type Type;
}

pub trait IntoContainer<FW, CLS>
where
    FW: Wraps<CLS>,
{
    type Type;

    fn into_framework_type(self) -> Self::Type;
}

pub trait Module<FW>: Sized {
    fn define<P>(module: JsModule<FW, Self, P>) -> Result<()>
    where
        FW: Framework;
}

pub trait TypeDef<CLS>: Sized + Framework + Wraps<CLS> {
    const CLASS_NAME: &'static str;

    unsafe fn get_id() -> &'static mut ClassId;

    fn define<P>(cls: Type<Self, CLS, P>) -> Result<()>;
    fn typename() -> &'static str {
        Self::CLASS_NAME
    }
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
