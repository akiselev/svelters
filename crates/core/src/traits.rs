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

pub type Type<FW, CLS> = <CLS as Repr<FW>>::Type;
pub type Casted<'a, FW, CLS, T> = <<CLS as Repr<FW>>::Type as Cast<T>>::Output<'a>;
pub type CastedMut<'a, FW, CLS, T> = <<CLS as Repr<FW>>::Type as CastMut<T>>::Output<'a>;

/// The Representation of a type for a framework
///
/// Since `svelters` is a library that bridges Svelte
/// and external types, users usually can't implement `svelters`
/// traits for the types directly beacuse of Rust's orphan rule
///
/// This trait supplies a custom representation (wrapper) of external
/// types so users can implement our traits
pub trait Repr<FW> {
    type Type;
}

pub trait IntoRepr<FW, CLS>
where
    CLS: Repr<FW>,
{
    fn into_framework_type(self) -> <CLS as Repr<FW>>::Type;
}

pub trait Module<FW>: Sized {
    fn define<P>(module: JsModule<FW, Self, P>) -> Result<()>
    where
        FW: Framework;
}

pub trait TypeDef<FW>: Sized + Repr<FW> {
    const CLASS_NAME: &'static str;

    unsafe fn get_id() -> &'static mut ClassId;

    fn define<P>(cls: TypeBuilder<FW, Self, P>) -> Result<()>;
    fn typename() -> &'static str {
        Self::CLASS_NAME
    }
}

pub trait Implements<P> {
    fn build(self, input: &P) -> Result<()>;
}

impl<T, P> Implements<P> for T {
    default fn build(self, _phase: &P) -> Result<()> {
        Ok(())
    }
}
