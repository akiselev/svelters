use crate::inner::*;

pub struct TypeBuilder<'a, CLS, PHASE> {
    phase: &'a mut PHASE,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, CLS, PHASE> TypeBuilder<'a, CLS, PHASE>
where
    CLS: Repr,
{
    #[inline(always)]
    pub fn new(phase: &'a mut PHASE) -> Self {
        Self {
            phase,
            framework: PhantomData,
        }
    }

    #[inline(always)]
    pub fn class(&mut self) -> Result<()>
    where
        CLS: Type,
        JsClassLoader<CLS>: Implements<PHASE>,
    {
        JsClassLoader::<CLS>::new().build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn prop<PROP, GET, SET>(
        &mut self,
        name: &'static str,
        getter: GET,
        setter: SET,
    ) -> Result<()>
    where
        CLS: Type,
        ReprOf<CLS>: Cast<CLS>,
        PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
        Property<CLS, PROP, GET, SET>: Implements<PHASE>,
    {
        Property::<CLS, PROP, GET, SET>::new(name, getter, setter).build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn readonly<PROP, GET>(&mut self, name: &'static str, getter: GET) -> Result<()>
    where
        CLS: Type,
        ReprOf<CLS>: Cast<CLS>,
        PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        ReadOnly<CLS, PROP, GET>: Implements<PHASE>,
    {
        ReadOnly::<CLS, PROP, GET>::new(name, getter).build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn superclass<SUPER>(&mut self) -> Result<()>
    where
        CLS: Type,
        SUPER: Type,
        ReprOf<CLS>: Cast<SUPER>,
        Superclass<CLS, SUPER>: Implements<PHASE>,
    {
        Superclass::<CLS, SUPER>::new().build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn constructor0<R, X>(&mut self, func: X) -> Result<()>
    where
        CLS: 'static + Type,
        X: 'static + Fn() -> R,
        R: 'static + IntoRepr<CLS>,
        Constructor<CLS, (), R, X>: Implements<PHASE>,
    {
        Constructor::new(func).build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn method0<THIS, RET, FN>(&mut self, name: &'static str, func: FN) -> Result<()>
    where
        CLS: 'static + Type,
        ReprOf<CLS>: Cast<THIS>,
        THIS: 'static + Repr,
        RET: 'static + for<'js> IntoJs<'js>,
        FN: 'static + for<'x> Fn(&'x THIS) -> RET,
        Method<CLS, (THIS,), RET, FN>: Implements<PHASE>,
    {
        Method::new(name, func).build(&mut self.phase)?;
        Ok(())
    }

    #[inline(always)]
    pub fn static_fn0<RET, FN>(&mut self, name: &'static str, func: FN) -> Result<()>
    where
        FN: 'static + Fn() -> RET,
        CLS: 'static + Type,
        RET: 'static + for<'js> IntoJs<'js>,
        StaticFn<CLS, (), RET, FN>: Implements<PHASE>,
    {
        StaticFn::new(name, func).build(&mut self.phase)?;
        Ok(())
    }
}

// macro_rules! tup {
//     ($($static_fn:ident $method:ident $constructor:ident $($type:ident)*,)*) => {
//         $(
//             pub fn $static_fn<$($type,)* R, X>(
//                 &mut self,
//                 name: &'static str,
//                 func: X,
//             ) -> ClassStaticFunc<CLS, ($($type,)*), R, X>
//             where
//                 FW: Container<CLS>,
//                 X: StaticFunc<CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
//                 $($type: for<'a> FromJs<'a>,)*
//                 R: for<'a> IntoJs<'a>,
//             {
//                 ClassStaticFunc::new(name, func)
//             }

//             pub fn $method<T, $($type,)* R, X>(
//                 &mut self,
//                 name: &'static str,
//                 func: X,
//             ) -> ClassMethod<CLS, (T, $($type,)*), R, X>
//             where
//                 X:  Method<CLS, (T, $($type,)*), R> + Fn($($type,)*) -> R,
//                 ClsBox<CLS>: for<'a> GetRef<'a, CLS>,
//                 ClsBox<CLS>: Cast<T>,
//                 R: for<'js> IntoJs<'js>,
//                 $($type: for<'a> FromJs<'a>,)*
//             {
//                 ClassMethod::new(name, func)
//             }

//             pub fn $constructor<$($type,)* R, X>(&mut self, func: X) -> ClassConstructor<CLS, ($($type,)*), R, X>
//             where
//                 X:  Constructor<CLS, ($($type,)*), R> + Fn($($type,)*) -> R,
//                 R: IntoContainer<CLS, Type = ClsBox<CLS>>,
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

// impl<Z: Phase,  CLS> JsClass<CLS, Z>
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
