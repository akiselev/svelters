use crate::inner::*;

pub struct Constructor<CLS, ARGS, THIS>(pub(crate) THIS, pub(crate) PhantomData<(CLS, ARGS)>)
where
    CLS: Type;

impl<THIS, ARGS, CLS> Constructor<CLS, ARGS, THIS>
where
    CLS: Type,
{
    pub(in crate) fn new(func: THIS) -> Self {
        Constructor(func, PhantomData)
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            impl<'a, 'b, 'js: 'a, T, CLS, $($arg,)*> Implements<ConstructorEval<'a, 'b, 'js, CLS>> for members::Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: 'static + Fn($($arg,)*) -> Result<CLS>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: ConstructorEval<'a, 'b, 'js, CLS>) -> Result<ConstructorEval<'a, 'b, 'js, CLS>> {
                    JsClass::<Obj<CLS>>::register(phase.ctx)?;
                    JsClass::<Virt<CLS>>::register(phase.ctx)?;
                    let rt = phase.rt.weak();
                    phase.module.set(
                        <CLS as Type>::CLASS_NAME,
                        rquickjs::Func::new(
                            <CLS as Type>::CLASS_NAME,
                            JsClass::<Obj<CLS>>::constructor(move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                                let args = args.into_inner();
                                let args_len = args.len();
                                tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;

                                let result = self.0(
                                    $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                )?;
                                JsClass::instance(ctx, Obj::<CLS>::new(rt.upgrade().unwrap(), result))
                            }),
                        ),
                    )?;
                    Ok(phase)
                }
            }

            impl<'a, 'js: 'a, T, CLS, R, $($arg,)*> Implements<ConstructorLoad<'a, 'js, CLS>> for members::Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: Fn($($arg,)*) -> R,
            {
                #[inline(always)]
                fn build_phase(self, phase: ConstructorLoad<'a, 'js, CLS>) -> Result<ConstructorLoad<'a, 'js, CLS>> {
                    phase.module.add(<CLS as Type>::name())?;
                    Ok(phase)
                }
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

tup! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
    A B C D E F G H I,
    A B C D E F G H I J,
    A B C D E F G H I J K,
    A B C D E F G H I J K L,
    A B C D E F G H I J K L M,
    A B C D E F G H I J K L M N,
    A B C D E F G H I J K L M N O,
    A B C D E F G H I J K L M N O P,
}
