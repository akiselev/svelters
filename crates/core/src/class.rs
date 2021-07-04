use crate::inner::*;

pub(in crate) mod constructor;
pub(in crate) mod method;
pub(in crate) mod static_fn;
pub(in crate) mod superclass;

pub struct JsClass<FW, CLS, P> {
    phase: P,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<FW, CLS, P> JsClass<FW, CLS, P>
where
    FW: Container<CLS>,
{
    pub fn new(phase: P) -> Self {
        Self {
            phase,
            framework: PhantomData,
        }
    }

    pub fn constructor0<'a, 'js: 'a, R, X>(&self, func: X) -> Result<()>
    where
        FW: Framework + FrameworkClass<CLS> + Container<CLS>,
        X: Fn() -> R,
        ClassConstructor<FW, CLS, (), R, X>: Implements<P>,
    {
        ClassConstructor::new(func).build(&self.phase)?;
        Ok(())
    }

    pub fn method0<T, R, X>(&self, name: &'static str, func: X) -> Result<()>
    where
        FW: Framework + FrameworkClass<CLS> + Container<CLS> + Container<T>,
        X: for<'a> Fn(&T) -> R,
        ClsBox<FW, CLS>: Cast<T>,
        R: for<'js> IntoJs<'js>,
        ClassMethod<FW, CLS, CLS, (T,), R, X>: Implements<P>,
        CLS: 'static,
    {
        ClassMethod::new(name, func).build(&self.phase)?;
        Ok(())
    }

    pub fn static_fn0<R, X>(&self, name: &'static str, func: X) -> Result<()>
    where
        X: Fn() -> R,
        R: for<'a> IntoJs<'a>,
        ClassStaticFunc<FW, CLS, (), R, X>: Implements<P>,
    {
        ClassStaticFunc::new(name, func).build(&self.phase)?;
        Ok(())
    }
}

// macro_rules! tup {
//     ($($static_fn:ident $method:ident $constructor:ident $($type:ident)*,)*) => {
//         $(
//             pub fn $static_fn<$($type,)* R, X>(
//                 &self,
//                 name: &'static str,
//                 func: X,
//             ) -> ClassStaticFunc<FW, CLS, ($($type,)*), R, X>
//             where
//                 FW: Container<CLS>,
//                 X: StaticFunc<FW, CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
//                 $($type: for<'a> FromJs<'a>,)*
//                 R: for<'a> IntoJs<'a>,
//             {
//                 ClassStaticFunc::new(name, func)
//             }

//             pub fn $method<T, $($type,)* R, X>(
//                 &self,
//                 name: &'static str,
//                 func: X,
//             ) -> ClassMethod<FW, CLS, (T, $($type,)*), R, X>
//             where
//                 X:  Method<FW, CLS, (T, $($type,)*), R> + Fn($($type,)*) -> R,
//                 ClsBox<FW, CLS>: for<'a> GetRef<'a, CLS>,
//                 ClsBox<FW, CLS>: Cast<T>,
//                 R: for<'js> IntoJs<'js>,
//                 $($type: for<'a> FromJs<'a>,)*
//             {
//                 ClassMethod::new(name, func)
//             }

//             pub fn $constructor<$($type,)* R, X>(&self, func: X) -> ClassConstructor<FW, CLS, ($($type,)*), R, X>
//             where
//                 X:  Constructor<FW, CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
//                 R: IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
//                 $($type: for<'a> FromJs<'a>,)*
//             {
//                 ClassConstructor::new(func)
//             }
//         )*
//     };
//     (@one $($t:tt)*) => { 1 };
//     (@idx A) => { 0 };
//     (@idx B) => { 1 };
//     (@idx C) => { 2 };
//     (@idx D) => { 3 };
//     (@idx E) => { 4 };
//     (@idx F) => { 5 };
//     (@idx G) => { 6 };
//     (@idx H) => { 7 };
//     (@idx I) => { 8 };
//     (@idx J) => { 9 };
//     (@idx K) => { 10 };
//     (@idx L) => { 11 };
//     (@idx M) => { 12 };
//     (@idx N) => { 13 };
//     (@idx O) => { 14 };
//     (@idx P) => { 15 };
// }

// impl<Z: Phase, FW: Framework, CLS> JsClass<FW, CLS, Z>
// where
//     FW: Container<CLS>,
// {
//     tup! {
//         static_fn1 method1 constructor1 A,
//         static_fn2 method2 constructor2 A B,
//         static_fn3 method3 constructor3 A B C,
//         static_fn4 method4 constructor4 A B C D,
//         static_fn5 method5 constructor5 A B C D E,
//         static_fn6 method6 constructor6 A B C D E F,
//         static_fn7 method7 constructor7 A B C D E F G,
//         static_fn8 method8 constructor8 A B C D E F G H,
//         static_fn9 method9 constructor9 A B C D E F G H I,
//         static_fn10 method10 constructor10 A B C D E F G H I J,
//         static_fn11 method11 constructor11 A B C D E F G H I J K,
//         static_fn12 method12 constructor12 A B C D E F G H I J K L,
//         static_fn13 method13 constructor13 A B C D E F G H I J K L M,
//         static_fn14 method14 constructor14 A B C D E F G H I J K L M N,
//         static_fn15 method15 constructor15 A B C D E F G H I J K L M N O,
//         static_fn16 method16 constructor16 A B C D E F G H I J K L M N O P,
//     }
// }

pub struct JsClassLoader<FW, CLS>(pub(in crate) PhantomData<(FW, CLS)>);

impl<FW: FrameworkClass<CLS>, CLS> JsClassLoader<FW, CLS> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a, 'b: 'a, FW, CLS> Implements<ModuleEval<'a, 'b>> for JsClassLoader<FW, CLS>
where
    FW: FrameworkClass<CLS> + Container<CLS>,
{
    fn build(self, phase: &ModuleEval<'a, 'b>) -> Result<()> {
        let cls: JsClass<FW, CLS, ConstructorEval<'a, 'b, FW, CLS>> =
            JsClass::new(ConstructorEval::new(phase.ctx, phase.module));
        <FW as FrameworkClass<CLS>>::define(cls)?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS> Implements<ModuleLoad<'a, 'b>> for JsClassLoader<FW, CLS>
where
    FW: FrameworkClass<CLS> + Container<CLS>,
{
    fn build(self, phase: &ModuleLoad<'a, 'b>) -> Result<()> {
        let cls: JsClass<FW, CLS, ConstructorLoad<'a, 'b, FW, CLS>> =
            JsClass::new(ConstructorLoad::new(phase.ctx, phase.module));
        <FW as FrameworkClass<CLS>>::define(cls)?;
        Ok(())
    }
}

// default impl<FW, CLS, P: Phase> Implements<P> for JsClassLoader<FW, CLS> {
//     fn build<'a, 'b: 'a>(self, phase: P) -> Result<()> {
//         Ok(())
//     }
// }
