use std::future::Future;

use rquickjs::{Constructor, Loader, Resolver};

use crate::inner::*;

pub trait Framework: Copy + Clone + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);

    fn module<const NAME: &'static str>() -> JsModule<Self, NAME>
    where
        Self: FrameworkModule<NAME>,
    {
        JsModule(PhantomData)
    }

    fn class<const NAME: &'static str>() -> JsClass<Self, NAME>
    where
        Self: FrameworkClass<NAME>,
    {
        JsClass(PhantomData)
    }
}

pub trait FrameworkModule<const N: &'static str>: Framework {
    type Output: JsModuleDef;

    fn get_module() -> Self::Output;
}

pub trait FrameworkClass<const N: &'static str>: Framework {
    type Type;
    type Wrapper: Container<Self, Self::Type>;
    type Definition: JsClassDef;

    unsafe fn get_id() -> &'static mut ClassId;

    fn register<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()>;

    fn get_definition() -> Self::Definition;

    fn get_name() -> &'static str {
        N
    }
}

pub trait JsModuleDef {
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    fn eval<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Loaded<Native>>) -> Result<()> {
        Ok(())
    }
}

pub trait JsClassDef {
    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

pub trait Container<F: Framework, T>: Sized {
    type Output<X>: AsRef<T>;

    fn wrap(self) -> Self::Output<Self>;
}

pub trait Cast<FW, const NAME: &'static str> {
    type Output<'a>: AsRef<<FW as FrameworkClass<NAME>>::Type>
    where
        Self: 'a,
        FW: FrameworkClass<NAME>,
        <FW as FrameworkClass<NAME>>::Type: 'a;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a>>
    where
        <FW as FrameworkClass<NAME>>::Type: 'a;
}

pub trait Convert<T, FW: Framework> {
    type Into<'js, X>: IntoJs<'js>;
    type From<'js, X>: FromJs<'js>;

    fn into_converter<'js>() -> Self::Into<'js, T>;
    fn from_converter<'js>() -> Self::From<'js, T>;
}

pub trait Member<const T: &'static str> {}

pub trait Instantiate {}

pub trait Func<A> {
    type Args;
    type Output;

    fn try_call<'js>(&self, args: Self::Args) -> Self::Output;
}

pub trait AsFunc<A> {
    type Output;

    fn as_func(self) -> Self::Output;
}
