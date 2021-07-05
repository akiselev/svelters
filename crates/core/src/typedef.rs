use crate::inner::*;

pub struct Type<FW, CLS, P> {
    phase: P,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<FW, CLS, P> Type<FW, CLS, P>
where
    FW: Wraps<CLS>,
{
    pub fn new(phase: P) -> Self {
        Self {
            phase,
            framework: PhantomData,
        }
    }

    // pub fn prop<THIS, PROP, GET, SET>(
    //     &self,
    //     name: &'static str,
    //     getter: GET,
    //     setter: SET,
    // ) -> Result<()>
    // where
    //     FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<THIS>,
    //     GET: for<'a> Fn(&THIS) -> PROP,
    //     SET: for<'a> Fn(&mut THIS, PROP),
    //     Wrapped<FW, CLS>: Cast<THIS>,
    //     PROP: for<'js> IntoJs<'js>,
    //     ClassMethod<FW, CLS, CLS, (THIS,), PROP, GET>: Implements<P>,
    //     CLS: 'static,
    // {
    //     ClassProp::new(name, getter, setter).build(&self.phase)?;
    //     Ok(())
    // }

    pub fn superclass<SUPER>(&self) -> Result<()>
    where
        FW: Framework + TypeDef<CLS> + TypeDef<SUPER> + Wraps<CLS> + Wraps<SUPER>,
        ClassSuper<FW, CLS, SUPER>: Implements<P>,
    {
        ClassSuper::<FW, CLS, SUPER>::new().build(&self.phase)?;
        Ok(())
    }

    pub fn constructor0<R, X>(&self, func: X) -> Result<()>
    where
        FW: Framework + TypeDef<CLS> + Wraps<CLS>,
        X: Fn() -> R,
        ClassConstructor<FW, CLS, (), R, X>: Implements<P>,
    {
        ClassConstructor::new(func).build(&self.phase)?;
        Ok(())
    }

    pub fn method0<T, R, X>(&self, name: &'static str, func: X) -> Result<()>
    where
        FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<T>,
        X: for<'a> Fn(&T) -> R,
        Wrapped<FW, CLS>: Cast<T>,
        R: for<'js> IntoJs<'js>,
        ClassMethod<FW, CLS, (T,), R, X>: Implements<P>,
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
