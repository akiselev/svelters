use crate::inner::*;

pub(in crate) mod constructor;
pub(in crate) mod method;
pub(in crate) mod static_fn;
pub(in crate) mod impls {
    pub(in crate) mod constructor;
    pub(in crate) mod method;
    pub(in crate) mod static_fn;
}

pub struct JsClass<'a, 'b: 'a, FW: Framework, const CLS: &'static str, P: 'static + Phase> {
    phase: P::Input<'a, 'b>,
    pub(in crate) framework: PhantomData<FW>,
}

impl<'x, 'b: 'x, FW: Framework, const CLS: &'static str, P: 'static + Phase>
    JsClass<'x, 'b, FW, CLS, P>
where
    FW: Container<CLS>,
{
    pub fn new(phase: P::Input<'x, 'b>) -> Self {
        Self {
            phase,
            framework: PhantomData,
        }
    }

    pub fn constructor0<'a, 'js: 'a, R, X>(&self, func: X) -> Result<()>
    where
        X: 'static + Fn() -> R,
        R: IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
        ClassConstructor<FW, CLS, (), R, X>: Implements<P>,
    {
        ClassConstructor::new(func).build(self.phase)
    }

    pub fn method0<T, R, X>(&self, name: &'static str, func: X) -> Result<()>
    where
        X: 'static + Fn(&T) -> R,
        ClsBox<FW, CLS>: Cast<T>,
        R: for<'js> IntoJs<'js>,
        ClassMethod<FW, CLS, (T,), R, X>: Implements<P>,
        ClsBox<FW, CLS>: for<'a> GetRef<'a, ClsTy<FW, CLS>>,
    {
        ClassMethod::new(name, func).build(self.phase)
    }

    pub fn static_fn0<R, X>(&self, name: &'static str, func: X) -> Result<()>
    where
        X: 'static + Fn() -> R,
        R: for<'a> IntoJs<'a>,
        ClassStaticFunc<FW, CLS, (), R, X>: Implements<P>,
    {
        ClassStaticFunc::new(name, func).build(self.phase)
    }
}

macro_rules! tup {
    ($($static_fn:ident $method:ident $constructor:ident $($type:ident)*,)*) => {
        $(
            pub const fn $static_fn<$($type,)* R, X>(
                &self,
                name: &'static str,
                func: X,
            ) -> ClassStaticFunc<FW, CLS, ($($type,)*), R, X>
            where
                FW: Container<CLS>,
                X: StaticFunc<FW, CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
                $($type: for<'a> FromJs<'a>,)*
                R: for<'a> IntoJs<'a>,
            {
                ClassStaticFunc::new(name, func)
            }

            pub const fn $method<T, $($type,)* R, X>(
                &self,
                name: &'static str,
                func: X,
            ) -> ClassMethod<FW, CLS, (T, $($type,)*), R, X>
            where
                X: 'static + Method<FW, CLS, (T, $($type,)*), R> + Fn($($type,)*) -> R,
                ClsBox<FW, CLS>: for<'a> GetRef<'a, ClsTy<FW, CLS>>,
                ClsBox<FW, CLS>: Cast<T>,
                R: for<'js> IntoJs<'js>,
                $($type: for<'a> FromJs<'a>,)*
            {
                ClassMethod::new(name, func)
            }

            pub const fn $constructor<$($type,)* R, X>(&self, func: X) -> ClassConstructor<FW, CLS, ($($type,)*), R, X>
            where
                X: 'static + Constructor<FW, CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
                R: IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
                $($type: for<'a> FromJs<'a>,)*
            {
                ClassConstructor::new(func)
            }
        )*
    };
    (@one $($t:tt)*) => { 1 };
    (@idx A) => { 0 };
    (@idx B) => { 1 };
    (@idx C) => { 2 };
    (@idx D) => { 3 };
    (@idx E) => { 4 };
    (@idx F) => { 5 };
    (@idx G) => { 6 };
    (@idx H) => { 7 };
    (@idx I) => { 8 };
    (@idx J) => { 9 };
    (@idx K) => { 10 };
    (@idx L) => { 11 };
    (@idx M) => { 12 };
    (@idx N) => { 13 };
    (@idx O) => { 14 };
    (@idx P) => { 15 };
}

impl<'x, 'b: 'x, Z: Phase, FW: Framework, const CLS: &'static str> JsClass<'x, 'b, FW, CLS, Z>
where
    FW: Container<CLS>,
{
    tup! {
        static_fn1 method1 constructor1 A,
        static_fn2 method2 constructor2 A B,
        static_fn3 method3 constructor3 A B C,
        static_fn4 method4 constructor4 A B C D,
        static_fn5 method5 constructor5 A B C D E,
        static_fn6 method6 constructor6 A B C D E F,
        static_fn7 method7 constructor7 A B C D E F G,
        static_fn8 method8 constructor8 A B C D E F G H,
        static_fn9 method9 constructor9 A B C D E F G H I,
        static_fn10 method10 constructor10 A B C D E F G H I J,
        static_fn11 method11 constructor11 A B C D E F G H I J K,
        static_fn12 method12 constructor12 A B C D E F G H I J K L,
        static_fn13 method13 constructor13 A B C D E F G H I J K L M,
        static_fn14 method14 constructor14 A B C D E F G H I J K L M N,
        static_fn15 method15 constructor15 A B C D E F G H I J K L M N O,
        static_fn16 method16 constructor16 A B C D E F G H I J K L M N O P,
    }
}

pub struct JsClassLoader<FW: 'static, const CLS: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW: FrameworkClass<CLS>, const CLS: &'static str> JsClassLoader<FW, CLS> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<FW: 'static, const CLS: &'static str> Implements<ModuleEval> for JsClassLoader<FW, CLS> {
    fn build<'a, 'b: 'a>(self, (ctx, module): <ModuleEval as Phase>::Input<'a, 'b>) -> Result<()> {
        Ok(())
    }
}

// default impl<FW: 'static, const CLS: &'static str, P: 'static + Phase> Implements<P>
//     for JsClassLoader<FW, CLS>
// {
//     fn build<'a, 'b: 'a>(self, phase: P::Input<'a, 'b>) -> Result<()> {
//         Ok(())
//     }
// }
