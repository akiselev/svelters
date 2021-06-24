use std::future::Future;

use crate::inner::*;

pub trait Framework: Copy + Clone + 'static {
    type AppTask: Future<Output = anyhow::Result<()>>;
    // type Object<T>;
    // type Handler<T>;

    fn init_app(&self, ctx: Context) -> Result<Self::AppTask>;
}

pub trait FrameworkModule<const N: &'static str> {}

pub trait Container<F: Framework, T>: Sized {
    type Output<X>: AsRef<T>;

    fn wrap(self) -> Self::Output<Self>;
}

pub trait Cast<T> {
    type Output<'a, U: 'a>: AsRef<U>
    where
        Self: 'a,
        T: 'a;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
    where
        T: 'a;
}

pub trait JsClass<FW: Framework>: Sized {
    const CLASS_NAME: &'static str;

    type Def: ClassDef;
    type Instance<T>: Convert<Self, FW>;

    fn class_id() -> &'static mut ClassId;
    fn definition() -> Self::Def;
}

pub trait Convert<T, FW: Framework> {
    type Into<'js, X>: IntoJs<'js>;
    type From<'js, X>: FromJs<'js>;

    fn into_converter<'js>() -> Self::Into<'js, T>;
    fn from_converter<'js>() -> Self::From<'js, T>;
}

pub trait Member<T> {}

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
