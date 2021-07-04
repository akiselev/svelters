use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub trait Framework: Copy + Clone + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask>;
    fn get_loader() -> (Self::Resolver, Self::Loader);

    fn module<MOD>() -> JsModuleLoader<Self, MOD, ()>
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

pub type ClsBox<FW, CLS> = <FW as Container<CLS>>::Type;

pub trait Container<CLS>: Framework + FrameworkClass<CLS> {
    type Type;
}

pub trait IntoContainer<FW, CLS>
where
    FW: Container<CLS>,
{
    type Type;

    fn into_framework_type(self) -> Self::Type;
}

pub trait FrameworkModule<CLS> {
    fn define<P>(module: JsModule<Self, CLS, P>) -> Result<()>
    where
        Self: Framework,
        JsModule<Self, CLS, P>: Implements<P>;
}

pub trait FrameworkClass<CLS> {
    unsafe fn get_id() -> &'static mut ClassId;

    fn define<P: Phase>(cls: JsClass<Self, CLS, P>) -> Result<()>
    where
        Self: Framework;
    fn get_name() -> &'static str {
        std::any::type_name::<CLS>()
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

pub trait Phase {}

pub trait Implements<P> {
    fn build(self, input: P) -> Result<()>;
}

default impl<T, P> Implements<P> for T {
    default fn build(self, phase: P) -> Result<()> {
        Ok(())
    }
}

// default impl<FW: 'static, CLS: 'static, P: Phase> Implements<P> for JsClassLoader<FW, CLS>
// where
//     FW: FrameworkClass<CLS>,
// {
//     fn build<'a, 'b: 'a>(self, phase: P::Input<'a, 'b>) -> Result<()> {
//         Ok(())
//     }
// }

// trait Trait<T> {
//     fn build();
// }

// default impl<T, U> Trait<T> for U {
//     fn build() {}
// }

// struct Type<const T: &'static str>;

// impl Trait<usize> for Type<"string"> {
//     fn build() {}
// }
